# Tasks

## 1. Client capabilities

- [x] 1.1 In `src/language/typescript.rs` `client_capabilities()`, add `"callHierarchy": { "dynamicRegistration": false }` under `textDocument`; add a unit test next to `no_pull_diagnostics_capability` asserting `textDocument.callHierarchy.dynamicRegistration == false`, and verify with `cargo test language::typescript`
- [x] 1.2 In `src/language/rust.rs` `client_capabilities()`, add the same `"callHierarchy": { "dynamicRegistration": false }` entry under `textDocument`; create a `#[cfg(test)] mod tests` (the file has none today) with a test asserting `textDocument.callHierarchy.dynamicRegistration == false` and that `textDocument.diagnostic` is still present, and verify with `cargo test language::rust`

## 2. TypeScript fixture and integration tests

- [x] 2.0 Extend `fixtures/ts-project/`: append `export { greetEntity, User };` as the last line of `index.ts` (do not modify lines 1-29); create `caller.ts` with `import { greetEntity, User } from "./index";` and `export function useGreeting(): string { return greetEntity(new User("Bob", 25)); }` (function on line 3, name starting at col 17); add `"caller.ts"` to `include` in `tsconfig.json`; verify with `cd fixtures/ts-project && npx -p typescript@5.8.3 tsc --noEmit` (or the locally installed `tsc`) reporting no errors, and `git diff fixtures/ts-project/index.ts` showing only one added line

- [x] 2.1 In `tests/integration_ts.rs`, delete the comment claiming typescript-language-server does not support call hierarchy (currently lines 181-182, after `test_find_implementations`), and verify with `grep -n "does not support the LSP Call Hierarchy" tests/integration_ts.rs` returning nothing
- [x] 2.2 Add `test_incoming_calls_cross_file` to `tests/integration_ts.rs`, mirroring `test_incoming_calls` in `tests/integration_rust.rs`: call `incoming_calls` on `greetEntity` in `fixtures/ts-project/index.ts` (line 21, col 10, `json: true`), assert the results are non-empty and that at least one caller's file path/URI ends with `caller.ts`; verify with `cargo test --features integration-typescript --test integration_ts test_incoming_calls_cross_file -- --test-threads=1`
- [x] 2.3 Add `test_outgoing_calls` to `tests/integration_ts.rs`: add a helper for the absolute path of `fixtures/ts-project/caller.ts` next to the existing `index.ts` helper, call `outgoing_calls` on `useGreeting` (`caller.ts` line 3, col 17, `json: true`) and assert at least one callee; verify with `cargo test --features integration-typescript --test integration_ts test_outgoing_calls -- --test-threads=1`

## 3. Changelog

- [x] 3.1 Under `## [Unreleased]` in `CHANGELOG.md`, add a `### Fixed` entry: `incoming_calls` and `outgoing_calls` now work on TypeScript/JavaScript projects; symtrace-mcp now declares the `textDocument.callHierarchy` client capability, without which typescript-language-server did not advertise call hierarchy (#28). Verify the section renders under `[Unreleased]` and not under `[0.6.0]`

## 4. Verification

- [x] 4.1 Run `cargo clippy --all-targets --tests -- -D warnings` and verify it reports no warnings
- [x] 4.2 Run `cargo fmt --all -- --check` and verify it reports no diffs
- [x] 4.3 Run `cargo test` and verify all unit tests pass
- [ ] 4.4 Sanity-check on a real multi-package workspace with a TypeScript frontend that `incoming_calls` on an exported hook imported by other files returns its callers (record the result in the PR without naming the workspace); CI now covers the cross-file case via 2.2, so this is a release check, not the only coverage
- [x] 4.5 Run `cargo test --features integration-rust -- --test-threads=1` and verify the Rust call hierarchy tests still pass with the added client capability
- [x] 4.6 Run `cargo test --features integration-typescript -- --test-threads=1` as the last step and verify all TypeScript integration tests pass, including the existing position-based tests (unchanged `index.ts` lines 1-29), `test_incoming_calls_cross_file` and `test_outgoing_calls`
