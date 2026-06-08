## 1. Dependencies & Config Struct

- [x] 1.1 Add `tracing`, `tracing-subscriber` (with `json` feature), and `tracing-appender` to `Cargo.toml`
- [x] 1.2 Add `LoggingConfig` struct to `src/config.rs` with `level: Option<String>` field; add `logging: LoggingConfig` (with `#[serde(default)]`) to `SymtraceConfig`
- [x] 1.3 Add unit tests for `[logging]` config parsing: with level, without level, omitted entirely

## 2. Logging Initialization

- [x] 2.1 Create `src/logging.rs` module with `init_logging(cwd: &Path, config_level: Option<&str>) -> Option<WorkerGuard>` function
- [x] 2.2 Implement log directory resolution: `SYMTRACE_LOG_DIR` env var → `<cwd>/.symtrace/logs/` fallback; create directory if needed
- [x] 2.3 Implement log file naming: `symtrace-mcp.YYYY-MM-DD_HHmmss.PID.log` using `std::time::SystemTime` and `std::process::id()`
- [x] 2.4 Implement log level resolution: `SYMTRACE_LOG` env var → config `level` → hardcoded `"info"`; support `off`, level names, and `Targets` filter syntax
- [x] 2.5 Build subscriber: `Registry → JSON formatter → NonBlocking(file)` with resolved filter; call `set_global_default()`
- [x] 2.6 Implement startup cleanup: scan log directory for `symtrace-mcp.*.log` files older than 7 days and delete them

## 3. Integrate into main.rs

- [x] 3.1 Call `init_logging()` in `run_server()` after config loading; hold `WorkerGuard` in scope for the server lifetime
- [x] 3.2 Add `info!("Server started", cwd, pid)` after successful initialization
- [x] 3.3 Ensure `stats` subcommand does NOT call `init_logging()`

## 4. Replace eprintln! with Tracing Macros

- [x] 4.1 Replace `eprintln!` calls in `src/main.rs` with `error!` (stats init, registry build, server run; config parse error keeps `eprintln!` since logging is not yet initialized)
- [x] 4.2 Replace 3 `eprintln!` calls in `src/mcp/tools.rs` with `warn!` (stats retention cleanup, stats recording)
- [x] 4.3 Replace 5 `eprintln!` calls in `src/server/manager.rs` with `warn!` (index wait, stats recording failures)
- [x] 4.4 Replace 3 `eprintln!` calls in `src/server/idle_monitor.rs` with `info!`/`warn!` (shutdown event, stats recording)
- [x] 4.5 Replace 1 `eprintln!` call in `src/lsp/transport.rs` with `warn!` (reader error)
- [x] 4.6 Leave `eprintln!` in `src/stats/query.rs` unchanged (stats CLI, no subscriber active)

## 5. Verification

- [x] 5.1 Run `cargo clippy --all-targets --tests -- -D warnings` and fix any warnings
- [x] 5.2 Run `cargo fmt --all -- --check` and fix any formatting
- [x] 5.3 Run `cargo test` and ensure all tests pass
- [x] 5.4 Manual smoke test: start `symtrace-mcp`, verify `.symtrace/logs/` is created with JSONL content, verify `symtrace-mcp stats` still works without creating log files
