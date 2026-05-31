use async_trait::async_trait;
use futures::StreamExt;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::agent::AgentStore;
use crate::autonomy::{AutonomyContract, Checkpoints, GitCheckpoints};
use crate::capabilities::{Curator, SkillLibrary};
use crate::config::Config;
use crate::event::{
    AgentStatus, AutonomyLevel, Block, Decision, Event, OutcomeSummary, RiskLevel, RunId,
    TokenUsage,
};
use crate::extras::Distiller;
use crate::hooks::HookRegistry;
use crate::memory::{Fact, Memory};
use crate::provider::{Brain, BrainEvent, BrainRequest, ContentBlock, Msg, ToolSpec};
use crate::reasoning::ReasoningEngine;
use crate::redaction::RedactionFilter;
use crate::router::{BudgetState, Router, TaskTier};
use crate::sandbox::Sandbox;
use crate::tools::{ToolCtx, ToolRegistry};

// ─── Agent identity ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Identity {
    pub name: String,
    pub role: String,
    pub personality: String,
}

impl Default for Identity {
    fn default() -> Self {
        Self {
            name: "sparrow".into(),
            role: "software engineer".into(),
            personality: "concise, competent, helpful".into(),
        }
    }
}

// ─── Brain policy ───────────────────────────────────────────────────────────────

pub struct BrainPolicy {
    /// The fallback chain selected by the Router for this run
    pub chain: Vec<Arc<dyn Brain>>,
    pub current_index: usize,
}

impl BrainPolicy {
    pub fn current(&self) -> Option<Arc<dyn Brain>> {
        self.chain.get(self.current_index).cloned()
    }

    pub fn next(&mut self) -> Option<Arc<dyn Brain>> {
        self.current_index += 1;
        self.current()
    }
}

// ─── Workspace ──────────────────────────────────────────────────────────────────

pub struct Workspace {
    pub root: PathBuf,
    pub sandbox: Arc<dyn Sandbox>,
}

// ─── Agent run ─────────────────────────────────────────────────────────────────

pub struct AgentRun {
    pub id: RunId,
    pub identity: Identity,
    pub brain_policy: BrainPolicy,
    pub autonomy: AutonomyContract,
    pub tools: Arc<ToolRegistry>,
    pub workspace: Workspace,
}

fn estimate_text_tokens(text: &str) -> u64 {
    let chars = text.chars().count() as u64;
    ((chars + 3) / 4).max(1)
}

fn estimate_content_tokens(blocks: &[ContentBlock]) -> u64 {
    blocks
        .iter()
        .map(|block| match block {
            ContentBlock::Text { text } => estimate_text_tokens(text),
            ContentBlock::Image { source } => match source {
                crate::provider::ImageSource::Base64 { data, .. } => {
                    256 + estimate_text_tokens(data).min(2_000)
                }
                crate::provider::ImageSource::Url { url } => 256 + estimate_text_tokens(url),
            },
            ContentBlock::ToolUse { name, input, .. } => {
                estimate_text_tokens(name) + estimate_text_tokens(&input.to_string())
            }
            ContentBlock::ToolResult { content, .. } => 8 + estimate_content_tokens(content),
        })
        .sum()
}

fn estimate_request_tokens(req: &BrainRequest) -> u64 {
    let system = req.system.as_deref().map(estimate_text_tokens).unwrap_or(0);
    let messages: u64 = req
        .messages
        .iter()
        .map(|msg| estimate_text_tokens(&msg.role) + estimate_content_tokens(&msg.content) + 4)
        .sum();
    let tools: u64 = req
        .tools
        .iter()
        .map(|tool| {
            estimate_text_tokens(&tool.name)
                + estimate_text_tokens(&tool.description)
                + estimate_text_tokens(&tool.input_schema.to_string())
        })
        .sum();
    system + messages + tools
}

// ─── System prompt / SOUL ───────────────────────────────────────────────────────

fn build_system_prompt(
    identity: &Identity,
    workspace_root: &PathBuf,
    facts: &[Fact],
    skills: &[crate::capabilities::Skill],
) -> String {
    let mut parts = vec![format!(
        r#"You are {name}, a {role}.

Personality: {personality}

You are working in the workspace: {workspace}
You have access to tools to read, write, edit, search, and execute code.
Always use absolute or relative paths from the workspace root.
Be concise and direct. When making edits, use exact string replacements.
Before making changes, read the relevant files first to understand the codebase.

You are not a standalone chat model. You are the Sparrow agent surface backed by an
external routing engine. Sparrow's core feature is automatic model routing: every
task is classified by tier, tool need, vision need, local preference, budget, and
provider availability, then a ranked fallback chain of models is selected before
this answer starts. If the user asks how routing works, explain Sparrow's actual
pipeline and the active route for the current run. Never claim that no routing
exists just because the current brain is a single selected model.
"#,
        name = identity.name,
        role = identity.role,
        personality = identity.personality,
        workspace = workspace_root.display(),
    )];

    if !facts.is_empty() {
        parts.push("## What you know about the user:".to_string());
        for fact in facts {
            parts.push(format!("- {}: {}", fact.key, fact.value));
        }
    }

    if !skills.is_empty() {
        parts.push("## Relevant skills for this task:".to_string());
        for skill in skills {
            parts.push(format!("### {}\n{}", skill.name, skill.body));
        }
    }

    parts.join("\n\n")
}

