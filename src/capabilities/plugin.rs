use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginManifest {
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub commands: Vec<PluginCommand>,
    #[serde(default)]
    pub agents: Vec<String>,
    #[serde(default)]
    pub skills: Vec<PluginSkill>,
    #[serde(default)]
    pub hooks: Vec<PluginHook>,
    #[serde(default)]
    pub mcp_servers: Vec<crate::capabilities::mcp::McpServer>,
    #[serde(default)]
    pub themes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginCommand {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginSkill {
    pub name: String,
    #[serde(default)]
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginHook {
    pub name: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plugin {
    pub manifest: PluginManifest,
    pub root: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginAudit {
    pub allowed: bool,
    pub warnings: Vec<String>,
}

pub struct PluginScanner {
    allowlist: Vec<String>,
}

impl PluginScanner {
    pub fn new(allowlist: Vec<String>) -> Self {
        Self { allowlist }
    }

    pub fn scan(&self, plugin: &Plugin) -> PluginAudit {
        let mut warnings = Vec::new();
        if plugin.manifest.name.trim().is_empty() {
            warnings.push("plugin name is empty".into());
        }
        if !self.allowlist.is_empty() && !self.allowlist.contains(&plugin.manifest.name) {
            warnings.push(format!(
                "plugin '{}' is not in the allowlist",
                plugin.manifest.name
            ));
        }
        for hook in &plugin.manifest.hooks {
            if hook.kind.eq_ignore_ascii_case("command") && command_looks_dangerous(&hook.command) {
                warnings.push(format!("dangerous hook '{}' is blocked", hook.name));
            }
        }
        PluginAudit {
            allowed: warnings.is_empty(),
            warnings,
        }
    }
}

pub struct PluginRegistry {
    plugins_dir: PathBuf,
    allowlist: Vec<String>,
}

impl PluginRegistry {
    pub fn new(plugins_dir: PathBuf) -> Self {
        std::fs::create_dir_all(&plugins_dir).ok();
        Self {
            plugins_dir,
            allowlist: Vec::new(),
        }
    }

    pub fn with_allowlist(mut self, allowlist: Vec<String>) -> Self {
        self.allowlist = allowlist;
        self
    }

    pub fn scan(&self) -> Vec<Plugin> {
        let Ok(entries) = std::fs::read_dir(&self.plugins_dir) else {
            return Vec::new();
        };
        entries
            .flatten()
            .filter_map(|entry| load_plugin(&entry.path()).ok())
            .collect()
    }

    pub fn audit(&self, plugin: &Plugin) -> PluginAudit {
        PluginScanner::new(self.allowlist.clone()).scan(plugin)
    }

    pub fn install_local(&self, source: &Path) -> anyhow::Result<Plugin> {
        let plugin = load_plugin(source)?;
        let audit = self.audit(&plugin);
        if !audit.allowed {
            anyhow::bail!("plugin blocked: {}", audit.warnings.join("; "));
        }
        let dest = self.plugins_dir.join(&plugin.manifest.name);
        if dest.exists() {
            std::fs::remove_dir_all(&dest)?;
        }
        copy_dir_all(source, &dest)?;
        load_plugin(&dest)
    }

    pub fn install_github(&self, repo: &str) -> anyhow::Result<Plugin> {
        let tmp = std::env::temp_dir().join(format!("sparrow-plugin-{}", uuid::Uuid::new_v4()));
        let status = std::process::Command::new("git")
            .args([
                "clone",
                "--depth",
                "1",
                repo,
                tmp.to_string_lossy().as_ref(),
            ])
            .status()?;
        if !status.success() {
            anyhow::bail!("git clone failed for plugin repo {}", repo);
        }
        let plugin = self.install_local(&tmp);
        let _ = std::fs::remove_dir_all(&tmp);
        plugin
    }
}

pub fn load_plugin(root: &Path) -> anyhow::Result<Plugin> {
    let toml_path = root.join(".sparrow-plugin").join("plugin.toml");
    let json_path = root.join(".sparrow-plugin").join("plugin.json");
    let manifest = if toml_path.exists() {
        toml::from_str::<PluginManifest>(&std::fs::read_to_string(toml_path)?)?
    } else if json_path.exists() {
        serde_json::from_str::<PluginManifest>(&std::fs::read_to_string(json_path)?)?
    } else {
        anyhow::bail!("plugin manifest not found in {}", root.display());
    };
    Ok(Plugin {
        manifest,
        root: root.to_path_buf(),
    })
}

pub fn namespace(plugin: &str, item: &str) -> String {
    format!("{}:{}", plugin.trim(), item.trim())
}

fn command_looks_dangerous(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    [
        "rm -rf",
        "remove-item",
        "format ",
        "del /s",
        "shutdown",
        "curl ",
        "invoke-webrequest",
        "powershell -enc",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn copy_dir_all(src: &Path, dst: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dest)?;
        } else {
            std::fs::copy(entry.path(), dest)?;
        }
    }
    Ok(())
}
