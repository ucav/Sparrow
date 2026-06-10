use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::mpsc;

use super::{Tool, ToolCtx, ToolResult};
use crate::config::Config;
use crate::engine::{Engine, Identity, Task};
use crate::event::{Block, Event, RiskLevel};
use crate::memory::Memory;
use crate::permissions::PermissionMode;
use crate::router::Router;

// ─── Subagent spawn ─────────────────────────────────────────────────────────────

/// Delegates a subtask to a child AgentRun with its own conversation and sandbox.
/// §15: "Each subagent gets its own conversation, terminal, and a Python RPC channel."
///
/// Holds the router + config (not a parent Engine) so it can build a fresh child
/// engine per call — this avoids a self-referential Arc<Engine> at registration.
pub struct SubagentSpawn {
    router: Arc<dyn Router>,
    config: Config,
    memory: Option<Arc<dyn Memory>>,
}

impl SubagentSpawn {
    pub fn new(router: Arc<dyn Router>, config: Config) -> Self {
        Self {
            router,
            config,
            memory: None,
        }
    }

    pub fn with_memory(mut self, memory: Arc<dyn Memory>) -> Self {
        self.memory = Some(memory);
        self
    }
}

#[async_trait]
impl Tool for SubagentSpawn {
    fn name(&self) -> &str {
        "subagent_spawn"
    }
    fn description(&self) -> &str {
        "Spawn a child agent to handle a subtask independently"
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "task": { "type": "string", "description": "Subtask description" },
                "role": { "type": "string", "description": "Role for the subagent (e.g. tester, researcher, reviewer)" },
                "model": { "type": "string", "description": "Optional: provider:model or provider/model for the subagent" },
                "permission_mode": { "type": "string", "description": "Optional: read-only, plan, supervised, trusted, autonomous, emergency-stop" },
                "tools": { "type": "array", "items": { "type": "string" }, "description": "Optional explicit allowed tool patterns" },
                "disallowed_tools": { "type": "array", "items": { "type": "string" }, "description": "Optional denied tool patterns for this subagent" }
            },
            "required": ["task"]
        })
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::Exec
    }
    async fn call(&self, args: serde_json::Value, _ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        let task_desc = args["task"].as_str().unwrap_or("");
        let role = args["role"].as_str().unwrap_or("helper");
        let mut child_config = self.config.clone();
        if let Some(model_ref) = args["model"].as_str() {
            if let Some((provider, model)) = parse_model_ref(model_ref) {
                child_config.forced_model = Some((provider.clone(), model.clone()));
                for tier in ["trivial", "small", "medium", "hard", "vision"] {
                    child_config
                        .routing
                        .policy
                        .insert(tier.to_string(), provider.clone());
                }
                child_config
                    .providers
                    .entry(provider)
                    .or_insert_with(|| crate::config::ProviderConfig {
                        adapter: "openai-compatible".into(),
                        base_url: None,
                        models: vec![],
                        api_key_env: None,
                    })
                    .models = vec![model];
            }
        }
        if let Some(mode) = args["permission_mode"]
            .as_str()
            .and_then(PermissionMode::parse)
        {
            child_config.defaults.autonomy = mode.autonomy_level();
            child_config.permissions.mode = mode;
        }
        for tool in string_array(&args["tools"]) {
            if !child_config.permissions.tools.allow.contains(&tool) {
                child_config.permissions.tools.allow.push(tool);
            }
        }
        for tool in string_array(&args["disallowed_tools"]) {
            if !child_config.permissions.tools.deny.contains(&tool) {
                child_config.permissions.tools.deny.push(tool);
            }
        }

        let (tx, mut rx) = mpsc::unbounded_channel();

        let task = Task {
            description: task_desc.to_string(),
            context: vec![],
        };

        // Build a fresh child engine for this subagent.
        let mut child = Engine::new(self.router.clone(), child_config).with_identity(Identity {
            name: role.to_string(),
            role: role.to_string(),
            personality: format!("Focused {} subagent. Be concise and return evidence.", role),
        });
        if let Some(mem) = &self.memory {
            child = child.with_memory(mem.clone());
        }
        let engine = Arc::new(child);

        let handle = tokio::spawn(async move {
            match engine.drive(task, tx).await {
                Ok(outcome) => outcome,
                Err(e) => crate::event::OutcomeSummary {
                    status: format!("error: {}", e),
                    diffs: vec![],
                    cost_usd: 0.0,
                    tokens: crate::event::TokenUsage {
                        input: 0,
                        output: 0,
                    },
                    cost_comparison: String::new(),
                    duration_ms: None,
                },
            }
        });

        // Collect subagent output
        let mut output = String::new();
        while let Some(event) = rx.recv().await {
            match &event {
                Event::ThinkingDelta { text, .. } => {
                    output.push_str(text);
                }
                Event::AgentStatus { note, .. } => {
                    output.push_str(&format!("\n[{}]", note));
                }
                Event::RunFinished { outcome, .. } => {
                    output.push_str(&format!(
                        "\n[Subagent done: {} | ${:.4}]",
                        outcome.status, outcome.cost_usd
                    ));
                }
                Event::Error { message, .. } => {
                    output.push_str(&format!("\n[Error: {}]", message));
                }
                _ => {}
            }
        }

        let outcome = handle
            .await
            .unwrap_or_else(|e| crate::event::OutcomeSummary {
                status: format!("subagent panicked: {}", e),
                diffs: vec![],
                cost_usd: 0.0,
                tokens: crate::event::TokenUsage {
                    input: 0,
                    output: 0,
                },
                cost_comparison: String::new(),
                duration_ms: None,
            });

        Ok(ToolResult::ok(vec![Block::Text(format!(
            "Subagent '{}' completed.\nStatus: {}\nOutput:\n{}",
            role, outcome.status, output
        ))]))
    }
}

