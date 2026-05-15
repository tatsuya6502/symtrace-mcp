## Context

symtrace-mcp is a single-process MCP server that manages language server instances per project. Tool calls flow through `handle_tools_call()` in `src/mcp/tools.rs`, and server lifecycle events occur in `src/server/manager.rs`. The server runs on tokio, communicating via stdio JSON-RPC. There are currently 5 MCP tools and support for one language (Rust).

## Goals / Non-Goals

**Goals:**
- Record per-tool-call events (tool name, file path, duration, success/error, timestamp) with zero impact on tool response latency
- Record language server lifecycle events (startup, shutdown, startup duration, shutdown reason)
- Provide a CLI command (`symtrace-mcp stats`) that prints a human-readable summary for the current project
- Store data per-project in `.symtrace/stats.db`
- Auto-delete data older than 30 days

**Non-Goals:**
- Multi-process concurrent writes (Turso is single-process; workaround: open/write/close per operation)
- Real-time dashboard or web UI
- Network or remote database storage
- Export to external formats (CSV, JSON dump)
- Per-call detailed logging (argument values, full error messages) beyond what's in the schema

## Decisions

### 1. Turso over rusqlite or plain JSON files

**Choice**: Turso (pure Rust, async, SQLite-compatible)

**Alternatives considered**:
- **rusqlite**: Battle-tested, multi-process, but requires C toolchain (SQLite C library). Cross-compilation friction.
- **JSON files**: Zero dependencies, but poor for append-heavy workloads, rolling windows, and aggregations.

**Rationale**: Pure Rust aligns with project goals. Async API fits the tokio runtime. The data model (event logs + aggregations) benefits from SQL. Single-process limitation is acceptable since the MCP server is the sole writer and uses open/write/close pattern.

### 2. Open/write/close pattern (workaround for single-process constraint)

**Choice**: Open the database, write, close it on every stats operation.

**Rationale**: Turso does not support multi-process access. Since `symtrace-mcp stats` CLI needs to read the DB while the MCP server might be running, the server MUST NOT hold the DB open. Each stats operation opens the DB, performs its write, and closes it immediately. The overhead of opening a SQLite file is microseconds — negligible compared to LSP round-trip times (50-200ms).

**Serialization**: An `Arc<Mutex<StatsRecorder>>` inside the MCP server serializes open/write/close so two concurrent tool calls don't clash.

### 3. Per-project storage at `.symtrace/stats.db`

**Choice**: Each project root gets its own `.symtrace/stats.db`.

**Rationale**: Aligns with the existing per-project model (`.symtrace.toml`). Stats are self-contained. `symtrace-mcp stats` opens `.symtrace/stats.db` relative to CWD. Single-project mode (no config file) uses CWD directly.

### 4. CLI via clap subcommand

**Choice**: `symtrace-mcp` becomes a multi-command binary: `symtrace-mcp` (default: run MCP server) and `symtrace-mcp stats` (print stats).

**Rationale**: Minimal disruption — running the binary with no subcommand preserves current behavior. `clap` with derive macros keeps the code small.

### 5. Data retention: 30-day rolling window

**Choice**: Delete rows older than 30 days on startup and periodically (every 24h during a session).

**Rationale**: Prevents unbounded growth. 30 days covers "last month" analysis while keeping the DB small (a few hundred KB even for heavy use).

### 6. Relative paths in Top Files display

**Choice**: Strip the project root prefix from file paths in the "Top Files" section, displaying relative paths (e.g., `src/stats.rs`).

**Rationale**: Absolute paths are noisy and redundant — the user already knows which project they're in (CWD). Relative paths are more readable and consistent with how developers think about project structure. If a file falls outside the project root (unlikely but possible for generated/dependency files), the full path is shown as a fallback.

### 7. Uptime tracking for unpaired server start events

**Choice**: Treat unpaired `"started"` events as still-running — count uptime from the start timestamp to `now`.

**Alternatives considered**:
- **Heartbeat table**: Periodic writes (every ~1 min) to a heartbeat table; `stats` would detect stale heartbeats and retroactively insert `"stopped"` events. Rejected: adds periodic write overhead, makes `stats` (a read command) mutate the database, and introduces a new table and cleanup logic for a cosmetic edge case.
- **Tool-call timestamps as implicit heartbeats**: Use the last tool call timestamp as a proxy for liveness. If an unpaired start's last tool call is older than the idle timeout, cap uptime at that point. No new writes needed, but deferred until the edge case proves bothersome in practice.

**Rationale**: The graceful shutdown path (stdin EOF → `shutdown_all()` → `"stopped"` event) already records matched pairs. Unpaired starts only occur when symtrace-mcp is killed (SIGKILL/SIGTERM), causing "phantom uptime" that grows until the next session. This is cosmetic and self-correcting — the next `"started"` event resets pairing. Counting to `now` is the simplest correct behavior; if phantom uptime becomes annoying, the tool-call-timestamp approach is a lightweight follow-up.

## Risks / Trade-offs

- **[Turso beta status]** → Acceptable for a toy project. If Turso becomes a problem, the schema is simple enough to migrate to rusqlite.
- **[Open/write/close overhead]** → Measured in microseconds per call. Even at 100 calls/minute, overhead is <1% of total latency.
- **[Data loss on crash]** → Since we close the DB after each write, committed data survives crashes. Only the current in-flight write could be lost.
- **[No concurrent reads during write]** → The Mutex serialization is per-process. The CLI reads only when the server has closed the DB, which is the common case (open/write/close is fast).
