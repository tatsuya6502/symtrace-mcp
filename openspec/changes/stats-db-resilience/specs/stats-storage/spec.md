## MODIFIED Requirements

### Requirement: Open/write/close pattern
The system SHALL store the database path at construction and defer opening the Turso database until the first operation requires it. When opened, the `Database` SHALL be built with `experimental_multiprocess_wal(true)`. The database SHALL be closed 15 seconds after the last write operation using a debounce pattern (each write cancels and reschedules the close timer). The system SHALL use `Arc<Mutex<Option<Database>>>` for thread-safe lazy access. A new `Connection` via `db.connect()` SHALL be created for each operation.

#### Scenario: First tool call opens database lazily
- **WHEN** a tool call occurs and the database has not been opened yet
- **THEN** the system opens the database with `experimental_multiprocess_wal(true)`, ensures the schema, and writes the tool call record

#### Scenario: Tool call records stats with open database
- **WHEN** a tool call completes, the database is open, and stats are recorded
- **THEN** the system creates a `Connection` from the `Database`, inserts a row, and drops the `Connection`

#### Scenario: Database closes after idle timeout
- **WHEN** no write operation has occurred for 15 seconds and the database is open
- **THEN** the system drops the `Database` handle, closing the connection and releasing the file lock

#### Scenario: CLI reads stats concurrently
- **WHEN** `symtrace-mcp stats` is run while the MCP server is running
- **THEN** the CLI can open and read the database concurrently via multiprocess WAL support

## REMOVED Requirements

### Requirement: Data retention with 30-day rolling window
**Reason**: Replaced by deferred retention cleanup in lazy-stats-db capability. The cleanup logic itself is unchanged, but the timing is now managed by the lazy DB lifecycle instead of running eagerly at startup.
**Migration**: Retention cleanup now runs on first DB open and periodically every 24 hours (only when DB is open). See lazy-stats-db spec.
