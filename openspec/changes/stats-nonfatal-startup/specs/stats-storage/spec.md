## ADDED Requirements

### Requirement: Non-fatal database initialization
If the stats database cannot be opened or initialized when the MCP server starts, the system SHALL log one warning via structured logging (`warn!` macro) that includes the cause and the database path, and SHALL continue with stats recording disabled. The MCP server SHALL start and serve all tools normally. While disabled, stats writes and retention cleanup SHALL be no-ops that report success.

#### Scenario: Database locked by another process
- **WHEN** the MCP server starts in a project directory whose `.symtrace/stats.db` is locked by another symtrace-mcp process
- **THEN** the system logs a warning containing the locking error and the database path, does not exit, and responds to `initialize`, `tools/list` and `tools/call` requests normally

#### Scenario: Stats directory cannot be created
- **WHEN** the MCP server starts and `<project_root>/.symtrace/` cannot be created or written (for example, a regular file named `.symtrace` exists)
- **THEN** the system logs a warning with the cause and the database path, does not exit, and serves tools normally with stats recording disabled

#### Scenario: Writes while disabled
- **WHEN** stats recording is disabled and a tool call, a server event, or a retention cleanup would write to the database
- **THEN** no database access occurs and the operation reports success

#### Scenario: Database opens normally
- **WHEN** the MCP server starts and `.symtrace/stats.db` can be opened
- **THEN** no warning is logged and stats are recorded and pruned as before
