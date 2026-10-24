## REMOVED Requirements

### Requirement: Language server shutdown instrumentation
**Reason**: superseded by "Language server stop instrumentation"; the explicit `stop_server` entry point had no caller outside tests, so the "manual" reason described an unreachable path.

**Migration**: no user action; existing `stopped` rows with `detail = "manual"` keep their meaning in `symtrace-mcp stats` (they are summed like any other stop).

## ADDED Requirements

### Requirement: Language server stop instrumentation
The system SHALL record exactly one `server_events` row each time a running language server is stopped. The row SHALL contain: `language`, `event: "stopped"`, `duration_ms` (time elapsed since the server process was spawned), and `detail` indicating the reason ("idle_timeout" or "session_end"). Stop events SHALL be logged via structured logging (`info!` for idle timeout, `warn!` for errors) rather than `eprintln!`.

#### Scenario: Idle timeout shutdown
- **WHEN** `IdleMonitor` shuts down a server due to idle timeout
- **THEN** a row is inserted with `event: "stopped"`, `detail: "idle_timeout"`, and `duration_ms` set to the time since that server was spawned

#### Scenario: Session end shutdown
- **WHEN** `shutdown_all` stops servers during MCP server shutdown
- **THEN** a row is inserted for each running server with `event: "stopped"`, `detail: "session_end"`, and `duration_ms` set to the time since that server was spawned

#### Scenario: Idle timeout records a single row
- **WHEN** `IdleMonitor` shuts down a server due to idle timeout
- **THEN** exactly one `stopped` row is inserted for that stop, with `detail: "idle_timeout"`

#### Scenario: Stop of a server that is not running
- **WHEN** a stop is requested for a language whose server is not running
- **THEN** no `stopped` row is inserted

#### Scenario: LSP shutdown request fails
- **WHEN** a running server is stopped and its LSP shutdown request returns an error
- **THEN** a `stopped` row with the duration and reason is still inserted
