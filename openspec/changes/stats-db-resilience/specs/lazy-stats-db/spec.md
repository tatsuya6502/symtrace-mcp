## ADDED Requirements

### Requirement: Lazy database connection
The `StatsRecorder` SHALL NOT open the Turso database at construction time. The database SHALL be opened on the first write or read operation that requires it. The `project_root` path SHALL be stored at construction time for deferred opening.

#### Scenario: First tool call opens the database
- **WHEN** a tool call completes and `record_tool_call` is invoked for the first time
- **THEN** the system opens the Turso database, runs schema migration, and inserts the tool call record

#### Scenario: StatsRecorder construction without database
- **WHEN** `StatsRecorder::new()` is called
- **THEN** the system stores the database path but does NOT open the database or acquire any file lock

#### Scenario: Subsequent tool calls reuse open database
- **WHEN** a tool call completes and the database is already open
- **THEN** the system uses the existing database connection without re-opening

### Requirement: Idle timeout with debounce close pattern
The `StatsRecorder` SHALL close the database connection 15 seconds after the last write operation. Each write operation SHALL cancel any pending close timer and schedule a new one. When the database is closed, the file lock SHALL be released.

#### Scenario: Database closes after 15 seconds of inactivity
- **WHEN** a tool call record is written and no further writes occur for 15 seconds
- **THEN** the system closes the database connection and releases the file lock

#### Scenario: Close timer resets on each write
- **WHEN** a tool call record is written at t=0 and another at t=10s
- **THEN** the close timer is cancelled at t=10s and rescheduled to fire at t=25s

#### Scenario: Database reopens after idle close
- **WHEN** the database was closed due to idle timeout and a new tool call occurs
- **THEN** the system reopens the database, runs schema migration if needed, and writes the record

### Requirement: Non-fatal database initialization
If `StatsRecorder::new()` fails (e.g., database path error, permissions), the system SHALL log a warning via structured logging (`warn!` macro) and continue without stats. The MCP server SHALL remain fully functional for all tool operations. Subsequent stats operations SHALL be silently skipped.

#### Scenario: Database locked by another process at startup
- **WHEN** `StatsRecorder::new()` fails due to a database lock held by a zombie process
- **THEN** the system logs a warning via structured logging (`warn!` macro), creates no StatsRecorder, and the MCP server starts normally

#### Scenario: Tool call with no stats recorder
- **WHEN** a tool call completes and the StatsRecorder is `None`
- **THEN** the tool response is returned normally; no stats are recorded

### Requirement: Deferred retention cleanup
The system SHALL NOT run retention cleanup at MCP server startup. Retention cleanup SHALL run after the database is first opened (lazily) and every 24 hours thereafter, but only when the database is currently open.

#### Scenario: No cleanup at startup
- **WHEN** the MCP server starts
- **THEN** no database access or retention cleanup occurs

#### Scenario: Cleanup runs on first database open
- **WHEN** the database is opened for the first time (on first tool call)
- **THEN** the system runs retention cleanup (deleting rows older than 30 days) after schema migration

#### Scenario: Periodic cleanup skips when database is closed
- **WHEN** the 24-hour periodic cleanup timer fires and the database is closed
- **THEN** the cleanup is skipped; no database access occurs

#### Scenario: Periodic cleanup runs when database is open
- **WHEN** the 24-hour periodic cleanup timer fires and the database is open
- **THEN** the system runs retention cleanup normally
