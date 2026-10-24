## ADDED Requirements

### Requirement: Recording while stats are disabled
When stats recording is disabled because the database could not be opened at startup, tool-call and language-server event instrumentation SHALL NOT affect tool responses or server lifecycle, and SHALL NOT log a warning per recording attempt. The only diagnostic SHALL be the single startup warning.

#### Scenario: Tool call with stats disabled
- **WHEN** stats recording is disabled and `handle_tools_call` dispatches a tool call
- **THEN** the tool response is returned to the caller as usual, no `tool_calls` row is written, and no "stats recording failed" warning is logged

#### Scenario: Server start and stop with stats disabled
- **WHEN** stats recording is disabled and a language server starts, fails to start, or stops
- **THEN** the server lifecycle proceeds as usual, no `server_events` row is written, and no warning about stats recording is logged

#### Scenario: Retention cleanup with stats disabled
- **WHEN** stats recording is disabled and the startup or periodic retention cleanup runs
- **THEN** it completes without database access and no "stats retention cleanup failed" warning is logged
