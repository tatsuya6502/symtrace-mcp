## Requirements

### Requirement: Stats database location
The system SHALL store usage data in a Turso database at `<project_root>/.symtrace/stats.db`. The directory `.symtrace/` SHALL be created automatically if it does not exist.

#### Scenario: First tool call in a new project
- **WHEN** a tool call occurs and `.symtrace/stats.db` does not exist
- **THEN** the system creates `.symtrace/` directory and `stats.db` with the required schema

#### Scenario: Existing database
- **WHEN** a tool call occurs and `.symtrace/stats.db` already exists
- **THEN** the system opens the existing database and writes to it

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

### Requirement: Open/write/close pattern
The system SHALL hold a shared `Database` handle and create a new `Connection` via `db.connect()` for each operation. The `Database` SHALL be built with `experimental_multiprocess_wal(true)` to enable concurrent access from the MCP server and the `symtrace-mcp stats` CLI.

#### Scenario: Tool call records stats
- **WHEN** a tool call completes and stats are recorded
- **THEN** the system creates a `Connection` from the shared `Database`, inserts a row, and drops the `Connection`

#### Scenario: CLI reads stats concurrently
- **WHEN** `symtrace-mcp stats` is run while the MCP server is running
- **THEN** the CLI can open and read the database concurrently via multiprocess WAL support

### Requirement: Data retention with 30-day rolling window
The system SHALL delete rows older than 30 days from both tables. Deletion SHALL occur on MCP server startup and periodically every 24 hours during a session.

#### Scenario: Old data is cleaned on startup
- **WHEN** the MCP server starts
- **THEN** rows with `timestamp` older than 30 days are deleted from `tool_calls` and `server_events`

#### Scenario: Periodic cleanup during session
- **WHEN** 24 hours have elapsed since the last cleanup
- **THEN** the system deletes rows older than 30 days
