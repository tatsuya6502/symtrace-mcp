# server-manager Specification

## Purpose
Manages the lifecycle of the language servers for one project, holding each client as `Box<dyn LspClientApi>` and spawning it with the project's environment variables, so production clients and test mocks share one dispatch path.

## Requirements

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
The system SHALL provide a `LanguageServerManager` that maps file paths to language servers and returns a guarded reference to the appropriate `LspClient`. Each `LanguageServerManager` instance SHALL be scoped to a single project root.

#### Scenario: Rust file resolves to rust-analyzer
- **WHEN** `get_client_for_file` is called with a `.rs` file path
- **THEN** the system SHALL return a reference to the rust-analyzer client (starting it if needed) rooted at this manager's project root

#### Scenario: Unsupported file type
- **WHEN** `get_client_for_file` is called with a file extension not mapped to any language server
- **THEN** the system SHALL return an error indicating the language is not supported

#### Scenario: File outside project root
- **WHEN** `get_client_for_file` is called with a file path that is not under this manager's project root
- **THEN** the system SHALL return an error indicating the file is outside the project scope

### Requirement: Idle monitor automatic shutdown
The system SHALL monitor language server usage and automatically shut down servers that have been idle for longer than the configured timeout (`idle_timeout_secs`, default 600 seconds for every language server).

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

### Requirement: TypeScript language variant
The system SHALL support `Language::TypeScript` as a language variant with default server configuration: command `typescript-language-server`, args `["--stdio"]`, extensions `["ts", "tsx", "js", "jsx"]`, language_id `"typescript"`, idle_timeout_secs 600.

#### Scenario: TypeScript file resolves to typescript-language-server
- **WHEN** `get_client_for_file` is called with a `.ts` or `.tsx` file path
- **THEN** the system SHALL return a reference to the typescript-language-server client (starting it if needed)

#### Scenario: JavaScript file resolves to typescript-language-server
- **WHEN** `get_client_for_file` is called with a `.js` or `.jsx` file path
- **THEN** the system SHALL return a reference to the typescript-language-server client (starting it if needed)

### Requirement: TypeScript client capabilities
The system SHALL provide TypeScript-specific client capabilities that match `typescript-language-server`'s supported features. The capabilities SHALL NOT include `textDocument.diagnostic` (pull diagnostics) since `typescript-language-server` does not support it.

#### Scenario: TypeScript initialization sends correct capabilities
- **WHEN** a TypeScript language server is started
- **THEN** the `initialize` request SHALL include capabilities for hover, references, definition, implementation, rename, document symbol, workspace symbol, and call hierarchy, but NOT `textDocument.diagnostic`

### Requirement: Language server lifecycle management
`LanguageServerManager` SHALL manage language server lifecycle. `ServerEntry` SHALL hold the client as `Box<dyn LspClientApi>` instead of `LspClient`, enabling trait-object dispatch for both production clients and test mocks. The manager SHALL store per-project environment variables and pass them to the LSP client when spawning a server.

#### Scenario: Server startup boxes the client
- **WHEN** a language server is started via `start_server_internal`
- **THEN** the system SHALL create `LspClient::start()` with the manager's environment variables, wait for indexing, and store it as `Box<dyn LspClientApi>` in `ServerEntry`

#### Scenario: Server shutdown via trait object
- **WHEN** `stop_server` removes a `ServerEntry`
- **THEN** the system SHALL call `FileManager::close_all` with the trait object, then call `shutdown` on the trait object

#### Scenario: Server spawned with project env vars
- **WHEN** a language server is started for a manager that has environment variables configured
- **THEN** the spawned process SHALL inherit the parent environment with the configured variables added or overridden
