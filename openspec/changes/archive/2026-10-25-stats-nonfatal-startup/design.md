# Design

## Context

See proposal.md (Why) for the failure. Current state relevant to the fix:

- `StatsRecorder::new(project_root)` (`src/stats/recorder.rs`) runs `create_dir_all(.symtrace)`, builds a turso `Database` with `experimental_multiprocess_wal(true)`, and runs `ensure_schema`. Any of the three can fail. In the reproduced case the turso build fails with a lock error on `stats.db-wal`.
- `run_server` (`src/main.rs`) turns that `Err` into `tracing::error!` + `std::process::exit(1)`.
- The recorder is shared as `Arc<StatsRecorder>`: `main.rs` → `ProjectRegistry::new(&config, cwd, stats.clone())` → each `LanguageServerManager` (and its `IdleMonitor`) → `McpServer::new(registry, stats)`.
- Every write site already treats an `Err` as non-fatal and logs `warn!`: `retention_cleanup` at startup and every 24 h (`src/mcp/tools.rs`), `record_tool_call` after each tool call (`src/mcp/tools.rs`), and `record_server_event` for started / startup_failed / stopped (`src/server/manager.rs`, `src/server/idle_monitor.rs`).
- Logging (`src/logging.rs`) installs a single JSON layer that writes to `.symtrace/logs/symtrace-mcp.<ts>.<pid>.log`. There is no stderr layer. `eprintln!` is used only before the subscriber exists (log directory or file creation failed, invalid level). The `structured-logging` spec forbids `eprintln!` in server code.
- The turso lock is per process. Two `StatsRecorder::new` calls on the same directory in one process both succeed (the existing `schema_creates_tables` test does exactly that), so the lock conflict can only be reproduced with two processes.
- `ReadonlyStatsRecorder` (the `stats` CLI, run with `LIMBO_DISABLE_FILE_LOCK=1`) is a separate type and is not touched.

## Goals / Non-Goals

**Goals:**
- A stats database that cannot be opened at startup never prevents the MCP server from starting.
- The failure is diagnosable from one log line that has the cause and the path.
- The diff stays inside `src/stats/` and `src/main.rs` for production code.

**Non-Goals:**
- Recording stats from several processes into one project (per-process DB files, lazy open, idle close, retrying the open later). Deferred to a follow-up change that builds on PR #21.
- Handling write errors after a successful open (for example, a lock that appears mid-session). These already log `warn!` per call and are unchanged.
- Changing the `stats` CLI or `ReadonlyStatsRecorder`.

## Decisions

### D1: The disabled state lives inside `StatsRecorder`

`StatsRecorder` holds `db: Option<turso::Database>` (`None` = disabled). `record_tool_call`, `record_server_event` and `retention_cleanup` return `Ok(())` right away when `db` is `None`. The public API and the `Arc<StatsRecorder>` type do not change.

Alternative considered: `Arc<Option<StatsRecorder>>` (or `Option<Arc<StatsRecorder>>`), as PR #21's spec wording suggests ("creates no StatsRecorder"). Rejected: it changes the type in `ProjectRegistry::new`, `LanguageServerManager::new`/`with_configs`, `IdleMonitor::new` and `McpServer::new`, and adds an `if let Some` to all eight write sites (three in `tools.rs`, four in `manager.rs`, one in `idle_monitor.rs`). That is more churn for the same behavior, and every new call site would have to remember the check. Keeping the state inside the recorder puts the "stats are optional" rule in one place.

The disabled methods return `Ok(())`, not an `Err`, on purpose. Every call site logs `warn!` on `Err`, so an `Err` would write one warning per tool call and per server event. That log spam would bury the one startup warning that explains the cause.

A small enum (`Enabled(Database)` / `Disabled`) works just as well. Use `Option` unless the implementation finds a reason not to. The choice does not affect behavior.

### D2: `StatsRecorder::new` becomes infallible

