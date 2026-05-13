## Requirements

### Requirement: LspClient lifecycle management
The system SHALL provide an `LspClient` that manages the full LSP server lifecycle: spawn the child process, send `initialize` with client capabilities, receive server capabilities, send `initialized` notification, and handle `shutdown` + `exit` for clean termination.

#### Scenario: Successful language server startup
- **WHEN** `LspClient::start` is called with a valid language server command and project root
- **THEN** the system SHALL spawn the child process, send `initialize`, receive `InitializeResult`, send `initialized`, and store the server capabilities

#### Scenario: Language server startup failure
- **WHEN** the language server process fails to start (command not found, permission denied)
- **THEN** the system SHALL return an error without hanging

#### Scenario: Clean shutdown
- **WHEN** `LspClient::shutdown` is called on a running client
- **THEN** the system SHALL send `shutdown`, wait for the response, send `exit`, and reap the child process

### Requirement: Index readiness wait
After initialization, the system SHALL wait for the language server to finish indexing before responding to queries. It SHALL poll `textDocument/documentSymbol` until a non-empty result is returned, with a configurable timeout.

#### Scenario: Server becomes ready within timeout
- **WHEN** the language server returns a non-empty `documentSymbol` result within the timeout period
- **THEN** the system SHALL proceed to accept queries

#### Scenario: Server fails to become ready
- **WHEN** the language server does not return a non-empty `documentSymbol` result within the timeout period
- **THEN** the system SHALL return an error indicating the server failed to initialize in time

### Requirement: File management via didOpen/didChange/didClose
The system SHALL send `textDocument/didOpen` before querying a file, `textDocument/didChange` when the file has been modified since last open (detected by mtime), and `textDocument/didClose` when the file is no longer needed.

#### Scenario: Opening a new file
- **WHEN** a query targets a file that is not currently open
- **THEN** the system SHALL read the file from disk, send `textDocument/didOpen` with the full content and language identifier, and track the file as open with its version and mtime

#### Scenario: Re-opening a modified file
- **WHEN** a query targets a file that is open but has been modified on disk (mtime changed)
- **THEN** the system SHALL read the updated content, send `textDocument/didChange` with the new content and incremented version, and update the tracked mtime

#### Scenario: Closing a file
- **WHEN** `close_file` is called for an open file
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
