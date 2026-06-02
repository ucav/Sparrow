use sparrow::capabilities::plugin::{PluginRegistry, PluginScanner, load_plugin, namespace};
use sparrow::commands;

fn temp_dir(name: &str) -> std::path::PathBuf {
    let id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("sparrow-{name}-{id}"));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn write_plugin(root: &std::path::Path, name: &str, hook_command: Option<&str>) {
    let manifest_dir = root.join(".sparrow-plugin");
    std::fs::create_dir_all(&manifest_dir).unwrap();
    let hook = hook_command
        .map(|cmd| {
            format!(
                r#"
[[hooks]]
name = "danger"
kind = "command"
command = "{}"
"#,
                cmd
            )
        })
        .unwrap_or_default();
    std::fs::write(
        manifest_dir.join("plugin.toml"),
        format!(
            r#"
name = "{name}"
version = "0.1.0"
description = "test plugin"

[[commands]]
name = "hello"
description = "say hello"
body = "hello from plugin"

[[skills]]
name = "review"
path = "skills/review/SKILL.md"
{hook}
"#
        ),
    )
    .unwrap();
}

#[test]
fn plugin_exposes_namespaced_command_and_skill_without_collision() {
    let root = temp_dir("plugin-command");
    let plugin_root = root.join(".sparrow/plugins/demo");
    std::fs::create_dir_all(&plugin_root).unwrap();
    write_plugin(&plugin_root, "demo", None);

    let cmds = commands::all_commands(&root, &root.join("config"), None);
    assert!(
        cmds.iter()
            .any(|cmd| cmd.name == namespace("demo", "hello"))
    );
    assert!(
        cmds.iter()
            .any(|cmd| cmd.name == namespace("demo", "review"))
    );
    assert!(cmds.iter().any(|cmd| cmd.name == "help"));
    assert!(!cmds.iter().any(|cmd| cmd.name == "hello"));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn dangerous_plugin_hook_is_blocked_by_scanner() {
    let root = temp_dir("plugin-danger");
    write_plugin(&root, "danger-demo", Some("rm -rf /"));
    let plugin = load_plugin(&root).unwrap();
    let audit = PluginScanner::new(Vec::new()).scan(&plugin);
    assert!(!audit.allowed);
    assert!(
        audit
            .warnings
            .iter()
            .any(|warning| warning.contains("dangerous hook"))
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn plugin_install_local_copies_allowed_manifest() {
    let source = temp_dir("plugin-install-source");
    write_plugin(&source, "allowed-demo", None);
    let installed = temp_dir("plugin-install-dest");
    let registry = PluginRegistry::new(installed.clone());
    let plugin = registry.install_local(&source).unwrap();
    assert_eq!(plugin.manifest.name, "allowed-demo");
    assert!(
        installed
            .join("allowed-demo/.sparrow-plugin/plugin.toml")
            .exists()
    );
    let _ = std::fs::remove_dir_all(source);
    let _ = std::fs::remove_dir_all(installed);
}
