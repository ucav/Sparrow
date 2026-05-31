use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::process::Command as TokioCommand;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::event::RiskLevel;
use crate::tools::{Tool, ToolCtx, ToolResult};

// ─── MCP server config ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServer {
    pub name: String,
    #[serde(default)]
    pub transport: Transport,
    /// For stdio: command + args
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    /// For url/sse: endpoint URL
    #[serde(default)]
    pub url: Option<String>,
    /// Environment variables
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
    /// Tool allow-list (empty = allow all)
    #[serde(default)]
    pub allow_tools: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Transport {
    #[serde(rename = "stdio")]
    Stdio,
    #[serde(rename = "sse")]
    Sse,
    #[serde(rename = "url")]
    Url,
}

impl Default for Transport {
    fn default() -> Self {
        Transport::Stdio
    }
}

// ─── JSON-RPC types ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: u64,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

#[derive(Debug, Deserialize)]
struct ToolsListResult {
    tools: Vec<McpToolDef>,
}

#[derive(Debug, Deserialize)]
struct McpToolDef {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    #[serde(rename = "inputSchema")]
    input_schema: Value,
}

// ─── MCP Tool wrapper ───────────────────────────────────────────────────────────

/// Wraps an MCP server tool so it implements our Tool trait.
struct McpToolWrapper {
    server_name: String,
    tool_def: McpToolDef,
    /// Function to call the tool (closure or channel sender)
    call_fn: Arc<dyn Fn(&str, Value) -> anyhow::Result<ToolResult> + Send + Sync>,
}

#[async_trait]
impl Tool for McpToolWrapper {
    fn name(&self) -> &str {
        &self.tool_def.name
    }

    fn description(&self) -> &str {
        &self.tool_def.description
    }

    fn schema(&self) -> Value {
        self.tool_def.input_schema.clone()
    }

    fn risk(&self) -> RiskLevel {
        // MCP tools are conservatively marked as Exec
        RiskLevel::Exec
    }

    async fn call(&self, args: Value, _ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        (self.call_fn)(&self.server_name, args)
    }
}

// ─── THE MCP CLIENT TRAIT ───────────────────────────────────────────────────────

#[async_trait]
pub trait McpClient: Send + Sync {
    async fn connect(
        &self,
        server: &McpServer,
    ) -> anyhow::Result<Vec<Arc<dyn Tool>>>;
    async fn disconnect(&self, server_name: &str) -> anyhow::Result<()>;
    async fn list_servers(&self) -> Vec<McpServer>;
}

// ─── Basic MCP client implementation ────────────────────────────────────────────

pub struct BasicMcpClient {
    config_dir: PathBuf,
}

impl BasicMcpClient {
    pub fn new(config_dir: PathBuf) -> Self {
        Self { config_dir }
    }

    fn servers_file(&self) -> PathBuf {
        self.config_dir.join("mcp_servers.json")
    }

    fn load_servers(&self) -> Vec<McpServer> {
        let path = self.servers_file();
        if !path.exists() {
            return vec![];
        }
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save_servers(&self, servers: &[McpServer]) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.config_dir)?;
        let json = serde_json::to_string_pretty(servers)?;
        std::fs::write(self.servers_file(), json)?;
        Ok(())
    }

    pub fn add_server(&self, server: McpServer) -> anyhow::Result<()> {
        let mut servers = self.load_servers();
        servers.retain(|s| s.name != server.name);
        servers.push(server);
        self.save_servers(&servers)
    }

    pub fn remove_server(&self, name: &str) -> anyhow::Result<()> {
        let mut servers = self.load_servers();
        servers.retain(|s| s.name != name);
        self.save_servers(&servers)
    }

    pub fn get_server(&self, name: &str) -> Option<McpServer> {
        self.load_servers().into_iter().find(|s| s.name == name)
    }
}

#[async_trait]
impl McpClient for BasicMcpClient {
    async fn connect(
        &self,
        server: &McpServer,
    ) -> anyhow::Result<Vec<Arc<dyn Tool>>> {
        match server.transport {
            Transport::Stdio => self.connect_stdio(server).await,
            Transport::Url | Transport::Sse => self.connect_http(server).await,
        }
    }

    async fn disconnect(&self, _server_name: &str) -> anyhow::Result<()> {
        // In a full implementation, we'd track active connections
        // For M3, connections are ephemeral per-connect
        Ok(())
    }

    async fn list_servers(&self) -> Vec<McpServer> {
        self.load_servers()
    }
}

