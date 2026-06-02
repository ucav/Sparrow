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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSearchHit {
    pub session_id: String,
    pub turn_index: usize,
    pub role: String,
    pub text: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSlice {
    pub session_id: String,
    pub start: usize,
    pub messages: Vec<crate::provider::Msg>,
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
            );
            CREATE TABLE IF NOT EXISTS session_messages (
                session_id TEXT NOT NULL,
                turn_index INTEGER NOT NULL,
                role TEXT NOT NULL,
                text TEXT NOT NULL,
                updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
                PRIMARY KEY (session_id, turn_index)
            );
            CREATE VIRTUAL TABLE IF NOT EXISTS session_messages_fts USING fts5(
                role, text, content='session_messages', content_rowid='rowid'
            );
            CREATE TRIGGER IF NOT EXISTS session_messages_ai AFTER INSERT ON session_messages BEGIN
                INSERT INTO session_messages_fts(rowid, role, text) VALUES (new.rowid, new.role, new.text);
            END;
            CREATE TRIGGER IF NOT EXISTS session_messages_ad AFTER DELETE ON session_messages BEGIN
                INSERT INTO session_messages_fts(session_messages_fts, rowid, role, text) VALUES ('delete', old.rowid, old.role, old.text);
            END;
            CREATE TRIGGER IF NOT EXISTS session_messages_au AFTER UPDATE ON session_messages BEGIN
                INSERT INTO session_messages_fts(session_messages_fts, rowid, role, text) VALUES ('delete', old.rowid, old.role, old.text);
                INSERT INTO session_messages_fts(rowid, role, text) VALUES (new.rowid, new.role, new.text);
            END;",
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
        let mut conn = self.conn.lock().unwrap();
        let json = serde_json::to_string(messages)?;
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT OR REPLACE INTO sessions (id, name, messages_json, updated_at) VALUES (?1, ?2, ?3, unixepoch())",
            params![id, name, json],
        )?;
        tx.execute(
            "DELETE FROM session_messages WHERE session_id = ?1",
            params![id],
        )?;
        for (turn_index, msg) in messages.iter().enumerate() {
            let text = message_text(msg);
            if text.trim().is_empty() {
                continue;
            }
            tx.execute(
                "INSERT INTO session_messages (session_id, turn_index, role, text, updated_at)
                 VALUES (?1, ?2, ?3, ?4, unixepoch())",
                params![id, turn_index as i64, msg.role, text],
            )?;
        }
        tx.commit()?;
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
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM session_messages WHERE session_id = ?1",
            params![id],
        )?;
        tx.execute("DELETE FROM sessions WHERE id = ?1", params![id])?;
        tx.commit()?;
        Ok(())
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<SessionSearchHit> {
        let query = query.trim();
        if query.is_empty() {
            return Vec::new();
        }
        let conn = self.conn.lock().unwrap();
        let pattern = query
            .split_whitespace()
            .map(|word| format!("{}*", escape_fts_token(word)))
            .collect::<Vec<_>>()
            .join(" ");
        let result = conn.prepare(
            "SELECT sm.session_id, sm.turn_index, sm.role, sm.text, sm.updated_at
             FROM session_messages sm
             INNER JOIN session_messages_fts fts ON sm.rowid = fts.rowid
             WHERE session_messages_fts MATCH ?1
             ORDER BY rank LIMIT ?2",
        );
        if let Ok(mut stmt) = result {
            if let Ok(rows) = stmt.query_map(params![pattern, limit as i64], |row| {
                Ok(SessionSearchHit {
                    session_id: row.get(0)?,
                    turn_index: row.get::<_, i64>(1)? as usize,
                    role: row.get(2)?,
                    text: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            }) {
                return rows.filter_map(|row| row.ok()).collect();
            }
        }

        let like_pattern = format!("%{}%", query);
        let Ok(mut stmt) = conn.prepare(
            "SELECT session_id, turn_index, role, text, updated_at
             FROM session_messages
             WHERE text LIKE ?1
             ORDER BY updated_at DESC LIMIT ?2",
        ) else {
            return Vec::new();
        };
        let Ok(rows) = stmt.query_map(params![like_pattern, limit as i64], |row| {
            Ok(SessionSearchHit {
                session_id: row.get(0)?,
                turn_index: row.get::<_, i64>(1)? as usize,
                role: row.get(2)?,
                text: row.get(3)?,
                updated_at: row.get(4)?,
            })
        }) else {
            return Vec::new();
        };
        rows.filter_map(|row| row.ok()).collect()
    }

    pub fn scroll(
        &self,
        id: &str,
        around: usize,
        before: usize,
        after: usize,
    ) -> Option<SessionSlice> {
        let session = self.load(id)?;
        let messages: Vec<crate::provider::Msg> =
            serde_json::from_str(&session.messages_json).ok()?;
        let start = around.saturating_sub(before);
        let end = (around + after + 1).min(messages.len());
        Some(SessionSlice {
            session_id: id.to_string(),
            start,
            messages: messages[start..end].to_vec(),
        })
    }

    pub fn recent_inputs(&self, limit: usize) -> Vec<String> {
        let limit = limit.clamp(1, 100);
        let conn = self.conn.lock().unwrap();
        let Ok(mut stmt) = conn.prepare(
            "SELECT text
             FROM session_messages
             WHERE role = 'user' AND trim(text) != ''
             ORDER BY updated_at DESC, session_id DESC, turn_index DESC
             LIMIT ?1",
        ) else {
            return Vec::new();
        };
        let Ok(rows) = stmt.query_map(params![limit as i64], |row| row.get::<_, String>(0)) else {
            return Vec::new();
        };
        let mut seen = std::collections::HashSet::new();
        rows.filter_map(|row| row.ok())
            .filter(|text| seen.insert(text.clone()))
            .collect()
    }
}

fn message_text(msg: &crate::provider::Msg) -> String {
    msg.content
        .iter()
        .filter_map(block_text)
        .collect::<Vec<_>>()
        .join("\n")
}

fn block_text(block: &crate::provider::ContentBlock) -> Option<String> {
    match block {
        crate::provider::ContentBlock::Text { text } => Some(text.clone()),
        crate::provider::ContentBlock::ToolUse { name, .. } => Some(name.clone()),
        crate::provider::ContentBlock::ToolResult { content, .. } => {
            let text = content
                .iter()
                .filter_map(block_text)
                .collect::<Vec<_>>()
                .join("\n");
            if text.trim().is_empty() {
                None
            } else {
                Some(text)
            }
        }
        crate::provider::ContentBlock::Image { .. } => None,
    }
}

fn escape_fts_token(token: &str) -> String {
    token
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .collect()
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
