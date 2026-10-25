# Tasks

## 1. Per-language initialization options

- [x] 1.1 Add `pub fn initialization_options() -> Option<Value>` to `src/language/typescript.rs` returning `Some(json!({"tsserver": {"useSyntaxServer": "never"}}))`, with a doc comment explaining the syntax-server routing it disables (Issue #30); add a unit test asserting `opts["tsserver"]["useSyntaxServer"] == "never"`; verify with `cargo test language::typescript`
- [x] 1.2 Add `pub fn initialization_options() -> Option<Value>` to `src/language/rust.rs` returning `None`, with a doc comment noting that rust-analyzer treats `initializationOptions` as its configuration; add a unit test asserting it is `None`; verify with `cargo test language::rust`

## 2. Plumb the options into `initialize`

- [x] 2.1 Add an `initialization_options: Option<Value>` parameter to `LspClient::start` (`src/lsp/client.rs`) and insert `"initializationOptions"` into the `initialize` params only when it is `Some` (no `null` for `None`); factor the params construction into a small private function (e.g. `build_initialize_params`) and unit-test both cases: `Some` → field present and equal, `None` → field absent; verify with `cargo test lsp::client`
- [x] 2.2 In `LanguageServerManager::start_server_internal` (`src/server/manager.rs`) select `rust::initialization_options()` / `typescript::initialization_options()` with the same `match language` as the capabilities and pass the result to `LspClient::start`; this is the only caller and no mock constructs `LspClient` via `start`, so no test doubles change; verify with `cargo build` and `cargo test`
- [x] 2.3 Check startup time with the syntax server disabled: run symtrace-mcp on `fixtures/ts-project` with debug logging, issue one TypeScript tool call, and confirm the startup `wait_for_index` still ends after the 5 failed/empty `workspace/symbol` polls within a few seconds (not the 30 s timeout); record the observed duration in the PR description

## 3. First-call integration tests

- [x] 3.1 In `tests/integration_ts.rs` add a `spawn_cold_client()` helper that calls `common::require_command("typescript-language-server")` and `McpClient::spawn(&cwd)` on `fixtures/ts-project` WITHOUT `wait_for_ready`, with a comment explaining why (the readiness poll's own `goto_definition` would open a file and move later queries out of the project-loading window); keep `spawn_client()` and all existing tests unchanged
- [x] 3.2 Add `test_goto_definition_first_call_cross_file`: on a fresh `spawn_cold_client()`, the FIRST `tools/call` is `goto_definition` on `fixture_caller()` line 4, column 10 with `json: true`; assert the first result's `file_path` ends with `index.ts` and its `line` is 21
- [x] 3.3 Add `test_find_references_first_call_cross_file`: on a fresh `spawn_cold_client()`, the FIRST `tools/call` is `find_references` on the same position with `json: true`; assert at least 5 results, at least one whose `file_path` ends with `index.ts`, and at least one whose `file_path` ends with `caller.ts`
- [x] 3.4 Confirm the new tests catch the bug: temporarily make `typescript::initialization_options()` return `None`, run `cargo test --features integration-typescript first_call -- --test-threads=1` and see both new tests fail (import line / 2 same-file references), then restore the option and see them pass; do not commit the temporary change

## 4. Changelog and final verification

- [x] 4.1 Add to `CHANGELOG.md` under `## [Unreleased]` → `### Fixed` an entry stating that `goto_definition`, `find_references`, `hover` and `rename` now return project-wide results on the first query of a TypeScript project instead of file-local ones, because symtrace-mcp disables typescript-language-server's syntax-only server; mention that the first query now waits for the project to load (#30)
- [x] 4.2 Run `cargo clippy --all-targets --tests -- -D warnings`, `cargo fmt --all -- --check` and `cargo test`; all pass
- [x] 4.3 As the final step, run `cargo test --features integration-typescript -- --test-threads=1` with typescript-language-server 5.2.0 / TypeScript 5.8.3; all TypeScript integration tests, old and new, pass

## Workflow follow-up

- Open a PR with "Fixes #30" in the description; after merge, archive the change. Note: the unarchived `ts-call-hierarchy` change also touches `integration-tests` and `server-manager` (via a MODIFIED and an ADDED requirement respectively); this change only ADDs requirements, so the archive order does not matter.
