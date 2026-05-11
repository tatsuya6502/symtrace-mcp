## ADDED Requirements

### Requirement: Lazy language server startup
The system SHALL start a language server on the first tool invocation that targets that language. Subsequent invocations SHALL reuse the already-running server.

#### Scenario: First tool call starts the server
- **WHEN** a tool call targets a language whose server is not running
- **THEN** the system SHALL start the language server, wait for readiness, and then execute the tool

#### Scenario: Subsequent tool calls reuse the server
- **WHEN** a tool call targets a language whose server is already running
- **THEN** the system SHALL execute the tool immediately without restarting the server

#### Scenario: Concurrent first tool calls
- **WHEN** multiple tool calls arrive simultaneously for a language whose server is not running
- **THEN** the system SHALL start the server once and queue the tool calls until it is ready

### Requirement: LanguageServerManager provides client access
The system SHALL provide a `LanguageServerManager` that maps file paths to language servers and returns a guarded reference to the appropriate `LspClient`.

#### Scenario: Rust file resolves to rust-analyzer
- **WHEN** `get_client_for_file` is called with a `.rs` file path
- **THEN** the system SHALL return a reference to the rust-analyzer client (starting it if needed)

#### Scenario: Unsupported file type
- **WHEN** `get_client_for_file` is called with a file extension not mapped to any language server
- **THEN** the system SHALL return an error indicating the language is not supported

### Requirement: Idle monitor automatic shutdown
The system SHALL monitor language server usage and automatically shut down servers that have been idle for longer than the configured timeout (default 300 seconds for rust-analyzer: 600 seconds).

#### Scenario: Idle server is shut down
- **WHEN** a language server has had no tool invocations for longer than its idle timeout
- **THEN** the idle monitor SHALL shut down the server and release its resources

#### Scenario: Active server is not shut down
- **WHEN** a language server has had a tool invocation within its idle timeout
- **THEN** the idle monitor SHALL NOT shut down the server

#### Scenario: Tool invocation updates last-used time
- **WHEN** a tool invocation is dispatched to a language server
- **THEN** the system SHALL update that server's last-used timestamp

### Requirement: Graceful shutdown on MCP disconnect
When the MCP server shuts down (stdin closed), the system SHALL shut down all running language servers cleanly.

#### Scenario: MCP server exits
- **WHEN** the MCP server event loop ends (stdin closed or fatal error)
- **THEN** the system SHALL send `shutdown` + `exit` to all running language servers and reap their processes
