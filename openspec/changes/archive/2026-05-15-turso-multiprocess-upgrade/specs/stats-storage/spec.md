## MODIFIED Requirements

### Requirement: Open/write/close pattern
The system SHALL hold a shared `Database` handle and create a new `Connection` via `db.connect()` for each operation. The `Database` SHALL be built with `experimental_multiprocess_wal(true)` to enable concurrent access from the MCP server and the `symtrace-mcp stats` CLI.

#### Scenario: Tool call records stats
- **WHEN** a tool call completes and stats are recorded
- **THEN** the system creates a `Connection` from the shared `Database`, inserts a row, and drops the `Connection`

#### Scenario: CLI reads stats concurrently
- **WHEN** `symtrace-mcp stats` is run while the MCP server is running
- **THEN** the CLI can open and read the database concurrently via multiprocess WAL support
