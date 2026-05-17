use std::path::Path;
use turso::Builder;

/// Records usage statistics to a per-project SQLite database.
///
/// Holds a shared `Database` handle built with multiprocess WAL support.
/// Each operation creates a fresh `Connection` via `db.connect()`.
pub struct StatsRecorder {
    db: turso::Database,
}

/// Read-only handle to the stats database, exposed to the CLI.
///
/// Wraps the same `turso::Database` but only provides query methods,
/// so the type system prevents accidental writes from the stats command.
pub struct ReadonlyStatsRecorder {
    db: turso::Database,
}

pub struct ToolUsage {
    pub tool: String,
    pub calls: i64,
    pub avg_ms: i64,
    pub errors: i64,
}

pub struct FileUsage {
    pub file_path: String,
    pub calls: i64,
}

pub struct ServerUsage {
    pub language: String,
    pub startups: i64,
    pub avg_startup_ms: i64,
    pub total_uptime_secs: i64,
}

impl ReadonlyStatsRecorder {
    /// Open the stats database for reading alongside a running MCP server.
    ///
    /// Callers must set `LIMBO_DISABLE_FILE_LOCK=1` before starting the
    /// async runtime so this read-only process does not conflict with the
    /// server's WAL locks.
    pub async fn open(project_root: &Path) -> Result<Self, turso::Error> {
        let db_path = project_root.join(".symtrace").join("stats.db");
        if !db_path.exists() {
            return Err(turso::Error::Error("stats database not found".into()));
        }

        let db = Builder::new_local(db_path.to_str().ok_or_else(|| {
            turso::Error::Error("database path contains invalid UTF-8".to_string())
        })?)
        .experimental_multiprocess_wal(true)
        .build()
        .await?;

        Ok(Self { db })
    }

    pub async fn query_tool_usage(&self) -> Result<Vec<ToolUsage>, turso::Error> {
        let conn = self.db.connect()?;
        let mut rows = conn
            .query(
                "SELECT tool, COUNT(*) as calls, \
                 CAST(ROUND(AVG(duration_ms)) AS INTEGER) as avg_ms, \
                 SUM(CASE WHEN success = 0 THEN 1 ELSE 0 END) as errors \
                 FROM tool_calls \
                 WHERE timestamp >= datetime('now', '-7 days') \
                 GROUP BY tool ORDER BY calls DESC",
                (),
            )
            .await?;

        let mut result = Vec::new();
        while let Some(row) = rows.next().await? {
            result.push(ToolUsage {
                tool: row.get::<String>(0)?,
                calls: row.get::<i64>(1)?,
                avg_ms: row.get::<i64>(2)?,
                errors: row.get::<i64>(3)?,
            });
        }
        Ok(result)
    }

    pub async fn query_top_files(&self) -> Result<Vec<FileUsage>, turso::Error> {
        let conn = self.db.connect()?;
        let mut rows = conn
            .query(
                "SELECT file_path, COUNT(*) as calls \
                 FROM tool_calls \
                 WHERE timestamp >= datetime('now', '-7 days') \
                 AND file_path IS NOT NULL \
                 GROUP BY file_path ORDER BY calls DESC LIMIT 10",
                (),
            )
            .await?;

        let mut result = Vec::new();
        while let Some(row) = rows.next().await? {
            result.push(FileUsage {
                file_path: row.get::<String>(0)?,
                calls: row.get::<i64>(1)?,
            });
        }
        Ok(result)
    }

    pub async fn query_server_usage(&self) -> Result<Vec<ServerUsage>, turso::Error> {
        let conn = self.db.connect()?;
        let mut rows = conn
            .query(
                "SELECT language, \
                 SUM(CASE WHEN event='started' THEN 1 ELSE 0 END) as startups, \
                 COALESCE(CAST(ROUND(AVG(CASE WHEN event='started' AND duration_ms IS NOT NULL THEN duration_ms ELSE NULL END)) AS INTEGER), 0) as avg_startup_ms, \
                 CAST(COALESCE(SUM(CASE WHEN event='stopped' AND duration_ms IS NOT NULL THEN duration_ms ELSE 0 END), 0) / 1000.0 AS INTEGER) as total_uptime_secs \
                 FROM server_events \
                 WHERE timestamp >= datetime('now', '-7 days') \
                 GROUP BY language ORDER BY startups DESC",
                (),
            )
            .await?;

        let mut result = Vec::new();
        while let Some(row) = rows.next().await? {
            result.push(ServerUsage {
                language: row.get::<String>(0)?,
                startups: row.get::<i64>(1)?,
                avg_startup_ms: row.get::<i64>(2)?,
                total_uptime_secs: row.get::<i64>(3)?,
            });
        }
        Ok(result)
    }
}

