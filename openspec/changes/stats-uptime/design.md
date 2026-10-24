# Design

## Context

See proposal.md (Why). Current state in code:

- `ServerEntry` (`src/server/manager.rs`) holds only `client` and `file_manager`; nothing remembers when a server started.
- `start_server_internal` already captures `let start = Instant::now()` before `LspClient::start` and uses it for the `started` row's startup duration.
- Three sites record `stopped` with `duration_ms = None`: `stop_server` ("manual"), `shutdown_all` ("session_end"), and `IdleMonitor::check_and_shutdown_idle` ("idle_timeout").
- `IdleMonitor::check_and_shutdown_idle` calls `manager.stop_server(language)`, which records "manual", then records its own "idle_timeout" row. It records that second row even when `stop_server` found no running server or returned an error.
- `stop_server` propagates a `client.shutdown()` error with `?` before recording, so a failed shutdown records no row although the entry has already been removed from the map. `shutdown_all` ignores the error and records.
- The tokio `Mutex` on `servers` is already held across `close_all` + `shutdown`; this change does not alter locking.
- `query_server_usage` (`src/stats/recorder.rs`) already sums `duration_ms` of `stopped` rows; it needs no change.

## Goals / Non-Goals

**Goals:**
- One recording site for `stopped` rows, inside the manager, so the duration and the reason cannot diverge between paths.
- Keep the diff small: `ts-call-hierarchy` (PR #33) and `ra-index-wait` also touch `ServerEntry`, `make_entry` and `CHANGELOG.md` `[Unreleased]`.

**Non-Goals:**
- The 300 s idle-timeout fallback in `idle_monitor.rs` (Issue #27).
- Stats DB schema changes, backfilling old rows, or counting still-running servers in uptime.
- Any change to the `symtrace-mcp stats` output format.

## Decisions

### D1. Uptime is measured from process spawn

`ServerEntry` gains `started_at: std::time::Instant`, set from the existing `start` Instant in `start_server_internal`. Every stop path computes `duration_ms = entry.started_at.elapsed().as_millis() as u64` and passes `Some(duration_ms)`.

- Chosen: from spawn. Uptime then means "how long a language server process existed", which is what the resource cost (memory, CPU while indexing) depends on. The value includes the index wait that is also reported as startup duration.
- Considered: from ready (after `wait_for_index`). It excludes startup time from uptime, but it would need a second `Instant` and makes uptime disagree with the process's real lifetime. Rejected.
- `Instant` (monotonic) rather than `SystemTime`, so wall-clock jumps do not produce negative or inflated values.

### D2. `StopReason` enum and a single recording site in the manager

Add in `manager.rs`:

```rust
pub(crate) enum StopReason { IdleTimeout, SessionEnd }
impl StopReason { fn as_str(&self) -> &'static str { /* "idle_timeout" | "session_end" */ } }
```

- `pub(crate) async fn stop_server_with_reason(&self, language, reason: StopReason)` does the remove → `close_all` → `shutdown` → record sequence.
- The public `stop_server(language)` and the `Manual` reason are removed. `stop_server` had no caller outside tests (the idle monitor was its only production caller and now uses `stop_server_with_reason`), so `manual` described an unreachable path. Keeping it would have needed `#[cfg_attr(not(test), expect(dead_code))]`, which turns into a build error the day a real caller appears, so the dead path is deleted instead. A future explicit-stop feature can add a reason back.
- `shutdown_all` records with `StopReason::SessionEnd` through a shared private helper that records one row from an entry's `started_at` and a reason. Both methods use it so the row format lives in one place.
- The `stopped` row is recorded before the shutdown error is propagated: once the entry is removed, the server is gone from the manager's point of view, so the stop is counted either way, and the error is still returned to the caller.
- No entry for the language → no row (matches today's `stop_server`; fixes the idle monitor's unconditional row).
- Considered: a `&str` reason parameter. Rejected: the enum makes the spec'd values the only possible ones.

### D3. `IdleMonitor` no longer records stats

`check_and_shutdown_idle` calls `manager.stop_server_with_reason(language, StopReason::IdleTimeout)` and drops its own `record_server_event` block. Its `stats: Arc<StatsRecorder>` field, the `IdleMonitor::new(stats)` parameter, and the `crate::stats::StatsRecorder` import become unused and are removed; `with_configs` calls `IdleMonitor::new()`. `IdleMonitor::new` has no other callers.

- Considered: keep the monitor recording and add a "don't record" flag to `stop_server`. Rejected: two recording sites again, and the monitor has no access to `started_at`.

## Risks / Trade-offs

- [Rows written before the upgrade still have `duration_ms = NULL`] → They contribute 0, so uptime ramps up over the 7-day window. Acceptable; noted in the CHANGELOG entry.
- [A server that crashes or is killed without going through a stop path records no `stopped` row] → Same as today; out of scope.
- [Rebase conflicts with PR #33 / `ra-index-wait` on `ServerEntry` and `make_entry`] → Add `started_at` as the last field and keep the `make_entry` change to one line.
