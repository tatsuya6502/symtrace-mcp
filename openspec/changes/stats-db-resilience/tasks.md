## 1. StatsRecorder Lazy DB Core

- [ ] 1.1 Change `StatsRecorder` struct: replace `db: turso::Database` with `db: Arc<Mutex<Option<turso::Database>>>`, `project_root: PathBuf`, `last_access: Arc<AtomicU64>`
- [ ] 1.2 Update `StatsRecorder::new()` to store only the DB path, without opening the database
- [ ] 1.3 Add private `ensure_open()` method that opens the DB if `None`, runs schema migration, and runs initial retention cleanup
- [ ] 1.4 Update `record_tool_call()` to call `ensure_open()`, acquire mutex briefly for connection, then release mutex before async INSERT
- [ ] 1.5 Update `record_server_event()` similarly to `record_tool_call()`
- [ ] 1.6 Update `retention_cleanup()` to skip if DB is `None`; otherwise acquire mutex, get connection, release mutex, then execute cleanup

## 2. Debounce Close Timer

- [ ] 2.1 Add `close_handle: Arc<Mutex<Option<JoinHandle<()>>>>` field to `StatsRecorder`
- [ ] 2.2 Add `schedule_close()` method that cancels any pending close task and spawns a new one: sleep 15s → lock mutex → check `last_access` → if ≥15s old, set DB to `None`
- [ ] 2.3 Call `schedule_close()` at the end of `record_tool_call()` and `record_server_event()`
- [ ] 2.4 Add `shutdown()` method that cancels the close timer (for graceful MCP server shutdown)

## 3. Non-Fatal Startup

- [ ] 3.1 Change `main.rs`: wrap stats in `Arc<Option<StatsRecorder>>`, log warning on failure instead of `process::exit(1)`
- [ ] 3.2 Change `McpServer` struct: `stats: Arc<StatsRecorder>` → `stats: Arc<Option<StatsRecorder>>`
- [ ] 3.3 Update `handle_tools_call()`: guard stats write with `if let Some(ref stats) = *self.stats`
- [ ] 3.4 Update `run()`: guard periodic retention cleanup with stats `Some` check; cancel close handle on shutdown

## 4. Update Existing Tests

- [ ] 4.1 Update `StatsRecorder` unit tests to work with new lazy-open API (construction no longer opens DB)
- [ ] 4.2 Add test: lazy open — DB is `None` after construction, opens on first `record_tool_call`
- [ ] 4.3 Add test: debounce — close timer fires after 15s of inactivity
- [ ] 4.4 Add test: debounce reset — consecutive writes reset the close timer
- [ ] 4.5 Add test: non-fatal startup — MCP server starts when DB initialization fails

## 5. Lint and Verify

- [ ] 5.1 Run `cargo clippy --all-targets --tests -- -D warnings`
- [ ] 5.2 Run `cargo fmt --all -- --check`
- [ ] 5.3 Run `cargo test`
