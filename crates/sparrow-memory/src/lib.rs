// Same lint policy as the main crate (this code was extracted from it).
#![allow(
    clippy::collapsible_if,
    clippy::collapsible_match,
    clippy::derivable_impls,
    clippy::format_in_format_args,
    clippy::if_same_then_else,
    clippy::iter_cloned_collect,
    clippy::manual_clamp,
    clippy::manual_div_ceil,
    clippy::manual_is_multiple_of,
    clippy::manual_pattern_char_comparison,
    clippy::needless_borrow,
    clippy::needless_range_loop,
    clippy::new_without_default,
    clippy::ptr_arg,
    clippy::should_implement_trait,
    clippy::single_match,
    clippy::type_complexity,
    clippy::unnecessary_cast,
    clippy::let_and_return,
    clippy::useless_conversion,
    clippy::useless_format,
    clippy::while_let_loop
)]

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use sparrow_core::Identity;
use sparrow_core::event::RunId;
use sparrow_providers::Msg;
use crate::redaction::RedactionFilter;

pub mod redaction;
#[cfg(feature = "treesitter")]
pub mod symbol_index;

pub mod cli;
pub mod fts;

pub const MEMORY_MD_LIMIT: usize = 2200;
pub const USER_MD_LIMIT: usize = 1375;

// ─── Repo map ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoMap {
    pub root: PathBuf,
    pub files: Vec<FileEntry>,
    pub symbols: Vec<SymbolEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub size: u64,
    pub modified: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolEntry {
    pub file: String,
    pub name: String,
    pub kind: String, // "fn", "struct", "impl", "mod", etc.
    pub line: u32,
}

impl RepoMap {
    pub fn scan(root: &Path) -> Self {
        let mut files = Vec::new();
        let mut symbols = Vec::new();
        scan_dir(root, root, &mut files, &mut symbols);
        Self {
            root: root.to_path_buf(),
            files,
            symbols,
        }
    }
}

fn scan_dir(base: &Path, dir: &Path, files: &mut Vec<FileEntry>, symbols: &mut Vec<SymbolEntry>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            // Skip hidden, node_modules, target, .git
            if name.starts_with('.')
                || name == "node_modules"
                || name == "target"
                || name == "build"
                || name == "dist"
            {
                continue;
            }

            if path.is_dir() {
                scan_dir(base, &path, files, symbols);
            } else {
                let rel = path
                    .strip_prefix(base)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();

                let modified = path
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| {
                        chrono::DateTime::from_timestamp(
                            t.duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs() as i64,
                            0,
                        )
                    })
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_default();

                files.push(FileEntry {
                    path: rel.clone(),
                    size: path.metadata().map(|m| m.len()).unwrap_or(0),
                    modified,
                });

                // Basic symbol extraction for Rust files
                if rel.ends_with(".rs") {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        for (i, line) in content.lines().enumerate() {
                            let trimmed = line.trim();
                            if trimmed.starts_with("pub fn ") || trimmed.starts_with("fn ") {
                                let name = trimmed
                                    .trim_start_matches("pub fn ")
                                    .trim_start_matches("fn ")
                                    .split('(')
                                    .next()
                                    .unwrap_or("");
                                symbols.push(SymbolEntry {
                                    file: rel.clone(),
                                    name: name.to_string(),
                                    kind: "fn".into(),
                                    line: (i + 1) as u32,
                                });
                            } else if trimmed.starts_with("pub struct ")
                                || trimmed.starts_with("struct ")
                            {
                                let name = trimmed
                                    .trim_start_matches("pub struct ")
                                    .trim_start_matches("struct ")
                                    .split(|c: char| c == '<' || c == '{' || c == '(')
                                    .next()
                                    .unwrap_or("");
                                symbols.push(SymbolEntry {
                                    file: rel.clone(),
                                    name: name.to_string(),
                                    kind: "struct".into(),
                                    line: (i + 1) as u32,
                                });
                            } else if trimmed.starts_with("pub enum ")
                                || trimmed.starts_with("enum ")
                            {
                                let name = trimmed
                                    .trim_start_matches("pub enum ")
                                    .trim_start_matches("enum ")
                                    .split(|c: char| c == '<' || c == '{')
                                    .next()
                                    .unwrap_or("");
                                symbols.push(SymbolEntry {
                                    file: rel.clone(),
                                    name: name.to_string(),
                                    kind: "enum".into(),
                                    line: (i + 1) as u32,
                                });
                            } else if trimmed.starts_with("pub trait ")
                                || trimmed.starts_with("trait ")
                            {
                                let name = trimmed
                                    .trim_start_matches("pub trait ")
                                    .trim_start_matches("trait ")
                                    .split(|c: char| c == '<' || c == '{')
                                    .next()
                                    .unwrap_or("");
                                symbols.push(SymbolEntry {
                                    file: rel.clone(),
                                    name: name.to_string(),
                                    kind: "trait".into(),
                                    line: (i + 1) as u32,
                                });
                            } else if trimmed.starts_with("pub mod ") || trimmed.starts_with("mod ")
                            {
                                let name = trimmed
                                    .trim_start_matches("pub mod ")
                                    .trim_start_matches("mod ")
                                    .split(';')
                                    .next()
                                    .unwrap_or("");
                                symbols.push(SymbolEntry {
                                    file: rel.clone(),
                                    name: name.to_string(),
                                    kind: "mod".into(),
                                    line: (i + 1) as u32,
                                });
                            }
                        }
                    }
                }
            }
        }
    }
}

