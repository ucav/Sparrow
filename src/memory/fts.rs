//! Full-Text Search (FTS5) for Sparrow sessions.
//!
//! Provides fast full-text search across all conversation sessions.
//! Uses SQLite FTS5 for indexing and querying.

use std::path::PathBuf;

/// A search hit in the session database.
#[derive(Debug, Clone)]
pub struct SessionHit {
    pub session_id: String,
    pub title: Option<String>,
    pub snippet: String,
    pub started_at: String,
    pub match_count: usize,
}

/// Session search engine backed by SQLite FTS5.
pub struct SessionSearch {
    db_path: PathBuf,
}

impl SessionSearch {
    /// Open or create the session search database.
    pub fn open(db_path: PathBuf) -> anyhow::Result<Self> {
        let conn = rusqlite::Connection::open(&db_path)?;

        // Enable FTS5
        conn.execute_batch("
            CREATE VIRTUAL TABLE IF NOT EXISTS sessions_fts USING fts5(
                session_id,
                title,
                content,
                tokenize='unicode61'
            );
        ")?;

        Ok(Self { db_path })
    }

    /// Index a session's content for full-text search.
    pub fn index_session(
        &self,
        session_id: &str,
        title: Option<&str>,
        content: &str,
    ) -> anyhow::Result<()> {
        let conn = rusqlite::Connection::open(&self.db_path)?;

        // Delete existing entry for this session
        conn.execute(
            "DELETE FROM sessions_fts WHERE session_id = ?1",
            rusqlite::params![session_id],
        )?;

        // Insert new entry
        conn.execute(
            "INSERT INTO sessions_fts (session_id, title, content) VALUES (?1, ?2, ?3)",
            rusqlite::params![session_id, title.unwrap_or(""), content],
        )?;

        Ok(())
    }

    /// Search sessions by query.
    pub fn search(&self, query: &str, limit: usize) -> anyhow::Result<Vec<SessionHit>> {
        let conn = rusqlite::Connection::open(&self.db_path)?;

        // Use FTS5 with snippet for highlighting
        let mut stmt = conn.prepare(
            "SELECT session_id, title, snippet(sessions_fts, 2, '<b>', '</b>', '...', 40) as snippet,
                    sessions_fts.rank as rank
             FROM sessions_fts
             WHERE sessions_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2"
        )?;

        let hits = stmt.query_map(rusqlite::params![query, limit as i64], |row| {
            Ok(SessionHit {
                session_id: row.get(0)?,
                title: row.get(1)?,
                snippet: row.get::<_, String>(2).unwrap_or_default(),
                started_at: String::new(),
                match_count: 1,
            })
        })?.filter_map(|r| r.ok()).collect();

        Ok(hits)
    }

    /// Get recent sessions (no search query).
    pub fn recent(&self, limit: usize) -> anyhow::Result<Vec<SessionHit>> {
        let conn = rusqlite::Connection::open(&self.db_path)?;

        let mut stmt = conn.prepare(
            "SELECT session_id, title, substr(content, 1, 200) as snippet
             FROM sessions_fts
             ORDER BY rowid DESC
             LIMIT ?1"
        )?;

        let hits = stmt.query_map(rusqlite::params![limit as i64], |row| {
            Ok(SessionHit {
                session_id: row.get(0)?,
                title: row.get(1)?,
                snippet: row.get::<_, String>(2).unwrap_or_default(),
                started_at: String::new(),
                match_count: 1,
            })
        })?.filter_map(|r| r.ok()).collect();

        Ok(hits)
    }

    /// Delete a session from the index.
    pub fn remove_session(&self, session_id: &str) -> anyhow::Result<bool> {
        let conn = rusqlite::Connection::open(&self.db_path)?;
        let count = conn.execute(
            "DELETE FROM sessions_fts WHERE session_id = ?1",
            rusqlite::params![session_id],
        )?;
        Ok(count > 0)
    }

    /// Get the total number of indexed sessions.
    pub fn count(&self) -> anyhow::Result<usize> {
        let conn = rusqlite::Connection::open(&self.db_path)?;
        let count: usize = conn.query_row("SELECT COUNT(*) FROM sessions_fts", [], |row| row.get(0))?;
        Ok(count)
    }

    /// Rebuild the FTS index (useful after corruption or migration).
    pub fn rebuild(&self) -> anyhow::Result<()> {
        let conn = rusqlite::Connection::open(&self.db_path)?;
        conn.execute_batch("
            DELETE FROM sessions_fts;
            INSERT INTO sessions_fts(sessions_fts) VALUES('rebuild');
        ")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_and_search() {
        let tmp = std::env::temp_dir().join("sparrow_test_fts.db");
        let search = SessionSearch::open(tmp.clone()).unwrap();

        search.index_session("sess1", Some("Test Session"), "This is about Rust programming").unwrap();
        search.index_session("sess2", Some("Another"), "Python is great for data science").unwrap();

        let hits = search.search("Rust programming", 5).unwrap();
        assert!(!hits.is_empty());

        let _ = std::fs::remove_file(&tmp);
    }
}
