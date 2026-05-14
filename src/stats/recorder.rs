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

    /// Check if the database file exists.
    pub fn db_exists(&self) -> bool {
        self.db_path.exists()
    }

    pub async fn query_tool_usage(&self) -> Result<Vec<ToolUsage>, turso::Error> {
        let (_db, conn) = self.open().await?;
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
        let (_db, conn) = self.open().await?;
        let mut rows = conn
            .query(
                "SELECT file_path, COUNT(*) as calls \
                 FROM tool_calls \
                 WHERE timestamp >= datetime('now', '-7 days') AND file_path IS NOT NULL \
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
        let (_db, conn) = self.open().await?;
        let mut rows = conn
            .query(
                "SELECT language, event, duration_ms, timestamp \
                 FROM server_events \
                 WHERE timestamp >= datetime('now', '-7 days') \
                 ORDER BY language, id",
                (),
            )
            .await?;

        let mut events_by_lang: std::collections::BTreeMap<
            String,
            Vec<(String, Option<i64>, String)>,
        > = std::collections::BTreeMap::new();
        while let Some(row) = rows.next().await? {
            let lang = row.get::<String>(0)?;
            let event = row.get::<String>(1)?;
            let duration_ms = row.get::<Option<i64>>(2)?;
            let timestamp = row.get::<String>(3)?;
            events_by_lang
                .entry(lang)
                .or_default()
                .push((event, duration_ms, timestamp));
        }

        let mut now_rows = conn.query("SELECT datetime('now')", ()).await?;
        let now_secs = if let Some(row) = now_rows.next().await? {
            ts_to_secs(&row.get::<String>(0)?).unwrap_or(0)
        } else {
            0
        };

        let mut result = Vec::new();
        for (language, events) in events_by_lang {
            let startups = events.iter().filter(|(e, _, _)| e == "started").count() as i64;
            let startup_durations: Vec<i64> = events
                .iter()
                .filter(|(e, _, _)| e == "started")
                .filter_map(|(_, d, _)| *d)
                .collect();
            let avg_startup_ms = if startup_durations.is_empty() {
                0
            } else {
                startup_durations.iter().sum::<i64>() / startup_durations.len() as i64
            };

            let mut total_uptime_secs: i64 = 0;
            let mut last_start_ts: Option<i64> = None;
            for (event, _, ts_str) in &events {
                match event.as_str() {
                    "started" => last_start_ts = ts_to_secs(ts_str),
                    "stopped" => {
                        if let (Some(start_ts), Some(stop_ts)) =
                            (last_start_ts.take(), ts_to_secs(ts_str))
                        {
                            total_uptime_secs += stop_ts - start_ts;
                        }
                    }
                    _ => {}
                }
            }
            if let Some(start_ts) = last_start_ts {
                total_uptime_secs += now_secs - start_ts;
            }

            result.push(ServerUsage {
                language,
                startups,
                avg_startup_ms,
                total_uptime_secs,
            });
        }
        Ok(result)
    }
}

/// Parse SQLite `datetime('now')` format (`YYYY-MM-DD HH:MM:SS`) to Unix seconds.
fn ts_to_secs(ts: &str) -> Option<i64> {
    let y: i64 = ts.get(0..4)?.parse().ok()?;
    let m: i64 = ts.get(5..7)?.parse().ok()?;
    let d: i64 = ts.get(8..10)?.parse().ok()?;
    let hh: i64 = ts.get(11..13)?.parse().ok()?;
    let mm: i64 = ts.get(14..16)?.parse().ok()?;
    let ss: i64 = ts.get(17..19)?.parse().ok()?;

    // Gregorian date to Julian Day Number
    let a = (14 - m) / 12;
    let y2 = y + 4800 - a;
    let m2 = m + 12 * a - 3;
    let jdn = d + (153 * m2 + 2) / 5 + 365 * y2 + y2 / 4 - y2 / 100 + y2 / 400 - 32045;

    let unix_days = jdn - 2440588;
    Some(unix_days * 86400 + hh * 3600 + mm * 60 + ss)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tokio::sync::Mutex;

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

    #[tokio::test]
    async fn end_to_end_tool_call_stats() {
        let dir = tempfile::tempdir().unwrap();
        let recorder = test_recorder(dir.path());
        recorder.ensure_schema().await.unwrap();

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

        let tool_usage = recorder.query_tool_usage().await.unwrap();
        assert_eq!(tool_usage.len(), 2);
        assert_eq!(tool_usage[0].tool, "goto_definition");
        assert_eq!(tool_usage[0].calls, 3);
        assert_eq!(tool_usage[0].errors, 0);
        assert_eq!(tool_usage[1].tool, "find_references");
        assert_eq!(tool_usage[1].calls, 1);
        assert_eq!(tool_usage[1].errors, 1);

        let top_files = recorder.query_top_files().await.unwrap();
        assert_eq!(top_files.len(), 2);
        assert_eq!(top_files[0].file_path, "src/main.rs");
        assert_eq!(top_files[0].calls, 3);
        assert_eq!(top_files[1].file_path, "src/lib.rs");
        assert_eq!(top_files[1].calls, 1);
    }

    #[tokio::test]
    async fn concurrent_tool_calls_serialized() {
        let dir = tempfile::tempdir().unwrap();
        let recorder = Arc::new(Mutex::new(test_recorder(dir.path())));
        recorder.lock().await.ensure_schema().await.unwrap();

        let mut handles = Vec::new();
        for i in 0..10 {
            let r = recorder.clone();
            handles.push(tokio::spawn(async move {
                r.lock()
                    .await
                    .record_tool_call("test_tool", Some("file.rs"), i, true, None)
                    .await
                    .unwrap();
            }));
        }

        for handle in handles {
            handle.await.unwrap();
        }

        let tool_usage = recorder.lock().await.query_tool_usage().await.unwrap();
        assert_eq!(tool_usage.len(), 1);
        assert_eq!(tool_usage[0].tool, "test_tool");
        assert_eq!(tool_usage[0].calls, 10);
    }

    #[tokio::test]
    async fn retention_cleanup_integration() {
        let dir = tempfile::tempdir().unwrap();
        let recorder = test_recorder(dir.path());
        recorder.ensure_schema().await.unwrap();

        let (_db, conn) = recorder.open().await.unwrap();
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

        // Old tool call should be gone, new one remains
        let tool_usage = recorder.query_tool_usage().await.unwrap();
        assert_eq!(tool_usage.len(), 1);
        assert_eq!(tool_usage[0].tool, "new_tool");

        // Old server event should be gone
        let server_usage = recorder.query_server_usage().await.unwrap();
        assert!(server_usage.is_empty());
    }
}
