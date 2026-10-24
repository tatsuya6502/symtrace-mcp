## ADDED Requirements

### Requirement: Uptime reflects recorded stop durations
The "total uptime" figure in the "Language Servers" section SHALL be the sum of `duration_ms` over that language's `stopped` events in the reporting window. A server that is still running SHALL contribute nothing until it stops; `stopped` events without a duration SHALL contribute zero. Each `stopped` duration SHALL be measured from the moment the language server process was spawned, so uptime includes the startup/index wait that `avg startup` also reports.

#### Scenario: Uptime summed from stopped events
- **WHEN** the window contains two `stopped` events for `rust` with durations of 1 hour and 30 minutes
- **THEN** the `rust` line reports `uptime 1h 30m total`

#### Scenario: Server still running
- **WHEN** the window contains a `started` event for `rust` and no `stopped` event
- **THEN** the `rust` line reports `uptime 0s total`
