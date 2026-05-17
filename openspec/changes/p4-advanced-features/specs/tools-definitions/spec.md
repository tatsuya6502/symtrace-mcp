## ADDED Requirements

### Requirement: hover tool
The system SHALL register an MCP tool named `hover` that accepts `file_path` (string, required), `line` (integer, required, 1-based), `column` (integer, required, 1-based), and `json` (boolean, optional, default false). It SHALL dispatch to `textDocument/hover` via the language server.

#### Scenario: Successful hover with text output
- **WHEN** `hover` is called with a valid file path and position where a symbol exists
- **THEN** the system SHALL return human-readable text with the hover contents (type info, documentation, signature)

#### Scenario: Successful hover with JSON output
- **WHEN** `hover` is called with `json: true`
- **THEN** the system SHALL return a JSON object with `contents` (raw LSP value) and optional `range`

#### Scenario: No hover information
- **WHEN** `hover` returns `None` for the given position
- **THEN** the system SHALL return "No hover information available"

#### Scenario: Hover not supported
- **WHEN** the language server does not support hover (`hover_provider` is `None`)
- **THEN** the system SHALL return an MCP error with code `-32603` indicating hover is not supported

### Requirement: diagnostics tool
The system SHALL register an MCP tool named `diagnostics` that accepts `file_path` (string, required) and `json` (boolean, optional, default false). It SHALL dispatch to `textDocument/diagnostic` via the language server.

#### Scenario: Successful diagnostics with text output
- **WHEN** `diagnostics` is called with a valid file path that has errors or warnings
- **THEN** the system SHALL return human-readable text listing each diagnostic as `line:col [severity] message`, followed by a summary line `(N diagnostics)`

#### Scenario: Successful diagnostics with JSON output
- **WHEN** `diagnostics` is called with `json: true`
- **THEN** the system SHALL return a JSON array of `{ file_path, line, column, severity, code, source, message }` objects

#### Scenario: No diagnostics
- **WHEN** `diagnostics` returns an empty list for the given file
- **THEN** the system SHALL return "No diagnostics found"

#### Scenario: Pull diagnostics not supported
- **WHEN** the language server does not support pull diagnostics (`diagnostic_provider` is `None`)
- **THEN** the system SHALL return an MCP error with code `-32603` indicating pull diagnostics is not supported

### Requirement: rename tool
The system SHALL register an MCP tool named `rename` that accepts `file_path` (string, required), `line` (integer, required, 1-based), `column` (integer, required, 1-based), `new_name` (string, required), and `json` (boolean, optional, default false). It SHALL dispatch to `textDocument/rename` via the language server and return a preview of changes WITHOUT applying them.

#### Scenario: Successful rename preview with text output
- **WHEN** `rename` is called with a valid file path, position, and new name
- **THEN** the system SHALL return human-readable text listing each proposed change as `file:line:col  old_text → new_text`, followed by a summary line `(N changes in M files)`

#### Scenario: Successful rename preview with JSON output
- **WHEN** `rename` is called with `json: true`
- **THEN** the system SHALL return a JSON object with `changes` mapping file paths to arrays of `{ line, column, old_text, new_text }`

#### Scenario: Nothing to rename
- **WHEN** `rename` returns `None` for the given position and new name
- **THEN** the system SHALL return "No rename changes"

#### Scenario: Rename not supported
- **WHEN** the language server does not support rename (`rename_provider` is `None`)
- **THEN** the system SHALL return an MCP error with code `-32603` indicating rename is not supported
