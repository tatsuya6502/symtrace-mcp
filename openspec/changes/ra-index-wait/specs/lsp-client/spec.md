## MODIFIED Requirements

### Requirement: Index readiness wait
After initialization, the system SHALL wait for the language server to become ready before accepting queries, bounded by the language's configured index timeout. If the server has reported its status, readiness SHALL be the server reporting `quiescent: true`. Otherwise the system SHALL poll `workspace/symbol` with an empty query every 500 ms (each request bounded to 5 seconds) until a non-empty result, ending the wait early after 5 consecutive empty or failed polls.

#### Scenario: Server becomes ready within timeout
- **WHEN** a server that has not reported its status returns a non-empty `workspace/symbol` result within the timeout period
- **THEN** the system SHALL proceed to accept queries

#### Scenario: Server fails to become ready
- **WHEN** a server that reports its status does not report `quiescent: true` within the timeout period, or a server that does not report its status neither returns a non-empty `workspace/symbol` result nor reaches 5 consecutive empty or failed polls within the timeout period
- **THEN** the system SHALL log a warning and register the server anyway so that queries can proceed

#### Scenario: Server has no workspace symbol support
- **WHEN** the server capabilities do not include `workspaceSymbolProvider` and the server has not reported its status
- **THEN** the system SHALL skip the readiness wait and accept queries immediately

#### Scenario: Status-reporting server becomes quiescent
- **WHEN** the server has reported `quiescent: false` and later reports `quiescent: true` within the timeout period
- **THEN** the system SHALL end the wait at that notification and proceed to accept queries, regardless of what `workspace/symbol` returns

#### Scenario: Status arrives while polling
- **WHEN** the system is polling `workspace/symbol` because no status had been reported yet, and the server then reports its status
- **THEN** the system SHALL stop polling and wait for `quiescent: true` for the rest of the timeout period, and the 5-poll early exit SHALL NOT end the wait

#### Scenario: Poll early exit is logged
- **WHEN** a server that has not reported its status returns 5 consecutive empty or failed `workspace/symbol` polls
- **THEN** the system SHALL end the wait, log that it ended after empty polls, and proceed to accept queries

## ADDED Requirements

### Requirement: Server status tracking
The system SHALL keep the most recent `experimental/serverStatus` notification received from a language server for the life of that server. The server SHALL be treated as indexing exactly while the most recent notification reports `quiescent: false`. A server that has never sent the notification SHALL be treated as not reporting status, never as indexing. The `health` and `message` fields SHALL NOT affect readiness.

#### Scenario: No status notification received
- **WHEN** the language server has not sent any `experimental/serverStatus` notification
- **THEN** the system SHALL treat the server as not reporting status and not indexing

#### Scenario: Server reports it is not quiescent
- **WHEN** the most recent `experimental/serverStatus` notification has `quiescent: false`
- **THEN** the system SHALL treat the server as indexing

#### Scenario: Server reports it is quiescent
- **WHEN** the most recent `experimental/serverStatus` notification has `quiescent: true`
- **THEN** the system SHALL treat the server as ready

#### Scenario: Server leaves quiescence after startup
- **WHEN** a server that reported `quiescent: true` later reports `quiescent: false` (for example during a workspace reload)
- **THEN** the system SHALL treat the server as indexing again until it reports `quiescent: true`

#### Scenario: Quiescent with degraded health
- **WHEN** a notification reports `quiescent: true` with `health` set to `warning` or `error`
- **THEN** the system SHALL treat the server as ready and log a warning that includes the reported `message`

#### Scenario: Malformed status notification
- **WHEN** an `experimental/serverStatus` notification lacks a boolean `quiescent` field
- **THEN** the system SHALL ignore it and keep the previously recorded status
