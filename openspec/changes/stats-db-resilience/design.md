## Context

symtrace-mcp uses Turso to record tool usage statistics in `<project_root>/.symtrace/stats.db`. The current architecture opens the database eagerly at startup (`StatsRecorder::new()`) and holds the `Database` handle for the entire process lifetime. This means the file lock is held from startup until process exit.

When Claude Code disconnects from a stdio MCP server without properly terminating it (observed during `/mcp` reconnection on macOS and `/clear` on Linux), the old process remains alive as a zombie, holding the DB lock. New processes fail to acquire the lock and exit with `process::exit(1)`.

The stats subsystem is internal-only — no external API depends on it. Tool calls already handle stats write failures gracefully (log to stderr, continue). The `symtrace-mcp stats` CLI uses a separate `ReadonlyStatsRecorder` and is unaffected.

## Goals / Non-Goals

**Goals:**
- Prevent zombie symtrace-mcp processes from blocking new processes via DB lock
- Make stats DB initialization failure non-fatal so the MCP server always starts
- Release DB file lock when stats are not actively being written to
- Preserve all existing stats functionality when DB is accessible

**Non-Goals:**
- Detecting or killing zombie processes (that's Claude Code's responsibility via issue [#43177][gh-issue-43177])
- Changing the `symtrace-mcp stats` CLI behavior
- Adding stdin idle timeout to the MCP server itself (too risky given #43177)
- Supporting concurrent writers across multiple symtrace-mcp processes

[gh-issue-43177]: https://github.com/anthropics/claude-code/issues/43177

## Decisions

### Decision 1: Lazy DB open with debounce close timer

**Choice**: Open the database on first write, close it 15 seconds after the last write. Each write cancels the previous close timer and schedules a new one (debounce pattern).

**Alternatives considered:**
- **Periodic idle check (every N seconds)**: Wastes resources when idle; imprecise timing. Rejected in favor of debounce.
- **Never close, just make startup non-fatal**: Zombie still holds lock indefinitely. Doesn't solve the root problem.
- **PID file + kill stale process**: Risk of PID reuse causing wrong process kill. Fragile across platforms.

**Rationale**: The debounce pattern is self-timing — it only runs when there's actual activity, and guarantees the DB closes exactly 15s after the last write. This is the same "start on demand, shut down after idle" pattern already used for LSP servers via `IdleMonitor`.

### Decision 2: `Arc<Mutex<Option<Database>>>` for thread-safe lazy access

**Choice**: Replace the current `db: turso::Database` field with `db: Arc<Mutex<Option<turso::Database>>>`. Each write operation acquires the mutex briefly to get a connection, then releases it. The actual INSERT executes without holding the mutex.

**Alternatives considered:**
- **`OnceCell<Database>`**: Doesn't support closing and reopening. Rejected.
- **`RwLock<Option<Database>>`**: More complex, no real benefit since the critical section is tiny (open DB or get connection — microseconds).

**Rationale**: The mutex is held only during connection acquisition (~microseconds), not during async I/O. This preserves concurrent write capability while enabling safe open/close lifecycle management.

### Decision 3: Non-fatal startup with `Arc<Option<StatsRecorder>>`

**Choice**: In `main.rs`, wrap stats in `Arc<Option<StatsRecorder>>`. If `StatsRecorder::new()` fails (e.g., DB locked by zombie), log a warning and use `None`. Tool call sites guard with `if let Some(ref stats) = self.stats`.

**Alternatives considered:**
- **No-op `NullStatsRecorder` impl**: Requires a trait or enum. More code for the same effect. Rejected in favor of `Option`.
- **Retry with backoff at startup**: Delays startup, zombie might not release lock for minutes. Rejected.

**Rationale**: `Option` is the simplest representation of "stats may or may not be available." All call sites already handle `Err` from stats writes; adding a `None` guard is minimal change.

### Decision 4: Defer retention cleanup to first DB open

**Choice**: Skip `retention_cleanup()` at startup. Run schema migration and retention cleanup when the DB is first opened lazily. Continue periodic cleanup every 24 hours (only if DB is open).

**Rationale**: Running cleanup at startup would immediately open the DB, defeating the lazy-open goal. Deferring to first access is consistent with the lazy pattern.

## Risks / Trade-offs

**[Race: close timer fires during a write]** → The mutex prevents this. The write holds the mutex to get a connection; the close timer waits for the mutex. By the time the timer acquires the mutex, `last_access` is recent, so it won't close.

**[Stats lost when DB is closed and process crashes]** → Acceptable. Stats are best-effort diagnostic data, not critical. At most 15 seconds of data could be lost.

**[Two processes briefly hold the DB open simultaneously]** → Turso's `experimental_multiprocess_wal(true)` is designed for this. If a zombie's DB hasn't closed yet when a new process opens, the new process's lazy open will retry and succeed once the zombie's debounce timer fires.

**[Mutex overhead on every stats write]** → Negligible. The mutex is held for ~microseconds (check Option, get Connection). The async INSERT happens after mutex release.
