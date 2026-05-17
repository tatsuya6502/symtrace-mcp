## ADDED Requirements

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
The system SHALL provide a `diagnostic` method on `LspClient` that sends `textDocument/diagnostic` with a URI, and returns a list of `Diagnostic` results.

#### Scenario: Diagnostics returns errors and warnings
- **WHEN** `diagnostic` is called with a valid URI
- **THEN** the system SHALL send `textDocument/diagnostic` and return a list of `Diagnostic` entries

#### Scenario: No diagnostics
- **WHEN** `diagnostic` is called and the LSP server returns an empty list
- **THEN** the system SHALL return an empty list (not an error)

#### Scenario: Diagnostics capability check
- **WHEN** `diagnostic_provider` is `None` in server capabilities
- **THEN** the system SHALL return a `ClientError::Protocol` indicating pull diagnostics is not supported

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
