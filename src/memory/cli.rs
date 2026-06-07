//! Memory CLI — visible memory management for Sparrow.
//!
//! Commands: show, edit, search, list facts stored in SQLite.

use std::path::PathBuf;

/// A single memory fact stored in the knowledge base.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemoryFact {
    pub id: i64,
    pub key: String,
    pub value: String,
    pub category: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Memory store backed by SQLite.
pub struct MemoryStore {
    db_path: PathBuf,
}

impl MemoryStore {
    /// Open or create the memory database.
    pub fn open(db_path: PathBuf) -> anyhow::Result<Self> {
        let conn = rusqlite::Connection::open(&db_path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS memory (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                key TEXT NOT NULL UNIQUE,
                value TEXT NOT NULL,
                category TEXT NOT NULL DEFAULT 'general',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_memory_key ON memory(key);
            CREATE INDEX IF NOT EXISTS idx_memory_category ON memory(category);",
        )?;
        Ok(Self { db_path })
    }

    /// Upsert a memory fact.
    pub fn set(&self, key: &str, value: &str, category: &str) -> anyhow::Result<()> {
        let conn = rusqlite::Connection::open(&self.db_path)?;
        conn.execute(
            "INSERT INTO memory (key, value, category) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = ?2, category = ?3, updated_at = datetime('now')",
            rusqlite::params![key, value, category],
        )?;
        Ok(())
    }

    /// Get a memory fact by key.
    pub fn get(&self, key: &str) -> anyhow::Result<Option<MemoryFact>> {
        let conn = rusqlite::Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT id, key, value, category, created_at, updated_at FROM memory WHERE key = ?1",
        )?;
        let result = stmt
            .query_row(rusqlite::params![key], |row| {
                Ok(MemoryFact {
                    id: row.get(0)?,
                    key: row.get(1)?,
                    value: row.get(2)?,
                    category: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })
            .ok();
        Ok(result)
    }

    /// Search memory facts by keyword.
    pub fn search(&self, query: &str) -> anyhow::Result<Vec<MemoryFact>> {
        let conn = rusqlite::Connection::open(&self.db_path)?;
        let pattern = format!("%{}%", query);
        let mut stmt = conn.prepare(
            "SELECT id, key, value, category, created_at, updated_at FROM memory
             WHERE key LIKE ?1 OR value LIKE ?1 OR category LIKE ?1
             ORDER BY updated_at DESC LIMIT 50",
        )?;
        let facts = stmt
            .query_map(rusqlite::params![pattern], |row| {
                Ok(MemoryFact {
                    id: row.get(0)?,
                    key: row.get(1)?,
                    value: row.get(2)?,
                    category: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(facts)
    }

    /// List all memory facts.
    pub fn list_all(&self) -> anyhow::Result<Vec<MemoryFact>> {
        let conn = rusqlite::Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT id, key, value, category, created_at, updated_at FROM memory
             ORDER BY updated_at DESC",
        )?;
        let facts = stmt
            .query_map([], |row| {
                Ok(MemoryFact {
                    id: row.get(0)?,
                    key: row.get(1)?,
                    value: row.get(2)?,
                    category: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(facts)
    }

    /// List facts by category.
    pub fn list_by_category(&self, category: &str) -> anyhow::Result<Vec<MemoryFact>> {
        let conn = rusqlite::Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT id, key, value, category, created_at, updated_at FROM memory
             WHERE category = ?1 ORDER BY updated_at DESC",
        )?;
        let facts = stmt
            .query_map(rusqlite::params![category], |row| {
                Ok(MemoryFact {
                    id: row.get(0)?,
                    key: row.get(1)?,
                    value: row.get(2)?,
                    category: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(facts)
    }

    /// Delete a memory fact by key.
    pub fn delete(&self, key: &str) -> anyhow::Result<bool> {
        let conn = rusqlite::Connection::open(&self.db_path)?;
        let count = conn.execute("DELETE FROM memory WHERE key = ?1", rusqlite::params![key])?;
        Ok(count > 0)
    }

    /// Get the total count of facts.
    pub fn count(&self) -> anyhow::Result<usize> {
        let conn = rusqlite::Connection::open(&self.db_path)?;
        let count: usize = conn.query_row("SELECT COUNT(*) FROM memory", [], |row| row.get(0))?;
        Ok(count)
    }

    /// Get all categories with counts.
    pub fn categories(&self) -> anyhow::Result<Vec<(String, usize)>> {
        let conn = rusqlite::Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT category, COUNT(*) as cnt FROM memory GROUP BY category ORDER BY cnt DESC",
        )?;
        let cats = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, usize>(1)?))
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(cats)
    }
}
