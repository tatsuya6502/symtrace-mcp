## MODIFIED Requirements

### Requirement: LspClient lifecycle management
The system SHALL provide an `LspClient` that manages the full LSP server lifecycle: spawn the child process, send `initialize` with client capabilities, receive server capabilities, send `initialized` notification, handle `shutdown` + `exit` for clean termination, and process incoming server notifications via an mpsc channel for push diagnostics caching.

#### Scenario: Successful language server startup
- **WHEN** `LspClient::start` is called with a valid language server command and project root
- **THEN** the system SHALL spawn the child process, send `initialize`, receive `InitializeResult`, send `initialized`, store the server capabilities, and start processing incoming notifications

#### Scenario: Language server startup failure
- **WHEN** the language server process fails to start (command not found, permission denied)
- **THEN** the system SHALL return an error without hanging

#### Scenario: Clean shutdown
- **WHEN** `LspClient::shutdown` is called on a running client
- **THEN** the system SHALL send `shutdown` and `exit` to the language server, close all open files, and drop the notification channel

#### Scenario: Push diagnostics notification received
- **WHEN** the language server sends a `textDocument/publishDiagnostics` notification
- **THEN** the system SHALL parse the notification and update the moka diagnostics cache for the notification's URI

## ADDED Requirements

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
