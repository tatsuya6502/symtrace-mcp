# lsp-client Specification

## Purpose
Provides `LspClient`, the production implementation of `LspClientApi`, which drives a language server through its LSP lifecycle (spawn, initialize, queries, shutdown) and keeps the server's view of files current via didOpen/didChange/didClose.

## Requirements

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

### Requirement: Index readiness wait
After initialization, the system SHALL wait for the language server to become ready before accepting queries, bounded by the language's configured index timeout. If the server has reported its status, readiness SHALL be the server reporting `quiescent: true`. Otherwise the system SHALL poll `workspace/symbol` with an empty query every 500 ms (each request bounded to 5 seconds) until a non-empty result, ending the wait early after 5 consecutive empty or failed polls.

#### Scenario: Server becomes ready within timeout
- **WHEN** a server that has not reported its status returns a non-empty `workspace/symbol` result within the timeout period
- **THEN** the system SHALL proceed to accept queries

#### Scenario: Server fails to become ready
- **WHEN** a server that reports its status does not report `quiescent: true` within the timeout period, or a server that does not report its status neither returns a non-empty `workspace/symbol` result nor reaches 5 consecutive empty or failed polls within the timeout period
- **THEN** the system SHALL log a warning and register the server anyway so that queries can proceed

#### Scenario: Server has no workspace symbol support
- **WHEN** the server capabilities do not include `workspaceSymbolProvider` and the server has not reported its status
- **THEN** the system SHALL skip the readiness wait and accept queries immediately

#### Scenario: Status-reporting server becomes quiescent
- **WHEN** the server has reported `quiescent: false` and later reports `quiescent: true` within the timeout period
- **THEN** the system SHALL end the wait at that notification and proceed to accept queries, regardless of what `workspace/symbol` returns

#### Scenario: Status arrives while polling
- **WHEN** the system is polling `workspace/symbol` because no status had been reported yet, and the server then reports its status
- **THEN** the system SHALL stop polling and wait for `quiescent: true` for the rest of the timeout period, and the 5-poll early exit SHALL NOT end the wait

#### Scenario: Poll early exit is logged
- **WHEN** a server that has not reported its status returns 5 consecutive empty or failed `workspace/symbol` polls
- **THEN** the system SHALL end the wait, log that it ended after empty polls, and proceed to accept queries

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

### Requirement: LSP query methods
The system SHALL provide `goto_definition`, `references`, and `implementations` methods that send the corresponding LSP requests and return parsed results.

#### Scenario: goto_definition returns locations
- **WHEN** `goto_definition` is called with a valid URI and position
- **THEN** the system SHALL send `textDocument/definition` and return a list of `Location` results

#### Scenario: find_references returns locations
- **WHEN** `references` is called with a valid URI and position
- **THEN** the system SHALL send `textDocument/references` with `includeDeclaration: false` and return a list of `Location` results

#### Scenario: find_implementations returns locations
- **WHEN** `implementations` is called with a valid URI and position
- **THEN** the system SHALL send `textDocument/implementation` and return a list of `Location` results

#### Scenario: Query returns empty result
- **WHEN** an LSP query returns `null` or an empty array
- **THEN** the system SHALL return an empty list (not an error)

### Requirement: Call hierarchy protocol methods
The system SHALL provide `prepare_call_hierarchy`, `incoming_calls`, and `outgoing_calls` methods on `LspClient`. These implement the two-step LSP callHierarchy protocol: `prepareCallHierarchy` resolves a position to a `CallHierarchyItem`, then `incomingCalls`/`outgoingCalls` retrieve the call relationships.

#### Scenario: prepareCallHierarchy returns items
- **WHEN** `prepare_call_hierarchy` is called with a valid URI and position
- **THEN** the system SHALL send `textDocument/prepareCallHierarchy` and return the resulting `CallHierarchyItem` list

#### Scenario: prepareCallHierarchy returns null
- **WHEN** `prepare_call_hierarchy` returns `null` or an empty array
- **THEN** the system SHALL return an empty list (not an error)

#### Scenario: incomingCalls returns callers
- **WHEN** `incoming_calls` is called with a `CallHierarchyItem`
- **THEN** the system SHALL send `callHierarchy/incomingCalls` with the item and return `CallHierarchyIncomingCall` results

#### Scenario: outgoingCalls returns callees
- **WHEN** `outgoing_calls` is called with a `CallHierarchyItem`
- **THEN** the system SHALL send `callHierarchy/outgoingCalls` with the item and return `CallHierarchyOutgoingCall` results

#### Scenario: incomingCalls or outgoingCalls returns null
- **WHEN** an incoming/outgoing call query returns `null` or an empty array
- **THEN** the system SHALL return an empty list (not an error)

### Requirement: Call hierarchy capability check
The system SHALL check `ServerCapabilities.call_hierarchy_provider` before sending any callHierarchy requests. If the capability is absent (`None`), the system SHALL return a protocol error indicating the language server does not support call hierarchy.

