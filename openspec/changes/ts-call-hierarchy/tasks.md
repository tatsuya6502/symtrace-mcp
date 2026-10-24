# Tasks

## 1. Client capabilities

- [ ] 1.1 In `src/language/typescript.rs` `client_capabilities()`, add `"callHierarchy": { "dynamicRegistration": false }` under `textDocument`; add a unit test next to `no_pull_diagnostics_capability` asserting `textDocument.callHierarchy.dynamicRegistration == false`, and verify with `cargo test language::typescript`
- [ ] 1.2 In `src/language/rust.rs` `client_capabilities()`, add the same `"callHierarchy": { "dynamicRegistration": false }` entry under `textDocument`; create a `#[cfg(test)] mod tests` (the file has none today) with a test asserting `textDocument.callHierarchy.dynamicRegistration == false` and that `textDocument.diagnostic` is still present, and verify with `cargo test language::rust`

## 2. TypeScript integration tests

- [ ] 2.1 In `tests/integration_ts.rs`, delete the comment claiming typescript-language-server does not support call hierarchy (currently lines 181-182, after `test_find_implementations`), and verify with `grep -n "does not support the LSP Call Hierarchy" tests/integration_ts.rs` returning nothing
- [ ] 2.2 Add `test_incoming_calls` to `tests/integration_ts.rs`, mirroring `test_incoming_calls` in `tests/integration_rust.rs`: call `incoming_calls` on `greetEntity` in `fixtures/ts-project/index.ts` (line 21, col 10, `json: true`) and assert `!extract_results(&result).is_empty()`; verify with `cargo test --features integration-typescript --test integration_ts test_incoming_calls -- --test-threads=1`
- [ ] 2.3 Add `test_outgoing_calls` to `tests/integration_ts.rs`: call `outgoing_calls` on `greetEntity` (line 21, col 10, `json: true`) and assert at least one callee; if the result is empty, append (do not insert above existing code) a named function to the end of `fixtures/ts-project/index.ts` that calls `greetEntity` and `new User(...)`, point the test at it, and verify with `cargo test --features integration-typescript --test integration_ts test_outgoing_calls -- --test-threads=1`

## 3. Changelog

- [ ] 3.1 Under `## [Unreleased]` in `CHANGELOG.md`, add a `### Fixed` entry: `incoming_calls` and `outgoing_calls` now work on TypeScript/JavaScript projects; symtrace-mcp now declares the `textDocument.callHierarchy` client capability, without which typescript-language-server did not advertise call hierarchy (#28). Verify the section renders under `[Unreleased]` and not under `[0.6.0]`

## 4. Verification

- [ ] 4.1 Run `cargo clippy --all-targets --tests -- -D warnings` and verify it reports no warnings
- [ ] 4.2 Run `cargo fmt --all -- --check` and verify it reports no diffs
- [ ] 4.3 Run `cargo test` and verify all unit tests pass
- [ ] 4.4 Manually verify on a real multi-package workspace with a TypeScript frontend that `incoming_calls` on an exported hook imported by other files returns its callers in those files instead of the "does not support call hierarchy" error (record the result in the PR without naming the workspace)
- [ ] 4.5 Run `cargo test --features integration-rust -- --test-threads=1` and verify the Rust call hierarchy tests still pass with the added client capability
- [ ] 4.6 Run `cargo test --features integration-typescript -- --test-threads=1` as the last step and verify all TypeScript integration tests pass, including `test_incoming_calls` and `test_outgoing_calls`