fn tool_result_text(blocks: &[Block]) -> String {
    let mut out = Vec::new();
    for block in blocks {
        match block {
            Block::Text(text) => out.push(text.clone()),
            Block::Json(value) => out.push(value.to_string()),
            Block::Image { mime, data } => {
                out.push(format!("[image: {}, {} bytes]", mime, data.len()));
            }
            Block::Diff { file, patch } => out.push(format!("diff for {}\n{}", file, patch)),
        }
    }
    out.join("\n")
}

// ─── Task ───────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Task {
    pub description: String,
    pub context: Vec<Msg>,
}

// ─── THE ENGINE ─────────────────────────────────────────────────────────────────

pub struct Engine {
    router: Arc<dyn Router>,
    config: Config,
    identity: Option<Identity>,
    memory: Option<Arc<dyn Memory>>,
    skills: Option<Arc<dyn SkillLibrary>>,
    redaction: RedactionFilter,
    approval_handler: Option<Arc<dyn ApprovalHandler>>,
    reasoning: ReasoningEngine,
    hooks: HookRegistry,
    agent_store: Option<Arc<dyn AgentStore>>,
    org_policy: Option<crate::onboarding::enterprise::OrgPolicy>,
}

#[derive(Debug, Clone)]
pub struct ApprovalRequest {
    pub run: RunId,
    pub id: String,
    pub tool_name: String,
    pub risk: RiskLevel,
    pub args: serde_json::Value,
    pub summary: String,
}

#[async_trait]
pub trait ApprovalHandler: Send + Sync {
    async fn request_approval(&self, request: ApprovalRequest) -> Decision;
}

impl Engine {
    pub fn new(router: Arc<dyn Router>, config: Config) -> Self {
        Self {
            router,
            config,
            identity: None,
            memory: None,
            skills: None,
            redaction: RedactionFilter::new(),
            approval_handler: None,
            reasoning: ReasoningEngine::default(),
            hooks: HookRegistry::new(Arc::new(crate::sandbox::LocalSandbox::new(
                std::env::current_dir().unwrap_or_default(),
            ))),
            agent_store: None,
            org_policy: None,
        }
    }

    pub fn with_memory(mut self, memory: Arc<dyn Memory>) -> Self {
        // Load secrets for redaction
        let secrets: Vec<String> = memory
            .all_facts()
            .iter()
            .filter(|f| f.key.starts_with("secret:"))
            .map(|f| f.value.clone())
            .collect();
        self.redaction.load_secrets(secrets);
        self.memory = Some(memory);
        self
    }

    pub fn with_skills(mut self, skills: Arc<dyn SkillLibrary>) -> Self {
        self.skills = Some(skills);
        self
    }

    pub fn with_identity(mut self, identity: Identity) -> Self {
        self.identity = Some(identity);
        self
    }

    pub fn with_agent_store(mut self, store: Arc<dyn AgentStore>) -> Self {
        self.agent_store = Some(store);
        self
    }

    pub fn with_org_policy(mut self, policy: crate::onboarding::enterprise::OrgPolicy) -> Self {
        self.org_policy = Some(policy);
        self
    }

    pub fn with_hooks_config(mut self, hooks: Vec<crate::hooks::Hook>) -> Self {
        self.hooks.load(hooks);
        self
    }

    pub fn with_approval_handler(mut self, approval_handler: Arc<dyn ApprovalHandler>) -> Self {
        self.approval_handler = Some(approval_handler);
        self
    }

    /// Classify a task into a TaskTier.
    fn classify(&self, task: &str) -> TaskTier {
        let lower = task.to_lowercase();
        if lower.contains("vision") || lower.contains("image") || lower.contains("screenshot") {
            TaskTier::Vision
        } else if lower.contains("architecture")
            || lower.contains("refactor")
            || lower.contains("audit")
            || lower.contains("répare")
            || lower.contains("repare")
            || lower.contains("livrer")
            || lower.contains("v1")
        {
            TaskTier::Hard
        } else if lower.contains("bug")
            || lower.contains("fix")
            || lower.contains("corrige")
            || lower.contains("debug")
        {
            TaskTier::Small
        } else if lower.contains("routing")
            || lower.contains("routeur")
            || lower.contains("modèle")
            || lower.contains("modele")
            || lower.contains("model")
            || lower.contains("sélectionne")
            || lower.contains("selectionne")
        {
            TaskTier::Small
        } else if lower.len() < 80 {
            TaskTier::Trivial
        } else {
            TaskTier::Medium
        }
    }

    fn task_summary(&self, task: &str, tier: &TaskTier) -> String {
        let lower = task.to_lowercase();
        if lower.contains("routing")
            || lower.contains("routeur")
            || lower.contains("modèle")
            || lower.contains("modele")
            || lower.contains("model")
        {
            "question meta sur le routing modele".into()
        } else if lower.contains("code") || lower.contains("bug") || lower.contains("fix") {
            format!("requete code/{:?}", tier).to_lowercase()
        } else if lower.contains("config") || lower.contains("provider") {
            "configuration provider/modele".into()
        } else {
            format!("requete {:?}", tier).to_lowercase()
        }
    }