fn string_array(value: &serde_json::Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str())
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn parse_model_ref(model_ref: &str) -> Option<(String, String)> {
    let model_ref = model_ref.trim();
    if model_ref.is_empty() {
        return None;
    }
    if let Some((provider, model)) = model_ref.split_once(':') {
        let provider = provider.trim();
        let model = model.trim();
        if !provider.is_empty() && !model.is_empty() {
            return Some((provider.to_string(), model.to_string()));
        }
    }
    if let Some((provider, rest)) = model_ref.split_once('/') {
        let provider = provider.trim();
        if !provider.is_empty() {
            return Some((provider.to_string(), model_ref.to_string()));
        }
        if !rest.trim().is_empty() {
            return Some(("custom".into(), model_ref.to_string()));
        }
    }
    Some(("custom".into(), model_ref.to_string()))
}

// ─── Persistent Python kernel ─────────────────────────────────────────────────
// A long-lived `python3` process that keeps a single globals dict across calls,
// so variables/imports/state persist between tool invocations (§15). A small
// driver loop reads one JSON request per line, execs it capturing stdout, and
// emits a JSON result terminated by a unique sentinel.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout};
use std::sync::Mutex;

const KERNEL_SENTINEL: &str = "__SPARROW_KERNEL_END__";

const KERNEL_DRIVER: &str = r#"
import sys, io, json, contextlib, traceback
_g = {"__name__": "__sparrow__"}
SENT = "__SPARROW_KERNEL_END__"
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        req = json.loads(line)
    except Exception:
        print(json.dumps({"out": "", "err": "bad request"}), flush=True)
        print(SENT, flush=True)
        continue
    code = req.get("code", "")
    buf = io.StringIO()
    err = ""
    try:
        with contextlib.redirect_stdout(buf):
            exec(compile(code, "<sparrow>", "exec"), _g)
    except Exception:
        err = traceback.format_exc()
    print(json.dumps({"out": buf.getvalue(), "err": err}), flush=True)
    print(SENT, flush=True)
"#;

