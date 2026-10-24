## MODIFIED Requirements

### Requirement: Language server shutdown instrumentation
The system SHALL record exactly one `server_events` row each time a running language server is stopped. The row SHALL contain: `language`, `event: "stopped"`, `duration_ms` (time elapsed since the server process was spawned), and `detail` indicating the reason ("idle_timeout", "session_end", or "manual"). Shutdown events SHALL be logged via structured logging (`info!` for idle timeout, `warn!` for errors) rather than `eprintln!`.

#### Scenario: Idle timeout shutdown
- **WHEN** `IdleMonitor` shuts down a server due to idle timeout
- **THEN** a row is inserted with `event: "stopped"`, `detail: "idle_timeout"`, and `duration_ms` set to the time since that server was spawned

#### Scenario: Session end shutdown
- **WHEN** `shutdown_all` stops servers during MCP server shutdown
- **THEN** a row is inserted for each running server with `event: "stopped"`, `detail: "session_end"`, and `duration_ms` set to the time since that server was spawned

#### Scenario: Explicit stop_server call
- **WHEN** `stop_server` is called directly
- **THEN** a row is inserted with `event: "stopped"`, `detail: "manual"`, and `duration_ms` set to the time since that server was spawned

#### Scenario: Idle timeout records a single row
- **WHEN** `IdleMonitor` shuts down a server due to idle timeout
- **THEN** exactly one `stopped` row is inserted for that stop, with `detail: "idle_timeout"`, and no row with `detail: "manual"` is inserted

#### Scenario: Stop of a server that is not running
- **WHEN** a stop is requested for a language whose server is not running
- **THEN** no `stopped` row is inserted

#### Scenario: LSP shutdown request fails
- **WHEN** a running server is stopped and its LSP shutdown request returns an error
- **THEN** a `stopped` row with the duration and reason is still inserted
