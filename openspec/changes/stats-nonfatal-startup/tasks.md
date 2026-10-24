# Tasks

## 1. StatsRecorder disabled state

- [ ] 1.1 In `src/stats/recorder.rs`, change `StatsRecorder` to hold `db: Option<turso::Database>`. Move today's create-dir / build / `ensure_schema` steps into a private `async fn open(..) -> Result<turso::Database, turso::Error>`. Make `pub async fn new(project_root: &Path) -> Self` infallible: on `Err` it logs one `tracing::warn!` with `error` and `path` fields and returns a disabled recorder (design D2). Verify with `cargo build`.
- [ ] 1.2 Make `record_tool_call`, `record_server_event` and `retention_cleanup` return `Ok(())` without touching the database when disabled, and keep the enabled path byte-for-byte equivalent (design D1). Update the doc comment on `StatsRecorder` to describe the disabled state. Verify with `cargo build`. The unit-test target does not compile again until 1.3 and 1.4 are done, because all `src/` tests share one test binary.
- [ ] 1.3 Add `#[cfg(test)]` helpers: an accessor returning `&turso::Database` that panics when disabled (named `db_for_test()` to match PR #35) and `is_enabled()`. Switch the existing recorder tests from `recorder.db.connect()` to the accessor, without changing any assertion. Make `test_recorder` drop `.unwrap()` and assert `is_enabled()`. Verify together with 1.4.
- [ ] 1.4 Drop `.unwrap()` from the `StatsRecorder::new(..)` calls in the `test_stats` helpers of `src/server/manager.rs` and `src/project/registry.rs`. Change test code only; leave production signatures alone. Verify 1.3 and 1.4 together: `cargo test` compiles and every existing test passes with unchanged assertions.
- [ ] 1.5 Add a unit test: create a temp dir with a regular file named `.symtrace`, call `StatsRecorder::new` on it, and assert that the recorder is disabled, that `record_tool_call`, `record_server_event` and `retention_cleanup` all return `Ok(())`, and that the `.symtrace` file is unchanged afterwards (no directory or DB created). Verify with `cargo test stats::`.

## 2. Server startup

- [ ] 2.1 In `src/main.rs` `run_server`, replace the `match StatsRecorder::new(cwd).await { .. process::exit(1) }` block with `let stats = Arc::new(StatsRecorder::new(cwd).await);`. Verify that `cargo build` passes and that `grep -n "failed to initialize stats database" src/main.rs` finds nothing.

## 3. Process-level verification

- [ ] 3.1 Manual check: in a scratch project directory, start `symtrace-mcp` (process A, stdin left open), then start a second `symtrace-mcp` (process B) in the same directory and send it an `initialize` request followed by `tools/list`. Verify that B answers both, does not exit, and that B's log file under `.symtrace/logs/` contains one `warn` line with the lock error and the `stats.db` path. Then send B a `tools/call` and verify that no `stats recording failed` warning appears.
- [ ] 3.2 (Optional) Automate 3.1 as `tests/stats_lock.rs`. Spawn `env!("CARGO_BIN_EXE_symtrace-mcp")` twice with a temp-dir cwd and `SYMTRACE_LOG_DIR` pointing into it, and assert that the second process answers `initialize`. Mark it `#[ignore]`: the lock is turso's per-process, platform-dependent file lock (not something this crate controls), and the test depends on the timing of two child processes, so it is not a reliable default CI test. Run it with `cargo test --test stats_lock -- --ignored`.

## 4. Changelog and checks

- [ ] 4.1 Add an entry under `## [Unreleased]` / `### Fixed` in `CHANGELOG.md`: the MCP server no longer exits at startup when the stats database cannot be opened (for example, locked by another symtrace-mcp process in the same directory). It logs a warning and runs with usage statistics disabled for that session. Verify that the entry is present under the existing `### Fixed` heading.
- [ ] 4.2 Run `cargo clippy --all-targets --tests -- -D warnings`, `cargo fmt --all -- --check` and `cargo test`, and verify all three pass.
