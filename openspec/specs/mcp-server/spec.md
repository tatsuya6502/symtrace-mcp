## MODIFIED Requirements

### Requirement: MCP tool registry supports dynamic registration
The tool system SHALL provide a registry where tool handlers can be registered by name. Each handler accepts parameters as `serde_json::Value` and returns a result as `serde_json::Value`. Tool handlers SHALL receive `Arc<ProjectRegistry>` instead of `Arc<LanguageServerManager>` for file routing.

#### Scenario: Register a tool
- **WHEN** a tool handler is registered with name "find_references"
- **THEN** it appears in `tools/list` responses and is callable via `tools/call`, routing through the project registry to find the correct manager

### Requirement: MCP server runs on tokio runtime
The server SHALL use tokio as its async runtime. stdin/stdout I/O SHALL be non-blocking. The binary SHALL support subcommand dispatch via clap: running with no subcommand starts the MCP server; `symtrace-mcp stats` runs the stats CLI.

#### Scenario: Server startup
- **WHEN** the binary is run with no subcommand
- **THEN** it initializes a tokio runtime and starts listening on stdin for MCP messages

#### Scenario: Stats subcommand
- **WHEN** the binary is run as `symtrace-mcp stats`
- **THEN** it prints a stats summary to stdout and exits
