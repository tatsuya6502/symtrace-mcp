## MODIFIED Requirements

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