struct Kernel {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

pub struct PythonRpc {
    kernel: Mutex<Option<Kernel>>,
    python_bin: String,
}

impl PythonRpc {
    pub fn new() -> Self {
        // Prefer python3, fall back to python (Windows often only has `python`).
        let python_bin = if which_python("python3") {
            "python3".to_string()
        } else {
            "python".to_string()
        };
        Self {
            kernel: Mutex::new(None),
            python_bin,
        }
    }

    fn ensure_kernel(&self, kernel: &mut Option<Kernel>) -> anyhow::Result<()> {
        if kernel.is_some() {
            return Ok(());
        }
        use std::process::{Command, Stdio};
        let mut child = Command::new(&self.python_bin)
            .arg("-u")
            .arg("-c")
            .arg(KERNEL_DRIVER)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("no stdin"))?;
        let stdout = BufReader::new(
            child
                .stdout
                .take()
                .ok_or_else(|| anyhow::anyhow!("no stdout"))?,
        );
        *kernel = Some(Kernel {
            child,
            stdin,
            stdout,
        });
        Ok(())
    }
}

fn which_python(bin: &str) -> bool {
    std::process::Command::new(bin)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[async_trait]
impl Tool for PythonRpc {
    fn name(&self) -> &str {
        "python_rpc"
    }
    fn description(&self) -> &str {
        "Execute Python in a PERSISTENT kernel — variables, imports and state persist across calls."
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "code": { "type": "string", "description": "Python code to execute in the persistent kernel" }
            },
            "required": ["code"]
        })
    }
    fn risk(&self) -> RiskLevel {
        RiskLevel::Exec
    }
    async fn call(&self, args: serde_json::Value, _ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        let code = args["code"].as_str().unwrap_or("").to_string();

        // Kernel IO is blocking; run on a blocking thread without holding the
        // lock across an await. We take the kernel out, use it, put it back.
        let mut guard = self.kernel.lock().unwrap();
        if let Err(e) = self.ensure_kernel(&mut guard) {
            return Ok(ToolResult::error(format!(
                "Python kernel unavailable ({}). Is '{}' installed?",
                e, self.python_bin
            )));
        }
        let kernel = guard.as_mut().unwrap();

        // Send request as a single JSON line.
        let req = serde_json::json!({ "code": code }).to_string();
        if writeln!(kernel.stdin, "{}", req)
            .and_then(|_| kernel.stdin.flush())
            .is_err()
        {
            *guard = None; // kernel died; drop it so it respawns next time
            return Ok(ToolResult::error(
                "Python kernel write failed (kernel reset)",
            ));
        }

        // Read lines until the sentinel; the line before it is the JSON result.
        let mut last_json = String::new();
        loop {
            let mut line = String::new();
            match kernel.stdout.read_line(&mut line) {
                Ok(0) => {
                    *guard = None;
                    return Ok(ToolResult::error("Python kernel closed unexpectedly"));
                }
                Ok(_) => {
                    let trimmed = line.trim_end();
                    if trimmed == KERNEL_SENTINEL {
                        break;
                    }
                    last_json = trimmed.to_string();
                }
                Err(e) => {
                    *guard = None;
                    return Ok(ToolResult::error(format!(
                        "Python kernel read error: {}",
                        e
                    )));
                }
            }
        }

        let parsed: serde_json::Value = serde_json::from_str(&last_json)
            .unwrap_or_else(|_| serde_json::json!({"out": last_json, "err": ""}));
        let out = parsed["out"].as_str().unwrap_or("");
        let err = parsed["err"].as_str().unwrap_or("");
        if !err.is_empty() {
            Ok(ToolResult::ok(vec![Block::Text(format!("{}{}", out, err))]))
        } else {
            Ok(ToolResult::text(out.to_string()))
        }
    }
}

impl Drop for PythonRpc {
    fn drop(&mut self) {
        if let Ok(mut g) = self.kernel.lock() {
            if let Some(mut k) = g.take() {
                let _ = k.child.kill();
            }
        }
    }
}
