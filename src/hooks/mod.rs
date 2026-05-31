use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::event::Event;
use crate::sandbox::Sandbox;

// ─── Hook event types (12 lifecycle points) ────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HookEvent {
    SessionStart,
    PreRun,
    PreToolUse,
    PostToolUse,
    PreCheckpoint,
    PostCheckpoint,
    PostRun,
    OnError,
    OnApprovalRequested,
    OnBudgetThreshold,
    OnSkillLearned,
    OnModelSwitched,
}

impl HookEvent {
    pub fn from_event(event: &Event) -> Option<Self> {
        match event {
            Event::RunStarted { .. } => Some(HookEvent::PreRun),
            Event::ToolUseProposed { .. } => Some(HookEvent::PreToolUse),
            Event::ToolOutput { .. } => Some(HookEvent::PostToolUse),
            Event::CheckpointCreated { .. } => Some(HookEvent::PreCheckpoint),
            Event::RunFinished { .. } => Some(HookEvent::PostRun),
            Event::Error { .. } => Some(HookEvent::OnError),
            Event::ApprovalRequested { .. } => Some(HookEvent::OnApprovalRequested),
            Event::CostUpdate { .. } => Some(HookEvent::OnBudgetThreshold),
            Event::SkillLearned { .. } => Some(HookEvent::OnSkillLearned),
            Event::ModelSwitched { .. } => Some(HookEvent::OnModelSwitched),
            _ => None,
        }
    }
}

// ─── Hook definition ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hook {
    pub id: String,
    pub event: HookEvent,
    /// Regex pattern to match (e.g., tool name for PreToolUse)
    pub matcher: Option<String>,
    /// Shell command to execute (or builtin name)
    pub command: String,
    /// Whether this hook blocks execution until complete
    pub blocking: bool,
    /// Whether this hook is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool { true }

impl Hook {
    pub fn matches(&self, event: &HookEvent, context: &str) -> bool {
        if self.event != *event {
            return false;
        }
        if let Some(ref pattern) = self.matcher {
            if let Ok(re) = regex::Regex::new(pattern) {
                return re.is_match(context);
            }
            return context.contains(pattern.as_str());
        }
        true
    }
}

// ─── Hook result ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct HookResult {
    pub hook_id: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub veto: bool,
    pub veto_reason: Option<String>,
}

// ─── Hook registry ─────────────────────────────────────────────────────────────

pub struct HookRegistry {
    hooks: Vec<Hook>,
    sandbox: Arc<dyn Sandbox>,
}

impl HookRegistry {
    pub fn new(sandbox: Arc<dyn Sandbox>) -> Self {
        Self { hooks: Vec::new(), sandbox }
    }

    pub fn load(&mut self, config_hooks: Vec<Hook>) {
        self.hooks = config_hooks;
    }

    pub fn add(&mut self, hook: Hook) {
        self.hooks.push(hook);
    }

    /// Execute all matching hooks for an event
    pub async fn execute(
        &self,
        event: &HookEvent,
        context: &str,
    ) -> Vec<HookResult> {
        let mut results = Vec::new();

        for hook in &self.hooks {
            if !hook.enabled || !hook.matches(event, context) {
                continue;
            }

            if hook.blocking {
                let result = self.run_command(&hook.command, &hook.id).await;
                if result.exit_code != 0 {
                    let mut r = result;
                    r.veto = true;
                    r.veto_reason = Some(format!(
                        "Hook '{}' blocked action (exit code {})",
                        hook.id, r.exit_code
                    ));
                    results.push(r);
                    break; // Blocking hook with veto stops execution
                }
                results.push(result);
            } else {
                let result = self.run_command(&hook.command, &hook.id).await;
                results.push(result);
            }
        }

        results
    }

    async fn run_command(&self, command: &str, hook_id: &str) -> HookResult {
        let cmd = crate::sandbox::Command {
            program: if cfg!(windows) { "cmd" } else { "sh" }.into(),
            args: vec![
                if cfg!(windows) { "/c" } else { "-c" }.into(),
                command.to_string(),
            ],
            env: HashMap::new(),
            workdir: self.sandbox.root().to_path_buf(),
        };

        let limits = crate::sandbox::Limits {
            timeout_ms: 30_000,
            max_output_bytes: 64 * 1024,
        };

        match self.sandbox.exec(&cmd, &limits).await {
            Ok(output) => HookResult {
                hook_id: hook_id.into(),
                exit_code: output.exit_code,
                stdout: output.stdout,
                stderr: output.stderr,
                veto: false,
                veto_reason: None,
            },
            Err(e) => HookResult {
                hook_id: hook_id.into(),
                exit_code: -1,
                stdout: String::new(),
                stderr: format!("Hook execution failed: {}", e),
                veto: false,
                veto_reason: None,
            },
        }
    }
}

// ─── Default hooks ─────────────────────────────────────────────────────────────

pub fn default_hooks() -> Vec<Hook> {
    vec![
        Hook {
            id: "format-on-edit".into(),
            event: HookEvent::PostToolUse,
            matcher: Some("edit|fs_write".into()),
            command: "echo 'hook: post-edit formatting' ".into(),
            blocking: false,
            enabled: true,
        },
        Hook {
            id: "block-lock-files".into(),
            event: HookEvent::PreToolUse,
            matcher: Some("fs_write".into()),
            command: "echo 'hook: lock file protected' && exit 1".into(),
            blocking: true,
            enabled: false, // Disabled by default — enable to protect lockfiles
        },
        Hook {
            id: "cost-threshold-notify".into(),
            event: HookEvent::OnBudgetThreshold,
            matcher: None,
            command: "echo 'hook: cost threshold reached'".into(),
            blocking: false,
            enabled: true,
        },
    ]
}
