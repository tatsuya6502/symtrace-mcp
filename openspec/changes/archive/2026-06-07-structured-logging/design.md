## Context

symtrace-mcp runs as a stdio-based MCP server: stdin receives JSON-RPC requests, stdout sends responses. stdout is reserved exclusively for the JSON-RPC protocol — any non-protocol output there corrupts the stream. Currently, 15 `eprintln!` calls write unstructured text to stderr without timestamps, log levels, or PID identification.

This creates a debugging blind spot: for example, running `/clear` in Claude Code on Linux may leave symtrace-mcp in an error state, and there are no logs to examine. The `stats-db-resilience` change needs logging in place first so we can observe the failure sequence in user's environment.

## Goals / Non-Goals

**Goals:**
- Provide structured, file-based JSONL logging for all server lifecycle events
- Enable post-mortem debugging of zombie process / DB lock issues
- Support per-invocation log files so concurrent processes (multiple projects) never share a file
- Allow log level control via environment variable and config file
- Minimize performance impact with non-blocking async writes

**Non-Goals:**
- Logging to stdout or stderr (stdout is JSON-RPC; stderr may interfere with MCP clients)
- Log search/analytics tooling (users can use `jq` / `grep`)
- Remote logging or log shipping
- Structured log spans / `#[instrument]` (keep it simple: `info!` / `warn!` / `error!` only)
- Log encryption or redaction of file paths

## Decisions

### Decision 1: `tracing` ecosystem

**Choice**: Use `tracing` + `tracing-subscriber` (with `json` feature) + `tracing-appender`.

**Alternatives considered:**
- **`log` crate + `env_logger`**: Simpler, but no structured output, no async appender. Rejected.
- **`slog`**: Powerful but more boilerplate, less ecosystem momentum. Rejected.
- **Custom JSON writer**: Full control but reinvents the wheel. Rejected.

**Rationale**: `tracing` is the Rust standard. `tracing-subscriber`'s JSON formatter outputs one JSON object per line (JSONL). `tracing-appender::non_blocking` wraps any `Write` impl for async writes that never block the tokio runtime.

### Decision 2: Per-invocation log files

**Choice**: Each server process creates its own log file named `symtrace-mcp.YYYY-MM-DD_HHmmss.PID.log`.

**Alternatives considered:**
- **Daily rotation via `tracing-appender::rolling::daily`**: All processes share one file. NonBlocking uses an in-process Mutex, which doesn't protect against cross-process interleaving. Rejected.
- **Per-invocation + PID-only filename**: No timestamp makes chronological sorting harder. Rejected.

**Rationale**: Per-invocation files eliminate cross-process write contention entirely. The PID in the filename makes it trivial to identify which process wrote which file — critical for zombie process investigation. Timestamp enables chronological sorting across files.

### Decision 3: JSONL format (not human-readable)

**Choice**: Use `tracing-subscriber`'s JSON formatter. Each log line is a self-contained JSON object.

**Rationale**: Humans rarely read these logs directly. JSONL is machine-parseable (`jq`, `grep`, future tooling). The standard `tracing-subscriber` JSON format includes `timestamp`, `level`, `target`, `fields.message`, and any structured fields attached to the event.

### Decision 4: Log directory at `<project>/.symtrace/logs/`

**Choice**: Log files live in `<project_root>/.symtrace/logs/`, alongside the existing `stats.db`.

**Alternatives considered:**
- **Global `~/.symtrace/logs/`**: Single location, but mixes logs from different projects. Harder to attribute. Rejected.
- **XDG `~/.local/state/symtrace/`**: Platform-correct but adds complexity (macOS vs Linux paths). Rejected.

**Rationale**: `.symtrace/` already exists as the project's data directory. Co-locating logs with `stats.db` is natural. Multiple projects get separate log directories automatically. `SYMTRACE_LOG_DIR` env var allows override for special cases.

### Decision 5: `NonBlocking` without `RollingFileAppender`

**Choice**: Use `tracing_appender::non_blocking(File)` directly. Handle filename generation and cleanup in application code.

**Rationale**: `RollingFileAppender` provides time-based rotation (daily/hourly), which doesn't match our per-invocation model. We create the `File` handle ourselves and wrap it with `NonBlocking` for async writes. Cleanup logic runs once at startup.

### Decision 6: Priority chain for log level

**Choice**: Environment variable > Config file > Hardcoded default (`info`).

```
SYMTRACE_LOG env var  →  highest priority (ad-hoc override)
.symtrace.toml [logging] level  →  project default
Hardcoded "info"  →  fallback
```

**Rationale**: Env vars are the standard mechanism for runtime overrides without editing config files. The config file provides project-specific defaults that survive across sessions.

### Decision 7: Custom `SYMTRACE_LOG` (not `RUST_LOG`)

**Choice**: Use `SYMTRACE_LOG` as the environment variable name.

**Rationale**: `RUST_LOG` would enable debug logging in all transitive dependencies (turso, tokio, etc.), producing excessive noise. A dedicated variable scopes control to symtrace-mcp's own targets. Users who want full dependency logging can set `SYMTRACE_LOG=symtrace_mcp=debug,turso=trace`.

## Risks / Trade-offs

**[Disk space growth]** → Log files are small (typically 10KB–1MB per session). 7-day cleanup limits accumulation. Users can set `SYMTRACE_LOG=off` to disable.

**[Logging adds dependency weight]** → `tracing` + `tracing-subscriber` + `tracing-appender` add ~1.5MB to release binary. Acceptable for a server binary.

**[NonBlocking guard must outlive subscriber]** → The `WorkerGuard` returned by `tracing_appender::non_blocking` must be held in `main()`'s scope. Dropping it flushes and closes the file. If dropped early, logs are silently lost.

**[`tracing` macros compile even without subscriber]** → `info!` / `warn!` / `error!` expand to no-ops when no subscriber is set. The `symtrace-mcp stats` command doesn't need initialization — macros in shared code (e.g., `stats/recorder.rs`) simply won't emit.

**[Config parse failure for `[logging]` section]** → If a user has an invalid `[logging]` section, the entire config fails to load (consistent with existing behavior for other sections). This is deliberate — silent config errors are worse than explicit failures.

**[PID reuse across restarts]** → PID could theoretically be reused by a different process. The timestamp in the filename disambiguates — two files with the same PID but different timestamps are clearly different invocations.