    fn is_routing_question(&self, task: &str) -> bool {
        let lower = task.to_lowercase();
        (lower.contains("routing") || lower.contains("routeur") || lower.contains("route"))
            && (lower.contains("modèle") || lower.contains("modele") || lower.contains("model"))
            || lower.contains("sélectionne tu le model")
            || lower.contains("selectionne tu le model")
    }

    fn requires_tools(&self, task: &str, tier: &TaskTier) -> bool {
        let lower = task.to_lowercase();
        let tool_keywords = [
            "outil",
            "tools",
            "fichier",
            "file",
            "readme",
            ".rs",
            ".ts",
            ".js",
            ".html",
            ".md",
            "repo",
            "dossier",
            "workspace",
            "git",
            "test",
            "build",
            "cargo",
            "npm",
            "pnpm",
            "corrige",
            "fix",
            "debug",
            "bug",
            "répare",
            "repare",
            "modifie",
            "édite",
            "edite",
            "ajoute",
            "supprime",
            "écris",
            "ecris",
            "write",
            "create",
            "crée",
            "cree",
            "audit",
        ];

        if tool_keywords.iter().any(|kw| lower.contains(kw)) {
            return true;
        }

        matches!(tier, TaskTier::Medium | TaskTier::Hard | TaskTier::Vision)
    }

    fn requires_vision(&self, task: &str, tier: &TaskTier) -> bool {
        let lower = task.to_lowercase();
        matches!(tier, TaskTier::Vision)
            || [
                "image",
                "screenshot",
                "capture",
                "photo",
                "vision",
                "logo",
                "visuel",
                "interface graphique",
            ]
            .iter()
            .any(|kw| lower.contains(kw))
    }

    fn routing_explanation(
        &self,
        tier: &TaskTier,
        need: &crate::router::RoutingNeed,
        chain_ids: &[String],
    ) -> String {
        format!(
            "Je suis Sparrow, donc je ne réponds pas comme un modèle isolé: avant chaque run, mon routeur classe ta demande puis choisit une chaîne de modèles.\n\nPour cette requête, j'ai détecté: tier `{}` · tools `{}` · vision `{}` · local `{}`.\n\nJe sélectionne ensuite le modèle avec ces critères: adéquation aux capacités demandées, support des tools, besoin vision, préférence local/free-first, budget restant, latence, taille de contexte, puis disponibilité provider. Le résultat est une fallback chain, pas un seul choix figé: `{}`.\n\nConcrètement: une question simple ou meta doit aller vers le modèle le moins coûteux capable de répondre; une tâche code complexe monte vers un modèle plus fort; une tâche avec fichiers/tools exige un modèle compatible tools; une tâche image demande vision; si un provider échoue, je bascule au suivant dans la chaîne.",
            tier.as_str(),
            need.required_tools,
            need.required_vision,
            need.prefer_local,
            chain_ids.join(" -> ")
        )
    }

    /// Drive one AgentRun to completion.
    pub async fn drive(
        &self,
        task: Task,
        event_tx: mpsc::UnboundedSender<Event>,
    ) -> anyhow::Result<OutcomeSummary> {
        self.drive_with_run_id(task, event_tx, RunId::new()).await
    }

