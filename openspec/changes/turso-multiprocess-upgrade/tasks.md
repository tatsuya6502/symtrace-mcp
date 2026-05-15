## 1. Dependency Upgrade

- [ ] 1.1 Bump `turso` from 0.5.3 to 0.6.0 in `Cargo.toml`
- [ ] 1.2 Run `cargo check` to verify backward compatibility

## 2. Refactor StatsRecorder

- [ ] 2.1 Replace `db_path`-only struct with `Database`-held struct: `StatsRecorder { db: Database, db_path: PathBuf }`
- [ ] 2.2 Make `new()` async — build `Database` with `experimental_multiprocess_wal(true)`, run `ensure_schema`, return `Result<StatsRecorder>`
- [ ] 2.3 Remove `open()` method — replace all internal usage with `self.db.connect()`
- [ ] 2.4 Update `record_tool_call`, `record_server_event`, `retention_cleanup` to use `self.db.connect()` pattern
- [ ] 2.5 Update query methods (`query_tool_usage`, `query_top_files`, `query_server_usage`) to use `self.db.connect()`

## 3. Update Call Sites (drop Mutex)

- [ ] 3.1 `src/main.rs` — change `Arc::new(Mutex::new(StatsRecorder::new(cwd)))` to `Arc::new(StatsRecorder::new(cwd).await?)`, remove `stats.lock().await.ensure_schema()` call
- [ ] 3.2 `src/mcp/tools.rs` — change `stats: Arc<Mutex<StatsRecorder>>` to `Arc<StatsRecorder>`, remove `.lock().await` from `handle_tools_call`
- [ ] 3.3 `src/server/manager.rs` — change `stats: Arc<Mutex<StatsRecorder>>` to `Arc<StatsRecorder>`, remove `.lock().await` from all stats calls
- [ ] 3.4 `src/server/idle_monitor.rs` — change `stats: Arc<Mutex<StatsRecorder>>` to `Arc<StatsRecorder>`, remove `.lock().await`
- [ ] 3.5 `src/project/registry.rs` — change `stats: Arc<Mutex<StatsRecorder>>` to `Arc<StatsRecorder>`, update constructor and tests

## 4. Update Tests

- [ ] 4.1 Rewrite `concurrent_tool_calls_serialized` test — verify concurrent writes produce correct data without Mutex
- [ ] 4.2 Update all other recorder tests to use new `StatsRecorder::new()` async API
- [ ] 4.3 Run `cargo test` to verify all tests pass

## 5. Cleanup

- [ ] 5.1 Remove unused `Mutex` imports from all modified files
- [ ] 5.2 Run `cargo clippy` and address warnings
