use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

use super::{Tool, ToolCtx, ToolResult};
use sparrow_core::event::{Block, RiskLevel};
use sparrow_memory::{GraphDirection, GraphEdge, GraphNode, KnowledgeGraph, Memory};

pub struct KnowledgeGraphTool {
    memory: Arc<dyn Memory>,
}

impl KnowledgeGraphTool {
    pub fn new(memory: Arc<dyn Memory>) -> Self {
        Self { memory }
    }
}

#[async_trait]
impl Tool for KnowledgeGraphTool {
    fn name(&self) -> &str {
        "knowledge_graph"
    }

    fn description(&self) -> &str {
        "Manage Sparrow's persistent knowledge graph: upsert nodes/edges, search, inspect neighbors, export, delete, and optionally sync to local Neo4j."
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["upsert_node", "upsert_edge", "get", "neighbors", "search", "export", "delete_node", "delete_edge", "sync_neo4j"]
                },
                "id": { "type": "string", "description": "Node id, edge id, or lookup id" },
                "label": { "type": "string", "description": "Human-readable node label" },
                "kind": { "type": "string", "description": "Node kind, e.g. user, project, file, decision, feature" },
                "from_id": { "type": "string", "description": "Source node id for edges" },
                "to_id": { "type": "string", "description": "Target node id for edges" },
                "relation": { "type": "string", "description": "Edge relation, e.g. works_on, depends_on, decided" },
                "weight": { "type": "number", "description": "Edge weight; defaults to 1.0" },
                "properties": { "type": "object", "description": "JSON metadata for a node or edge" },
                "query": { "type": "string", "description": "Search query" },
                "direction": { "type": "string", "enum": ["incoming", "outgoing", "both"] },
                "limit": { "type": "integer", "description": "Maximum rows to return" }
            },
            "required": ["action"]
        })
    }

    fn risk(&self) -> RiskLevel {
        RiskLevel::Mutating
    }

    async fn call(&self, args: serde_json::Value, _ctx: &ToolCtx) -> anyhow::Result<ToolResult> {
        let action = args["action"].as_str().unwrap_or("search");
        let limit = args["limit"].as_u64().unwrap_or(20).clamp(1, 100) as usize;
        match action {
            "upsert_node" => {
                let node = node_from_args(&args)?;
                self.memory.upsert_graph_node(node.clone())?;
                Ok(ToolResult::text(format!(
                    "graph node stored: {} [{}] {}",
                    node.id, node.kind, node.label
                )))
            }
            "upsert_edge" => {
                let edge = edge_from_args(&args)?;
                self.memory.upsert_graph_edge(edge.clone())?;
                Ok(ToolResult::text(format!(
                    "graph edge stored: {} {} -{}-> {}",
                    edge.id, edge.from_id, edge.relation, edge.to_id
                )))
            }
            "get" => {
                let id = required_str(&args, "id")?;
                match self.memory.graph_node(&id) {
                    Some(node) => Ok(ToolResult::ok(vec![Block::Json(json!(node))])),
                    None => Ok(ToolResult::error(format!("graph node not found: {}", id))),
                }
            }
            "neighbors" => {
                let id = required_str(&args, "id")?;
                let direction = GraphDirection::parse(args["direction"].as_str().unwrap_or("both"));
                let rows = self.memory.graph_neighbors(&id, direction, limit);
                if rows.is_empty() {
                    return Ok(ToolResult::text(format!("no graph neighbors for {}", id)));
                }
                Ok(ToolResult::ok(vec![Block::Json(json!(rows))]))
            }
            "search" => {
                let query = args["query"].as_str().unwrap_or("");
                if query.trim().is_empty() {
                    return Ok(ToolResult::error("knowledge_graph search requires query"));
                }
                let nodes = self.memory.search_graph(query, limit);
                Ok(ToolResult::ok(vec![Block::Json(json!(nodes))]))
            }
            "export" => {
                let graph = self.memory.graph_export();
                Ok(ToolResult::ok(vec![Block::Json(json!(graph))]))
            }
            "delete_node" => {
                let id = required_str(&args, "id")?;
                self.memory.delete_graph_node(&id)?;
                Ok(ToolResult::text(format!("graph node deleted: {}", id)))
            }
            "delete_edge" => {
                let id = required_str(&args, "id")?;
                self.memory.delete_graph_edge(&id)?;
                Ok(ToolResult::text(format!("graph edge deleted: {}", id)))
            }
            "sync_neo4j" => {
                let graph = self.memory.graph_export();
                let count = sync_graph_to_neo4j(&graph).await?;
                Ok(ToolResult::text(format!(
                    "synced graph to Neo4j: {} nodes, {} edges, {} statements",
                    graph.nodes.len(),
                    graph.edges.len(),
                    count
                )))
            }
            _ => Ok(ToolResult::error("unknown knowledge_graph action")),
        }
    }
}

