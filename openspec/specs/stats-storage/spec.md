# stats-storage Specification

## Purpose
Persists usage statistics in a per-project Turso database at `.symtrace/stats.db` through a shared `Database` handle with multiprocess WAL, so the MCP server and the stats CLI can access it concurrently, and prunes rows older than 30 days.

## Requirements

### Requirement: Stats database location
The system SHALL store usage data in a Turso database at `<project_root>/.symtrace/stats.db`. The directory `.symtrace/` SHALL be created automatically if it does not exist. Both apply when stats recording is enabled.

#### Scenario: First tool call in a new project
- **WHEN** a tool call occurs and `.symtrace/stats.db` does not exist
- **THEN** the system creates `.symtrace/` directory and `stats.db` with the required schema

#### Scenario: Existing database
- **WHEN** a tool call occurs and `.symtrace/stats.db` already exists
- **THEN** the system opens the existing database and writes to it

#### Scenario: Stats recording disabled
- **WHEN** stats recording is disabled because the database could not be opened at startup, and a tool call occurs
- **THEN** the system creates no `.symtrace/` directory or `stats.db` and writes nothing

### Requirement: Tool calls table schema
The system SHALL maintain a `tool_calls` table with columns: `id` (INTEGER PRIMARY KEY), `timestamp` (DATETIME NOT NULL), `tool` (TEXT NOT NULL), `file_path` (TEXT), `duration_ms` (INTEGER NOT NULL), `success` (BOOLEAN NOT NULL), `error_msg` (TEXT). The table SHALL have indexes on `timestamp` and `(tool)`.

#### Scenario: Table created on first use
- **WHEN** the database is newly created
- **THEN** the `tool_calls` table and its indexes exist

### Requirement: Server events table schema
The system SHALL maintain a `server_events` table with columns: `id` (INTEGER PRIMARY KEY), `timestamp` (DATETIME NOT NULL), `language` (TEXT NOT NULL), `event` (TEXT NOT NULL, one of: "started", "stopped", "startup_failed"), `duration_ms` (INTEGER), `detail` (TEXT). The table SHALL have an index on `timestamp`.

#### Scenario: Table created on first use
- **WHEN** the database is newly created
- **THEN** the `server_events` table and its index exist

### Requirement: Shared database handle with multiprocess WAL
The system SHALL hold a shared `Database` handle and create a new `Connection` via `db.connect()` for each operation. The `Database` SHALL be built with `experimental_multiprocess_wal(true)` to enable concurrent access from the MCP server and the `symtrace-mcp stats` CLI.

#### Scenario: Tool call records stats
- **WHEN** a tool call completes and stats are recorded
- **THEN** the system creates a `Connection` from the shared `Database`, inserts a row, and drops the `Connection`

#### Scenario: CLI reads stats concurrently
- **WHEN** `symtrace-mcp stats` is run while the MCP server is running
- **THEN** the CLI can open and read the database concurrently via multiprocess WAL support

### Requirement: Data retention with 30-day rolling window
The system SHALL delete rows older than 30 days from both tables. Deletion SHALL occur on MCP server startup and periodically every 24 hours during a session. This applies when stats recording is enabled.

#### Scenario: Old data is cleaned on startup
- **WHEN** the MCP server starts
- **THEN** rows with `timestamp` older than 30 days are deleted from `tool_calls` and `server_events`

#### Scenario: Periodic cleanup during session
- **WHEN** 24 hours have elapsed since the last cleanup
- **THEN** the system deletes rows older than 30 days

#### Scenario: Stats recording disabled
- **WHEN** stats recording is disabled because the database could not be opened at startup
- **THEN** no cleanup runs against any database, on startup or every 24 hours

### Requirement: Non-fatal database initialization
If the stats database cannot be opened or initialized when the MCP server starts, the system SHALL continue with stats recording disabled, and the MCP server SHALL start and serve all tools normally. When a log subscriber is active, the system SHALL emit one warning event via structured logging (`warn!` macro) that includes the cause and the database path. While disabled, stats writes and retention cleanup SHALL be no-ops that report success.

#### Scenario: Database locked by another process
- **WHEN** the MCP server starts in a project directory whose `.symtrace/stats.db` is locked by another symtrace-mcp process
- **THEN** the system logs a warning containing the locking error and the database path, does not exit, and responds to `initialize`, `tools/list` and `tools/call` requests normally

#### Scenario: Stats directory cannot be created
- **WHEN** the MCP server starts and `<project_root>/.symtrace/` cannot be created or written (for example, a regular file named `.symtrace` exists)
- **THEN** the system does not exit and serves tools normally with stats recording disabled; the stats warning may be lost because the default log directory shares the same parent and logging initialization fails first, in which case the logging initialization's own stderr message points at the cause

#### Scenario: Writes while disabled
- **WHEN** stats recording is disabled and a tool call, a server event, or a retention cleanup would write to the database
- **THEN** no database access occurs and the operation reports success

#### Scenario: Database opens normally
- **WHEN** the MCP server starts and `.symtrace/stats.db` can be opened
- **THEN** no warning is logged and stats are recorded and pruned as before
