# Tasks

## 1. Record the server start time

- [x] 1.1 Add `started_at: std::time::Instant` as the last field of `ServerEntry` in `src/server/manager.rs` and set it from the existing `start` Instant in `start_server_internal`; verify with `cargo build`
- [x] 1.2 Update the test helper `make_entry` in `manager.rs` to set `started_at: Instant::now()`; verify with `cargo test server::manager`

## 2. Stop reason and single recording site

- [x] 2.1 Add `pub(crate) enum StopReason { Manual, IdleTimeout, SessionEnd }` with `as_str()` returning `"manual"`, `"idle_timeout"`, `"session_end"`; verify with a unit test asserting the three strings
- [x] 2.2 Add a private helper that records one `stopped` row with `Some(entry.started_at.elapsed().as_millis() as u64)` and `reason.as_str()`, logging failures with `tracing::warn!`; verify with `cargo build`
- [x] 2.3 Add `pub(crate) async fn stop_server_with_reason(language, reason)` that removes the entry, runs `close_all` + `shutdown`, records the row via the helper before propagating any shutdown error, and records nothing when no entry exists; make `stop_server` delegate with `StopReason::Manual`; verify with `cargo build`
- [x] 2.4 Switch `shutdown_all` to the helper with `StopReason::SessionEnd`; verify with `cargo build`

## 3. Idle monitor

- [x] 3.1 In `src/server/idle_monitor.rs`, call `manager.stop_server_with_reason(language, StopReason::IdleTimeout)` and delete the monitor's own `record_server_event` block; verify with `cargo build`
- [x] 3.2 Remove the `stats` field, the `IdleMonitor::new` parameter and the `StatsRecorder` import, and change `with_configs` to `IdleMonitor::new()`; verify `cargo clippy --all-targets --tests -- -D warnings` reports no unused items

## 4. Tests

- [x] 4.1 Add manager tests that insert an entry with `started_at` backdated (e.g. `Instant::now() - Duration::from_millis(50)`) into `manager.servers`, then set `mock.expect_shutdown().returning(|| Ok(()))` (the trait already carries `#[cfg_attr(test, mockall::automock)]`), call `stop_server`, `stop_server_with_reason(.., IdleTimeout)` and `shutdown_all`; using `test_stats()`, assert exactly one `stopped` row per stop with the matching detail and `duration_ms >= 50`; verify with `cargo test server::manager`
- [x] 4.2 Add a manager test that `stop_server_with_reason` for a language with no running server inserts no row, and one where `shutdown` returns an error (`expect_shutdown().returning(|| Err(..))`) still inserts the `stopped` row and returns the error; verify with `cargo test server::manager`
- [x] 4.3 Add a recorder test in `src/stats/recorder.rs` that inserts `started` and `stopped` rows (including one `stopped` row with `NULL` duration) and asserts `query_server_usage` returns `total_uptime_secs` equal to the sum of the non-null durations; verify with `cargo test stats::recorder`

## 5. Changelog and verification

- [x] 5.1 Add to `CHANGELOG.md` `## [Unreleased]` a `### Fixed` section with two entries: `symtrace-mcp stats` uptime always showed `0s` because stop events carried no duration (#25; only stops recorded after upgrading count), and idle-timeout shutdowns recorded an extra `manual` stop event; verify the section renders under `[Unreleased]`
- [x] 5.2 Run `cargo clippy --all-targets --tests -- -D warnings`, `cargo fmt --all -- --check` and `cargo test`; verify all pass (integration tests are not needed for this change)