pub async fn sync_graph_to_neo4j(graph: &KnowledgeGraph) -> anyhow::Result<usize> {
    let url = std::env::var("NEO4J_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:7474/db/neo4j/tx/commit".into());
    let user = std::env::var("NEO4J_USER")
        .or_else(|_| std::env::var("NEO4J_USERNAME"))
        .map_err(|_| anyhow::anyhow!("Neo4j sync requires NEO4J_USER and NEO4J_PASSWORD"))?;
    let password = std::env::var("NEO4J_PASSWORD")
        .map_err(|_| anyhow::anyhow!("Neo4j sync requires NEO4J_USER and NEO4J_PASSWORD"))?;
    let statements = neo4j_statements(graph);
    if statements.is_empty() {
        return Ok(0);
    }
    let resp = reqwest::Client::new()
        .post(url)
        .basic_auth(user, Some(password))
        .json(&json!({ "statements": statements }))
        .send()
        .await?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.unwrap_or_else(|_| json!({}));
    if !status.is_success() || body["errors"].as_array().is_some_and(|e| !e.is_empty()) {
        anyhow::bail!("Neo4j sync failed: HTTP {} {}", status, body);
    }
    Ok(statements.len())
}

fn neo4j_statements(graph: &KnowledgeGraph) -> Vec<serde_json::Value> {
    let mut statements = Vec::new();
    for node in &graph.nodes {
        statements.push(json!({
            "statement": "MERGE (n:SparrowNode {id: $id}) SET n.label = $label, n.kind = $kind, n.properties = $properties, n.updated_at = $updated_at",
            "parameters": {
                "id": node.id,
                "label": node.label,
                "kind": node.kind,
                "properties": node.properties.to_string(),
                "updated_at": node.updated_at,
            }
        }));
    }
    for edge in &graph.edges {
        statements.push(json!({
            "statement": "MATCH (a:SparrowNode {id: $from_id}), (b:SparrowNode {id: $to_id}) MERGE (a)-[r:SPARROW_REL {id: $id}]->(b) SET r.relation = $relation, r.weight = $weight, r.properties = $properties, r.updated_at = $updated_at",
            "parameters": {
                "id": edge.id,
                "from_id": edge.from_id,
                "to_id": edge.to_id,
                "relation": edge.relation,
                "weight": edge.weight,
                "properties": edge.properties.to_string(),
                "updated_at": edge.updated_at,
            }
        }));
    }
    statements
}

fn node_from_args(args: &serde_json::Value) -> anyhow::Result<GraphNode> {
    let id = required_str(args, "id")?;
    let label = required_str(args, "label")?;
    let kind = args["kind"].as_str().unwrap_or("entity").trim().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    Ok(GraphNode {
        id,
        label,
        kind,
        properties: args.get("properties").cloned().unwrap_or_else(|| json!({})),
        created_at: now.clone(),
        updated_at: now,
    })
}

fn edge_from_args(args: &serde_json::Value) -> anyhow::Result<GraphEdge> {
    let id = args["id"]
        .as_str()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            format!(
                "{}:{}:{}",
                args["from_id"].as_str().unwrap_or(""),
                args["relation"].as_str().unwrap_or("related_to"),
                args["to_id"].as_str().unwrap_or("")
            )
        });
    let now = chrono::Utc::now().to_rfc3339();
    Ok(GraphEdge {
        id,
        from_id: required_str(args, "from_id")?,
        to_id: required_str(args, "to_id")?,
        relation: args["relation"]
            .as_str()
            .unwrap_or("related_to")
            .trim()
            .to_string(),
        weight: args["weight"].as_f64().unwrap_or(1.0),
        properties: args.get("properties").cloned().unwrap_or_else(|| json!({})),
        created_at: now.clone(),
        updated_at: now,
    })
}

fn required_str(args: &serde_json::Value, key: &str) -> anyhow::Result<String> {
    args[key]
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("knowledge_graph requires '{}'", key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neo4j_payload_uses_parameterized_statements() {
        let graph = KnowledgeGraph {
            nodes: vec![GraphNode {
                id: "user:alice".into(),
                label: "Alice".into(),
                kind: "user".into(),
                properties: json!({"prefers": "local-first"}),
                created_at: "2026-06-04T00:00:00Z".into(),
                updated_at: "2026-06-04T00:00:00Z".into(),
            }],
            edges: vec![GraphEdge {
                id: "user:alice:works_on:project:sparrow".into(),
                from_id: "user:alice".into(),
                to_id: "project:sparrow".into(),
                relation: "works_on".into(),
                weight: 1.0,
                properties: json!({}),
                created_at: "2026-06-04T00:00:00Z".into(),
                updated_at: "2026-06-04T00:00:00Z".into(),
            }],
        };
        let statements = neo4j_statements(&graph);
        assert_eq!(statements.len(), 2);
        assert!(
            statements[0]["statement"]
                .as_str()
                .unwrap()
                .contains("MERGE")
        );
        assert_eq!(statements[0]["parameters"]["id"], "user:alice");
    }
}