Signature: `pub async fn new(project_root: &Path) -> Self`. Internally a private `async fn open(project_root) -> Result<Database, turso::Error>` does today's three steps (create dir, build, ensure schema). `new` matches on it. On `Err(e)` it logs once:

```
tracing::warn!(error = %e, path = %db_path.display(),
    "failed to open stats database; usage statistics are disabled for this session");
```

and then returns the disabled recorder.

Why infallible instead of keeping `Result` for some cases: every failure mode is equally non-fatal from the server's point of view. That includes `create_dir_all` failing, a non-UTF-8 path, a lock or I/O error from turso, and a schema error. If `new` kept a `Result` for any of them, `main.rs` would need a second error branch that has to decide between exiting (the bug this change fixes) and building a disabled recorder anyway. An infallible `new` makes the only correct handling the only possible one, and the warning is logged where the cause and path are known.

Cost: the three test helpers that call `StatsRecorder::new(..).await.unwrap()` (`src/stats/recorder.rs` `test_recorder`, `src/server/manager.rs` `test_stats`, `src/project/registry.rs` `test_stats`) must drop `.unwrap()`. This is test code only. The production signatures in those files stay the same. `test_recorder` should also assert that the recorder is enabled, so a broken test environment fails loudly instead of silently testing no-ops. Recorder tests that use `recorder.db.connect()` switch to a `#[cfg(test)]` accessor that returns `&turso::Database` and panics if disabled. That fits the `db_for_test()` accessor in PR #35 (`stats-uptime`, unmerged): whichever branch merges second implements `db_for_test()` as that accessor. A `#[cfg(test)] fn is_enabled(&self) -> bool` (or the accessor returning `Option`) lets the new unit test assert the disabled state.

`main.rs` becomes `let stats = Arc::new(StatsRecorder::new(cwd).await);`.

### D3: Visibility of the warning: the log file only

`warn!` goes to the per-invocation JSONL log under `.symtrace/logs/` (or `SYMTRACE_LOG_DIR`), the same place as every other server diagnostic. We add no stderr output. The project replaced `eprintln!` with tracing on purpose (structured-logging spec), and adding a stderr layer for one event would be a logging-design change of its own. The behavior change also makes the failure much less urgent: the server works, so the user sees a working MCP server whose `stats` output is missing this session's calls, and the log line explains why.

Accepted gaps:
- With `SYMTRACE_LOG=off` there is no subscriber and the warning is dropped. The user turned logging off on purpose.
- If `.symtrace/` itself cannot be created, `init_logging` fails first (same parent directory). Then there is no subscriber and the stats warning is lost. But `init_logging` already prints `warning: failed to create log directory ...` to stderr, and Claude Code keeps that in its MCP server log. That line points at the same root cause.

Alternatives considered: an MCP `notifications/message` log notification, or a note in the `initialize` result. Rejected for this change: it adds protocol surface for an auxiliary feature. It can be revisited together with the multi-process design.

## Risks / Trade-offs

- [The second process's stats are silently lost for its whole session] → Accepted for this change; the multi-process follow-up addresses it. The startup warning records that it happened.
- [A transient failure, such as a zombie that exits a second later, disables stats for the whole session because nothing retries the open] → Accepted; retrying or opening lazily is the follow-up's design space. Restarting the server (or reconnecting MCP) recovers.
- [The lock scenario cannot be covered by a normal unit test, because the turso lock is per process] → The unit test covers the shared code path (any `open` failure yields a disabled recorder) using an unopenable path. The lock case is covered by an optional `#[ignore]`d two-process test and by manual verification (tasks 3.x).
- [Conflict with PR #35 in `recorder.rs` tests] → Both changes are small. The accessor approach in D2 makes the merge mechanical.
- [The `stats-storage` requirement "Shared database handle with multiprocess WAL" now holds only when the recorder is enabled] → The new ADDED requirement states the disabled case explicitly. The existing requirement stays correct for the enabled case, so it is not modified.
