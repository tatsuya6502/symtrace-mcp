## Context

symtrace-mcp records tool calls and server lifecycle events to a per-project Turso (libSQL/SQLite) database at `.symtrace/stats.db`. The current implementation uses turso 0.5.3, which lacks multi-process WAL support. To allow the `symtrace-mcp stats` CLI to read the database while the MCP server is running, `StatsRecorder` uses an open/write/close pattern — opening a new database connection for every single write operation, then dropping it. This forces `Arc<Mutex<StatsRecorder>>` across the entire codebase to serialize concurrent opens.

Turso 0.6.0 introduces `experimental_multiprocess_wal`, enabling safe concurrent access from multiple processes (server + CLI) via WAL-based file locking.

## Goals / Non-Goals

**Goals:**
- Upgrade turso to 0.6.0 and enable multiprocess WAL
- Replace `Arc<Mutex<StatsRecorder>>` with `Arc<StatsRecorder>` — drop the Mutex
- Replace open/write/close with a shared `Database` handle (internally `Arc`-wrapped)
- Simplify all call sites (remove `.lock().await`)

**Non-Goals:**
- Performance benchmarking — LSP round-trips dominate tool call latency; DB overhead is negligible
- Changing the database schema or retention logic
- Adding connection pooling beyond `Database::connect()` per call

## Decisions

### Decision 1: `Database`-only approach (no persistent `Connection`)

**Choice:** `StatsRecorder` holds a `Database` handle. Each method calls `self.db.connect()` to get a fresh `Connection`, uses it, and drops it.

**Alternatives considered:**
- Persistent `Connection` under `Arc<Mutex<Connection>>` — still needs Mutex since `Connection` is not `Send`. Minimal improvement over current code.
- Connection pool — overkill for this workload (infrequent writes, no high-concurrency requirement).

**Rationale:** `Database` is `Clone + Send + Sync` (internally `Arc`-wrapped). `db.connect()` is cheap — it creates a new `Connection` from the already-open database handle, no file I/O. This eliminates the Mutex entirely while keeping the code simple.

### Decision 2: `StatsRecorder::new()` becomes async

**Choice:** Constructor opens the database, enables multiprocess WAL, and runs `ensure_schema` — all async operations. Returns `Result<StatsRecorder>`.

**Rationale:** Eliminates the separate `ensure_schema()` call at construction sites. The `open()` method is no longer needed.

### Decision 3: `db_path` retained for `db_exists()` check

**Choice:** Keep a `db_path: PathBuf` field solely for the `db_exists()` method used by the CLI.

**Rationale:** The CLI checks file existence before attempting queries. `Database::build()` would create the file, so we can't use it for the existence check. The `PathBuf` is minimal overhead.

### Decision 4: `stats-recorder-serialization` spec replaced

**Choice:** The `Stats recorder serialization` requirement in `stats-instrumentation` is removed. Concurrent access is now safe by design — `Database` handles connection multiplexing internally. No replacement requirement needed.

## Risks / Trade-offs

- **`experimental_multiprocess_wal` stability** → The feature is marked "experimental" in turso. If it proves unreliable, we can fall back to `Arc<Mutex<Connection>>` (persistent connection under Mutex) without reverting the turso upgrade. Mitigation: monitor for DB corruption in testing.
- **`connect()` per call overhead** → Negligible compared to LSP round-trips. No mitigation needed.
- **API surface change across 5 files** → Mechanical change (drop `Mutex`, drop `.lock().await`). Low risk, easy to review.
