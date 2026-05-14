## 1. Dependencies & Module Setup

- [ ] 1.1 Add `turso` and `clap` dependencies to `Cargo.toml`
- [ ] 1.2 Create `src/stats/` module with `mod.rs` re-exports

## 2. Stats Storage Layer

- [ ] 2.1 Implement `StatsRecorder` struct with open/write/close pattern (Turso DB)
- [ ] 2.2 Implement schema initialization: `CREATE TABLE IF NOT EXISTS` for `tool_calls` and `server_events` with indexes
- [ ] 2.3 Implement `record_tool_call(tool, file_path, duration_ms, success, error_msg)` method
- [ ] 2.4 Implement `record_server_event(language, event, duration_ms, detail)` method
- [ ] 2.5 Implement `retention_cleanup()` method that deletes rows older than 30 days
- [ ] 2.6 Add unit tests for `StatsRecorder` (schema creation, insert, retention)

## 3. Tool Call Instrumentation

- [ ] 3.1 Add `Arc<Mutex<StatsRecorder>>` to `McpServer` struct
- [ ] 3.2 Wrap tool handler dispatch in `handle_tools_call` with `Instant::now()` timing and stats recording
- [ ] 3.3 Extract `file_path` from tool arguments for the stats row
- [ ] 3.4 Handle stats recording errors gracefully (log to stderr, don't fail tool call)

## 4. Server Lifecycle Instrumentation

- [ ] 4.1 Instrument `start_server_internal` to record startup events with duration
- [ ] 4.2 Instrument `stop_server` to record shutdown events with reason
- [ ] 4.3 Instrument `shutdown_all` to record session-end shutdown events
- [ ] 4.4 Instrument `IdleMonitor` shutdown to record idle-timeout events
- [ ] 4.5 Wire `StatsRecorder` through `LanguageServerManager` and `IdleMonitor`

## 5. Periodic Retention Cleanup

- [ ] 5.1 Run `retention_cleanup()` on MCP server startup
- [ ] 5.2 Spawn a background tokio task that runs cleanup every 24 hours

## 6. CLI Subcommand (stats)

- [ ] 6.1 Add clap-based CLI arg parsing to `src/main.rs` (default: run server, `stats`: print stats)
- [ ] 6.2 Implement `print_stats(project_root)` function: query last 7 days, format tool usage section
- [ ] 6.3 Add top files section (top 10 by call count)
- [ ] 6.4 Add language servers section (startup count, avg startup time, total uptime)
- [ ] 6.5 Handle missing database gracefully ("No stats data found")

## 7. Integration Testing

- [ ] 7.1 Test end-to-end: tool calls produce stats rows, `stats` subcommand reads them
- [ ] 7.2 Test concurrent tool calls don't cause DB errors (Mutex serialization)
- [ ] 7.3 Test retention cleanup removes old rows
