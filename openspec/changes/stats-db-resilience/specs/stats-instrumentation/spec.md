## MODIFIED Requirements

### Requirement: Stats recorder concurrency
The `StatsRecorder` SHALL be held behind `Arc<Mutex<Option<turso::Database>>>`. The mutex SHALL be held only during connection acquisition (opening the database if needed, or getting a `Connection` from an existing `Database`). The actual async INSERT/DELETE operations SHALL execute after releasing the mutex. The `StatsRecorder` itself SHALL be wrapped in `Arc<Option<StatsRecorder>>` in the MCP server, allowing graceful degradation when the database is unavailable.

#### Scenario: Concurrent tool calls
- **WHEN** two tool calls arrive concurrently
- **THEN** their stats recordings proceed concurrently; the mutex is held only for connection acquisition (~microseconds), and the async writes execute without the mutex

#### Scenario: Stats recorder is None
- **WHEN** the MCP server started without a StatsRecorder (DB initialization failed)
- **THEN** all tool operations proceed normally; stats recording is silently skipped

#### Scenario: Database open race with close timer
- **WHEN** a write operation and the idle close timer attempt to access the database simultaneously
- **THEN** the mutex serializes access; the write completes first, and the close timer rechecks `last_access` before closing
