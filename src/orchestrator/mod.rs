use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

use crate::config::Config;
use crate::engine::{Identity, Workspace};
use crate::event::{AgentStatus, Block, Event, OutcomeSummary, RiskLevel, RunId, TokenUsage};
use crate::memory::Memory;
use crate::provider::{Brain, BrainRequest, BrainStream, ContentBlock, ModelCaps, Msg};
use crate::router::{BudgetState, Router, TaskTier};
use crate::sandbox::LocalSandbox;
use crate::tools::edit::{Edit, MultiEdit};
use crate::tools::exec::Exec;
use crate::tools::fs::{FsList, FsRead, FsWrite};
use crate::tools::git::Git;
use crate::tools::search_and_web::Search;
use crate::tools::{ToolCtx, ToolRegistry};

// ─── Fallback brain ──────────────────────────────────────────────────────────
// Wraps an ordered chain of brains and tries each on `complete()` until one
// succeeds. Lets each swarm role (planner/coder/verifier) survive a model that
// 404s or rate-limits — discovery sometimes lists models that aren't callable.

struct FallbackBrain {
    id: String,
    caps: ModelCaps,
    chain: Vec<Arc<dyn Brain>>,
}

impl FallbackBrain {
    fn new(chain: Vec<Arc<dyn Brain>>) -> Self {
        let id = chain
            .first()
            .map(|b| b.id().to_string())
            .unwrap_or_else(|| "none".into());
        let caps = chain.first().map(|b| b.caps()).unwrap_or_default();
        Self { id, caps, chain }
    }
}

#[async_trait::async_trait]
impl Brain for FallbackBrain {
    fn id(&self) -> &str {
        &self.id
    }
    fn caps(&self) -> ModelCaps {
        self.caps.clone()
    }
    async fn complete(&self, req: BrainRequest) -> anyhow::Result<BrainStream> {
        let mut last_err: Option<anyhow::Error> = None;
        for brain in &self.chain {
            match brain.complete(req.clone()).await {
                Ok(stream) => return Ok(stream),
                Err(e) => {
                    tracing::warn!("swarm brain {} failed, trying next: {}", brain.id(), e);
                    last_err = Some(e);
                }
            }
        }
        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("no brains in fallback chain")))
    }
}

// ─── Swarm types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SwarmPlan {
    pub task: String,
    pub workspace: PathBuf,
    pub max_reworks: u32,
}

