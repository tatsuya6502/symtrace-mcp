## REMOVED Requirements

### Requirement: Stats recorder serialization
**Reason**: With turso 0.6.0 multiprocess WAL and `Database`-only approach, concurrent database access is safe by design. The `Database` type is `Clone + Send + Sync` and handles connection multiplexing internally. `Arc<Mutex<StatsRecorder>>` serialization is no longer needed.
**Migration**: Replace `Arc<Mutex<StatsRecorder>>` with `Arc<StatsRecorder>` at all call sites.
