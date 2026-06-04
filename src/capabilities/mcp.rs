use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::process::Command as TokioCommand;

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

// ─── MCP Tool wrapper (real JSON-RPC execution) ────────────────────────────────

use tokio::sync::mpsc;

/// Wraps an MCP server tool. Two transports are supported, picked at connect time.
struct McpToolWrapper {
    tool_def: McpToolDef,
    backend: McpBackend,
}

enum McpBackend {
    /// Long-lived child process talking JSON-RPC over stdio.
    Stdio {
        request_tx: mpsc::Sender<McpRequest>,
    },
    /// One POST per call against a JSON-RPC HTTP endpoint.
    Http {
        url: String,
        client: reqwest::Client,
    },
}

struct McpRequest {
    tool_name: String,
    args: Value,
    response_tx: tokio::sync::oneshot::Sender<anyhow::Result<ToolResult>>,
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
        RiskLevel::Exec
    }

    async fn call(&self, args: Value, _ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        match &self.backend {
            McpBackend::Stdio { request_tx } => {
                let (tx, rx) = tokio::sync::oneshot::channel();
                request_tx
                    .send(McpRequest {
                        tool_name: self.tool_def.name.clone(),
                        args,
                        response_tx: tx,
                    })
                    .await
                    .map_err(|_| anyhow::anyhow!("MCP server process has stopped"))?;

                tokio::time::timeout(std::time::Duration::from_secs(30), rx)
                    .await
                    .map_err(|_| anyhow::anyhow!("MCP tool call timed out"))?
                    .map_err(|_| anyhow::anyhow!("MCP tool call channel closed"))?
            }
            McpBackend::Http { url, client } => {
                static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
                let id = NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let body = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "method": "tools/call",
                    "params": {
                        "name": self.tool_def.name,
                        "arguments": args,
                    }
                });
                let resp = tokio::time::timeout(
                    std::time::Duration::from_secs(30),
                    client.post(url).json(&body).send(),
                )
                .await
                .map_err(|_| anyhow::anyhow!("MCP HTTP call timed out"))??;
                if !resp.status().is_success() {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    return Ok(ToolResult::error(format!(
                        "MCP HTTP error {}: {}",
                        status, body
                    )));
                }
                let value: Value = resp.json().await?;
                if let Some(err) = value.get("error") {
                    return Ok(ToolResult::error(format!("MCP error: {}", err)));
                }
                if let Some(result) = value.get("result") {
                    Ok(ToolResult::text(result.to_string()))
                } else {
                    Ok(ToolResult::text("(empty MCP response)"))
                }
            }
        }
    }
}

// ─── THE MCP CLIENT TRAIT ───────────────────────────────────────────────────────

#[async_trait]
pub trait McpClient: Send + Sync {
    async fn connect(&self, server: &McpServer) -> anyhow::Result<Vec<Arc<dyn Tool>>>;
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
    async fn connect(&self, server: &McpServer) -> anyhow::Result<Vec<Arc<dyn Tool>>> {
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
    async fn connect_stdio(&self, server: &McpServer) -> anyhow::Result<Vec<Arc<dyn Tool>>> {
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

        let (mut writer, mut reader) = (tokio::io::BufWriter::new(stdin), BufReader::new(stdout));

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

        // Read the initialize response. Skip any notifications (no `id`).
        let _ = read_jsonrpc_response(&mut reader, 1).await?;

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

        let tools_resp_value = read_jsonrpc_response(&mut reader, 2).await?;

        // Spawn background task for JSON-RPC request handling
        let (request_tx, mut request_rx) = mpsc::channel::<McpRequest>(32);

        tokio::spawn(async move {
            // Keep the child alive for the lifetime of the channel: dropping `child`
            // when the writer task exits also kills the process (kill_on_drop).
            let _child_guard = child;
            let mut call_id: u64 = 3; // Start after initialize(1) and tools/list(2)
            while let Some(req) = request_rx.recv().await {
                call_id += 1;
                let call_req = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": call_id,
                    "method": "tools/call",
                    "params": {
                        "name": req.tool_name,
                        "arguments": req.args,
                    }
                });
                if writer
                    .write_all((serde_json::to_string(&call_req).unwrap() + "\n").as_bytes())
                    .await
                    .is_err()
                    || writer.flush().await.is_err()
                {
                    let _ = req
                        .response_tx
                        .send(Err(anyhow::anyhow!("MCP stdin closed")));
                    break;
                }

                // Drain lines until we see one with the matching id (or an error).
                match read_jsonrpc_response(&mut reader, call_id).await {
                    Ok(value) => {
                        let result = if let Some(err) = value.get("error") {
                            Ok(ToolResult::error(format!("MCP error: {}", err)))
                        } else if let Some(val) = value.get("result") {
                            Ok(ToolResult::text(val.to_string()))
                        } else {
                            Ok(ToolResult::text("(empty MCP response)"))
                        };
                        let _ = req.response_tx.send(result);
                    }
                    Err(e) => {
                        let _ = req
                            .response_tx
                            .send(Err(anyhow::anyhow!("MCP read error: {}", e)));
                        break;
                    }
                }
            }
        });

        // Parse tools
        let server_name = server.name.clone();
        let allow_list = server.allow_tools.clone();

        let tools: Vec<Arc<dyn Tool>> = if let Some(result) = tools_resp_value.get("result") {
            if let Ok(list) = serde_json::from_value::<ToolsListResult>(result.clone()) {
                list.tools
                    .into_iter()
                    .filter(|t| allow_list.is_empty() || allow_list.contains(&t.name))
                    .map(|t| {
                        let _srv = server_name.clone();
                        Arc::new(McpToolWrapper {
                            tool_def: t,
                            backend: McpBackend::Stdio {
                                request_tx: request_tx.clone(),
                            },
                        }) as Arc<dyn Tool>
                    })
                    .collect()
            } else {
                vec![]
            }
        } else {
            tracing::warn!("MCP server {} returned no tools/list result", server.name);
            vec![]
        };

        Ok(tools)
    }

    /// Connect via HTTP/SSE
    async fn connect_http(&self, server: &McpServer) -> anyhow::Result<Vec<Arc<dyn Tool>>> {
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
                        let _srv = server_name.clone();
                        Arc::new(McpToolWrapper {
                            tool_def: t,
                            backend: McpBackend::Http {
                                url: url.clone(),
                                client: client.clone(),
                            },
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

/// Read JSON-RPC frames (one per line) until we find one whose `id` matches
/// `expected_id`. Notifications (no `id`) are skipped. A read failure or EOF
/// surfaces as an error so callers can stop the loop.
async fn read_jsonrpc_response<R: tokio::io::AsyncBufRead + Unpin>(
    reader: &mut R,
    expected_id: u64,
) -> anyhow::Result<Value> {
    use tokio::io::AsyncBufReadExt;
    let mut line = String::new();
    for _ in 0..64 {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            anyhow::bail!("MCP server closed stdout");
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => {
                // Not JSON — likely a stderr leak on stdout, ignore and keep reading.
                tracing::debug!("MCP non-JSON stdout line: {}", trimmed);
                continue;
            }
        };
        // Notifications (no "id") are skipped.
        match value.get("id").and_then(|v| v.as_u64()) {
            Some(id) if id == expected_id => return Ok(value),
            Some(_) => continue, // response to an earlier/later request, drop
            None => continue,    // notification
        }
    }
    anyhow::bail!(
        "MCP server did not respond to id={} within 64 frames",
        expected_id
    )
}
