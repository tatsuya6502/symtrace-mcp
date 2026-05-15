## MODIFIED Requirements

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
