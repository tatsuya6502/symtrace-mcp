# Proposal

## Why

`symtrace-mcp stats` always prints `uptime 0s total` in its "Language Servers" section (Issue #25). The query sums `duration_ms` of `stopped` events, but every code path that records a `stopped` event stores `NULL` there, so the uptime figure has never worked. While tracing this, a second defect turned up: each idle-timeout shutdown records two `stopped` rows, one of them with the wrong reason (`manual`).

## What Changes

- Every `stopped` server event records `duration_ms` = time elapsed since the language server process was spawned, so the existing uptime query produces real figures.
- Each stop records exactly one `stopped` row. Idle-timeout shutdowns record a single row with detail `idle_timeout` instead of a `manual` row followed by an `idle_timeout` row.
- A `stopped` row is recorded whenever a running server is removed, including when its LSP `shutdown` request fails on the manual-stop path (today that path returns before recording anything).
- No change to the stats DB schema or to the `symtrace-mcp stats` output format.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `stats-instrumentation`: the "Language server shutdown instrumentation" requirement now requires `duration_ms` on `stopped` rows and exactly one row per stop.
- `stats-cli`: adds a requirement that defines the "total uptime" figure as the sum of recorded stop durations in the reporting window.

## Impact

- Code: `src/server/manager.rs` (`ServerEntry`, `start_server_internal`, `stop_server`, `shutdown_all`, tests), `src/server/idle_monitor.rs` (stop call, removal of its own stats recording and `StatsRecorder` handle), `src/stats/recorder.rs` (tests only).
- Data: existing `stopped` rows keep `duration_ms = NULL` and still contribute 0; uptime becomes accurate only for stops recorded after upgrade. Duplicate `manual` rows already written by past idle shutdowns stay in the DB until the 30-day retention cleanup removes them.
- Docs: `CHANGELOG.md` `[Unreleased]` / `Fixed`.
- No API, dependency, or configuration changes.
