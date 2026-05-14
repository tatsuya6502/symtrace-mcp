use std::path::{Path, PathBuf};
use turso::Builder;

/// Records usage statistics to a per-project SQLite database.
///
/// Uses an open/write/close pattern: each operation opens the database,
/// performs its write, and drops the connection. This allows the
/// `symtrace-mcp stats` CLI to read the database while the MCP server runs.
pub struct StatsRecorder {
    db_path: PathBuf,
}

impl StatsRecorder {
    pub fn new(project_root: &Path) -> Self {
        Self {
            db_path: project_root.join(".symtrace").join("stats.db"),
        }
    }

    /// Open the database, ensuring the parent directory exists.
    async fn open(&self) -> Result<(turso::Database, turso::Connection), turso::Error> {
        if let Some(parent) = self.db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let db = Builder::new_local(self.db_path.to_str().unwrap_or_default())
            .build()
            .await?;
        let conn = db.connect()?;
        Ok((db, conn))
    }

    /// Initialize the database schema. Safe to call multiple times.
    pub async fn ensure_schema(&self) -> Result<(), turso::Error> {
        let (_db, conn) = self.open().await?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS tool_calls (\
                id INTEGER PRIMARY KEY,\
                timestamp DATETIME NOT NULL,\
                tool TEXT NOT NULL,\
                file_path TEXT,\
                duration_ms INTEGER NOT NULL,\
                success BOOLEAN NOT NULL,\
                error_msg TEXT\
            );\
            CREATE INDEX IF NOT EXISTS idx_tool_calls_timestamp ON tool_calls(timestamp);\
            CREATE INDEX IF NOT EXISTS idx_tool_calls_tool ON tool_calls(tool);\
            CREATE TABLE IF NOT EXISTS server_events (\
                id INTEGER PRIMARY KEY,\
                timestamp DATETIME NOT NULL,\
                language TEXT NOT NULL,\
                event TEXT NOT NULL,\
                duration_ms INTEGER,\
                detail TEXT\
            );\
            CREATE INDEX IF NOT EXISTS idx_server_events_timestamp ON server_events(timestamp);",
        )
        .await?;
        Ok(())
    }

    /// Record a tool call event.
    pub async fn record_tool_call(
        &self,
        tool: &str,
        file_path: Option<&str>,
        duration_ms: u64,
        success: bool,
        error_msg: Option<&str>,
    ) -> Result<(), turso::Error> {
        let (_db, conn) = self.open().await?;
        conn.execute(
            "INSERT INTO tool_calls (timestamp, tool, file_path, duration_ms, success, error_msg) \
             VALUES (datetime('now'), ?, ?, ?, ?, ?)",
            (tool, file_path, duration_ms as i64, success, error_msg),
        )
        .await?;
        Ok(())
    }

    /// Record a server lifecycle event.
    pub async fn record_server_event(
        &self,
        language: &str,
        event: &str,
        duration_ms: Option<u64>,
        detail: Option<&str>,
    ) -> Result<(), turso::Error> {
        let (_db, conn) = self.open().await?;
        conn.execute(
            "INSERT INTO server_events (timestamp, language, event, duration_ms, detail) \
             VALUES (datetime('now'), ?, ?, ?, ?)",
            (language, event, duration_ms.map(|d| d as i64), detail),
        )
        .await?;
        Ok(())
    }

    /// Delete rows older than 30 days from both tables.
    pub async fn retention_cleanup(&self) -> Result<(), turso::Error> {
        let (_db, conn) = self.open().await?;
        conn.execute(
            "DELETE FROM tool_calls WHERE timestamp < datetime('now', '-30 days')",
            (),
        )
        .await?;
        conn.execute(
            "DELETE FROM server_events WHERE timestamp < datetime('now', '-30 days')",
            (),
        )
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_recorder(dir: &std::path::Path) -> StatsRecorder {
        StatsRecorder::new(dir)
    }

    #[tokio::test]
    async fn schema_creates_tables() {
        let dir = tempfile::tempdir().unwrap();
        let recorder = test_recorder(dir.path());
        recorder.ensure_schema().await.unwrap();

        // Second call should succeed (IF NOT EXISTS)
        recorder.ensure_schema().await.unwrap();
    }

    #[tokio::test]
    async fn record_tool_call_inserts_row() {
        let dir = tempfile::tempdir().unwrap();
        let recorder = test_recorder(dir.path());
        recorder.ensure_schema().await.unwrap();

        recorder
            .record_tool_call("goto_definition", Some("src/main.rs"), 42, true, None)
            .await
            .unwrap();

        recorder
            .record_tool_call("find_references", None, 10, false, Some("not found"))
            .await
            .unwrap();

        // Verify rows via direct query
        let (_db, conn) = recorder.open().await.unwrap();
        let mut rows = conn
            .query("SELECT tool, file_path, success, error_msg FROM tool_calls ORDER BY id", ())
            .await
            .unwrap();

        let row = rows.next().await.unwrap().unwrap();
        assert_eq!(row.get::<String>(0).unwrap(), "goto_definition");
        assert_eq!(row.get::<String>(1).unwrap(), "src/main.rs");
        assert_eq!(row.get::<i64>(2).unwrap(), 1);
        assert!(row.get::<Option<String>>(3).unwrap().is_none());

        let row = rows.next().await.unwrap().unwrap();
        assert_eq!(row.get::<String>(0).unwrap(), "find_references");
        assert!(row.get::<Option<String>>(1).unwrap().is_none());
        assert_eq!(row.get::<i64>(2).unwrap(), 0);
        assert_eq!(row.get::<String>(3).unwrap(), "not found");
    }

    #[tokio::test]
    async fn record_server_event_inserts_row() {
        let dir = tempfile::tempdir().unwrap();
        let recorder = test_recorder(dir.path());
        recorder.ensure_schema().await.unwrap();

        recorder
            .record_server_event("rust", "started", Some(2300), None)
            .await
            .unwrap();

        let (_db, conn) = recorder.open().await.unwrap();
        let mut rows = conn
            .query(
                "SELECT language, event, duration_ms, detail FROM server_events",
                (),
            )
            .await
            .unwrap();

        let row = rows.next().await.unwrap().unwrap();
        assert_eq!(row.get::<String>(0).unwrap(), "rust");
        assert_eq!(row.get::<String>(1).unwrap(), "started");
        assert_eq!(row.get::<i64>(2).unwrap(), 2300);
        assert!(row.get::<Option<String>>(3).unwrap().is_none());
    }

    #[tokio::test]
    async fn retention_cleanup_removes_old_rows() {
        let dir = tempfile::tempdir().unwrap();
        let recorder = test_recorder(dir.path());
        recorder.ensure_schema().await.unwrap();

        // Insert a row with an explicit old timestamp
        let (_db, conn) = recorder.open().await.unwrap();
        conn.execute(
            "INSERT INTO tool_calls (timestamp, tool, file_path, duration_ms, success, error_msg) \
             VALUES (datetime('now', '-31 days'), 'old_tool', 'old.rs', 1, 1, NULL)",
            (),
        )
        .await
        .unwrap();

        // Insert a current row
        conn.execute(
            "INSERT INTO tool_calls (timestamp, tool, file_path, duration_ms, success, error_msg) \
             VALUES (datetime('now'), 'new_tool', 'new.rs', 1, 1, NULL)",
            (),
        )
        .await
        .unwrap();

        // Run retention
        drop(conn);
        recorder.retention_cleanup().await.unwrap();

        // Verify only the new row remains
        let (_db, conn) = recorder.open().await.unwrap();
        let mut rows = conn
            .query("SELECT tool FROM tool_calls ORDER BY id", ())
            .await
            .unwrap();

        let row = rows.next().await.unwrap().unwrap();
        assert_eq!(row.get::<String>(0).unwrap(), "new_tool");
        assert!(rows.next().await.unwrap().is_none());
    }
}
