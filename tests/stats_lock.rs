//! Two-process check that a locked stats database does not stop the server.
//!
//! Ignored by default: the lock is turso's per-process, platform-dependent
//! file lock, and the test depends on the timing of two child processes.
//! Run with `cargo test --test stats_lock -- --ignored`.

use std::io::Write;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_symtrace-mcp");

fn spawn_server(cwd: &Path, log_dir: &Path) -> Child {
    Command::new(BIN)
        .current_dir(cwd)
        .env("SYMTRACE_LOG_DIR", log_dir)
        .env_remove("SYMTRACE_LOG")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn symtrace-mcp")
}

/// Kills the child on drop so a failed assertion does not leak process A.
struct KillOnDrop(Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
#[ignore = "two-process test of turso's file lock; run with --ignored"]
fn second_server_starts_when_stats_db_is_locked() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path();
    let log_a = cwd.join("logs-a");
    let log_b = cwd.join("logs-b");

    // Process A opens the stats database and keeps it locked while its
    // stdin stays open.
    let _a = KillOnDrop(spawn_server(cwd, &log_a));
    let wal = cwd.join(".symtrace").join("stats.db-wal");
    let deadline = Instant::now() + Duration::from_secs(15);
    while !wal.exists() {
        assert!(
            Instant::now() < deadline,
            "process A did not create {}",
            wal.display()
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    // Give A a moment to finish schema setup after creating the WAL file.
    std::thread::sleep(Duration::from_millis(500));

    // Process B: same directory, so its stats open hits A's lock.
    let mut b = spawn_server(cwd, &log_b);
    {
        let mut stdin = b.stdin.take().unwrap();
        let requests = [
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"stats_lock","version":"0"}}}"#,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
        ];
        for req in requests {
            writeln!(stdin, "{req}").unwrap();
        }
        // Dropping stdin closes it, so B exits after answering.
    }

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(b.wait_with_output());
    });
    let output = rx
        .recv_timeout(Duration::from_secs(30))
        .expect("process B did not exit within 30 s")
        .expect("failed to wait for process B");

    assert!(
        output.status.success(),
        "process B exited with {:?}; stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let responses: Vec<serde_json::Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).expect("non-JSON line on stdout"))
        .collect();
    let by_id = |id: i64| {
        responses
            .iter()
            .find(|r| r["id"] == id)
            .unwrap_or_else(|| panic!("no response for id {id}: {responses:?}"))
    };
    assert_eq!(by_id(1)["result"]["serverInfo"]["name"], "symtrace-mcp");
    assert!(
        by_id(2)["result"]["tools"]
            .as_array()
            .is_some_and(|t| !t.is_empty()),
        "tools/list returned no tools"
    );

    // B's log has exactly one stats warning, carrying the lock error.
    let logs: String = std::fs::read_dir(&log_b)
        .expect("process B wrote no log directory")
        .map(|e| std::fs::read_to_string(e.unwrap().path()).unwrap())
        .collect();
    let warns: Vec<&str> = logs
        .lines()
        .filter(|l| l.contains("failed to open stats database"))
        .collect();
    assert_eq!(warns.len(), 1, "expected one stats warning, log:\n{logs}");
    assert!(warns[0].contains("\"level\":\"WARN\""), "{}", warns[0]);
    assert!(warns[0].contains("stats.db"), "{}", warns[0]);
    assert!(
        warns[0].to_lowercase().contains("lock"),
        "warning does not carry the lock error: {}",
        warns[0]
    );
}
