// src/cmd_handlers/handle_memory_graph_cmd.rs
use super::prelude::*;
pub fn handle_memory_graph(
    action: sparrow::cli::GraphAction,
    memory: &Arc<dyn Memory>,
) -> anyhow::Result<()> {
    use sparrow::memory::{GraphDirection, GraphEdge, GraphNode};
    let now = chrono::Utc::now().to_rfc3339();
    match action {
        sparrow::cli::GraphAction::UpsertNode {
            id,
            label,
            kind,
            properties,
        } => {
            let properties = parse_json_properties(&properties)?;
            memory.upsert_graph_node(GraphNode {
                id: id.clone(),
                label,
                kind,
                properties,
                created_at: now.clone(),
                updated_at: now,
            })?;
            println!("Graph node stored: {}", id);
        }
        sparrow::cli::GraphAction::UpsertEdge {
            from_id,
            relation,
            to_id,
            id,
            weight,
            properties,
        } => {
            let edge_id = id.unwrap_or_else(|| format!("{}:{}:{}", from_id, relation, to_id));
            let properties = parse_json_properties(&properties)?;
            memory.upsert_graph_edge(GraphEdge {
                id: edge_id.clone(),
                from_id,
                to_id,
                relation,
                weight,
                properties,
                created_at: now.clone(),
                updated_at: now,
            })?;
            println!("Graph edge stored: {}", edge_id);
        }
        sparrow::cli::GraphAction::Get { id } => {
            if let Some(node) = memory.graph_node(&id) {
                println!("{}", serde_json::to_string_pretty(&node)?);
            } else {
                println!("Graph node '{}' not found.", id);
            }
        }
        sparrow::cli::GraphAction::Neighbors {
            id,
            direction,
            limit,
        } => {
            let rows = memory.graph_neighbors(&id, GraphDirection::parse(&direction), limit);
            println!("{}", serde_json::to_string_pretty(&rows)?);
        }
        sparrow::cli::GraphAction::Search { query, limit } => {
            let nodes = memory.search_graph(&query, limit);
            println!("{}", serde_json::to_string_pretty(&nodes)?);
        }
        sparrow::cli::GraphAction::Export => {
            println!("{}", serde_json::to_string_pretty(&memory.graph_export())?);
        }
        sparrow::cli::GraphAction::DeleteNode { id } => {
            memory.delete_graph_node(&id)?;
            println!("Graph node deleted: {}", id);
        }
        sparrow::cli::GraphAction::DeleteEdge { id } => {
            memory.delete_graph_edge(&id)?;
            println!("Graph edge deleted: {}", id);
        }
        sparrow::cli::GraphAction::SyncNeo4j => {
            let graph = memory.graph_export();
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            let statements =
                runtime.block_on(sparrow::tools::knowledge_graph::sync_graph_to_neo4j(&graph))?;
            println!(
                "Synced graph to Neo4j: {} nodes, {} edges, {} statements",
                graph.nodes.len(),
                graph.edges.len(),
                statements
            );
        }
    }
    Ok(())
}

pub fn parse_json_properties(raw: &str) -> anyhow::Result<serde_json::Value> {
    let value: serde_json::Value = serde_json::from_str(raw)
        .map_err(|e| anyhow::anyhow!("properties must be valid JSON object: {}", e))?;
    if !value.is_object() {
        anyhow::bail!("properties must be a JSON object");
    }
    Ok(value)
}

// ─── JSON NDJSON run ────────────────────────────────────────────────────────────

pub fn current_repo_head() -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let head = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if head.is_empty() { None } else { Some(head) }
}
