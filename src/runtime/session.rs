// ─── Session persistence (Phase 9 Item 27) ─────────────────────────────────────

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub name: Option<String>,
    pub status: String,
    pub messages_json: String,
    pub created_at: i64,
    pub updated_at: i64,
}

pub struct SessionStore {
    conn: Mutex<Connection>,
}

impl SessionStore {
    pub fn open(db_path: &std::path::Path) -> anyhow::Result<Self> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                created_at INTEGER NOT NULL DEFAULT (unixepoch()),
                updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
                name TEXT,
                status TEXT DEFAULT 'active',
                messages_json TEXT NOT NULL DEFAULT '[]'
            );",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn save(
        &self,
        id: &str,
        messages: &[crate::provider::Msg],
        name: Option<&str>,
    ) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        let json = serde_json::to_string(messages)?;
        conn.execute(
            "INSERT OR REPLACE INTO sessions (id, name, messages_json, updated_at) VALUES (?1, ?2, ?3, unixepoch())",
            params![id, name, json],
        )?;
        Ok(())
    }

    pub fn load(&self, id: &str) -> Option<Session> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, name, status, messages_json, created_at, updated_at FROM sessions WHERE id = ?1",
            params![id],
            |row| Ok(Session {
                id: row.get(0)?, name: row.get(1)?, status: row.get(2)?,
                messages_json: row.get(3)?, created_at: row.get(4)?, updated_at: row.get(5)?,
            }),
        ).ok()
    }

    pub fn list(&self) -> Vec<Session> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, status, messages_json, created_at, updated_at FROM sessions ORDER BY updated_at DESC LIMIT 100"
        ).unwrap();
        stmt.query_map([], |row| {
            Ok(Session {
                id: row.get(0)?,
                name: row.get(1)?,
                status: row.get(2)?,
                messages_json: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
    }

    pub fn delete(&self, id: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM sessions WHERE id = ?1", params![id])?;
        Ok(())
    }
}

// ─── Prometheus metrics (Phase 10 Item 29) ─────────────────────────────────────

use std::sync::atomic::{AtomicU64, Ordering};

pub struct Metrics {
    pub requests_total: AtomicU64,
    pub requests_errors: AtomicU64,
    pub tokens_input: AtomicU64,
    pub tokens_output: AtomicU64,
    pub tool_calls_total: AtomicU64,
    pub tool_calls_errors: AtomicU64,
    pub cost_usd_cents: AtomicU64,
    pub active_sessions: AtomicU64,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            requests_total: AtomicU64::new(0),
            requests_errors: AtomicU64::new(0),
            tokens_input: AtomicU64::new(0),
            tokens_output: AtomicU64::new(0),
            tool_calls_total: AtomicU64::new(0),
            tool_calls_errors: AtomicU64::new(0),
            cost_usd_cents: AtomicU64::new(0),
            active_sessions: AtomicU64::new(0),
        }
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "# HELP sparrow_requests_total Total number of requests\n"
        ));
        out.push_str(&format!("# TYPE sparrow_requests_total counter\n"));
        out.push_str(&format!(
            "sparrow_requests_total{{status=\"ok\"}} {}\n",
            self.requests_total.load(Ordering::Relaxed)
        ));
        out.push_str(&format!(
            "sparrow_requests_total{{status=\"error\"}} {}\n",
            self.requests_errors.load(Ordering::Relaxed)
        ));

        out.push_str(&format!(
            "# HELP sparrow_tokens_used_total Total tokens used\n"
        ));
        out.push_str(&format!("# TYPE sparrow_tokens_used_total counter\n"));
        out.push_str(&format!(
            "sparrow_tokens_used_total{{direction=\"input\"}} {}\n",
            self.tokens_input.load(Ordering::Relaxed)
        ));
        out.push_str(&format!(
            "sparrow_tokens_used_total{{direction=\"output\"}} {}\n",
            self.tokens_output.load(Ordering::Relaxed)
        ));

        out.push_str(&format!(
            "# HELP sparrow_tool_calls_total Total tool calls\n"
        ));
        out.push_str(&format!("# TYPE sparrow_tool_calls_total counter\n"));
        out.push_str(&format!(
            "sparrow_tool_calls_total{{status=\"ok\"}} {}\n",
            self.tool_calls_total.load(Ordering::Relaxed)
        ));
        out.push_str(&format!(
            "sparrow_tool_calls_total{{status=\"error\"}} {}\n",
            self.tool_calls_errors.load(Ordering::Relaxed)
        ));

        out.push_str(&format!(
            "# HELP sparrow_cost_usd_total Total cost in USD cents\n"
        ));
        out.push_str(&format!("# TYPE sparrow_cost_usd_total counter\n"));
        out.push_str(&format!(
            "sparrow_cost_usd_total {}\n",
            self.cost_usd_cents.load(Ordering::Relaxed)
        ));

        out.push_str(&format!("# HELP sparrow_active_sessions Active sessions\n"));
        out.push_str(&format!("# TYPE sparrow_active_sessions gauge\n"));
        out.push_str(&format!(
            "sparrow_active_sessions {}\n",
            self.active_sessions.load(Ordering::Relaxed)
        ));

        out
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}
