## ADDED Requirements

### Requirement: MCP server reads JSON-RPC over stdio
The MCP server SHALL read JSON-RPC 2.0 messages from stdin using Content-Length framing (identical to LSP transport framing).

#### Scenario: Receive an MCP request
- **WHEN** a JSON-RPC request arrives on stdin with Content-Length framing
- **THEN** the server parses it into a structured request with method, params, and id

### Requirement: MCP server writes JSON-RPC responses to stdout
The MCP server SHALL write JSON-RPC 2.0 responses to stdout using Content-Length framing.

#### Scenario: Send an MCP response
- **WHEN** the server produces a response
- **THEN** it writes `Content-Length: <N>\r\n\r\n<json>` to stdout

### Requirement: MCP server handles initialize request
The server SHALL respond to `initialize` requests with server capabilities including protocol version and tool list support.

#### Scenario: Client sends initialize
- **WHEN** an MCP `initialize` request arrives
- **THEN** the server responds with capabilities including `tools` capability

### Requirement: MCP server handles tools/list request
The server SHALL respond to `tools/list` requests with a list of available tools. In P0 the list is empty; tools are added in P1.

#### Scenario: Client requests tool list
- **WHEN** a `tools/list` request arrives
- **THEN** the server responds with `{ "tools": [] }`

### Requirement: MCP server handles tools/call with dispatch
The server SHALL handle `tools/call` requests by looking up the tool name and dispatching to a handler function. Unknown tools SHALL return an error response.

#### Scenario: Call a known tool
- **WHEN** a `tools/call` request arrives with a known tool name
- **THEN** the server dispatches to the registered handler and returns its result

#### Scenario: Call an unknown tool
- **WHEN** a `tools/call` request arrives with an unrecognized tool name
- **THEN** the server returns a JSON-RPC error with code -32601 (Method not found)

### Requirement: MCP server returns JSON-RPC errors for malformed requests
The server SHALL return standard JSON-RPC error codes for invalid messages: -32700 (Parse error), -32600 (Invalid Request), -32601 (Method not found).

#### Scenario: Malformed JSON
- **WHEN** the server receives a message that is not valid JSON
- **THEN** it responds with error code -32700

#### Scenario: Missing required field
- **WHEN** a request is missing the `method` field
- **THEN** it responds with error code -32600

### Requirement: MCP tool registry supports dynamic registration
The tool system SHALL provide a registry where tool handlers can be registered by name. Each handler accepts parameters as `serde_json::Value` and returns a result as `serde_json::Value`.

#### Scenario: Register a tool
- **WHEN** a tool handler is registered with name "find_references"
- **THEN** it appears in `tools/list` responses and is callable via `tools/call`

### Requirement: MCP server runs on tokio runtime
The server SHALL use tokio as its async runtime. stdin/stdout I/O SHALL be non-blocking.

#### Scenario: Server startup
- **WHEN** the binary is run
- **THEN** it initializes a tokio runtime and starts listening on stdin for MCP messages
