## Requirements

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

### Requirement: incoming_calls tool
The system SHALL register an MCP tool named `incoming_calls` that accepts `file_path` (string, required), `line` (integer, required, 1-based), `column` (integer, required, 1-based), `depth` (integer, optional, default 1), and `json` (boolean, optional, default false). It SHALL use the callHierarchy protocol to return callers of the symbol at the given position.

#### Scenario: Successful incoming_calls with text output
- **WHEN** `incoming_calls` is called with a valid file path and position containing a callable symbol
- **THEN** the system SHALL return human-readable text listing each caller as `← file:line:col  symbol_name()`, followed by a summary line `(N callers)`

#### Scenario: Successful incoming_calls with JSON output
- **WHEN** `incoming_calls` is called with `json: true`
- **THEN** the system SHALL return a JSON array of `{ name, file_path, line, column, line_text }` objects

#### Scenario: No callers found
- **WHEN** `incoming_calls` returns an empty result (no callers for the symbol)
- **THEN** the system SHALL return "No callers found"

#### Scenario: Position has no callable symbol
- **WHEN** `prepareCallHierarchy` returns an empty result for the given position
- **THEN** the system SHALL return "No callable symbol at this position"

### Requirement: outgoing_calls tool
The system SHALL register an MCP tool named `outgoing_calls` that accepts `file_path` (string, required), `line` (integer, required, 1-based), `column` (integer, required, 1-based), `depth` (integer, optional, default 1), and `json` (boolean, optional, default false). It SHALL use the callHierarchy protocol to return callees from the symbol at the given position.

#### Scenario: Successful outgoing_calls with text output
- **WHEN** `outgoing_calls` is called with a valid file path and position containing a callable symbol
- **THEN** the system SHALL return human-readable text listing each callee as `→ file:line:col  symbol_name()`, followed by a summary line `(N callees)`

#### Scenario: Successful outgoing_calls with JSON output
- **WHEN** `outgoing_calls` is called with `json: true`
- **THEN** the system SHALL return a JSON array of `{ name, file_path, line, column, line_text }` objects

#### Scenario: No callees found
- **WHEN** `outgoing_calls` returns an empty result (no callees from the symbol)
- **THEN** the system SHALL return "No callees found"

#### Scenario: Position has no callable symbol
- **WHEN** `prepareCallHierarchy` returns an empty result for the given position
- **THEN** the system SHALL return "No callable symbol at this position"

### Requirement: depth parameter validation
The `depth` parameter SHALL accept only the value `1`. When a value other than `1` is provided, the system SHALL return an MCP error with code `-32602` (Invalid Params) and a message indicating only depth 1 is supported.

#### Scenario: depth is 1 (default)
- **WHEN** `incoming_calls` or `outgoing_calls` is called with `depth: 1` or without `depth`
- **THEN** the system SHALL proceed normally

#### Scenario: depth is greater than 1
- **WHEN** `incoming_calls` or `outgoing_calls` is called with `depth > 1`
- **THEN** the system SHALL return an MCP error with code `-32602` and message "Only depth 1 is supported"

### Requirement: Tool error handling
All tools SHALL return MCP errors (with appropriate error codes) when the tool call fails due to invalid input, unsupported language, or language server errors. Tool dispatch SHALL be instrumented to record call metrics (duration, success/error) to the stats database without affecting the tool response.

#### Scenario: Invalid file path
- **WHEN** a tool is called with a file path that does not exist on disk
- **THEN** the system SHALL return an MCP error with code `-32602` (Invalid Params)

#### Scenario: Language server error
- **WHEN** the language server returns an error response for a tool's LSP request
- **THEN** the system SHALL return an MCP error with the LSP error code and message

#### Scenario: Language server not available
- **WHEN** the language server crashes or becomes unresponsive during a tool call
- **THEN** the system SHALL return an MCP error indicating the server is unavailable

#### Scenario: Tool call recorded on success
- **WHEN** a tool handler returns a successful result
- **THEN** the dispatch layer records a `tool_calls` row with `success: true` and the measured duration

#### Scenario: Tool call recorded on error
- **WHEN** a tool handler returns an error
- **THEN** the dispatch layer records a `tool_calls` row with `success: false` and the error message

### Requirement: Call hierarchy unsupported error
When the language server does not support call hierarchy (capability absent), the MCP tool SHALL return an MCP error with code `-32603` (Internal Error) and a message indicating the language server does not support call hierarchy.

#### Scenario: Language server lacks callHierarchy support
- **WHEN** `incoming_calls` or `outgoing_calls` is called and the language server's capabilities do not include `call_hierarchy_provider`
- **THEN** the system SHALL return an MCP error with code `-32603` and message indicating call hierarchy is not supported by this language server

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
