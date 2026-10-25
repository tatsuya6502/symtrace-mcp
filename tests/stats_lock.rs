//! Two-process check that a locked stats database does not stop the server.
//!
//! Ignored by default: the lock is turso's per-process, platform-dependent
//! file lock, and the test depends on the timing of two child processes.
//! Run with `cargo test --test stats_lock -- --ignored`.

use std::io::{BufRead, Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_symtrace-mcp");
const INITIALIZE: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"stats_lock","version":"0"}}}"#;

fn spawn_server(cwd: &Path, log_dir: &Path) -> Child {
    Command::new(BIN)
        .current_dir(cwd)
        .env("SYMTRACE_LOG_DIR", log_dir)
        .env_remove("SYMTRACE_LOG")
        // A developer shell may export this for the `stats` CLI; with it set,
        // process B would ignore A's lock and the test would prove nothing.
        .env_remove("LIMBO_DISABLE_FILE_LOCK")
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
    // stdin stays open. Its `initialize` response is the readiness signal:
    // the server answers requests only after `StatsRecorder::new` has
    // returned, so by then A either holds the lock or has given up on stats.
    let mut a = KillOnDrop(spawn_server(cwd, &log_a));
    {
        let stdin = a.0.stdin.as_mut().unwrap();
        writeln!(stdin, "{INITIALIZE}").unwrap();
    }
    // Read A's stdout on its own thread: the first line is the readiness
    // signal, and the thread then keeps draining so A's pipe stays open
    // (and never fills) for as long as A lives.
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let a_stdout = a.0.stdout.take().unwrap();
    std::thread::spawn(move || {
        let mut reader = std::io::BufReader::new(a_stdout);
        let mut first_line = String::new();
        let _ = reader.read_line(&mut first_line);
        let _ = ready_tx.send(first_line);
        let _ = std::io::copy(&mut reader, &mut std::io::sink());
    });
    let first_line = ready_rx
        .recv_timeout(Duration::from_secs(15))
        .expect("process A did not answer initialize within 15 s");
    assert!(
        first_line.contains("\"serverInfo\""),
        "unexpected first line from process A: {first_line}"
    );
    assert!(
        cwd.join(".symtrace").join("stats.db").exists(),
        "process A did not create the stats database"
    );

    // Process B: same directory, so its stats open hits A's lock. Wrapped in
    // KillOnDrop so the timeout path below does not leak it.
    let mut b = KillOnDrop(spawn_server(cwd, &log_b));
    {
        let mut stdin = b.0.stdin.take().unwrap();
        let requests = [
            INITIALIZE,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
        ];
        for req in requests {
            writeln!(stdin, "{req}").unwrap();
        }
        // Dropping stdin closes it, so B exits after answering.
    }

    // Drain stdout and stderr on their own threads so B never blocks on a
    // full pipe while the main thread polls for its exit.
    let mut stdout = b.0.stdout.take().unwrap();
    let mut stderr = b.0.stderr.take().unwrap();
    let stdout_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        buf
    });
    let stderr_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stderr.read_to_end(&mut buf);
        buf
    });

    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = b.0.try_wait().expect("failed to wait for process B") {
            break status;
        }
        // Panicking here drops `b`, which kills process B.
        assert!(
            Instant::now() < deadline,
            "process B did not exit within 30 s"
        );
        std::thread::sleep(Duration::from_millis(100));
    };
    let output = std::process::Output {
        status,
        stdout: stdout_reader.join().unwrap(),
        stderr: stderr_reader.join().unwrap(),
    };

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
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| std::fs::read_to_string(e.path()).unwrap())
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