#### Scenario: Language server supports call hierarchy
- **WHEN** `call_hierarchy_provider` is present in server capabilities
- **THEN** the system SHALL proceed with callHierarchy protocol requests

#### Scenario: Language server does not support call hierarchy
- **WHEN** `call_hierarchy_provider` is `None` in server capabilities
- **THEN** the system SHALL return a `ClientError::Protocol` with message indicating call hierarchy is not supported

### Requirement: Hover query method
The system SHALL provide a `hover` method on `LspClient` that sends `textDocument/hover` with a URI and position, and returns an `Option<Hover>` result.

#### Scenario: Hover returns type information
- **WHEN** `hover` is called with a valid URI and position
- **THEN** the system SHALL send `textDocument/hover` and return `Some(Hover)` with the contents and optional range

#### Scenario: Hover returns null
- **WHEN** `hover` is called and the LSP server returns `null`
- **THEN** the system SHALL return `None` (not an error)

#### Scenario: Hover capability check
- **WHEN** `hover_provider` is `None` in server capabilities
- **THEN** the system SHALL return a `ClientError::Protocol` indicating hover is not supported

### Requirement: Pull diagnostics query method
The system SHALL provide a `diagnostic` method on `LspClient` that sends `textDocument/diagnostic` with a URI when the server supports pull diagnostics, and otherwise returns the cached push diagnostics for that URI (see the push-diagnostics capability), as a list of `Diagnostic` results.

#### Scenario: Diagnostics returns errors and warnings
- **WHEN** `diagnostic` is called with a valid URI and the server supports pull diagnostics
- **THEN** the system SHALL send `textDocument/diagnostic` and return a list of `Diagnostic` entries

#### Scenario: No diagnostics
- **WHEN** `diagnostic` is called and the LSP server returns an empty list
- **THEN** the system SHALL return an empty list (not an error)

#### Scenario: Diagnostics capability check
- **WHEN** `diagnostic_provider` is `None` in server capabilities
- **THEN** the system SHALL return the cached push diagnostics for the URI, or an empty list when nothing is cached, instead of an error

### Requirement: Rename query method
The system SHALL provide a `rename` method on `LspClient` that sends `textDocument/rename` with a URI, position, and new name, and returns an `Option<WorkspaceEdit>` result.

#### Scenario: Rename returns workspace edit
- **WHEN** `rename` is called with a valid URI, position, and new name
- **THEN** the system SHALL send `textDocument/rename` and return `Some(WorkspaceEdit)` with the proposed changes

#### Scenario: Rename returns null
- **WHEN** `rename` is called and the LSP server returns `null` (nothing to rename)
- **THEN** the system SHALL return `None` (not an error)

#### Scenario: Rename capability check
- **WHEN** `rename_provider` is `None` in server capabilities
- **THEN** the system SHALL return a `ClientError::Protocol` indicating rename is not supported

### Requirement: Server status tracking
The system SHALL keep the most recent `experimental/serverStatus` notification received from a language server for the life of that server. The server SHALL be treated as indexing exactly while the most recent notification reports `quiescent: false`. A server that has never sent the notification SHALL be treated as not reporting status, never as indexing. The `health` and `message` fields SHALL NOT affect readiness.

#### Scenario: No status notification received
- **WHEN** the language server has not sent any `experimental/serverStatus` notification
- **THEN** the system SHALL treat the server as not reporting status and not indexing

#### Scenario: Server reports it is not quiescent
- **WHEN** the most recent `experimental/serverStatus` notification has `quiescent: false`
- **THEN** the system SHALL treat the server as indexing

#### Scenario: Server reports it is quiescent
- **WHEN** the most recent `experimental/serverStatus` notification has `quiescent: true`
- **THEN** the system SHALL treat the server as ready

#### Scenario: Server leaves quiescence after startup
- **WHEN** a server that reported `quiescent: true` later reports `quiescent: false` (for example during a workspace reload)
- **THEN** the system SHALL treat the server as indexing again until it reports `quiescent: true`

#### Scenario: Quiescent with degraded health
- **WHEN** a notification reports `quiescent: true` with `health` set to `warning` or `error`
- **THEN** the system SHALL treat the server as ready and log a warning that includes the reported `message`

#### Scenario: Malformed status notification
- **WHEN** an `experimental/serverStatus` notification lacks a boolean `quiescent` field
- **THEN** the system SHALL ignore it and keep the previously recorded status

### Requirement: Optional initialization options
`LspClient::start` SHALL accept optional initialization options from its caller. When options are provided, the `initialize` request SHALL carry them unchanged as `initializationOptions`. When no options are provided, the `initialize` request SHALL omit the `initializationOptions` field entirely rather than sending `null`.

#### Scenario: Options provided
- **WHEN** `LspClient::start` is called with initialization options
- **THEN** the `initialize` request SHALL include an `initializationOptions` field equal to the provided value

#### Scenario: No options provided
- **WHEN** `LspClient::start` is called without initialization options
- **THEN** the `initialize` request SHALL NOT include an `initializationOptions` field
