use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::engine::Identity;
use crate::event::RunId;
use crate::provider::Msg;

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

fn scan_dir(
    base: &Path,
    dir: &Path,
    files: &mut Vec<FileEntry>,
    symbols: &mut Vec<SymbolEntry>,
) {
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
                            } else if trimmed.starts_with("pub enum ") || trimmed.starts_with("enum ")
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

// ─── THE MEMORY TRAIT ───────────────────────────────────────────────────────────

pub trait Memory: Send + Sync {
    fn repo_map(&self, root: &Path) -> RepoMap;
    fn identity(&self, agent: &str) -> Option<Identity>;
    fn save_identity(&self, identity: &Identity) -> anyhow::Result<()>;
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
            ",
        )?;
        Ok(())
    }
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

    fn save_identity(&self, identity: &Identity) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO identities (agent, name, role, personality) VALUES (?1, ?2, ?3, ?4)",
            params![identity.name, identity.name, identity.role, identity.personality],
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
                let messages: Vec<Msg> =
                    serde_json::from_str(&messages_json).unwrap_or_default();
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
            .prepare("SELECT id, title, content, updated_at FROM working_docs ORDER BY updated_at DESC")
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
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO facts (id, key, value, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![fact.id, fact.key, fact.value, fact.created_at, fact.updated_at],
        )?;
        Ok(())
    }

    fn recall(&self, q: &str, k: usize) -> Vec<Fact> {
        // Simple SQL LIKE search (no embeddings for M1)
        let conn = self.conn.lock().unwrap();
        let pattern = format!("%{}%", q);
        let mut stmt = conn
            .prepare(
                "SELECT id, key, value, created_at, updated_at FROM facts WHERE key LIKE ?1 OR value LIKE ?1 LIMIT ?2",
            )
            .unwrap();
        let facts = stmt
            .query_map(params![pattern, k as i64], |row| {
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
            .collect();
        facts
    }

    fn all_facts(&self) -> Vec<Fact> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT id, key, value, created_at, updated_at FROM facts ORDER BY updated_at DESC")
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
}
