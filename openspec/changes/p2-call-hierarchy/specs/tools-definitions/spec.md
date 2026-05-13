## ADDED Requirements

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

### Requirement: Call hierarchy unsupported error
When the language server does not support call hierarchy (capability absent), the MCP tool SHALL return an MCP error with code `-32603` (Internal Error) and a message indicating the language server does not support call hierarchy.

#### Scenario: Language server lacks callHierarchy support
- **WHEN** `incoming_calls` or `outgoing_calls` is called and the language server's capabilities do not include `call_hierarchy_provider`
- **THEN** the system SHALL return an MCP error with code `-32603` and message indicating call hierarchy is not supported by this language server
