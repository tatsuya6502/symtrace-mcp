## Why

When Claude Code disconnects from a stdio MCP server (via `/clear`, `/mcp` reconnect, or crash), the old symtrace-mcp process may remain alive as a zombie, holding the Turso database file lock indefinitely. When Claude Code starts a new symtrace-mcp process, it fails to acquire the lock and exits with `process::exit(1)`, rendering the server unusable until the zombie is manually killed. This is particularly problematic on Linux (confirmed at work) but can also occur on macOS during `/mcp` reconnection attempts.

## What Changes

- Make stats database initialization failure non-fatal: log a warning and continue without stats instead of calling `process::exit(1)`
- Implement lazy database connection: defer opening the Turso database until the first write operation, instead of opening it eagerly at startup
- Add idle timeout with debounce pattern: close the database connection 15 seconds after the last write, releasing the file lock. Each write cancels the previous close timer and schedules a new one
- Defer retention cleanup: run schema migration and retention cleanup on first database open rather than at startup

## Capabilities

### New Capabilities

- `lazy-stats-db`: Lazy database connection lifecycle management — open on first access, close after idle timeout (15s debounce pattern), reopen on next access

### Modified Capabilities

- `stats-storage`: Requirements changing — database open/close pattern shifts from "open at startup, hold forever" to "open lazily, close on idle"; startup failure is non-fatal
- `stats-instrumentation`: Requirements changing — `StatsRecorder` synchronization model changes from `Arc<StatsRecorder>` (no Mutex) to `Arc<Mutex<Option<Database>>>`; stats recording silently skips when DB is unavailable

## Impact

- Modified: `src/stats/recorder.rs` (StatsRecorder struct and all methods)
- Modified: `src/main.rs` (startup error handling for stats)
- Modified: `src/mcp/tools.rs` (retention cleanup scheduling, stats field type)
- No new dependencies
- No breaking API changes — the stats subsystem is internal-only
- `symtrace-mcp stats` CLI is unaffected (uses its own `ReadonlyStatsRecorder`)