impl StatsRecorder {
    pub async fn new(project_root: &Path) -> Result<Self, turso::Error> {
        let db_path = project_root.join(".symtrace").join("stats.db");
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                turso::Error::Error(format!("failed to create stats directory: {e}"))
            })?;
        }
        let db = Builder::new_local(db_path.to_str().ok_or_else(|| {
            turso::Error::Error("database path contains invalid UTF-8".to_string())
        })?)
        .experimental_multiprocess_wal(true)
        .build()
        .await?;

        let recorder = Self { db };
        recorder.ensure_schema().await?;
        Ok(recorder)
    }

    async fn ensure_schema(&self) -> Result<(), turso::Error> {
        let conn = self.db.connect()?;
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

    pub async fn record_tool_call(
        &self,
        tool: &str,
        file_path: Option<&str>,
        duration_ms: u64,
        success: bool,
        error_msg: Option<&str>,
    ) -> Result<(), turso::Error> {
        let conn = self.db.connect()?;
        conn.execute(
            "INSERT INTO tool_calls (timestamp, tool, file_path, duration_ms, success, error_msg) \
             VALUES (datetime('now'), ?, ?, ?, ?, ?)",
            (tool, file_path, duration_ms as i64, success, error_msg),
        )
        .await?;
        Ok(())
    }

    pub async fn record_server_event(
        &self,
        language: &str,
        event: &str,
        duration_ms: Option<u64>,
        detail: Option<&str>,
    ) -> Result<(), turso::Error> {
        let conn = self.db.connect()?;
        conn.execute(
            "INSERT INTO server_events (timestamp, language, event, duration_ms, detail) \
             VALUES (datetime('now'), ?, ?, ?, ?)",
            (language, event, duration_ms.map(|d| d as i64), detail),
        )
        .await?;
        Ok(())
    }

    pub async fn retention_cleanup(&self) -> Result<(), turso::Error> {
        let conn = self.db.connect()?;
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
    use std::sync::Arc;

    use super::*;

    async fn test_recorder(dir: &std::path::Path) -> StatsRecorder {
        StatsRecorder::new(dir).await.unwrap()
    }

    #[tokio::test]
    async fn schema_creates_tables() {
        let dir = tempfile::tempdir().unwrap();
        let _recorder = test_recorder(dir.path()).await;

        // Second instance should succeed (IF NOT EXISTS)
        let _recorder2 = test_recorder(dir.path()).await;
    }

    #[tokio::test]
    async fn record_tool_call_inserts_row() {
        let dir = tempfile::tempdir().unwrap();
        let recorder = test_recorder(dir.path()).await;

        recorder
            .record_tool_call("goto_definition", Some("src/main.rs"), 42, true, None)
            .await
            .unwrap();

        recorder
            .record_tool_call("find_references", None, 10, false, Some("not found"))
            .await
            .unwrap();

        let conn = recorder.db.connect().unwrap();
        let mut rows = conn
            .query(
                "SELECT tool, file_path, success, error_msg FROM tool_calls ORDER BY id",
                (),
            )
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
        let recorder = test_recorder(dir.path()).await;

        recorder
            .record_server_event("rust", "started", Some(2300), None)
            .await
            .unwrap();

        let conn = recorder.db.connect().unwrap();
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
        let recorder = test_recorder(dir.path()).await;

        let conn = recorder.db.connect().unwrap();
        conn.execute(
            "INSERT INTO tool_calls (timestamp, tool, file_path, duration_ms, success, error_msg) \
             VALUES (datetime('now', '-31 days'), 'old_tool', 'old.rs', 1, 1, NULL)",
            (),
        )
        .await
        .unwrap();

        conn.execute(
            "INSERT INTO tool_calls (timestamp, tool, file_path, duration_ms, success, error_msg) \
             VALUES (datetime('now'), 'new_tool', 'new.rs', 1, 1, NULL)",
            (),
        )
        .await
        .unwrap();

        drop(conn);
        recorder.retention_cleanup().await.unwrap();

        let conn = recorder.db.connect().unwrap();
        let mut rows = conn
            .query("SELECT tool FROM tool_calls ORDER BY id", ())
            .await
            .unwrap();

        let row = rows.next().await.unwrap().unwrap();
        assert_eq!(row.get::<String>(0).unwrap(), "new_tool");
        assert!(rows.next().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn end_to_end_tool_call_stats() {
        let dir = tempfile::tempdir().unwrap();
        let recorder = test_recorder(dir.path()).await;

        recorder
            .record_tool_call("goto_definition", Some("src/main.rs"), 50, true, None)
            .await
            .unwrap();
        recorder
            .record_tool_call("goto_definition", Some("src/lib.rs"), 30, true, None)
            .await
            .unwrap();
        recorder
            .record_tool_call("goto_definition", Some("src/main.rs"), 70, true, None)
            .await
            .unwrap();
        recorder
            .record_tool_call(
                "find_references",
                Some("src/main.rs"),
                20,
                false,
                Some("not found"),
            )
            .await
            .unwrap();

        let reader = ReadonlyStatsRecorder::open(dir.path()).await.unwrap();
        let tool_usage = reader.query_tool_usage().await.unwrap();
        assert_eq!(tool_usage.len(), 2);
        assert_eq!(tool_usage[0].tool, "goto_definition");
        assert_eq!(tool_usage[0].calls, 3);
        assert_eq!(tool_usage[0].errors, 0);
        assert_eq!(tool_usage[1].tool, "find_references");
        assert_eq!(tool_usage[1].calls, 1);
        assert_eq!(tool_usage[1].errors, 1);

        let top_files = reader.query_top_files().await.unwrap();
        assert_eq!(top_files.len(), 2);
        assert_eq!(top_files[0].file_path, "src/main.rs");
        assert_eq!(top_files[0].calls, 3);
        assert_eq!(top_files[1].file_path, "src/lib.rs");
        assert_eq!(top_files[1].calls, 1);
    }

    #[tokio::test]
    async fn concurrent_tool_calls_persist_all_rows() {
        let dir = tempfile::tempdir().unwrap();
        let recorder = Arc::new(test_recorder(dir.path()).await);

        let mut handles = Vec::new();
        for i in 0..10 {
            let r = recorder.clone();
            handles.push(tokio::spawn(async move {
                r.record_tool_call("test_tool", Some("file.rs"), i, true, None)
                    .await
                    .unwrap();
            }));
        }

        for handle in handles {
            handle.await.unwrap();
        }

        let reader = ReadonlyStatsRecorder::open(dir.path()).await.unwrap();
        let tool_usage = reader.query_tool_usage().await.unwrap();
        assert_eq!(tool_usage.len(), 1);
        assert_eq!(tool_usage[0].tool, "test_tool");
        assert_eq!(tool_usage[0].calls, 10);
    }

    #[tokio::test]
    async fn retention_cleanup_integration() {
        let dir = tempfile::tempdir().unwrap();
        let recorder = test_recorder(dir.path()).await;

        let conn = recorder.db.connect().unwrap();
        conn.execute(
            "INSERT INTO tool_calls (timestamp, tool, file_path, duration_ms, success, error_msg) \
             VALUES (datetime('now', '-31 days'), 'old_tool', 'old.rs', 1, 1, NULL)",
            (),
        )
        .await
        .unwrap();
        conn.execute(
            "INSERT INTO server_events (timestamp, language, event, duration_ms, detail) \
             VALUES (datetime('now', '-31 days'), 'rust', 'started', 1000, NULL)",
            (),
        )
        .await
        .unwrap();
        conn.execute(
            "INSERT INTO tool_calls (timestamp, tool, file_path, duration_ms, success, error_msg) \
             VALUES (datetime('now'), 'new_tool', 'new.rs', 1, 1, NULL)",
            (),
        )
        .await
        .unwrap();
        drop(conn);

        recorder.retention_cleanup().await.unwrap();

        let reader = ReadonlyStatsRecorder::open(dir.path()).await.unwrap();
        let tool_usage = reader.query_tool_usage().await.unwrap();
        assert_eq!(tool_usage.len(), 1);
        assert_eq!(tool_usage[0].tool, "new_tool");

        let server_usage = reader.query_server_usage().await.unwrap();
        assert!(server_usage.is_empty());
    }
}