    /// Drive one AgentRun with a caller-provided run id.
    pub async fn drive_with_run_id(
        &self,
        task: Task,
        event_tx: mpsc::UnboundedSender<Event>,
        run_id: RunId,
    ) -> anyhow::Result<OutcomeSummary> {
        let mut messages: Vec<Msg> = task.context.clone();

        // Classify task
        let tier = self.classify(&task.description);
        let task_summary = self.task_summary(&task.description, &tier);

        // Route: select brain chain
        let budget = BudgetState {
            daily_limit_usd: self.config.budget.daily_usd,
            daily_spent_usd: 0.0,
            session_limit_usd: self.config.budget.session_usd,
            session_spent_usd: 0.0,
        };

        let required_tools = self.requires_tools(&task.description, &tier);
        let required_vision = self.requires_vision(&task.description, &tier);
        let need = crate::router::RoutingNeed {
            tier: tier.clone(),
            required_tools,
            required_vision,
            prefer_local: false,
        };

        let chain = self.router.select(&need, &budget);
        let chain_ids: Vec<String> = chain.iter().map(|b| b.id().to_string()).collect();

        let _ = event_tx.send(Event::RunStarted {
            run: run_id.clone(),
            task: task.description.clone(),
            agent: "sparrow".into(),
        });

        let _ = event_tx.send(Event::Message {
            run: run_id.clone(),
            role: "router".into(),
            text: format!(
                "requete: {} · tier: {} · tools: {} · vision: {} · local: {}",
                task_summary,
                tier.as_str(),
                need.required_tools,
                need.required_vision,
                need.prefer_local
            ),
        });

        let _ = event_tx.send(Event::RouteSelected {
            run: run_id.clone(),
            chain: chain_ids.clone(),
        });

        if chain.is_empty() {
            let _ = event_tx.send(Event::Error {
                run: run_id.clone(),
                message: "No available models (budget exhausted or no providers configured)".into(),
            });
            return Ok(OutcomeSummary {
                status: "error: no models".into(),
                diffs: vec![],
                cost_usd: 0.0,
                tokens: TokenUsage {
                    input: 0,
                    output: 0,
                },
            });
        }

        if self.is_routing_question(&task.description) {
            let text = self.routing_explanation(&tier, &need, &chain_ids);
            let input_tokens =
                estimate_text_tokens(&task.description) + estimate_text_tokens(&task_summary);
            let output_tokens = estimate_text_tokens(&text);
            let _ = event_tx.send(Event::TokenUsageEstimated {
                run: run_id.clone(),
                input: input_tokens,
                output: 0,
                reason: "router meta request estimate".into(),
            });
            let _ = event_tx.send(Event::TokenUsageEstimated {
                run: run_id.clone(),
                input: 0,
                output: output_tokens,
                reason: "router meta response estimate".into(),
            });
            let _ = event_tx.send(Event::ThinkingDelta {
                run: run_id.clone(),
                text: text.clone(),
            });
            let outcome = OutcomeSummary {
                status: "completed".into(),
                diffs: vec![],
                cost_usd: 0.0,
                tokens: TokenUsage {
                    input: input_tokens,
                    output: output_tokens,
                },
            };
            let _ = event_tx.send(Event::RunFinished {
                run: run_id.clone(),
                outcome: outcome.clone(),
            });
            return Ok(outcome);
        }

        // Build tools and workspace
        let workspace_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let sandbox: Arc<dyn Sandbox> = if self.config.defaults.sandbox == "local-hardened" {
            Arc::new(crate::sandbox::LocalSandbox::hardened(
                workspace_root.clone(),
            ))
        } else {
            Arc::new(crate::sandbox::LocalSandbox::new(workspace_root.clone()))
        };

        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(crate::tools::fs::FsRead));
        registry.register(Arc::new(crate::tools::fs::FsList));
        registry.register(Arc::new(crate::tools::fs::FsWrite));
        registry.register(Arc::new(crate::tools::edit::Edit));
        registry.register(Arc::new(crate::tools::edit::MultiEdit));
        registry.register(Arc::new(crate::tools::search_and_web::Search));
        registry.register(Arc::new(crate::tools::search_and_web::WebSearch));
        registry.register(Arc::new(crate::tools::search_and_web::WebFetch));
        registry.register(Arc::new(crate::tools::git::Git));
        registry.register(Arc::new(crate::tools::todo::Todo));
        registry.register(Arc::new(crate::tools::exec::Exec::new(sandbox.clone())));
        let tools = Arc::new(registry);
        let tool_specs: Vec<ToolSpec> = tools.to_specs();

        let workspace = Workspace {
            root: workspace_root,
            sandbox,
        };

        let identity = self.identity.clone().unwrap_or_else(|| Identity {
            name: "sparrow".into(),
            role: "senior software engineer".into(),
            personality: "concise, competent, direct".into(),
        });

        let brain_policy = BrainPolicy {
            chain,
            current_index: 0,
        };

        let mut autonomy = match self.config.defaults.autonomy {
            AutonomyLevel::Supervised => AutonomyContract::supervised(),
            AutonomyLevel::Trusted => AutonomyContract::trusted(),
            AutonomyLevel::Autonomous => AutonomyContract::autonomous(),
        };
        autonomy.budget.max_usd = self.config.budget.session_usd;
        let _ = event_tx.send(Event::AutonomyChanged {
            run: run_id.clone(),
            level: autonomy.level.clone(),
        });

        // Load relevant skills
        let relevant_skills: Vec<crate::capabilities::Skill> = self
            .skills
            .as_ref()
            .map(|s| s.relevant(&task.description, 3))
            .unwrap_or_default();

        let system = build_system_prompt(
            &identity,
            &workspace.root,
            &self
                .memory
                .as_ref()
                .map(|m| m.all_facts())
                .unwrap_or_default(),
            &relevant_skills,
        );
        let system = format!(
            "{}\n\n## Active Sparrow Routing Context\nRequest category: {}\nTask tier: {}\nRequired tools: {}\nRequired vision: {}\nPreferred local: {}\nSelected fallback chain: {}\nRouting policy: free_first={}, session_budget_usd={:.2}.\nWhen answering routing questions, describe this context concretely.",
            system,
            task_summary,
            tier.as_str(),
            need.required_tools,
            need.required_vision,
            need.prefer_local,
            chain_ids.join(" -> "),
            self.config.routing.free_first,
            self.config.budget.session_usd
        );

        // Build initial messages
        messages.push(Msg {
            role: "user".into(),
            content: vec![ContentBlock::Text {
                text: task.description.clone(),
            }],
        });

        let mut total_input: u64 = 0;
        let mut total_output: u64 = 0;
        let mut estimated_input_unconfirmed: u64 = 0;
        let mut estimated_output_unconfirmed: u64 = 0;
        let mut estimated_cost_unconfirmed: f64 = 0.0;
        let mut cost_usd: f64 = 0.0;
        let diffs: Vec<crate::event::FileDiff> = Vec::new();
        let mut current_chain_idx = 0usize;
        let mut tool_results_pending: Vec<(String, String, serde_json::Value, String, bool)> =
            Vec::new();
        let budget_session = self.config.budget.session_usd;
        let _budget_daily = self.config.budget.daily_usd;
        let redaction = &self.redaction;
        let mut had_error = false;
        let mut last_error: Option<String> = None;
        let mut waiting_for_approval = false;
        let mut denied_by_approval = false;
        let mut skill_evidence = String::new();

