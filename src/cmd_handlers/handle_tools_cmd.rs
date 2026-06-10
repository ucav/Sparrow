// src/cmd_handlers/handle_tools_cmd.rs
use super::prelude::*;
pub fn handle_tools(
    action: sparrow::cli::ToolsAction,
    config_store: &FsConfigStore,
) -> anyhow::Result<()> {
    match action {
        sparrow::cli::ToolsAction::List { surface } => {
            let metas = sparrow::tools::known_tool_metadata(surface.as_deref());
            println!("Toolsets: {}", sparrow::tools::TOOLSETS.join(", "));
            println!("Tools ({}):", metas.len());
            for meta in metas {
                println!(
                    "  {:16} set:{:14} risk:{:?} auth:{} mutates:{} network:{} exec:{}",
                    meta.name,
                    meta.toolset,
                    meta.risk,
                    meta.requires_auth,
                    meta.mutates_files,
                    meta.network,
                    meta.exec
                );
            }
        }
        sparrow::cli::ToolsAction::Enable { tool } => {
            let mut cfg = config_store.load()?;
            cfg.permissions.tools.deny.retain(|item| item != &tool);
            if !cfg.permissions.tools.allow.contains(&tool) {
                cfg.permissions.tools.allow.push(tool.clone());
            }
            config_store.save(&cfg)?;
            println!("Tool '{}' enabled in permissions.", tool);
        }
        sparrow::cli::ToolsAction::Disable { tool } => {
            let mut cfg = config_store.load()?;
            cfg.permissions.tools.allow.retain(|item| item != &tool);
            if !cfg.permissions.tools.deny.contains(&tool) {
                cfg.permissions.tools.deny.push(tool.clone());
            }
            config_store.save(&cfg)?;
            println!("Tool '{}' disabled in permissions.", tool);
        }
    }
    Ok(())
}

// ─── Security audit ─────────────────────────────────────────────────────────────
