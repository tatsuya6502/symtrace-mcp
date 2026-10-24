## ADDED Requirements

### Requirement: Configurable index wait timeout
When starting a language server, the system SHALL bound the index readiness wait by that language's configured `index_timeout_secs` (default 30 seconds). A value of `0` SHALL skip the wait. After a timeout or a skipped wait the system SHALL register the server and serve tool calls.

#### Scenario: Default index timeout
- **WHEN** a language server is started and its configuration does not set `index_timeout_secs`
- **THEN** the system SHALL wait at most 30 seconds for the server to become ready

#### Scenario: Configured index timeout
- **WHEN** a Rust language server is started with `index_timeout_secs = 180`
- **THEN** the system SHALL wait at most 180 seconds for the server to become ready

#### Scenario: Index wait times out
- **WHEN** the server does not become ready within its index timeout
- **THEN** the system SHALL log a warning that names the language and the timeout, register the server, and execute the pending tool call

#### Scenario: Index wait disabled
- **WHEN** a language server is started with `index_timeout_secs = 0`
- **THEN** the system SHALL register the server without waiting for readiness

### Requirement: Rust server status opt-in
The system SHALL declare the client capability `experimental.serverStatusNotification: true` in the `initialize` request sent to the Rust language server, so that rust-analyzer reports its indexing status. The TypeScript client capabilities SHALL NOT declare it.

#### Scenario: Rust initialization requests server status
- **WHEN** a Rust language server is started
- **THEN** the `initialize` request SHALL include `capabilities.experimental.serverStatusNotification` set to `true`

#### Scenario: TypeScript initialization does not request server status
- **WHEN** a TypeScript language server is started
- **THEN** the `initialize` request SHALL NOT include `capabilities.experimental.serverStatusNotification`
