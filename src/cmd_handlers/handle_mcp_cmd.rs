// src/cmd_handlers/handle_mcp_cmd.rs
use super::prelude::*;
pub async fn handle_mcp(
    action: sparrow::cli::McpAction,
    config_dir: &std::path::PathBuf,
) -> anyhow::Result<()> {
    let client = BasicMcpClient::new(config_dir.join("mcp"));

    match action {
        sparrow::cli::McpAction::List => {
            let servers = client.list_servers().await;
            if servers.is_empty() {
                println!("No MCP servers configured.");
                println!("Add one: sparrow mcp add <name> --command <cmd> --args <args>");
            } else {
                println!("MCP servers:");
                for s in &servers {
                    let transport = match s.transport {
                        Transport::Stdio => "stdio",
                        Transport::Sse => "sse",
                        Transport::Url => "url",
                    };
                    println!(
                        "  {} ({}) | {} tools allowed",
                        s.name,
                        transport,
                        if s.allow_tools.is_empty() {
                            "all".to_string()
                        } else {
                            s.allow_tools.len().to_string()
                        }
                    );
                }
            }
        }
        sparrow::cli::McpAction::Add {
            server,
            command,
            args,
            transport,
        } => {
            if let Some(command) = command {
                let transport = match transport.as_deref().unwrap_or("stdio") {
                    "stdio" => Transport::Stdio,
                    "sse" => Transport::Sse,
                    "url" => Transport::Url,
                    other => anyhow::bail!("Unsupported MCP transport: {}", other),
                };
                client.add_server(McpServer {
                    name: server.clone(),
                    transport,
                    command: Some(command),
                    args,
                    url: None,
                    env: Default::default(),
                    allow_tools: vec![],
                })?;
                println!("Added MCP server: {}", server);
            } else {
                println!("Adding MCP server: {}", server);
                println!(
                    "Usage: sparrow mcp add {} --command <cmd> --args \"<args>\"",
                    server
                );
                println!("Example:");
                println!(
                    r#"  sparrow mcp add {} --command npx --args "-y @modelcontextprotocol/server-filesystem C:\Sparrow""#,
                    server
                );
            }
        }
        sparrow::cli::McpAction::Rm { server } => {
            client.remove_server(&server)?;
            println!("Removed MCP server: {}", server);
        }
    }
    Ok(())
}

// ─── Schedule command ───────────────────────────────────────────────────────────
