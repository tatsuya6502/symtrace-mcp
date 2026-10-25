# stats-instrumentation Specification

## Purpose
Records every tool call and language server start/stop event in the stats database as a side effect of normal operation, with recording failures logged via structured logging rather than surfaced to the caller.

## Requirements

### Requirement: Tool call instrumentation
The system SHALL record a `tool_calls` row for every `tools/call` dispatch in `handle_tools_call`. The row SHALL contain: tool name, target `file_path` (extracted from arguments), `duration_ms` (measured with `Instant::now()` / `elapsed()`), `success` (based on handler result), and `error_msg` (if the handler returned an error). Stats recording failures SHALL be logged via structured logging (`warn!` macro) rather than `eprintln!`.

#### Scenario: Successful tool call
- **WHEN** `handle_tools_call` dispatches to a tool handler and it returns success
- **THEN** a row is inserted with `success: true`, the measured duration, and `error_msg: NULL`

#### Scenario: Failed tool call
- **WHEN** `handle_tools_call` dispatches to a tool handler and it returns an error
- **THEN** a row is inserted with `success: false`, the measured duration, and the error message

#### Scenario: Unknown tool call
- **WHEN** `handle_tools_call` receives an unknown tool name
- **THEN** a row is inserted with `success: false` and error message "unknown tool"

#### Scenario: Stats recording failure does not affect tool response
- **WHEN** the stats recording fails (e.g., DB write error)
- **THEN** the tool response is still returned to the caller; the stats error is logged via structured logging

### Requirement: Language server startup instrumentation
The system SHALL record a `server_events` row when a language server starts. The row SHALL contain: `language`, `event: "started"`, `duration_ms` (time from spawn to ready), and `detail: NULL` on success or the error message on failure.

#### Scenario: Successful server startup
- **WHEN** `start_server_internal` completes successfully
- **THEN** a row is inserted with `event: "started"`, the startup duration, and `detail: NULL`

#### Scenario: Failed server startup
- **WHEN** `start_server_internal` fails
- **THEN** a row is inserted with `event: "startup_failed"` and the error message in `detail`

### Requirement: Stats recorder concurrency
The `StatsRecorder` SHALL be held behind `Arc<StatsRecorder>` (no `Mutex`). The `Database` type from turso 0.6.0 is `Clone + Send + Sync` and handles connection multiplexing internally, making concurrent database access safe by design.

#### Scenario: Concurrent tool calls
- **WHEN** two tool calls arrive concurrently
- **THEN** their stats recordings proceed concurrently via the `Database` handle; connection multiplexing is handled internally

### Requirement: Language server stop instrumentation
The system SHALL record exactly one `server_events` row each time a running language server is stopped. The row SHALL contain: `language`, `event: "stopped"`, `duration_ms` (time elapsed since the server process was spawned), and `detail` indicating the reason ("idle_timeout" or "session_end"). Stop events SHALL be logged via structured logging (`info!` for idle timeout, `warn!` for errors) rather than `eprintln!`.

#### Scenario: Idle timeout shutdown
- **WHEN** `IdleMonitor` shuts down a server due to idle timeout
- **THEN** a row is inserted with `event: "stopped"`, `detail: "idle_timeout"`, and `duration_ms` set to the time since that server was spawned

#### Scenario: Session end shutdown
- **WHEN** `shutdown_all` stops servers during MCP server shutdown
- **THEN** a row is inserted for each running server with `event: "stopped"`, `detail: "session_end"`, and `duration_ms` set to the time since that server was spawned

#### Scenario: Idle timeout records a single row
- **WHEN** `IdleMonitor` shuts down a server due to idle timeout
- **THEN** exactly one `stopped` row is inserted for that stop, with `detail: "idle_timeout"`

#### Scenario: Stop of a server that is not running
- **WHEN** a stop is requested for a language whose server is not running
- **THEN** no `stopped` row is inserted

#### Scenario: LSP shutdown request fails
- **WHEN** a running server is stopped and its LSP shutdown request returns an error
- **THEN** a `stopped` row with the duration and reason is still inserted

### Requirement: Recording while stats are disabled
When stats recording is disabled because the database could not be opened at startup, tool-call and language-server event instrumentation SHALL NOT affect tool responses or server lifecycle, and SHALL NOT log a warning per recording attempt. The only diagnostic SHALL be the single startup warning.

#### Scenario: Tool call with stats disabled
- **WHEN** stats recording is disabled and `handle_tools_call` dispatches a tool call
- **THEN** the tool response is returned to the caller as usual, no `tool_calls` row is written, and no "stats recording failed" warning is logged

#### Scenario: Server start and stop with stats disabled
- **WHEN** stats recording is disabled and a language server starts, fails to start, or stops
- **THEN** the server lifecycle proceeds as usual, no `server_events` row is written, and no warning about stats recording is logged

#### Scenario: Retention cleanup with stats disabled
- **WHEN** stats recording is disabled and the startup or periodic retention cleanup runs
- **THEN** it completes without database access and no "stats retention cleanup failed" warning is logged