        // Helper to send redacted events
        let send = |event: Event| {
            let _ = event_tx.send(redaction.redact_event(&event));
        };

        // Main agentic loop
        loop {
            // Budget check: hard stop if exceeded
            if cost_usd + estimated_cost_unconfirmed >= budget_session {
                send(Event::Error {
                    run: run_id.clone(),
                    message: format!(
                        "Budget exceeded: ${:.4} of ${:.2} session cap",
                        cost_usd + estimated_cost_unconfirmed,
                        budget_session
                    ),
                });
                had_error = true;
                last_error = Some("budget exceeded".into());
                break;
            }
            if let Some(_approval_handler) = &self.approval_handler {
                if waiting_for_approval {
                    // Route to approval handler (e.g., Telegram inline buttons)
                    // The handler will resolve and we continue
                }
            }

            // ─── Org policy enforcement ──────────────────────────────────
            if let Some(ref policy) = self.org_policy {
                let proposed_file = tool_results_pending
                    .last()
                    .map(|(_, _, args, _, _)| {
                        args.get("path").and_then(|v| v.as_str()).unwrap_or("")
                    })
                    .unwrap_or("");
                if let Err(violation) =
                    policy.enforce(&self.config.defaults.autonomy, cost_usd, proposed_file)
                {
                    send(Event::Error {
                        run: run_id.clone(),
                        message: format!("Org policy violation: {}", violation),
                    });
                    break;
                }
            }

            let brain = match brain_policy.chain.get(current_chain_idx) {
                Some(b) => b.clone(),
                None => break,
            };

            let caps = brain.caps();

            let req = BrainRequest {
                system: Some(system.clone()),
                messages: messages.clone(),
                tools: if need.required_tools {
                    tool_specs.clone()
                } else {
                    vec![]
                },
                max_tokens: caps.max_output as u32,
                temperature: 0.0,
                stop: vec![],
            };

            let estimated_input = estimate_request_tokens(&req);
            estimated_input_unconfirmed += estimated_input;
            estimated_cost_unconfirmed +=
                caps.cost_input_per_mtok * (estimated_input as f64) / 1_000_000.0;
            let _ = event_tx.send(Event::TokenUsageEstimated {
                run: run_id.clone(),
                input: estimated_input,
                output: 0,
                reason: "prompt estimate before provider usage".into(),
            });
            let _ = event_tx.send(Event::CostUpdate {
                run: run_id.clone(),
                usd: cost_usd + estimated_cost_unconfirmed,
            });

            let _ = event_tx.send(Event::AgentStatus {
                run: run_id.clone(),
                role: "main".into(),
                status: AgentStatus::Thinking,
                note: format!("using {}", brain.id()),
            });

            match brain.complete(req).await {
                Ok(mut stream) => {
                    let mut current_tool_name = String::new();
                    let mut current_tool_json = String::new();
                    let mut output_chars_seen: u64 = 0;
                    let mut output_tokens_emitted: u64 = 0;
                    let mut continue_agent_loop = false;
                    let mut stop_after_tool_result = false;
                    let mut assistant_text = String::new();
                    let mut tool_output_seen_this_completion = false;

                    while let Some(event) = stream.next().await {
                        match event {
                            BrainEvent::TextDelta(text) => {
                                assistant_text.push_str(&text);
                                output_chars_seen += text.chars().count() as u64;
                                let estimated_output = (output_chars_seen + 3) / 4;
                                let output_delta =
                                    estimated_output.saturating_sub(output_tokens_emitted);
                                if output_delta > 0 {
                                    output_tokens_emitted += output_delta;
                                    estimated_output_unconfirmed += output_delta;
                                    estimated_cost_unconfirmed += caps.cost_output_per_mtok
                                        * (output_delta as f64)
                                        / 1_000_000.0;
                                    let _ = event_tx.send(Event::TokenUsageEstimated {
                                        run: run_id.clone(),
                                        input: 0,
                                        output: output_delta,
                                        reason: "streamed output estimate".into(),
                                    });
                                    let _ = event_tx.send(Event::CostUpdate {
                                        run: run_id.clone(),
                                        usd: cost_usd + estimated_cost_unconfirmed,
                                    });
                                }
                                let _ = event_tx.send(Event::ThinkingDelta {
                                    run: run_id.clone(),
                                    text: text.clone(),
                                });
                            }
                            BrainEvent::ToolUseStart { id, name } => {
                                current_tool_name = name.clone();
                                current_tool_json.clear();
                                let risk = tools
                                    .get(&name)
                                    .map(|tool| tool.risk())
                                    .unwrap_or(RiskLevel::ReadOnly);
                                let _ = event_tx.send(Event::ToolUseProposed {
                                    run: run_id.clone(),
                                    id: id.clone(),
                                    name: name.clone(),
                                    args: json!({}),
                                    risk,
                                });
                            }
                            BrainEvent::ToolUseDelta { id, json } => {
                                let _ = id;
                                current_tool_json.push_str(&json);
                            }
                            BrainEvent::ToolUseEnd { id } => {
                                // Parse accumulated JSON
                                let args: serde_json::Value =
                                    serde_json::from_str(&current_tool_json).unwrap_or(json!({}));

                                // Check autonomy gate
                                let tool_name = if current_tool_name.is_empty() {
                                    "unknown".to_string()
                                } else {
                                    current_tool_name.clone()
                                };
                                let tool = tools.get(&tool_name);
                                let risk = tool
                                    .as_ref()
                                    .map(|tool| tool.risk())
                                    .unwrap_or(RiskLevel::ReadOnly);
                                let proposed = crate::autonomy::ProposedAction {
                                    tool_name: tool_name.clone(),
                                    risk,
                                    args: args.clone(),
                                };

                                let mut decision = autonomy.decide(&proposed);
                                if matches!(decision, Decision::AskUser) {
                                    let summary = format!(
                                        "Approve {} with args: {}",
                                        proposed.tool_name, args
                                    );
                                    let _ = event_tx.send(Event::ApprovalRequested {
                                        run: run_id.clone(),
                                        id: id.clone(),
                                        summary: summary.clone(),
                                    });
                                    if let Some(handler) = &self.approval_handler {
                                        decision = handler
                                            .request_approval(ApprovalRequest {
                                                run: run_id.clone(),
                                                id: id.clone(),
                                                tool_name: proposed.tool_name.clone(),
                                                risk: proposed.risk.clone(),
                                                args: args.clone(),
                                                summary,
                                            })
                                            .await;
                                    }
                                }

                                let _ = event_tx.send(Event::ApprovalResolved {
                                    run: run_id.clone(),
                                    id: id.clone(),
                                    decision: decision.clone(),
                                });

                                match decision {
                                    Decision::Allow => {
                                        // Auto-checkpoint before mutating/exec/destructive
                                        if matches!(
                                            proposed.risk,
                                            RiskLevel::Mutating
                                                | RiskLevel::Exec
                                                | RiskLevel::Destructive
                                        ) {
                                            let checkpoints =
                                                GitCheckpoints::new(workspace.root.clone());
                                            if let Ok(cp_id) = checkpoints
                                                .snapshot(&format!("pre-{}", proposed.tool_name))
                                            {
                                                let _ = event_tx.send(Event::CheckpointCreated {
                                                    run: run_id.clone(),
                                                    id: cp_id,
                                                    label: format!("pre-{}", proposed.tool_name),
                                                });
                                            }
                                        }

                                        let _ = event_tx.send(Event::ToolUseStarted {
                                            run: run_id.clone(),
                                            id: id.clone(),
                                        });

                                        let result = if let Some(tool) = tool {
                                            let ctx = ToolCtx {
                                                workspace_root: workspace.root.clone(),
                                                run_id: run_id.clone(),
                                            };
                                            match tool.call(args.clone(), &ctx).await {
                                                Ok(result) => result,
                                                Err(e) => crate::tools::ToolResult::error(format!(
                                                    "Tool {} failed: {}",
                                                    proposed.tool_name, e
                                                )),
                                            }
                                        } else {
                                            crate::tools::ToolResult::error(format!(
                                                "Unknown tool: {}",
                                                proposed.tool_name
                                            ))
                                        };

                                        for block in &result.content {
                                            if let Block::Diff { file, patch } = block {
                                                let plus = patch
                                                    .lines()
                                                    .filter(|l| {
                                                        l.starts_with('+') && !l.starts_with("+++")
                                                    })
                                                    .count()
                                                    as u32;
                                                let minus = patch
                                                    .lines()
                                                    .filter(|l| {
                                                        l.starts_with('-') && !l.starts_with("---")
                                                    })
                                                    .count()
                                                    as u32;
                                                let _ = event_tx.send(Event::DiffProposed {
                                                    run: run_id.clone(),
                                                    file: file.clone(),
                                                    patch: patch.clone(),
                                                    plus,
                                                    minus,
                                                });
                                            }
                                        }

                                        let blocks = result.content.clone();
                                        let text = tool_result_text(&blocks);
                                        let is_error = result.is_error;
                                        skill_evidence.push_str(&text);
                                        skill_evidence.push('\n');
                                        let _ = event_tx.send(Event::ToolOutput {
                                            run: run_id.clone(),
                                            id: id.clone(),
                                            blocks,
                                        });
                                        tool_output_seen_this_completion = true;
                                        tool_results_pending.push((
                                            id.clone(),
                                            proposed.tool_name.clone(),
                                            args.clone(),
                                            text,
                                            is_error,
                                        ));
                                    }
                                    Decision::AskUser => {
                                        // Supervised mode: prompt user on stdin
                                        waiting_for_approval = true;
                                        let approval_id = id.clone();
                                        let approval_name = proposed.tool_name.clone();
                                        let approval_args = args.clone();
                                        let approval_risk = proposed.risk;

                                        // Emit approval requested
                                        let _ = event_tx.send(Event::ApprovalRequested {
                                            run: run_id.clone(),
                                            id: approval_id.clone(),
                                            summary: format!(
                                                "{} tool '{}' with args: {}",
                                                format!("{:?}", approval_risk),
                                                approval_name,
                                                approval_args
                                            ),
                                        });

                                        // Wait for user input on stdin
                                        use std::io::{self, Write};
                                        print!(
                                            "\n\x1b[1;33mApprove {}? [y/N]\x1b[0m ",
                                            approval_name
                                        );
                                        io::stdout().flush().ok();
                                        let mut input = String::new();
                                        io::stdin().read_line(&mut input).ok();
                                        let approved = input.trim().to_lowercase() == "y";

                                        if approved {
                                            waiting_for_approval = false;
                                            // Auto-checkpoint before mutating/exec/destructive
                                            if matches!(
                                                approval_risk,
                                                RiskLevel::Mutating
                                                    | RiskLevel::Exec
                                                    | RiskLevel::Destructive
                                            ) {
                                                let checkpoints =
                                                    GitCheckpoints::new(workspace.root.clone());
                                                if let Ok(cp_id) = checkpoints
                                                    .snapshot(&format!("pre-{}", approval_name))
                                                {
                                                    let _ =
                                                        event_tx.send(Event::CheckpointCreated {
                                                            run: run_id.clone(),
                                                            id: cp_id,
                                                            label: format!("pre-{}", approval_name),
                                                        });
                                                }
                                            }
                                            let _ = event_tx.send(Event::ToolUseStarted {
                                                run: run_id.clone(),
                                                id: approval_id.clone(),
                                            });
                                            let result = if let Some(tool) = tool {
                                                let ctx = ToolCtx {
                                                    workspace_root: workspace.root.clone(),
                                                    run_id: run_id.clone(),
                                                };
                                                match tool.call(approval_args.clone(), &ctx).await {
                                                    Ok(r) => r,
                                                    Err(e) => {
                                                        crate::tools::ToolResult::error(format!(
                                                            "Tool {} failed: {}",
                                                            approval_name, e
                                                        ))
                                                    }
                                                }
                                            } else {
                                                crate::tools::ToolResult::error(format!(
                                                    "Unknown tool: {}",
                                                    approval_name
                                                ))
                                            };
                                            let blocks = result.content.clone();
                                            let text = tool_result_text(&blocks);
                                            let is_error = result.is_error;
                                            skill_evidence.push_str(&text);
                                            skill_evidence.push('\n');
                                            let _ = event_tx.send(Event::ToolOutput {
                                                run: run_id.clone(),
                                                id: approval_id.clone(),
                                                blocks,
                                            });
                                            tool_output_seen_this_completion = true;
                                            tool_results_pending.push((
                                                approval_id,
                                                approval_name,
                                                approval_args,
                                                text,
                                                is_error,
                                            ));
                                        } else {
                                            let _ = event_tx.send(Event::ToolOutput {
                                                run: run_id.clone(),
                                                id: approval_id.clone(),
                                                blocks: vec![Block::Text("Denied by user".into())],
                                            });
                                            tool_output_seen_this_completion = true;
                                            tool_results_pending.push((
                                                approval_id,
                                                approval_name,
                                                approval_args,
                                                "Denied by user".into(),
                                                true,
                                            ));
                                        }
                                    }
                                    Decision::Deny => {
                                        denied_by_approval = true;
                                        stop_after_tool_result = true;
                                        let _ = event_tx.send(Event::ToolOutput {
                                            run: run_id.clone(),
                                            id: id.clone(),
                                            blocks: vec![Block::Text(
                                                "Denied by autonomy policy".into(),
                                            )],
                                        });
                                        tool_output_seen_this_completion = true;
                                        tool_results_pending.push((
                                            id.clone(),
                                            proposed.tool_name.clone(),
                                            args.clone(),
                                            "Denied by autonomy policy".into(),
                                            true,
                                        ));
                                    }
                                }

                                current_tool_json.clear();
                                current_tool_name.clear();
                            }
                            BrainEvent::Usage(usage) => {
                                total_input += usage.input;
                                total_output += usage.output;
                                estimated_input_unconfirmed =
                                    estimated_input_unconfirmed.saturating_sub(usage.input);
                                estimated_output_unconfirmed =
                                    estimated_output_unconfirmed.saturating_sub(usage.output);
                                let _ = event_tx.send(Event::TokenUsage {
                                    run: run_id.clone(),
                                    input: usage.input,
                                    output: usage.output,
                                });

                                // Calculate cost
                                let input_cost =
                                    caps.cost_input_per_mtok * (usage.input as f64) / 1_000_000.0;
                                let output_cost =
                                    caps.cost_output_per_mtok * (usage.output as f64) / 1_000_000.0;
                                let actual_cost = input_cost + output_cost;
                                cost_usd += actual_cost;
                                estimated_cost_unconfirmed =
                                    (estimated_cost_unconfirmed - actual_cost).max(0.0);

                                let _ = event_tx.send(Event::CostUpdate {
                                    run: run_id.clone(),
                                    usd: cost_usd + estimated_cost_unconfirmed,
                                });
                            }
                            BrainEvent::Done(reason) => {
                                match reason {
                                    crate::event::StopReason::EndTurn => {
                                        if !assistant_text.trim().is_empty() {
                                            let assistant_msg = Msg {
                                                role: "assistant".into(),
                                                content: vec![ContentBlock::Text {
                                                    text: assistant_text.clone(),
                                                }],
                                            };
                                            let turn_messages = vec![assistant_msg.clone()];
                                            let has_verified_tool_context =
                                                tool_output_seen_this_completion
                                                    || messages.iter().any(|m| {
                                                        m.content.iter().any(|block| {
                                                            matches!(
                                                                block,
                                                                ContentBlock::ToolResult { .. }
                                                            )
                                                        })
                                                    });

                                            if let Some(correction) = self.reasoning.guard_turn(
                                                &turn_messages,
                                                has_verified_tool_context,
                                            ) {
                                                messages.push(assistant_msg);
                                                let _ = event_tx.send(Event::Message {
                                                    run: run_id.clone(),
                                                    role: "guard".into(),
                                                    text: correction.clone(),
                                                });
                                                messages.push(Msg {
                                                    role: "user".into(),
                                                    content: vec![ContentBlock::Text {
                                                        text: format!("SYSTEM: {}. Execute the relevant tool first, then report the actual raw result.", correction),
                                                    }],
                                                });
                                                continue_agent_loop = true;
                                                break;
                                            }

                                            skill_evidence.push_str(&assistant_text);
                                            skill_evidence.push('\n');
                                            messages.push(assistant_msg);
                                        }
                                    }
                                    crate::event::StopReason::ToolUse => {
                                        // Feed tool results back
                                        for (tool_id, tool_name, args, text, is_error) in
                                            tool_results_pending.drain(..)
                                        {
                                            messages.push(Msg {
                                                role: "assistant".into(),
                                                content: vec![ContentBlock::ToolUse {
                                                    id: tool_id.clone(),
                                                    name: tool_name,
                                                    input: args,
                                                }],
                                            });
                                            messages.push(Msg {
                                                role: "user".into(),
                                                content: vec![ContentBlock::ToolResult {
                                                    tool_use_id: tool_id,
                                                    content: vec![ContentBlock::Text { text }],
                                                    is_error: Some(is_error),
                                                }],
                                            });
                                        }
                                        continue_agent_loop =
                                            !waiting_for_approval && !stop_after_tool_result;
                                        break;
                                    }
                                    _ => {}
                                }
                                break; // Done
                            }
                            BrainEvent::Error(msg) => {
                                let _ = event_tx.send(Event::Error {
                                    run: run_id.clone(),
                                    message: msg.clone(),
                                });
                                let next_idx = current_chain_idx + 1;
                                if next_idx < brain_policy.chain.len() {
                                    current_chain_idx = next_idx;
                                    let _ = event_tx.send(Event::ModelSwitched {
                                        run: run_id.clone(),
                                        from: brain.id().to_string(),
                                        to: brain_policy.chain[current_chain_idx].id().to_string(),
                                        reason: msg,
                                    });
                                    continue_agent_loop = true;
                                } else {
                                    had_error = true;
                                    last_error = Some(msg);
                                }
                                break;
                            }
                        }
                    }
                    if continue_agent_loop {
                        continue;
                    }
                    break; // Task complete
                }
                Err(e) => {
                    let err_msg = format!("{}", e);
                    let _ = event_tx.send(Event::Error {
                        run: run_id.clone(),
                        message: err_msg.clone(),
                    });

                    // Try next in chain
                    let next_idx = current_chain_idx + 1;
                    if next_idx < brain_policy.chain.len() {
                        current_chain_idx = next_idx;
                        let _ = event_tx.send(Event::ModelSwitched {
                            run: run_id.clone(),
                            from: brain.id().to_string(),
                            to: brain_policy.chain[current_chain_idx].id().to_string(),
                            reason: err_msg,
                        });
                    } else {
                        had_error = true;
                        last_error = Some(err_msg);
                        break;
                    }
                }
            }
        }