// ─── Task memory ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskMem {
    pub run_id: String,
    pub messages: Vec<Msg>,
    pub created_at: String,
}

// ─── Shared memory ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedSignal {
    pub id: String,
    pub from_agent: String,
    pub to_agent: String,
    pub content: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkingDoc {
    pub id: String,
    pub title: String,
    pub content: String,
    pub updated_at: String,
}

// ─── Durable facts ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fact {
    pub id: String,
    pub key: String,
    pub value: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryDocKind {
    Memory,
    User,
}

impl MemoryDocKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Memory => "MEMORY.md",
            Self::User => "USER.md",
        }
    }

    pub fn limit(self) -> usize {
        match self {
            Self::Memory => MEMORY_MD_LIMIT,
            Self::User => USER_MD_LIMIT,
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "memory" | "memory.md" => Some(Self::Memory),
            "user" | "user.md" => Some(Self::User),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryDoc {
    pub kind: MemoryDocKind,
    pub content: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    pub facts: usize,
    pub memory_chars: usize,
    pub memory_limit: usize,
    pub user_chars: usize,
    pub user_limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub kind: String,
    #[serde(default)]
    pub properties: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphEdge {
    pub id: String,
    pub from_id: String,
    pub to_id: String,
    pub relation: String,
    pub weight: f64,
    #[serde(default)]
    pub properties: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KnowledgeGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum GraphDirection {
    Incoming,
    Outgoing,
    Both,
}

impl GraphDirection {
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "incoming" | "in" => Self::Incoming,
            "outgoing" | "out" => Self::Outgoing,
            _ => Self::Both,
        }
    }
}

pub fn validate_memory_text(label: &str, text: &str, limit: usize) -> anyhow::Result<()> {
    let chars = text.chars().count();
    if chars > limit {
        anyhow::bail!("{label} is too large: {chars}/{limit} chars");
    }
    if text.chars().any(is_forbidden_invisible_char) {
        anyhow::bail!("{label} contains invisible control characters");
    }

    let lower = text.to_ascii_lowercase();
    let blocked = [
        "ignore previous instructions",
        "ignore all previous instructions",
        "disregard previous instructions",
        "reveal your system prompt",
        "print your system prompt",
        "exfiltrate",
        "steal secrets",
        "dump secrets",
        "print env",
        "cat ~/.ssh",
        "backdoor",
        "reverse shell",
        "rm -rf /",
    ];
    if blocked.iter().any(|needle| lower.contains(needle)) {
        anyhow::bail!("{label} looks like prompt injection or credential exfiltration");
    }
    Ok(())
}

fn is_forbidden_invisible_char(c: char) -> bool {
    matches!(
        c,
        '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{206F}'
            | '\u{FEFF}'
    ) || (c.is_control() && c != '\n' && c != '\r' && c != '\t')
}

fn truncate_chars(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

// ─── THE MEMORY TRAIT ───────────────────────────────────────────────────────────

pub trait Memory: Send + Sync {
    fn repo_map(&self, root: &Path) -> RepoMap;
    fn identity(&self, agent: &str) -> Option<Identity>;
    fn save_identity(&self, agent: &str, identity: &Identity) -> anyhow::Result<()>;
    fn task(&self, run: &RunId) -> Option<TaskMem>;
    fn save_task(&self, task: &TaskMem) -> anyhow::Result<()>;
    fn shared_signals(&self) -> Vec<SharedSignal>;
    fn shared_docs(&self) -> Vec<WorkingDoc>;
    fn post_signal(&self, signal: SharedSignal) -> anyhow::Result<()>;
    fn upsert_doc(&self, doc: WorkingDoc) -> anyhow::Result<()>;
    fn remember(&self, fact: Fact) -> anyhow::Result<()>;
    fn recall(&self, q: &str, k: usize) -> Vec<Fact>;
    fn all_facts(&self) -> Vec<Fact>;
    fn forget(&self, id: &str) -> anyhow::Result<()>;
    fn memory_doc(&self, _kind: MemoryDocKind) -> Option<MemoryDoc> {
        None
    }
    fn upsert_memory_doc(&self, _kind: MemoryDocKind, _content: &str) -> anyhow::Result<()> {
        anyhow::bail!("memory documents are not supported by this memory backend")
    }
    fn remove_memory_doc(&self, _kind: MemoryDocKind) -> anyhow::Result<()> {
        anyhow::bail!("memory documents are not supported by this memory backend")
    }
    fn memory_stats(&self) -> MemoryStats {
        MemoryStats {
            facts: self.all_facts().len(),
            memory_chars: 0,
            memory_limit: MEMORY_MD_LIMIT,
            user_chars: 0,
            user_limit: USER_MD_LIMIT,
        }
    }
    fn consolidate_memory(&self) -> anyhow::Result<()> {
        anyhow::bail!("memory consolidation is not supported by this memory backend")
    }
    fn cache_discovered_models(&self, provider_id: &str, models: &[String]) -> anyhow::Result<()>;
    fn get_discovered_models(&self, provider_id: &str) -> Vec<String>;
    fn upsert_graph_node(&self, _node: GraphNode) -> anyhow::Result<()> {
        anyhow::bail!("knowledge graph is not supported by this memory backend")
    }
    fn upsert_graph_edge(&self, _edge: GraphEdge) -> anyhow::Result<()> {
        anyhow::bail!("knowledge graph is not supported by this memory backend")
    }
    fn graph_node(&self, _id: &str) -> Option<GraphNode> {
        None
    }
    fn graph_neighbors(
        &self,
        _id: &str,
        _direction: GraphDirection,
        _limit: usize,
    ) -> Vec<(GraphEdge, GraphNode)> {
        Vec::new()
    }
    fn search_graph(&self, _query: &str, _limit: usize) -> Vec<GraphNode> {
        Vec::new()
    }
    fn graph_export(&self) -> KnowledgeGraph {
        KnowledgeGraph::default()
    }
    fn delete_graph_node(&self, _id: &str) -> anyhow::Result<()> {
        anyhow::bail!("knowledge graph is not supported by this memory backend")
    }
    fn delete_graph_edge(&self, _id: &str) -> anyhow::Result<()> {
        anyhow::bail!("knowledge graph is not supported by this memory backend")
    }
}

pub trait MemoryProvider: Send + Sync {
    fn name(&self) -> &str;
    fn remember(&self, fact: Fact) -> anyhow::Result<()>;
    fn recall(&self, query: &str, limit: usize) -> anyhow::Result<Vec<Fact>>;
    fn memory_doc(&self, kind: MemoryDocKind) -> anyhow::Result<Option<MemoryDoc>>;
    fn upsert_memory_doc(&self, kind: MemoryDocKind, content: &str) -> anyhow::Result<()>;
}

pub struct LocalMemoryProvider {
    memory: Arc<dyn Memory>,
}

impl LocalMemoryProvider {
    pub fn new(memory: Arc<dyn Memory>) -> Self {
        Self { memory }
    }
}

impl MemoryProvider for LocalMemoryProvider {
    fn name(&self) -> &str {
        "local"
    }

    fn remember(&self, fact: Fact) -> anyhow::Result<()> {
        self.memory.remember(fact)
    }

    fn recall(&self, query: &str, limit: usize) -> anyhow::Result<Vec<Fact>> {
        Ok(self.memory.recall(query, limit))
    }

    fn memory_doc(&self, kind: MemoryDocKind) -> anyhow::Result<Option<MemoryDoc>> {
        Ok(self.memory.memory_doc(kind))
    }

    fn upsert_memory_doc(&self, kind: MemoryDocKind, content: &str) -> anyhow::Result<()> {
        self.memory.upsert_memory_doc(kind, content)
    }
}

pub struct ExternalMemoryProvider {
    name: String,
}

impl ExternalMemoryProvider {
    pub fn mem0() -> Self {
        Self {
            name: "mem0".into(),
        }
    }

    pub fn honcho() -> Self {
        Self {
            name: "honcho".into(),
        }
    }

    pub fn supermemory() -> Self {
        Self {
            name: "supermemory".into(),
        }
    }

    fn not_configured(&self) -> anyhow::Error {
        anyhow::anyhow!(
            "external memory provider '{}' is not configured; use local SQLite memory or configure a connector first",
            self.name
        )
    }
}

impl MemoryProvider for ExternalMemoryProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn remember(&self, _fact: Fact) -> anyhow::Result<()> {
        Err(self.not_configured())
    }

    fn recall(&self, _query: &str, _limit: usize) -> anyhow::Result<Vec<Fact>> {
        Err(self.not_configured())
    }

    fn memory_doc(&self, _kind: MemoryDocKind) -> anyhow::Result<Option<MemoryDoc>> {
        Err(self.not_configured())
    }

    fn upsert_memory_doc(&self, _kind: MemoryDocKind, _content: &str) -> anyhow::Result<()> {
        Err(self.not_configured())
    }
}

// ─── SQLite-backed memory implementation ────────────────────────────────────────

pub struct SqliteMemory {
    conn: Mutex<Connection>,
}

impl SqliteMemory {
    pub fn open(db_path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        let memory = Self {
            conn: Mutex::new(conn),
        };
        memory.migrate()?;
        Ok(memory)
    }

    fn migrate(&self) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS identities (
                agent TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                role TEXT NOT NULL,
                personality TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS tasks (
                run_id TEXT PRIMARY KEY,
                messages_json TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS signals (
                id TEXT PRIMARY KEY,
                from_agent TEXT NOT NULL,
                to_agent TEXT NOT NULL,
                content TEXT NOT NULL,
                timestamp TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS working_docs (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                content TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS facts (
                id TEXT PRIMARY KEY,
                key TEXT NOT NULL UNIQUE,
                value TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS discovered_models (
                provider_id TEXT NOT NULL,
                model_name TEXT NOT NULL,
                fetched_at TEXT NOT NULL,
                PRIMARY KEY (provider_id, model_name)
            );
            CREATE TABLE IF NOT EXISTS memory_docs (
                kind TEXT PRIMARY KEY,
                content TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS kg_nodes (
                id TEXT PRIMARY KEY,
                label TEXT NOT NULL,
                kind TEXT NOT NULL,
                properties_json TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS kg_edges (
                id TEXT PRIMARY KEY,
                from_id TEXT NOT NULL,
                to_id TEXT NOT NULL,
                relation TEXT NOT NULL,
                weight REAL NOT NULL DEFAULT 1.0,
                properties_json TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY(from_id) REFERENCES kg_nodes(id) ON DELETE CASCADE,
                FOREIGN KEY(to_id) REFERENCES kg_nodes(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS kg_nodes_label_idx ON kg_nodes(label);
            CREATE INDEX IF NOT EXISTS kg_nodes_kind_idx ON kg_nodes(kind);
            CREATE INDEX IF NOT EXISTS kg_edges_from_idx ON kg_edges(from_id);
            CREATE INDEX IF NOT EXISTS kg_edges_to_idx ON kg_edges(to_id);
            CREATE INDEX IF NOT EXISTS kg_edges_relation_idx ON kg_edges(relation);
            -- FTS5 full-text search for memory recall (M1)
            CREATE VIRTUAL TABLE IF NOT EXISTS facts_fts USING fts5(
                key, value, content='facts', content_rowid='rowid'
            );
            -- Triggers to keep FTS5 index in sync
            CREATE TRIGGER IF NOT EXISTS facts_ai AFTER INSERT ON facts BEGIN
                INSERT INTO facts_fts(rowid, key, value) VALUES (new.rowid, new.key, new.value);
            END;
            CREATE TRIGGER IF NOT EXISTS facts_ad AFTER DELETE ON facts BEGIN
                INSERT INTO facts_fts(facts_fts, rowid, key, value) VALUES ('delete', old.rowid, old.key, old.value);
            END;
            CREATE TRIGGER IF NOT EXISTS facts_au AFTER UPDATE ON facts BEGIN
                INSERT INTO facts_fts(facts_fts, rowid, key, value) VALUES ('delete', old.rowid, old.key, old.value);
                INSERT INTO facts_fts(rowid, key, value) VALUES (new.rowid, new.key, new.value);
            END;
            ",
        )?;
        Ok(())
    }
}

fn graph_node_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<GraphNode> {
    let properties_json: String = row.get(3)?;
    Ok(GraphNode {
        id: row.get(0)?,
        label: row.get(1)?,
        kind: row.get(2)?,
        properties: serde_json::from_str(&properties_json).unwrap_or(serde_json::Value::Null),
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

fn graph_edge_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<GraphEdge> {
    let properties_json: String = row.get(5)?;
    Ok(GraphEdge {
        id: row.get(0)?,
        from_id: row.get(1)?,
        to_id: row.get(2)?,
        relation: row.get(3)?,
        weight: row.get(4)?,
        properties: serde_json::from_str(&properties_json).unwrap_or(serde_json::Value::Null),
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

impl Memory for SqliteMemory {
    fn repo_map(&self, root: &Path) -> RepoMap {
        RepoMap::scan(root)
    }

    fn identity(&self, agent: &str) -> Option<Identity> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT name, role, personality FROM identities WHERE agent = ?1",
            params![agent],
            |row| {
                Ok(Identity {
                    name: row.get(0)?,
                    role: row.get(1)?,
                    personality: row.get(2)?,
                })
            },
        )
        .ok()
    }

    fn save_identity(&self, agent: &str, identity: &Identity) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO identities (agent, name, role, personality) VALUES (?1, ?2, ?3, ?4)",
            params![agent, identity.name, identity.role, identity.personality],
        )?;
        Ok(())
    }

    fn task(&self, run: &RunId) -> Option<TaskMem> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT run_id, messages_json, created_at FROM tasks WHERE run_id = ?1",
            params![run.0],
            |row| {
                let messages_json: String = row.get(1)?;
                let messages: Vec<Msg> = serde_json::from_str(&messages_json).unwrap_or_default();
                Ok(TaskMem {
                    run_id: row.get(0)?,
                    messages,
                    created_at: row.get(2)?,
                })
            },
        )
        .ok()
    }

    fn save_task(&self, task: &TaskMem) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        let messages_json = serde_json::to_string(&task.messages)?;
        conn.execute(
            "INSERT OR REPLACE INTO tasks (run_id, messages_json, created_at) VALUES (?1, ?2, ?3)",
            params![task.run_id, messages_json, task.created_at],
        )?;
        Ok(())
    }

    fn shared_signals(&self) -> Vec<SharedSignal> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT id, from_agent, to_agent, content, timestamp FROM signals ORDER BY timestamp DESC")
            .unwrap();
        let signals = stmt
            .query_map([], |row| {
                Ok(SharedSignal {
                    id: row.get(0)?,
                    from_agent: row.get(1)?,
                    to_agent: row.get(2)?,
                    content: row.get(3)?,
                    timestamp: row.get(4)?,
                })
            })
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        signals
    }

    fn shared_docs(&self) -> Vec<WorkingDoc> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, title, content, updated_at FROM working_docs ORDER BY updated_at DESC",
            )
            .unwrap();
        let docs = stmt
            .query_map([], |row| {
                Ok(WorkingDoc {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    content: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            })
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        docs
    }

    fn post_signal(&self, signal: SharedSignal) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO signals (id, from_agent, to_agent, content, timestamp) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![signal.id, signal.from_agent, signal.to_agent, signal.content, signal.timestamp],
        )?;
        Ok(())
    }

    fn upsert_doc(&self, doc: WorkingDoc) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO working_docs (id, title, content, updated_at) VALUES (?1, ?2, ?3, ?4)",
            params![doc.id, doc.title, doc.content, doc.updated_at],
        )?;
        Ok(())
    }

    fn remember(&self, fact: Fact) -> anyhow::Result<()> {
        let redaction = RedactionFilter::new();
        validate_memory_text("fact key", &fact.key, 256)?;
        validate_memory_text("fact value", &fact.value, 1200)?;
        let safe_value = redaction.redact_str(&fact.value);
        let safe_key = redaction.redact_str(&fact.key);
        if redaction.contains_secret(&fact.value) {
            tracing::warn!("Redacted secret from memory fact: {}", fact.key);
        }
        let conn = self.conn.lock().unwrap();
        let existing: Option<String> = conn
            .query_row(
                "SELECT id FROM facts WHERE key = ?1",
                params![safe_key.clone()],
                |row| row.get(0),
            )
            .ok();
        if existing.as_deref().is_some_and(|id| id != fact.id) {
            anyhow::bail!(
                "memory fact '{}' already exists; use replace/consolidate",
                safe_key
            );
        }
        conn.execute(
            "INSERT OR REPLACE INTO facts (id, key, value, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![fact.id, safe_key, safe_value, fact.created_at, fact.updated_at],
        )?;
        Ok(())
    }

    fn recall(&self, q: &str, k: usize) -> Vec<Fact> {
        let conn = self.conn.lock().unwrap();

        // Build a safe FTS5 MATCH expression. FTS5 chokes on unescaped tokens
        // containing punctuation, hyphens, quotes, or its reserved keywords
        // (AND, OR, NOT, NEAR). Strategy: split on whitespace, drop non-word
        // chars from each token, double-quote it, and append a prefix wildcard.
        // Example: `users.id rm -rf` -> `"usersid"* "rm"* "rf"*`.
        let tokens: Vec<String> = q
            .split_whitespace()
            .map(|w| {
                w.chars()
                    .filter(|c| c.is_alphanumeric() || *c == '_')
                    .collect::<String>()
            })
            .filter(|w| !w.is_empty())
            .map(|w| format!("\"{}\"*", w))
            .collect();

        // No usable tokens (e.g. query was only punctuation) — go straight to LIKE.
        let try_fts = !tokens.is_empty();
        if try_fts {
            let pattern = tokens.join(" ");
            if let Ok(mut stmt) = conn.prepare(
                "SELECT f.id, f.key, f.value, f.created_at, f.updated_at FROM facts f
                 INNER JOIN facts_fts ft ON f.rowid = ft.rowid
                 WHERE facts_fts MATCH ?1 ORDER BY rank LIMIT ?2",
            ) {
                if let Ok(rows) = stmt.query_map(params![pattern, k as i64], |row| {
                    Ok(Fact {
                        id: row.get(0)?,
                        key: row.get(1)?,
                        value: row.get(2)?,
                        created_at: row.get(3)?,
                        updated_at: row.get(4)?,
                    })
                }) {
                    let facts: Vec<Fact> = rows.filter_map(|r| r.ok()).collect();
                    if !facts.is_empty() {
                        return facts;
                    }
                    // Fall through to LIKE if FTS returned nothing — LIKE catches
                    // substring matches inside larger tokens that FTS won't.
                }
            }
        }

        // LIKE fallback. Escape % and _ in the user pattern so they don't act
        // as wildcards. Use ESCAPE '\' so the literal characters survive.
        let escaped = q
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let like_pattern = format!("%{}%", escaped);
        let Ok(mut stmt) = conn.prepare(
            "SELECT id, key, value, created_at, updated_at FROM facts \
             WHERE key LIKE ?1 ESCAPE '\\' OR value LIKE ?1 ESCAPE '\\' LIMIT ?2",
        ) else {
            return Vec::new();
        };
        let Ok(rows) = stmt.query_map(params![like_pattern, k as i64], |row| {
            Ok(Fact {
                id: row.get(0)?,
                key: row.get(1)?,
                value: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        }) else {
            return Vec::new();
        };
        rows.filter_map(|r| r.ok()).collect()
    }

    fn all_facts(&self) -> Vec<Fact> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, key, value, created_at, updated_at FROM facts ORDER BY updated_at DESC",
            )
            .unwrap();
        stmt.query_map([], |row| {
            Ok(Fact {
                id: row.get(0)?,
                key: row.get(1)?,
                value: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
    }

    fn forget(&self, id: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM facts WHERE id = ?1", params![id])?;
        Ok(())
    }

    fn memory_doc(&self, kind: MemoryDocKind) -> Option<MemoryDoc> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT content, updated_at FROM memory_docs WHERE kind = ?1",
            params![kind.as_str()],
            |row| {
                Ok(MemoryDoc {
                    kind,
                    content: row.get(0)?,
                    updated_at: row.get(1)?,
                })
            },
        )
        .ok()
    }

    fn upsert_memory_doc(&self, kind: MemoryDocKind, content: &str) -> anyhow::Result<()> {
        validate_memory_text(kind.as_str(), content, kind.limit())?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO memory_docs (kind, content, updated_at)
             VALUES (?1, ?2, datetime('now'))",
            params![kind.as_str(), content],
        )?;
        Ok(())
    }

    fn remove_memory_doc(&self, kind: MemoryDocKind) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM memory_docs WHERE kind = ?1",
            params![kind.as_str()],
        )?;
        Ok(())
    }

    fn memory_stats(&self) -> MemoryStats {
        let memory_chars = self
            .memory_doc(MemoryDocKind::Memory)
            .map(|doc| doc.content.chars().count())
            .unwrap_or(0);
        let user_chars = self
            .memory_doc(MemoryDocKind::User)
            .map(|doc| doc.content.chars().count())
            .unwrap_or(0);
        MemoryStats {
            facts: self.all_facts().len(),
            memory_chars,
            memory_limit: MEMORY_MD_LIMIT,
            user_chars,
            user_limit: USER_MD_LIMIT,
        }
    }

    fn consolidate_memory(&self) -> anyhow::Result<()> {
        let facts = self.all_facts();
        let mut memory_lines = Vec::new();
        memory_lines.push("# MEMORY.md".to_string());
        memory_lines.push("Durable Sparrow memory distilled from accepted facts.".to_string());
        for fact in facts.iter().take(40) {
            memory_lines.push(format!("- {}: {}", fact.key, fact.value));
        }
        let memory = truncate_chars(&memory_lines.join("\n"), MEMORY_MD_LIMIT);
        self.upsert_memory_doc(MemoryDocKind::Memory, &memory)?;

        let mut user_lines = Vec::new();
        user_lines.push("# USER.md".to_string());
        for fact in facts
            .iter()
            .filter(|fact| fact.key.starts_with("user"))
            .take(24)
        {
            user_lines.push(format!("- {}: {}", fact.key, fact.value));
        }
        if user_lines.len() > 1 {
            let user = truncate_chars(&user_lines.join("\n"), USER_MD_LIMIT);
            self.upsert_memory_doc(MemoryDocKind::User, &user)?;
        }
        Ok(())
    }

    fn cache_discovered_models(&self, provider_id: &str, models: &[String]) -> anyhow::Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM discovered_models WHERE provider_id = ?1",
            params![provider_id],
        )?;
        let fetched_at = chrono::Utc::now().to_rfc3339();
        for model in models {
            let model = model.trim();
            if model.is_empty() {
                continue;
            }
            tx.execute(
                "INSERT OR REPLACE INTO discovered_models (provider_id, model_name, fetched_at)
                 VALUES (?1, ?2, ?3)",
                params![provider_id, model, fetched_at],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    fn get_discovered_models(&self, provider_id: &str) -> Vec<String> {
        let conn = self.conn.lock().unwrap();
        // Discovered model lists are stable for weeks at a time — vendors don't
        // rotate the catalogue daily. The previous 24-hour TTL silently dropped
        // a working scan after one day, forcing the user to redo it. Keep the
        // cache for 30 days; an explicit `sparrow scan` always overwrites it.
        let Ok(mut stmt) = conn.prepare(
            "SELECT model_name FROM discovered_models
             WHERE provider_id = ?1
               AND datetime(fetched_at) >= datetime('now', '-30 days')
             ORDER BY model_name ASC",
        ) else {
            return Vec::new();
        };
        let Ok(rows) = stmt.query_map(params![provider_id], |row| row.get::<_, String>(0)) else {
            return Vec::new();
        };
        rows.filter_map(|row| row.ok()).collect()
    }

    fn upsert_graph_node(&self, node: GraphNode) -> anyhow::Result<()> {
        validate_memory_text("graph node id", &node.id, 160)?;
        validate_memory_text("graph node label", &node.label, 512)?;
        validate_memory_text("graph node kind", &node.kind, 80)?;
        validate_memory_text("graph node properties", &node.properties.to_string(), 4000)?;
        let redaction = RedactionFilter::new();
        let safe_id = redaction.redact_str(&node.id);
        let safe_label = redaction.redact_str(&node.label);
        let safe_kind = redaction.redact_str(&node.kind);
        let safe_properties = serde_json::to_string(&node.properties)?;
        let safe_properties = redaction.redact_str(&safe_properties);
        let now = chrono::Utc::now().to_rfc3339();
        let created_at = if node.created_at.trim().is_empty() {
            now.clone()
        } else {
            node.created_at
        };
        let updated_at = if node.updated_at.trim().is_empty() {
            now
        } else {
            node.updated_at
        };
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO kg_nodes (id, label, kind, properties_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                label = excluded.label,
                kind = excluded.kind,
                properties_json = excluded.properties_json,
                updated_at = excluded.updated_at",
            params![
                safe_id,
                safe_label,
                safe_kind,
                safe_properties,
                created_at,
                updated_at
            ],
        )?;
        Ok(())
    }

    fn upsert_graph_edge(&self, edge: GraphEdge) -> anyhow::Result<()> {
        validate_memory_text("graph edge id", &edge.id, 160)?;
        validate_memory_text("graph edge from_id", &edge.from_id, 160)?;
        validate_memory_text("graph edge to_id", &edge.to_id, 160)?;
        validate_memory_text("graph edge relation", &edge.relation, 120)?;
        validate_memory_text("graph edge properties", &edge.properties.to_string(), 4000)?;
        let redaction = RedactionFilter::new();
        let safe_properties = serde_json::to_string(&edge.properties)?;
        let safe_properties = redaction.redact_str(&safe_properties);
        let now = chrono::Utc::now().to_rfc3339();
        let created_at = if edge.created_at.trim().is_empty() {
            now.clone()
        } else {
            edge.created_at
        };
        let updated_at = if edge.updated_at.trim().is_empty() {
            now
        } else {
            edge.updated_at
        };
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO kg_edges (id, from_id, to_id, relation, weight, properties_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
                from_id = excluded.from_id,
                to_id = excluded.to_id,
                relation = excluded.relation,
                weight = excluded.weight,
                properties_json = excluded.properties_json,
                updated_at = excluded.updated_at",
            params![
                redaction.redact_str(&edge.id),
                redaction.redact_str(&edge.from_id),
                redaction.redact_str(&edge.to_id),
                redaction.redact_str(&edge.relation),
                edge.weight,
                safe_properties,
                created_at,
                updated_at
            ],
        )?;
        Ok(())
    }

    fn graph_node(&self, id: &str) -> Option<GraphNode> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, label, kind, properties_json, created_at, updated_at
             FROM kg_nodes WHERE id = ?1",
            params![id],
            graph_node_from_row,
        )
        .ok()
    }

    fn graph_neighbors(
        &self,
        id: &str,
        direction: GraphDirection,
        limit: usize,
    ) -> Vec<(GraphEdge, GraphNode)> {
        let conn = self.conn.lock().unwrap();
        let limit = limit.clamp(1, 100) as i64;
        let sql = match direction {
            GraphDirection::Outgoing => {
                "SELECT e.id, e.from_id, e.to_id, e.relation, e.weight, e.properties_json, e.created_at, e.updated_at,
                        n.id, n.label, n.kind, n.properties_json, n.created_at, n.updated_at
                 FROM kg_edges e
                 JOIN kg_nodes n ON n.id = e.to_id
                 WHERE e.from_id = ?1
                 ORDER BY e.weight DESC, e.updated_at DESC
                 LIMIT ?2"
            }
            GraphDirection::Incoming => {
                "SELECT e.id, e.from_id, e.to_id, e.relation, e.weight, e.properties_json, e.created_at, e.updated_at,
                        n.id, n.label, n.kind, n.properties_json, n.created_at, n.updated_at
                 FROM kg_edges e
                 JOIN kg_nodes n ON n.id = e.from_id
                 WHERE e.to_id = ?1
                 ORDER BY e.weight DESC, e.updated_at DESC
                 LIMIT ?2"
            }
            GraphDirection::Both => {
                "SELECT e.id, e.from_id, e.to_id, e.relation, e.weight, e.properties_json, e.created_at, e.updated_at,
                        n.id, n.label, n.kind, n.properties_json, n.created_at, n.updated_at
                 FROM kg_edges e
                 JOIN kg_nodes n ON n.id = CASE WHEN e.from_id = ?1 THEN e.to_id ELSE e.from_id END
                 WHERE e.from_id = ?1 OR e.to_id = ?1
                 ORDER BY e.weight DESC, e.updated_at DESC
                 LIMIT ?2"
            }
        };
        let Ok(mut stmt) = conn.prepare(sql) else {
            return Vec::new();
        };
        let Ok(rows) = stmt.query_map(params![id, limit], |row| {
            let edge = graph_edge_from_row(row)?;
            let properties_json: String = row.get(11)?;
            let node = GraphNode {
                id: row.get(8)?,
                label: row.get(9)?,
                kind: row.get(10)?,
                properties: serde_json::from_str(&properties_json)
                    .unwrap_or(serde_json::Value::Null),
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
            };
            Ok((edge, node))
        }) else {
            return Vec::new();
        };
        rows.filter_map(|row| row.ok()).collect()
    }

    fn search_graph(&self, query: &str, limit: usize) -> Vec<GraphNode> {
        let query = query.trim();
        if query.is_empty() {
            return Vec::new();
        }
        let escaped = query
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let pattern = format!("%{}%", escaped);
        let conn = self.conn.lock().unwrap();
        let Ok(mut stmt) = conn.prepare(
            "SELECT id, label, kind, properties_json, created_at, updated_at
             FROM kg_nodes
             WHERE id LIKE ?1 ESCAPE '\\'
                OR label LIKE ?1 ESCAPE '\\'
                OR kind LIKE ?1 ESCAPE '\\'
                OR properties_json LIKE ?1 ESCAPE '\\'
             ORDER BY updated_at DESC
             LIMIT ?2",
        ) else {
            return Vec::new();
        };
        let Ok(rows) = stmt.query_map(
            params![pattern, limit.clamp(1, 100) as i64],
            graph_node_from_row,
        ) else {
            return Vec::new();
        };
        rows.filter_map(|row| row.ok()).collect()
    }

    fn graph_export(&self) -> KnowledgeGraph {
        let conn = self.conn.lock().unwrap();
        let nodes = conn
            .prepare(
                "SELECT id, label, kind, properties_json, created_at, updated_at
                 FROM kg_nodes ORDER BY kind, label",
            )
            .and_then(|mut stmt| {
                let rows = stmt.query_map([], graph_node_from_row)?;
                Ok(rows.filter_map(|row| row.ok()).collect::<Vec<_>>())
            })
            .unwrap_or_default();
        let edges = conn
            .prepare(
                "SELECT id, from_id, to_id, relation, weight, properties_json, created_at, updated_at
                 FROM kg_edges ORDER BY relation, from_id, to_id",
            )
            .and_then(|mut stmt| {
                let rows = stmt.query_map([], graph_edge_from_row)?;
                Ok(rows.filter_map(|row| row.ok()).collect::<Vec<_>>())
            })
            .unwrap_or_default();
        KnowledgeGraph { nodes, edges }
    }

    fn delete_graph_node(&self, id: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM kg_edges WHERE from_id = ?1 OR to_id = ?1",
            params![id],
        )?;
        conn.execute("DELETE FROM kg_nodes WHERE id = ?1", params![id])?;
        Ok(())
    }

    fn delete_graph_edge(&self, id: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM kg_edges WHERE id = ?1", params![id])?;
        Ok(())
    }
}
