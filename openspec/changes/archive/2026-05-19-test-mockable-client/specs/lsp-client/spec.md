## MODIFIED Requirements

### Requirement: LspClient lifecycle management
The system SHALL provide an `LspClient` that manages the full LSP server lifecycle: spawn the child process, send `initialize` with client capabilities, receive server capabilities, send `initialized` notification, and handle `shutdown` + `exit` for clean termination. `LspClient` SHALL implement the `LspClientApi` trait.

#### Scenario: Successful language server startup
- **WHEN** `LspClient::start` is called with a valid language server command and project root
- **THEN** the system SHALL spawn the child process, send `initialize`, receive `InitializeResult`, send `initialized`, and store the server capabilities

#### Scenario: Language server startup failure
- **WHEN** the language server process fails to start (command not found, permission denied)
- **THEN** the system SHALL return an error without hanging

#### Scenario: Clean shutdown
- **WHEN** `LspClient::shutdown` is called on a running client
- **THEN** the system SHALL send `shutdown`, wait for the response, send `exit`, and reap the child process

#### Scenario: LspClient implements LspClientApi
- **WHEN** `LspClient` is constructed via `start()`
- **THEN** it SHALL implement all `LspClientApi` trait methods, delegating to its internal `LspTransport`

### Requirement: File management via didOpen/didChange/didClose
The system SHALL send `textDocument/didOpen` before querying a file, `textDocument/didChange` when the file has been modified since last open (detected by mtime), and `textDocument/didClose` when the file is no longer needed. These methods SHALL be part of the `LspClientApi` trait so that `FileManager` can accept any implementation (including mocks).

#### Scenario: Opening a new file
- **WHEN** a query targets a file that is not currently open
- **THEN** the system SHALL read the file from disk, send `textDocument/didOpen` with the full content and language identifier, and track the file as open with its version and mtime

#### Scenario: Re-opening a modified file
- **WHEN** a query targets a file that is open but has been modified on disk (mtime changed)
- **THEN** the system SHALL read the updated content, send `textDocument/didChange` with the new content and incremented version, and update the tracked mtime

#### Scenario: Closing a file
- **WHEN** a file is closed via `FileManager::close` or during server shutdown
- **THEN** the system SHALL send `textDocument/didClose` and remove the file from tracking
