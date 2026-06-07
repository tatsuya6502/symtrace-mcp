## Why

symtrace-mcp lacks structured logging. The 15 ad-hoc `eprintln!` calls scattered across the codebase provide no timestamps, no log levels, no PID identification, and no file-based output. This makes post-mortem debugging nearly impossible — particularly on Linux where Claude Code `/clear` command leaves symtrace-mcp in an error state. Logging must be in place before implementing the `stats-db-resilience` fix so we can observe the exact failure sequence in user's environment.

## What Changes

- Add `tracing` + `tracing-subscriber` (JSON formatter) + `tracing-appender` (non-blocking file writer) dependencies
- Initialize a global tracing subscriber at server startup that writes JSONL to per-invocation log files under `<project>/.symtrace/logs/`
- Replace all 15 `eprintln!` calls with structured `info!`/`warn!`/`error!` macros
- Log file naming: `symtrace-mcp.YYYY-MM-DD_HHmmss.PID.log` — one file per process invocation
- On startup, clean up log files older than 7 days
- Support `SYMTRACE_LOG` env var for level control (`off`/`error`/`warn`/`info`/`debug`/`trace`)
- Support `SYMTRACE_LOG_DIR` env var for custom log directory
- Add `[logging]` section to `.symtrace.toml` with `level` field (overridden by `SYMTRACE_LOG` env var)
- Do NOT log to stdout (reserved for JSON-RPC) or stderr (may interfere with MCP client)

## Capabilities

### New Capabilities
- `structured-logging`: Per-invocation JSONL file logging with level control, rotation cleanup, and config/env-var overrides

### Modified Capabilities
- `config-file`: Add optional `[logging]` section with `level` field to `.symtrace.toml` schema
- `stats-instrumentation`: Change "logged to stderr" requirement to "logged via structured logging" (file-based)

## Impact

- **Dependencies**: Add `tracing`, `tracing-subscriber` (with `json` feature), `tracing-appender` to `Cargo.toml`
- **Code**: `main.rs` (subscriber init, cleanup), `mcp/tools.rs`, `server/manager.rs`, `server/idle_monitor.rs`, `lsp/transport.rs`, `stats/recorder.rs`, `stats/query.rs` (replace `eprintln!`)
- **Config**: `.symtrace.toml` gains optional `[logging]` section
- **Disk**: `.symtrace/logs/` directory created automatically; typical log file is 10KB–1MB per session
- **Performance**: Non-blocking writer ensures logging never blocks the async runtime
