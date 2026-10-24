# Proposal

## Why

When the stats database cannot be opened, `symtrace-mcp` exits with status 1 at startup. The database `.symtrace/stats.db` is opened with a per-process exclusive file lock, so a second server started in the same project directory dies with `failed to initialize stats database` / `Locking error: ... File is locked by another process`. Two Claude Code sessions in one repository trigger this, and so does a zombie stdio process left behind after `/clear` or an MCP reconnect. Nothing is written to stderr, so Claude Code only reports that the MCP server failed. Usage statistics are an auxiliary feature and must never stop the server from serving tools.

## What Changes

- If the stats database cannot be opened or initialized at startup (lock held by another process, `.symtrace/` not creatable, schema creation failure, and so on), the MCP server logs a single `warn` event naming the cause and the database path, then starts normally with stats recording disabled.
- While stats are disabled, tool-call and language-server event recording and the retention cleanup do nothing and report success, so tool responses are unaffected and no per-call warning is logged.
- The startup path no longer calls `std::process::exit(1)` on a stats initialization failure.
- No change when the database opens normally. No change to the `symtrace-mcp stats` CLI.
- Out of scope: making several server processes record into the same project's stats at once (per-process files, lazy open, idle close, as drafted in PR #21 `stats-db-resilience`). This change is the minimal piece of that draft that does not depend on that design. A later change will cover it.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `stats-storage`: adds the requirement that a stats database initialization failure is non-fatal (server starts, stats writes become no-ops, a warning is logged with the cause).
- `stats-instrumentation`: adds the requirement that recording on a disabled stats recorder leaves tool responses and server lifecycle unaffected and logs nothing per call.

## Impact

- Code: `src/stats/recorder.rs` (`StatsRecorder` gets a disabled state; `new` no longer fails), `src/main.rs` (drops the `process::exit(1)` branch). The `Arc<StatsRecorder>` plumbing through `ProjectRegistry`, `LanguageServerManager`, `IdleMonitor` and `McpServer` keeps its signatures; no production call site changes.
- Tests: the test helpers that call `StatsRecorder::new(..).await.unwrap()` (`src/stats/recorder.rs`, `src/server/manager.rs`, `src/project/registry.rs`) and the recorder tests that read `recorder.db` directly need a mechanical update. Their assertions stay the same.
- Interaction with PR #35 (`stats-uptime`, unmerged): its `#[cfg(test)] db_for_test()` accessor has to go through the same "enabled" accessor. Whichever branch merges second adapts it.
- Behavior: a second server in the same directory now serves tools, but records no stats for its session.
- No dependency, schema, configuration or CLI changes. CHANGELOG gets a `Fixed` entry.
