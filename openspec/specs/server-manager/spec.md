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
- **WHEN** a tool request names a file path that is not under any configured project root
- **THEN** the request SHALL be rejected by `ProjectRegistry` before any manager is selected (see project-registry "File not under any project"); `get_client_for_file` itself only resolves the language from the file extension and is reached only through the registry

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

### Requirement: Call hierarchy client capability
For every supported language, the `initialize` request SHALL declare the `textDocument.callHierarchy` client capability with `dynamicRegistration` set to `false`. Some language servers advertise `callHierarchyProvider` only when the client declares this capability, so the call hierarchy tools depend on it.

#### Scenario: TypeScript initialization declares call hierarchy
- **WHEN** a TypeScript language server is started
- **THEN** the `initialize` request capabilities SHALL contain `textDocument.callHierarchy` with `dynamicRegistration: false`

#### Scenario: Rust initialization declares call hierarchy
- **WHEN** a Rust language server is started
- **THEN** the `initialize` request capabilities SHALL contain `textDocument.callHierarchy` with `dynamicRegistration: false`

#### Scenario: Call hierarchy tools work once the server advertises the provider
- **WHEN** a TypeScript language server that gates `callHierarchyProvider` on this client capability (typescript-language-server 3.0.0 or later with TypeScript 3.8 or later) is initialized with these capabilities
- **THEN** the system SHALL treat call hierarchy as supported and `incoming_calls` / `outgoing_calls` SHALL NOT return the "does not support call hierarchy" error

### Requirement: Configurable index wait timeout
When starting a language server, the system SHALL bound the index readiness wait by that language's configured `index_timeout_secs` (default 30 seconds). A value of `0` SHALL skip the wait. After a timeout or a skipped wait the system SHALL register the server and serve tool calls. If the server stops during the wait, the startup SHALL fail.

#### Scenario: Default index timeout
- **WHEN** a language server is started and its configuration does not set `index_timeout_secs`
- **THEN** the system SHALL wait at most 30 seconds for the server to become ready

#### Scenario: Configured index timeout
- **WHEN** a Rust language server is started with `index_timeout_secs = 180`
- **THEN** the system SHALL wait at most 180 seconds for the server to become ready

#### Scenario: Index wait times out
- **WHEN** the server does not become ready within its index timeout
- **THEN** the system SHALL log a warning that names the language and the timeout, register the server, and execute the pending tool call

#### Scenario: Server stops during the index wait
- **WHEN** the language server stops (its notification stream ends) before it becomes ready and before the index timeout expires
- **THEN** the system SHALL record a `startup_failed` event and return a startup error instead of registering the server

#### Scenario: Index wait disabled
- **WHEN** a language server is started with `index_timeout_secs = 0`
- **THEN** the system SHALL register the server without waiting for readiness

### Requirement: Rust server status opt-in
The system SHALL declare the client capability `experimental.serverStatusNotification: true` in the `initialize` request sent to the Rust language server, so that rust-analyzer reports its indexing status. The TypeScript client capabilities SHALL NOT declare it.

#### Scenario: Rust initialization requests server status
- **WHEN** a Rust language server is started
- **THEN** the `initialize` request SHALL include `capabilities.experimental.serverStatusNotification` set to `true`

#### Scenario: TypeScript initialization does not request server status
- **WHEN** a TypeScript language server is started
- **THEN** the `initialize` request SHALL NOT include `capabilities.experimental.serverStatusNotification`

### Requirement: TypeScript semantic-server-only mode
The `initialize` request sent to a TypeScript language server SHALL include `initializationOptions` with `tsserver.useSyntaxServer` set to `"never"`, so that every request is answered by the semantic tsserver with whole-project knowledge, including requests that arrive while the project is still loading. The `initialize` request sent to a Rust language server SHALL NOT contain an `initializationOptions` field.

#### Scenario: TypeScript initialization disables the syntax server
- **WHEN** a TypeScript language server is started
- **THEN** the `initialize` request SHALL contain `initializationOptions` equal to `{"tsserver": {"useSyntaxServer": "never"}}`

#### Scenario: Rust initialization sends no initialization options
- **WHEN** a Rust language server is started
- **THEN** the `initialize` request SHALL NOT contain an `initializationOptions` field (neither an object nor `null`)

#### Scenario: First TypeScript query resolves across files
- **WHEN** the first tool call after a TypeScript language server starts is `goto_definition` on a call to a function imported from another file
- **THEN** the response SHALL point to the function's definition in the other file, not to the import statement in the queried file