        let outcome = OutcomeSummary {
            status: if had_error {
                format!(
                    "error: {}",
                    last_error.unwrap_or_else(|| "run failed".into())
                )
            } else if waiting_for_approval {
                "waiting_for_approval".into()
            } else if denied_by_approval {
                "denied".into()
            } else {
                "completed".into()
            },
            diffs,
            cost_usd: cost_usd + estimated_cost_unconfirmed,
            tokens: TokenUsage {
                input: total_input + estimated_input_unconfirmed,
                output: total_output + estimated_output_unconfirmed,
            },
        };

        // Persist task to memory
        if let Some(mem) = &self.memory {
            let _ = mem.save_task(&crate::memory::TaskMem {
                run_id: run_id.0.clone(),
                messages: messages.clone(),
                created_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            });
        }

        // Propose skill candidate from successful run
        if outcome.status == "completed" {
            if let Some(skills) = &self.skills {
                if let Some(candidate) = Curator::propose_skill_if_missing(
                    &task.description,
                    &skill_evidence,
                    skills.as_ref(),
                ) {
                    let _ = event_tx.send(Event::SkillLearned {
                        run: run_id.clone(),
                        name: candidate.name.clone(),
                    });
                    let _ = skills.add(candidate);
                }
            }

            // Auto-distill facts from successful run
            if let Some(mem) = &self.memory {
                let events: Vec<Event> = Vec::new(); // In production, collect from event stream
                Distiller::distill(mem, &events, &task.description).await;
            }
        }

        let _ = event_tx.send(Event::RunFinished {
            run: run_id.clone(),
            outcome: outcome.clone(),
        });

        Ok(outcome)
    }
}