impl Default for SwarmPlan {
    fn default() -> Self {
        Self {
            task: String::new(),
            workspace: PathBuf::from("."),
            max_reworks: 3,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SwarmOutcome {
    pub status: String,
    pub plan: Option<String>,
    pub diffs: Vec<crate::event::FileDiff>,
    pub passes: u32,
    pub reworks: u32,
    pub cost_usd: f64,
}

#[derive(Debug, Clone)]
pub enum Verdict {
    Pass,
    Rework { findings: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwarmPhase {
    Planning,
    Coding,
    Verifying,
    Reworking,
    Done,
}

impl std::fmt::Display for SwarmPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SwarmPhase::Planning => write!(f, "planning"),
            SwarmPhase::Coding => write!(f, "coding"),
            SwarmPhase::Verifying => write!(f, "verifying"),
            SwarmPhase::Reworking => write!(f, "reworking"),
            SwarmPhase::Done => write!(f, "done"),
        }
    }
}

// ─── Anti-collision: file-level locks ───────────────────────────────────────────

pub struct FileLocks {
    locked: Mutex<HashSet<String>>,
}

impl FileLocks {
    pub fn new() -> Self {
        Self {
            locked: Mutex::new(HashSet::new()),
        }
    }

    pub async fn try_lock(&self, files: &[String]) -> Result<FileLockGuard, Vec<String>> {
        let mut locked = self.locked.lock().await;
        let mut conflicts = Vec::new();
        for f in files {
            if locked.contains(f) {
                conflicts.push(f.clone());
            }
        }
        if !conflicts.is_empty() {
            return Err(conflicts);
        }
        for f in files {
            locked.insert(f.clone());
        }
        Ok(FileLockGuard {
            _files: files.to_vec(),
        })
    }

    pub async fn release(&self, files: &[String]) {
        let mut locked = self.locked.lock().await;
        for f in files {
            locked.remove(f);
        }
    }
}

pub struct FileLockGuard {
    _files: Vec<String>,
}

fn swarm_tool_registry(workspace: &Workspace, write_enabled: bool) -> Arc<ToolRegistry> {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(FsRead));
    registry.register(Arc::new(FsList));
    if write_enabled {
        registry.register(Arc::new(FsWrite));
        registry.register(Arc::new(Edit));
        registry.register(Arc::new(MultiEdit));
        registry.register(Arc::new(Search));
        registry.register(Arc::new(Git));
        registry.register(Arc::new(Exec::new(workspace.sandbox.clone())));
    }
    Arc::new(registry)
}

fn tool_blocks_text(blocks: &[Block]) -> String {
    blocks
        .iter()
        .map(|block| match block {
            Block::Text(text) => text.clone(),
            Block::Json(value) => value.to_string(),
            Block::Image { mime, data } => format!("[image: {}, {} bytes]", mime, data.len()),
            Block::Diff { file, patch } => format!("diff for {}\n{}", file, patch),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn track_tool_diff(
    diffs: &mut Vec<crate::event::FileDiff>,
    tool_name: &str,
    args: &serde_json::Value,
    blocks: &[Block],
) {
    for block in blocks {
        if let Block::Diff { file, patch } = block {
            let plus = patch
                .lines()
                .filter(|line| line.starts_with('+') && !line.starts_with("+++"))
                .count() as u32;
            let minus = patch
                .lines()
                .filter(|line| line.starts_with('-') && !line.starts_with("---"))
                .count() as u32;
            if !diffs.iter().any(|diff| diff.file == *file) {
                diffs.push(crate::event::FileDiff {
                    file: file.clone(),
                    plus,
                    minus,
                });
            }
        }
    }

    if matches!(tool_name, "fs_write" | "edit" | "multi_edit") {
        if let Some(path) = args.get("path").and_then(|value| value.as_str()) {
            if !diffs.iter().any(|diff| diff.file == path) {
                diffs.push(crate::event::FileDiff {
                    file: path.to_string(),
                    plus: 0,
                    minus: 0,
                });
            }
        }
    }
}

// ─── THE ORCHESTRATOR TRAIT ─────────────────────────────────────────────────────

#[async_trait::async_trait]
pub trait Orchestrator: Send + Sync {
    async fn run_swarm(
        &self,
        plan: SwarmPlan,
        event_tx: mpsc::UnboundedSender<Event>,
    ) -> anyhow::Result<SwarmOutcome>;
}

// ─── Default orchestrator: Planner → Coder → Verifier ───────────────────────────

pub struct DefaultOrchestrator {
    router: Arc<dyn Router>,
    config: Config,
    memory: Arc<dyn Memory>,
    file_locks: Arc<FileLocks>,
}

impl DefaultOrchestrator {
    pub fn new(router: Arc<dyn Router>, config: Config, memory: Arc<dyn Memory>) -> Self {
        Self {
            router,
            config,
            memory,
            file_locks: Arc::new(FileLocks::new()),
        }
    }

    /// Classify task tier for model selection
    fn classify(&self, task: &str) -> TaskTier {
        let lower = task.to_lowercase();
        if lower.len() < 20 {
            TaskTier::Trivial
        } else if lower.contains("refactor") || lower.contains("architecture") {
            TaskTier::Hard
        } else if lower.contains("bug") || lower.contains("fix") {
            TaskTier::Small
        } else {
            TaskTier::Medium
        }
    }

    /// Select a brain for a given role and tier
    fn select_brain(&self, role: &str, tier: TaskTier) -> Option<Arc<dyn Brain>> {
        let need = match role {
            "planner" => crate::router::RoutingNeed {
                tier: TaskTier::Hard, // Planner always uses a strong model
                required_tools: false,
                required_vision: false,
                prefer_local: false,
            },
            "verifier" => crate::router::RoutingNeed {
                tier: TaskTier::Medium, // Verifier uses medium model
                required_tools: true,
                required_vision: false,
                prefer_local: false,
            },
            _ => crate::router::RoutingNeed {
                // Coder writes files via tools — it must get a tool-capable,
                // competent model. A "trivial"/"small" classification would route
                // to the cheapest free model that often emits prose instead of
                // tool calls (no diff lands). Floor the coder at Medium.
                tier: match tier {
                    TaskTier::Trivial | TaskTier::Small => TaskTier::Medium,
                    other => other,
                },
                required_tools: true,
                required_vision: false,
                prefer_local: false,
            },
        };

        let budget = BudgetState {
            daily_limit_usd: self.config.budget.daily_usd,
            daily_spent_usd: 0.0,
            session_limit_usd: self.config.budget.session_usd,
            session_spent_usd: 0.0,
        };

        // Wrap the whole fallback chain so a 404/ratelimit on the top model
        // transparently advances to the next instead of killing the role.
        let chain = self.router.select(&need, &budget);
        if chain.is_empty() {
            None
        } else {
            Some(Arc::new(FallbackBrain::new(chain)) as Arc<dyn Brain>)
        }
    }

    /// Run the planner agent
    async fn run_planner(
        &self,
        task: &str,
        workspace: &Workspace,
        brain: Arc<dyn Brain>,
        event_tx: &mpsc::UnboundedSender<Event>,
        parent_run: &RunId,
    ) -> anyhow::Result<String> {
        let planner_identity = Identity {
            name: "planner".into(),
            role: "technical architect and planner".into(),
            personality: "analytical, thorough, produces clear structured plans with concrete steps and acceptance criteria.".into(),
        };

        let system = format!(
            r#"You are the PLANNER agent in a swarm.

{personality}

Your job: take a task and produce a detailed implementation SPEC.
- Break the task into clear, numbered steps.
- For each step, specify what files to create/modify.
- Include acceptance criteria for the verifier.
- Output ONLY the spec. No code. No implementation.

Output format:
## SPEC: <title>

### Step 1: <description>
- Files: <list>
- Changes: <what changes>
- Acceptance: <verification criteria>

### Step 2: ...
"#,
            personality = planner_identity.personality,
        );

        let messages = vec![Msg {
            role: "user".into(),
            content: vec![ContentBlock::Text {
                text: format!("Task to plan:\n\n{}", task),
            }],
        }];

        let tools = swarm_tool_registry(workspace, false);

        let req = BrainRequest {
            system: Some(system),
            messages,
            tools: tools.to_specs(),
            max_tokens: 4096,
            temperature: 0.0,
            stop: vec![],
        };

        let _ = event_tx.send(Event::AgentStatus {
            run: parent_run.clone(),
            role: "planner".into(),
            status: AgentStatus::Thinking,
            note: format!("planning with {}", brain.id()),
        });

        let mut stream = brain.complete(req).await?;
        let mut plan = String::new();

        while let Some(ev) = futures::StreamExt::next(&mut stream).await {
            match ev {
                crate::provider::BrainEvent::TextDelta(text) => {
                    plan.push_str(&text);
                    let _ = event_tx.send(Event::ThinkingDelta {
                        run: parent_run.clone(),
                        text,
                    });
                }
                crate::provider::BrainEvent::Done(_) => break,
                crate::provider::BrainEvent::Error(e) => {
                    anyhow::bail!("Planner error: {}", e)
                }
                _ => {}
            }
        }

        let _ = event_tx.send(Event::AgentStatus {
            run: parent_run.clone(),
            role: "planner".into(),
            status: AgentStatus::Done,
            note: "plan complete".into(),
        });

        Ok(plan)
    }

    /// Run the coder agent with a given spec
    async fn run_coder(
        &self,
        spec: &str,
        rework_notes: Option<&[String]>,
        workspace: &Workspace,
        brain: Arc<dyn Brain>,
        event_tx: &mpsc::UnboundedSender<Event>,
        parent_run: &RunId,
    ) -> anyhow::Result<Vec<crate::event::FileDiff>> {
        let coder_identity = Identity {
            name: "coder".into(),
            role: "implementation engineer".into(),
            personality:
                "precise, produces clean working code, uses exact file edits with the edit tool."
                    .into(),
        };

        let rework_section = if let Some(notes) = rework_notes {
            if notes.is_empty() {
                String::new()
            } else {
                format!(
                    "\n## REWORK NOTES (from verifier)\nThe previous implementation had issues. Fix these:\n{}",
                    notes
                        .iter()
                        .enumerate()
                        .map(|(i, n)| format!("{}. {}", i + 1, n))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            }
        } else {
            String::new()
        };

        let system = format!(
            r#"You are the CODER agent in a swarm.

{}

Your job: implement the SPEC exactly. Use tools to read existing files and write edits.
- Follow the spec steps in order.
- Use the edit or fs_write tool to make changes.
- After each file edit, note what you changed.
- Produce working, compilable code.
{}
"#,
            coder_identity.personality, rework_section,
        );

        // Build repo map context
        let repo_map = self.memory.repo_map(&workspace.root);
        let file_list: Vec<String> = repo_map
            .files
            .iter()
            .map(|f| format!("  {}", f.path))
            .collect();

        let context_msg = format!(
            "## SPEC TO IMPLEMENT\n\n{}\n\n## WORKSPACE FILES\n{}",
            spec,
            file_list.join("\n"),
        );

        let mut messages = vec![Msg {
            role: "user".into(),
            content: vec![ContentBlock::Text { text: context_msg }],
        }];
        let tools = swarm_tool_registry(workspace, true);
        let tool_specs = tools.to_specs();

        let _ = event_tx.send(Event::AgentStatus {
            run: parent_run.clone(),
            role: "coder".into(),
            status: AgentStatus::Working,
            note: format!("implementing with {}", brain.id()),
        });

        let mut output = String::new();
        let mut diffs = Vec::new();

        for _turn in 0..8 {
            let req = BrainRequest {
                system: Some(system.clone()),
                messages: messages.clone(),
                tools: tool_specs.clone(),
                max_tokens: 8192,
                temperature: 0.0,
                stop: vec![],
            };

            let mut stream = brain.complete(req).await?;
            let mut assistant_text = String::new();
            let mut assistant_blocks = Vec::new();
            let mut tool_result_blocks = Vec::new();
            let mut current_tool_id = String::new();
            let mut current_tool_name = String::new();
            let mut current_tool_json = String::new();

            while let Some(ev) = futures::StreamExt::next(&mut stream).await {
                match ev {
                    crate::provider::BrainEvent::TextDelta(text) => {
                        output.push_str(&text);
                        assistant_text.push_str(&text);
                        let _ = event_tx.send(Event::ThinkingDelta {
                            run: parent_run.clone(),
                            text,
                        });
                    }
                    crate::provider::BrainEvent::ToolUseStart { id, name } => {
                        current_tool_id = id.clone();
                        current_tool_name = name.clone();
                        current_tool_json.clear();
                        let risk = tools
                            .get(&name)
                            .map(|tool| tool.risk())
                            .unwrap_or(RiskLevel::ReadOnly);
                        let _ = event_tx.send(Event::ToolUseProposed {
                            run: parent_run.clone(),
                            id,
                            name,
                            args: serde_json::json!({}),
                            risk,
                        });
                    }
                    crate::provider::BrainEvent::ToolUseDelta { id: _, json } => {
                        current_tool_json.push_str(&json);
                    }
                    crate::provider::BrainEvent::ToolUseEnd { id } => {
                        let args = serde_json::from_str::<serde_json::Value>(&current_tool_json)
                            .unwrap_or_else(|_| serde_json::json!({}));
                        let tool_name = if current_tool_name.is_empty() {
                            "unknown".to_string()
                        } else {
                            current_tool_name.clone()
                        };
                        assistant_blocks.push(ContentBlock::ToolUse {
                            id: id.clone(),
                            name: tool_name.clone(),
                            input: args.clone(),
                        });
                        let _ = event_tx.send(Event::ToolUseStarted {
                            run: parent_run.clone(),
                            id: id.clone(),
                        });
                        let result = if let Some(tool) = tools.get(&tool_name) {
                            let ctx = ToolCtx {
                                workspace_root: workspace.root.clone(),
                                run_id: parent_run.clone(),
                            };
                            match tool.call(args.clone(), &ctx).await {
                                Ok(result) => result,
                                Err(err) => crate::tools::ToolResult::error(format!(
                                    "Tool {} failed: {}",
                                    tool_name, err
                                )),
                            }
                        } else {
                            crate::tools::ToolResult::error(format!("Unknown tool: {}", tool_name))
                        };
                        track_tool_diff(&mut diffs, &tool_name, &args, &result.content);
                        for diff in &diffs {
                            let _ = event_tx.send(Event::DiffProposed {
                                run: parent_run.clone(),
                                file: diff.file.clone(),
                                patch: String::new(),
                                plus: diff.plus,
                                minus: diff.minus,
                            });
                        }
                        let blocks = result.content.clone();
                        let text = tool_blocks_text(&blocks);
                        let _ = event_tx.send(Event::ToolOutput {
                            run: parent_run.clone(),
                            id: id.clone(),
                            blocks,
                        });
                        tool_result_blocks.push(ContentBlock::ToolResult {
                            tool_use_id: id,
                            content: vec![ContentBlock::Text { text }],
                            is_error: Some(result.is_error),
                        });
                        current_tool_id.clear();
                        current_tool_name.clear();
                        current_tool_json.clear();
                    }
                    crate::provider::BrainEvent::Done(_) => break,
                    crate::provider::BrainEvent::Error(e) => anyhow::bail!("Coder error: {}", e),
                    crate::provider::BrainEvent::Usage(_) => {}
                }
            }

            if !assistant_text.is_empty() {
                assistant_blocks.insert(
                    0,
                    ContentBlock::Text {
                        text: assistant_text,
                    },
                );
            }
            if tool_result_blocks.is_empty() {
                break;
            }
            messages.push(Msg {
                role: "assistant".into(),
                content: assistant_blocks,
            });
            messages.push(Msg {
                role: "user".into(),
                content: tool_result_blocks,
            });
        }

        if diffs.is_empty() {
            for line in output.lines() {
                if let Some(file) = line.strip_prefix("Edited ") {
                    let file = file
                        .trim()
                        .split(':')
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    if !file.is_empty() && !diffs.iter().any(|d| d.file == file) {
                        diffs.push(crate::event::FileDiff {
                            file,
                            plus: 1,
                            minus: 1,
                        });
                    }
                }
            }
        }

        // Emit diff events
        for diff in &diffs {
            let _ = event_tx.send(Event::DiffProposed {
                run: parent_run.clone(),
                file: diff.file.clone(),
                patch: String::new(),
                plus: diff.plus,
                minus: diff.minus,
            });
        }

        let _ = event_tx.send(Event::AgentStatus {
            run: parent_run.clone(),
            role: "coder".into(),
            status: AgentStatus::Done,
            note: format!("{} files changed", diffs.len()),
        });

        Ok(diffs)
    }

    /// Run the verifier agent
    async fn run_verifier(
        &self,
        spec: &str,
        diffs: &[crate::event::FileDiff],
        workspace: &Workspace,
        brain: Arc<dyn Brain>,
        event_tx: &mpsc::UnboundedSender<Event>,
        parent_run: &RunId,
    ) -> anyhow::Result<Verdict> {
        let verifier_identity = Identity {
            name: "verifier".into(),
            role: "code reviewer and quality assurance".into(),
            personality: "adversarial, meticulous, catches issues the coder missed. Checks correctness, style, edge cases, and spec compliance.".into(),
        };

        let diff_summary: String = diffs
            .iter()
            .map(|d| format!("  {}: +{} -{}", d.file, d.plus, d.minus))
            .collect::<Vec<_>>()
            .join("\n");

        let tools = swarm_tool_registry(workspace, false);
        let read_tool = tools.get("fs_read");
        let mut files_to_check = Vec::new();
        for d in diffs {
            let content = if let Some(tool) = &read_tool {
                let ctx = ToolCtx {
                    workspace_root: workspace.root.clone(),
                    run_id: parent_run.clone(),
                };
                let args = serde_json::json!({ "path": d.file, "limit": 220 });
                match tool.call(args, &ctx).await {
                    Ok(result) => tool_blocks_text(&result.content),
                    Err(_) => format!("[cannot read {}]", d.file),
                }
            } else {
                format!("[cannot read {}]", d.file)
            };
            files_to_check.push(content);
        }

        let files_context: String = diffs
            .iter()
            .zip(files_to_check.iter())
            .map(|(d, content)| {
                format!(
                    "### {}\n```\n{}\n```",
                    d.file,
                    if content.len() > 3000 {
                        format!("{}... [truncated]", &content[..3000])
                    } else {
                        content.clone()
                    }
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let system = format!(
            r#"You are the VERIFIER agent in a swarm.

{personality}

Your job: review the coder's implementation against the SPEC.
- For each spec requirement, check if it's satisfied.
- Find bugs, style issues, missing edge cases, spec violations.
- Output EXACTLY one of:
  ✓ PASS — if everything is correct and complete.
  ✗ REWORK — followed by numbered concrete findings.

Format:
✓ PASS
(no issues found)

or:

✗ REWORK
1. <specific finding with file:line>
2. <another finding>
"#,
            personality = verifier_identity.personality,
        );

        let context = format!(
            "## SPEC\n{}\n\n## CHANGED FILES\n{}\n\n## FILE CONTENTS\n{}",
            spec, diff_summary, files_context,
        );

        let messages = vec![Msg {
            role: "user".into(),
            content: vec![ContentBlock::Text { text: context }],
        }];

        let req = BrainRequest {
            system: Some(system),
            messages,
            tools: tools.to_specs(),
            max_tokens: 4096,
            temperature: 0.0,
            stop: vec![],
        };

        let _ = event_tx.send(Event::AgentStatus {
            run: parent_run.clone(),
            role: "verifier".into(),
            status: AgentStatus::Working,
            note: format!("reviewing with {}", brain.id()),
        });

        let mut stream = brain.complete(req).await?;
        let mut verdict_text = String::new();

        while let Some(ev) = futures::StreamExt::next(&mut stream).await {
            match ev {
                crate::provider::BrainEvent::TextDelta(text) => {
                    verdict_text.push_str(&text);
                }
                crate::provider::BrainEvent::Done(_) => break,
                _ => {}
            }
        }

        let verdict = if verdict_text.contains("✗ REWORK") || verdict_text.contains("REWORK") {
            let findings: Vec<String> = verdict_text
                .lines()
                .filter(|l| l.trim().starts_with(|c: char| c.is_ascii_digit()) && l.contains('.'))
                .map(|l| l.trim().to_string())
                .collect();

            if findings.is_empty() {
                let findings = vec![verdict_text.clone()];
                Verdict::Rework { findings }
            } else {
                Verdict::Rework { findings }
            }
        } else {
            Verdict::Pass
        };

        let _ = event_tx.send(Event::AgentStatus {
            run: parent_run.clone(),
            role: "verifier".into(),
            status: AgentStatus::Done,
            note: match &verdict {
                Verdict::Pass => "PASS".into(),
                Verdict::Rework { findings } => format!("REWORK ({} issues)", findings.len()),
            },
        });

        Ok(verdict)
    }
}

#[async_trait::async_trait]
impl Orchestrator for DefaultOrchestrator {
    async fn run_swarm(
        &self,
        plan: SwarmPlan,
        event_tx: mpsc::UnboundedSender<Event>,
    ) -> anyhow::Result<SwarmOutcome> {
        let run_id = RunId::new();
        let task = plan.task.clone();

        let _ = event_tx.send(Event::RunStarted {
            run: run_id.clone(),
            task: task.clone(),
            agent: "swarm".into(),
        });

        // Select brains for each role
        let planner_brain = self
            .select_brain("planner", self.classify(&task))
            .ok_or_else(|| anyhow::anyhow!("No model available for planner"))?;

        let coder_brain = self
            .select_brain("coder", self.classify(&task))
            .unwrap_or_else(|| planner_brain.clone());

        let verifier_brain = self
            .select_brain("verifier", self.classify(&task))
            .unwrap_or_else(|| coder_brain.clone());

        // Create workspace
        let sandbox = LocalSandbox::new(plan.workspace.clone());
        let workspace = Workspace {
            root: plan.workspace.clone(),
            sandbox: Arc::new(sandbox),
        };

        let total_cost: f64 = 0.0;

        // ▸ PHASE 1: PLANNING
        let _ = event_tx.send(Event::AgentSpawned {
            run: run_id.clone(),
            role: "planner".into(),
            model: planner_brain.id().to_string(),
        });

        let spec = self
            .run_planner(&task, &workspace, planner_brain, &event_tx, &run_id)
            .await?;

        // Post plan to shared memory
        let _ = self.memory.upsert_doc(crate::memory::WorkingDoc {
            id: format!("plan-{}", run_id.0),
            title: format!("Plan: {}", &task[..task.len().min(60)]),
            content: spec.clone(),
            updated_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        });

        // ▸ PHASE 2–3: CODING + VERIFYING (with REWORK loop)
        let _ = event_tx.send(Event::AgentSpawned {
            run: run_id.clone(),
            role: "coder".into(),
            model: coder_brain.id().to_string(),
        });

        let _ = event_tx.send(Event::AgentSpawned {
            run: run_id.clone(),
            role: "verifier".into(),
            model: verifier_brain.id().to_string(),
        });

        let mut rework_notes: Option<Vec<String>> = None;
        let mut all_diffs: Vec<crate::event::FileDiff> = Vec::new();
        let mut reworks = 0u32;
        let mut passes = 0u32;

        for attempt in 0..=plan.max_reworks {
            // ▸ CODER
            let diffs = self
                .run_coder(
                    &spec,
                    rework_notes.as_deref(),
                    &workspace,
                    coder_brain.clone(),
                    &event_tx,
                    &run_id,
                )
                .await?;

            // Release any locks from previous attempt
            if attempt > 0 {
                let prev_files: Vec<String> = all_diffs.iter().map(|d| d.file.clone()).collect();
                self.file_locks.release(&prev_files).await;
            }

            // Acquire locks for new diffs
            let new_files: Vec<String> = diffs.iter().map(|d| d.file.clone()).collect();
            if let Err(conflicts) = self.file_locks.try_lock(&new_files).await {
                let _ = event_tx.send(Event::Error {
                    run: run_id.clone(),
                    message: format!("File collision detected: {:?}", conflicts),
                });
            }

            all_diffs = diffs.clone();

            // ▸ Empty-diff guard: if the coder changed no files, don't waste a
            // verify pass (and never let it PASS) — force a REWORK with an
            // explicit instruction to actually call the tools. This rescues weak
            // models that describe a change in prose instead of emitting tool calls.
            if diffs.is_empty() && attempt < plan.max_reworks {
                reworks += 1;
                let note = "You made NO file changes. You MUST call the fs_write or edit \
                    tool to create/modify the files in the spec — do not merely describe \
                    the change in prose. Emit the tool call now."
                    .to_string();
                rework_notes = Some(vec![note.clone()]);
                let _ = event_tx.send(Event::TestResult {
                    run: run_id.clone(),
                    passed: 0,
                    failed: 1,
                    detail: "no file changes — coder must use tools".into(),
                });
                let _ = event_tx.send(Event::AgentStatus {
                    run: run_id.clone(),
                    role: "coder".into(),
                    status: AgentStatus::Working,
                    note: format!("no diff — forcing tool use (attempt {})", attempt + 1),
                });
                continue;
            }

            // ▸ VERIFIER
            let verdict = self
                .run_verifier(
                    &spec,
                    &diffs,
                    &workspace,
                    verifier_brain.clone(),
                    &event_tx,
                    &run_id,
                )
                .await?;

            match verdict {
                Verdict::Pass => {
                    passes += 1;
                    // Signal PASS to shared memory
                    let _ = self.memory.post_signal(crate::memory::SharedSignal {
                        id: format!("pass-{}-{}", run_id.0, attempt),
                        from_agent: "verifier".into(),
                        to_agent: "coder".into(),
                        content: "PASS — all checks satisfied".into(),
                        timestamp: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                    });

                    let _ = event_tx.send(Event::TestResult {
                        run: run_id.clone(),
                        passed: 1,
                        failed: 0,
                        detail: "Verifier PASS".into(),
                    });
                    for diff in &diffs {
                        let _ = event_tx.send(Event::DiffApplied {
                            run: run_id.clone(),
                            file: diff.file.clone(),
                        });
                    }
                    break; // Done!
                }
                Verdict::Rework { findings } => {
                    reworks += 1;
                    rework_notes = Some(findings.clone());

                    // Signal REWORK to shared memory
                    let _ = self.memory.post_signal(crate::memory::SharedSignal {
                        id: format!("rework-{}-{}", run_id.0, attempt),
                        from_agent: "verifier".into(),
                        to_agent: "coder".into(),
                        content: format!("REWORK: {}", findings.join("; ")),
                        timestamp: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                    });

                    let _ = event_tx.send(Event::TestResult {
                        run: run_id.clone(),
                        passed: 0,
                        failed: findings.len() as u32,
                        detail: findings.join("\n"),
                    });

                    let _ = event_tx.send(Event::AgentStatus {
                        run: run_id.clone(),
                        role: "coder".into(),
                        status: AgentStatus::Working,
                        note: format!("rework attempt {}/{}", attempt + 1, plan.max_reworks),
                    });
                }
            }
        }

        let outcome = SwarmOutcome {
            status: if passes > 0 {
                "PASS".into()
            } else {
                format!("FAILED after {} reworks", reworks)
            },
            plan: Some(spec),
            diffs: all_diffs,
            passes,
            reworks,
            cost_usd: total_cost,
        };

        let outcome_summary = OutcomeSummary {
            status: outcome.status.clone(),
            diffs: outcome.diffs.clone(),
            cost_usd: outcome.cost_usd,
            tokens: TokenUsage {
                input: 0,
                output: 0,
            },
        };

        let _ = event_tx.send(Event::RunFinished {
            run: run_id,
            outcome: outcome_summary,
        });

        Ok(outcome)
    }
}
