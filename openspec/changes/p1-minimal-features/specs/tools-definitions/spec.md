## ADDED Requirements

### Requirement: find_references tool
The system SHALL register an MCP tool named `find_references` that accepts `file_path` (string, required), `line` (integer, required, 1-based), `column` (integer, required, 1-based), and `json` (boolean, optional, default false). It SHALL dispatch to `textDocument/references` via the language server.

#### Scenario: Successful find_references with text output
- **WHEN** `find_references` is called with a valid file path and position
- **THEN** the system SHALL return human-readable text listing each reference as `file:line:col  line_text`, followed by a summary line `(N references in M files)`

#### Scenario: Successful find_references with JSON output
- **WHEN** `find_references` is called with `json: true`
- **THEN** the system SHALL return a JSON array of `{ file_path, line, column, line_text }` objects

#### Scenario: No references found
- **WHEN** `find_references` returns an empty result
- **THEN** the system SHALL return "No references found"

### Requirement: goto_definition tool
The system SHALL register an MCP tool named `goto_definition` that accepts `file_path` (string, required), `line` (integer, required, 1-based), `column` (integer, required, 1-based), and `json` (boolean, optional, default false). It SHALL dispatch to `textDocument/definition` via the language server.

#### Scenario: Successful goto_definition with text output
- **WHEN** `goto_definition` is called with a valid file path and position
- **THEN** the system SHALL return human-readable text listing each definition location as `file:line:col  line_text`

#### Scenario: Successful goto_definition with JSON output
- **WHEN** `goto_definition` is called with `json: true`
- **THEN** the system SHALL return a JSON array of `{ file_path, line, column, line_text }` objects

#### Scenario: No definition found
- **WHEN** `goto_definition` returns an empty result
- **THEN** the system SHALL return "No definition found"

### Requirement: find_implementations tool
The system SHALL register an MCP tool named `find_implementations` that accepts `file_path` (string, required), `line` (integer, required, 1-based), `column` (integer, required, 1-based), and `json` (boolean, optional, default false). It SHALL dispatch to `textDocument/implementation` via the language server.

#### Scenario: Successful find_implementations with text output
- **WHEN** `find_implementations` is called with a valid file path and position
- **THEN** the system SHALL return human-readable text listing each implementation location as `file:line:col  line_text`

#### Scenario: Successful find_implementations with JSON output
- **WHEN** `find_implementations` is called with `json: true`
- **THEN** the system SHALL return a JSON array of `{ file_path, line, column, line_text }` objects

#### Scenario: No implementations found
- **WHEN** `find_implementations` returns an empty result
- **THEN** the system SHALL return "No implementations found"

### Requirement: Tool error handling
All tools SHALL return MCP errors (with appropriate error codes) when the tool call fails due to invalid input, unsupported language, or language server errors.

#### Scenario: Invalid file path
- **WHEN** a tool is called with a file path that does not exist on disk
- **THEN** the system SHALL return an MCP error with code `-32602` (Invalid Params)

#### Scenario: Language server error
- **WHEN** the language server returns an error response for a tool's LSP request
- **THEN** the system SHALL return an MCP error with the LSP error code and message

#### Scenario: Language server not available
- **WHEN** the language server crashes or becomes unresponsive during a tool call
- **THEN** the system SHALL return an MCP error indicating the server is unavailable
