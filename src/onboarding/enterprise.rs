// ─── IDE Integration stubs (WS9) ──────────────────────────────────────────────

/// VS Code extension manifest. The extension is a thin WebSocket client
/// connecting to Sparrow's runtime API (ws://127.0.0.1:9338).
/// No business logic in the extension — all intelligence lives in the core.
pub const VSCODE_EXTENSION_MANIFEST: &str = r#"{
  "name": "sparrow",
  "displayName": "Sparrow",
  "description": "The only CLI you install — now in your editor",
  "version": "0.1.0",
  "publisher": "sparrow-dev",
  "engines": { "vscode": "^1.90.0" },
  "activationEvents": ["onStartupFinished"],
  "main": "./dist/extension.js",
  "contributes": {
    "commands": [
      { "command": "sparrow.chat", "title": "Sparrow: Open Chat" },
      { "command": "sparrow.run", "title": "Sparrow: Run Task" },
      { "command": "sparrow.approve", "title": "Sparrow: Approve" },
      { "command": "sparrow.deny", "title": "Sparrow: Deny" },
      { "command": "sparrow.rewind", "title": "Sparrow: Rewind" }
    ],
    "configuration": {
      "title": "Sparrow",
      "properties": {
        "sparrow.apiUrl": {
          "type": "string", "default": "ws://127.0.0.1:9338/ws",
          "description": "Sparrow API WebSocket URL"
        }
      }
    }
  }
}"#;

/// JetBrains plugin descriptor
pub const JETBRAINS_PLUGIN_XML: &str = r#"<idea-plugin>
  <id>dev.sparrow</id>
  <name>Sparrow</name>
  <vendor>Sparrow</vendor>
  <description>The only CLI you install — now in your IDE</description>
  <depends>com.intellij.modules.platform</depends>
  <extensions defaultExtensionNs="com.intellij">
    <toolWindow id="Sparrow" anchor="right" factoryClass="dev.sparrow.SparrowToolWindowFactory"/>
  </extensions>
</idea-plugin>"#;

/// Neovim plugin (Lua stub)
pub const NEOVIM_PLUGIN_LUA: &str = r#"-- Sparrow Neovim plugin
-- Connects to Sparrow runtime via WebSocket
-- Usage: require('sparrow').setup({ api_url = 'ws://127.0.0.1:9338/ws' })

local M = {}

function M.setup(opts)
  opts = opts or {}
  local api_url = opts.api_url or 'ws://127.0.0.1:9338/ws'
  -- Thin renderer: connects to runtime, renders events
  vim.api.nvim_create_user_command('SparrowRun', function(cmd)
    vim.fn.jobstart({'sparrow', 'run', cmd.args}, {})
  end, { nargs = 1 })
end

return M
"#;

// ─── Teams & Enterprise (WS12) ─────────────────────────────────────────────────

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgPolicy {
    pub max_autonomy: String,          // "supervised" | "trusted" | "autonomous"
    pub allowed_providers: Vec<String>,
    pub budget_per_seat_daily: f64,
    pub blocked_paths: Vec<String>,     // e.g., [".env", "*.pem", "secrets/"]
    pub require_approval_for: Vec<String>, // risk levels requiring approval
    pub audit_enabled: bool,
    pub sso_provider: Option<String>,
    pub air_gapped: bool,
}

impl Default for OrgPolicy {
    fn default() -> Self {
        Self {
            max_autonomy: "trusted".into(),
            allowed_providers: vec![],
            budget_per_seat_daily: 10.0,
            blocked_paths: vec![".env".into(), "*.pem".into(), "secrets/".into()],
            require_approval_for: vec!["destructive".into()],
            audit_enabled: true,
            sso_provider: None,
            air_gapped: false,
        }
    }
}

impl OrgPolicy {
    pub fn enforce(&self, autonomy: &crate::event::AutonomyLevel, cost: f64, path: &str) -> Result<(), String> {
        // Check autonomy ceiling
        let max = match self.max_autonomy.as_str() {
            "supervised" => crate::event::AutonomyLevel::Supervised,
            "trusted" => crate::event::AutonomyLevel::Trusted,
            _ => crate::event::AutonomyLevel::Autonomous,
        };
        if autonomy.as_float() > max.as_float() {
            return Err(format!("Org policy limits autonomy to {}", self.max_autonomy));
        }

        // Check budget
        if cost > self.budget_per_seat_daily {
            return Err(format!("Budget exceeded: ${:.2} > ${:.2}/day", cost, self.budget_per_seat_daily));
        }

        // Check blocked paths
        for blocked in &self.blocked_paths {
            if blocked.ends_with('/') && path.starts_with(blocked) {
                return Err(format!("Path '{}' is blocked by org policy", path));
            }
            if path == *blocked || (blocked.contains('*') && path.contains(&blocked.replace('*', ""))) {
                return Err(format!("File '{}' is protected by org policy", path));
            }
        }

        Ok(())
    }
}

// ─── Audit log ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp: String,
    pub user: String,
    pub action: String,
    pub run_id: String,
    pub cost_usd: f64,
    pub tokens: u64,
    pub autonomy: String,
    pub status: String,
}

pub fn export_audit_log(entries: &[AuditEntry], format: &str) -> String {
    match format {
        "json" => serde_json::to_string_pretty(entries).unwrap_or_default(),
        "csv" => {
            let mut csv = String::from("timestamp,user,action,run_id,cost,tokens,autonomy,status\n");
            for e in entries {
                csv.push_str(&format!(
                    "{},{},{},{},{:.4},{},{},{}\n",
                    e.timestamp, e.user, e.action, e.run_id, e.cost_usd, e.tokens, e.autonomy, e.status
                ));
            }
            csv
        }
        _ => format!("{} entries", entries.len()),
    }
}
