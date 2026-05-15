## Requirements

### Requirement: Tool call instrumentation
The system SHALL record a `tool_calls` row for every `tools/call` dispatch in `handle_tools_call`. The row SHALL contain: tool name, target `file_path` (extracted from arguments), `duration_ms` (measured with `Instant::now()` / `elapsed()`), `success` (based on handler result), and `error_msg` (if the handler returned an error).

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
- **THEN** the tool response is still returned to the caller; the stats error is logged to stderr

### Requirement: Language server startup instrumentation
The system SHALL record a `server_events` row when a language server starts. The row SHALL contain: `language`, `event: "started"`, `duration_ms` (time from spawn to ready), and `detail: NULL` on success or the error message on failure.

#### Scenario: Successful server startup
- **WHEN** `start_server_internal` completes successfully
- **THEN** a row is inserted with `event: "started"`, the startup duration, and `detail: NULL`

#### Scenario: Failed server startup
- **WHEN** `start_server_internal` fails
- **THEN** a row is inserted with `event: "startup_failed"` and the error message in `detail`

### Requirement: Language server shutdown instrumentation
The system SHALL record a `server_events` row when a language server is stopped. The row SHALL contain: `language`, `event: "stopped"`, and `detail` indicating the reason ("idle_timeout", "session_end", or "manual").

#### Scenario: Idle timeout shutdown
- **WHEN** `IdleMonitor` shuts down a server due to idle timeout
- **THEN** a row is inserted with `event: "stopped"` and `detail: "idle_timeout"`

#### Scenario: Session end shutdown
- **WHEN** `shutdown_all` stops servers during MCP server shutdown
- **THEN** a row is inserted with `event: "stopped"` and `detail: "session_end"`

#### Scenario: Explicit stop_server call
- **WHEN** `stop_server` is called directly
- **THEN** a row is inserted with `event: "stopped"` and `detail: "manual"`

### Requirement: Stats recorder concurrency
The `StatsRecorder` SHALL be held behind `Arc<StatsRecorder>` (no `Mutex`). The `Database` type from turso 0.6.0 is `Clone + Send + Sync` and handles connection multiplexing internally, making concurrent database access safe by design.

#### Scenario: Concurrent tool calls
- **WHEN** two tool calls arrive concurrently
- **THEN** their stats recordings proceed concurrently via the `Database` handle; connection multiplexing is handled internally
