## Why

The current `StatsRecorder` uses an open/write/close pattern on every database operation — a workaround for turso 0.5.x lacking multi-process WAL support. This forces `Arc<Mutex<StatsRecorder>>` across the entire codebase, serializing all stats writes unnecessarily. Turso 0.6.0 introduces `experimental_multiprocess_wal`, allowing the server to share the database file with the `symtrace-mcp stats` CLI without coordination.

## What Changes

- Bump `turso` dependency from 0.5.3 to 0.6.0
- Replace `Arc<Mutex<StatsRecorder>>` with `Arc<StatsRecorder>` — `StatsRecorder` holds a shared `Database` (internally `Arc`-wrapped), and each method calls `db.connect()` to get a fresh `Connection`
- Remove the open/write/close pattern — no more per-operation `Builder::new_local().build().await`
- Drop the `Mutex` from all call sites (`tools.rs`, `manager.rs`, `idle_monitor.rs`, `registry.rs`, `main.rs`)

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `stats-storage`: Replace open/write/close pattern with persistent `Database` handle and per-call `connect()`. Remove `Arc<Mutex<_>>` serialization requirement.
- `stats-instrumentation`: Remove `Mutex` from stats recorder serialization requirement — concurrent access now safe via `Database`-level connection pooling.

## Impact

- **Dependencies**: `turso` 0.5.3 → 0.6.0
- **Code**: `src/stats/recorder.rs` (rewrite), `src/main.rs`, `src/mcp/tools.rs`, `src/server/manager.rs`, `src/server/idle_monitor.rs`, `src/project/registry.rs` (signature changes)
- **Tests**: Update `concurrent_tool_calls_serialized` test — no longer tests Mutex serialization, verifies concurrent writes produce correct data instead