impl BasicMcpClient {
    /// Connect via stdio (spawn process, JSON-RPC over stdin/stdout)
    async fn connect_stdio(
        &self,
        server: &McpServer,
    ) -> anyhow::Result<Vec<Arc<dyn Tool>>> {
        let command = server
            .command
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("stdio transport requires 'command'"))?;

        let mut child = TokioCommand::new(command)
            .args(&server.args)
            .envs(&server.env)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;

        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let (mut writer, mut reader) = (
            tokio::io::BufWriter::new(stdin),
            BufReader::new(stdout),
        );

        // Send initialize request
        let init_req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: 1,
            method: "initialize".into(),
            params: serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "sparrow",
                    "version": "0.1.0"
                }
            }),
        };

        let req_json = serde_json::to_string(&init_req)? + "\n";
        writer.write_all(req_json.as_bytes()).await?;
        writer.flush().await?;

        // Read response (simplified: read one line)
        let mut _resp_line = String::new();
        reader.read_line(&mut _resp_line).await?;

        // Send initialized notification
        let notif = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        });
        writer
            .write_all((serde_json::to_string(&notif)? + "\n").as_bytes())
            .await?;
        writer.flush().await?;

        // Request tools/list
        let list_req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: 2,
            method: "tools/list".into(),
            params: Value::Null,
        };

        writer
            .write_all((serde_json::to_string(&list_req)? + "\n").as_bytes())
            .await?;
        writer.flush().await?;

        let mut tools_resp = String::new();
        reader.read_line(&mut tools_resp).await?;

        // Parse tools
        let server_name = server.name.clone();
        let allow_list = server.allow_tools.clone();

        let tools: Vec<Arc<dyn Tool>> = if let Ok(resp) =
            serde_json::from_str::<JsonRpcResponse>(&tools_resp)
        {
            if let Some(result) = resp.result {
                if let Ok(list) = serde_json::from_value::<ToolsListResult>(result) {
                    list.tools
                        .into_iter()
                        .filter(|t| {
                            allow_list.is_empty() || allow_list.contains(&t.name)
                        })
                        .map(|t| {
                            let srv = server_name.clone();
                            let tool_name = t.name.clone();
                            Arc::new(McpToolWrapper {
                                server_name: srv.clone(),
                                tool_def: t,
                                call_fn: Arc::new(
                                    move |_, _args| {
                                        // Stub: real call would use the process
                                        Ok(ToolResult::text(format!(
                                            "MCP tool '{}' from server '{}' — call via stdio",
                                            tool_name, srv
                                        )))
                                    },
                                ),
                            }) as Arc<dyn Tool>
                        })
                        .collect()
                } else {
                    vec![]
                }
            } else {
                vec![]
            }
        } else {
            tracing::warn!("Failed to parse MCP response from {}", server.name);
            vec![]
        };

        Ok(tools)
    }

    /// Connect via HTTP/SSE
    async fn connect_http(
        &self,
        server: &McpServer,
    ) -> anyhow::Result<Vec<Arc<dyn Tool>>> {
        let url = server
            .url
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("url/sse transport requires 'url'"))?;

        let client = reqwest::Client::new();

        // Send initialize via HTTP POST
        let _init_resp: Value = client
            .post(url)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": { "name": "sparrow", "version": "0.1.0" }
                }
            }))
            .send()
            .await?
            .json()
            .await?;

        // List tools
        let tools_resp: Value = client
            .post(url)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/list",
                "params": {}
            }))
            .send()
            .await?
            .json()
            .await?;

        let server_name = server.name.clone();
        let allow_list = server.allow_tools.clone();

        let tools: Vec<Arc<dyn Tool>> = if let Some(result) = tools_resp.get("result") {
            if let Ok(list) = serde_json::from_value::<ToolsListResult>(result.clone()) {
                list.tools
                    .into_iter()
                    .filter(|t| allow_list.is_empty() || allow_list.contains(&t.name))
                    .map(|t| {
                        let srv = server_name.clone();
                        let tool_name = t.name.clone();
                        let endpoint = url.clone();
                        Arc::new(McpToolWrapper {
                            server_name: srv.clone(),
                            tool_def: t,
                            call_fn: Arc::new(move |_, args| {
                                // In real impl, we'd make an async HTTP call
                                Ok(ToolResult::text(format!(
                                    "MCP tool '{}' from '{}' at {} with args: {}",
                                    tool_name,
                                    srv,
                                    endpoint,
                                    serde_json::to_string_pretty(&args)
                                        .unwrap_or_default()
                                )))
                            }),
                        }) as Arc<dyn Tool>
                    })
                    .collect()
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        Ok(tools)
    }
}
