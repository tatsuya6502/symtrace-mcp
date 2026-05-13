## MODIFIED Requirements

### Requirement: MCP tool registry supports dynamic registration
The tool system SHALL provide a registry where tool handlers can be registered by name. Each handler accepts parameters as `serde_json::Value` and returns a result as `serde_json::Value`. Tool handlers SHALL receive `Arc<ProjectRegistry>` instead of `Arc<LanguageServerManager>` for file routing.

#### Scenario: Register a tool
- **WHEN** a tool handler is registered with name "find_references"
- **THEN** it appears in `tools/list` responses and is callable via `tools/call`, routing through the project registry to find the correct manager

### Requirement: MCP server runs on tokio runtime
The server SHALL use tokio as its async runtime. stdin/stdout I/O SHALL be non-blocking. The server SHALL spawn an idle monitor task for each project manager in the registry.

#### Scenario: Server startup with multiple projects
- **WHEN** the binary is run with a `.symtrace.toml` configuring two projects
- **THEN** it initializes a tokio runtime, builds the project registry, and spawns one idle monitor task per project manager

#### Scenario: Server startup without config
- **WHEN** the binary is run without a `.symtrace.toml`
- **THEN** it initializes with an implicit single-project registry rooted at CWD, spawning one idle monitor task — behavior identical to the pre-config single-project mode
