# Tasks

## 1. Configurable index timeout

- [x] 1.1 Add `index_timeout_secs: u64` with `#[serde(default = "default_index_timeout")]` (returns 30) to `ServerConfig` in `src/config.rs`; add unit tests `load_server_with_index_timeout` (value 180 parsed), `load_server_index_timeout_default` (omitted → 30) and `load_server_index_timeout_negative` (parse error); verify with `cargo test config::`
- [x] 1.2 Add `index_timeout_secs: u64` to `LanguageServerConfig` in `src/server/manager.rs`, set 30 for Rust and TypeScript in `default_configs()`, extend `typescript_default_config` and add a Rust default test asserting 30; verify with `cargo test server::manager`
- [x] 1.3 In `src/project/registry.rs`, set 30 in both entries of `default_server_configs()` and copy `cfg.index_timeout_secs` in `build_server_configs`; add a registry test that a `[server.rust]` entry with `index_timeout_secs = 180` reaches the Rust `LanguageServerConfig` and that the TypeScript default stays 30; verify with `cargo test project::registry`
- [x] 1.4 Document `index_timeout_secs` (default 30, `0` skips the wait, raise it for large Rust workspaces) in the README "Multi-Project Support" section (TOML example and the `[server.rust]` / `[server.typescript]` bullets) and in the project `CLAUDE.md` Configuration example; verify both TOML snippets parse by pasting them into a `config::tests` case or by inspection against the 1.1 tests
- [x] 1.5 Add `### Added` under `## [Unreleased]` in `CHANGELOG.md` (create the heading if absent) with an entry for `index_timeout_secs`; verify the section renders in order Added/Fixed per Keep a Changelog

## 2. Server status tracking

- [x] 2.1 Add `ServerStatusParams { health: String, quiescent: bool, message: Option<String> }` (`Deserialize`, camelCase) to `src/lsp/types.rs` with tests for the rust-analyzer payload with `message: null`, an unknown `health` value, and a missing `quiescent` (error); verify with `cargo test lsp::types`
- [x] 2.2 Add `pub enum IndexState { NotReported, Indexing, Ready }` and a `watch` channel to `LspClient` in `src/lsp/client.rs`; move the notification handling out of the spawned task in `LspClient::start` into a function that takes `(method, params)`, the diagnostics cache and the `watch::Sender`, so tests can call it directly; handle `experimental/serverStatus` per design D1 (state from `quiescent` only, `send_if_modified`, debug log on change, warning with `message` when `health` is not `ok` and changed, ignore malformed params); expose `LspClient::index_state() -> watch::Receiver<IndexState>`; verify `cargo build`
- [x] 2.3 Add unit tests in `src/lsp/client.rs` that call the extracted notification function: initial state `NotReported`; `quiescent: false` → `Indexing`; then `true` → `Ready`; `Ready` then `false` → `Indexing`; `health: "warning"` with `quiescent: true` → `Ready`; params without `quiescent` keep the previous state; `publishDiagnostics` still fills the cache; verify with `cargo test lsp::client`

## 3. Rust server status opt-in

- [x] 3.1 Add `"experimental": { "serverStatusNotification": true }` to `client_capabilities()` in `src/language/rust.rs` and a `#[cfg(test)]` test asserting it; add a test in `src/language/typescript.rs` asserting `experimental.serverStatusNotification` is absent; verify with `cargo test language::`; if `ts-call-hierarchy` has already landed, rebase and keep both changes to the JSON literal

## 4. Status-gated index wait

- [x] 4.1 Rewrite `LspClient::wait_for_index` per design D2: extract the loop into a free async function that takes the `watch::Receiver<IndexState>`, whether `workspaceSymbolProvider` is enabled, a poll closure returning `Result<bool, ClientError>`, and the timeout; status present → `wait_for(Ready)` with the remaining timeout; no status and no provider → `Ok`; otherwise poll every 500 ms (5 s per request) with both the request and the inter-poll sleep raced against `changed()`, and read the state with `borrow_and_update()` at the top of each iteration so a status processed before the wait started is seen at once, return `Ok` on non-empty, and after 5 empty/failed polls re-check the state before logging at info and returning `Ok`; `Err(IndexTimeout)` on timeout; update the doc comment; verify `cargo build`
- [x] 4.2 Add `#[tokio::test(start_paused = true)]` tests for the extracted function with fake poll closures and a `watch::Sender`: status `Indexing` then `Ready` before the timeout → `Ok` without polling; `Indexing` never becoming `Ready` → `Err(IndexTimeout)` even though polls return empty 5+ times; no status and 5 empty polls → `Ok` (TypeScript path unchanged); status arriving after 2 empty polls → waits for `Ready`; no status, no provider → `Ok` immediately; non-empty poll → `Ok`; verify with `cargo test lsp::client`

## 5. Manager wiring

- [x] 5.1 Add `index_state: watch::Receiver<IndexState>` to `ServerEntry` in `src/server/manager.rs`; in `start_server_internal` take `client.index_state()` before boxing, skip `wait_for_index` when `cfg.index_timeout_secs == 0`, otherwise use `Duration::from_secs(cfg.index_timeout_secs)`, and make the timeout warning include the language and the timeout; update `make_entry` in the manager tests to build a `watch` channel (returning the `Sender` where a test needs it); verify `cargo test server::manager`

## 6. "Still indexing" tool error

- [x] 6.1 Add `ToolError::indexing()` to `src/mcp/tools.rs` (code `protocol::INTERNAL_ERROR`, message `language server is still indexing the workspace; retry in a few seconds`) and `reject_empty_while_indexing(before, after, empty)` to `src/mcp/handlers.rs`; unit-test the helper: empty with either sample `Indexing` → error with code -32603 and the message prefix; non-empty while `Indexing` → `Ok`; empty with `NotReported` or `Ready` on both samples → `Ok`; verify with `cargo test mcp::handlers`
- [x] 6.2 Call the helper in `execute_query`, `execute_call_hierarchy` (after `prepare_call_hierarchy` before the "No callable symbol" return, and after `incoming_calls` / `outgoing_calls`), `hover`, `rename` and `diagnostics`, sampling `entry.index_state` before the first LSP request and after the response (design D4); verify `cargo build` and that `grep -n reject_empty_while_indexing src/mcp/handlers.rs` shows 7 call sites
- [x] 6.3 Add manager-level tests in the existing "Handler query dispatch" group of `src/server/manager.rs`: a `make_entry` with `MockLspClientApi` returning an empty `references` / `None` `hover` and the state set to `Indexing` yields the indexing error through the helper, and the same with `Ready` yields the normal empty result; verify with `cargo test server::manager`
- [x] 6.4 In `CHANGELOG.md` under `## [Unreleased]`, add `### Fixed` for #29 (first queries on a large Rust workspace no longer return empty results before rust-analyzer finishes loading; an explicit "still indexing" error is returned instead of an empty answer) and `### Changed` noting that the `stats` startup time for rust-analyzer now measures time to readiness; verify the order is Added, Changed, Fixed below the 1.5 entry

## 7. Integration checks

- [x] 7.0 Raise the `wait_for_ready` budget in `tests/integration_rust.rs` from 30 s to 60 s (above the default `index_timeout_secs`), because the harness gives up without retrying when one probe outlasts its budget and the first probe now blocks inside the server for up to the index timeout; verify with 7.3
- [x] 7.1 Add a Rust integration test in `tests/integration_rust.rs` that spawns a fresh server, sends `find_implementations` (or `find_references`) on the fixture before `wait_for_ready`, and asserts the reply is either a non-empty result or a JSON-RPC error whose message starts with `language server is still indexing`, never "No ... found"; verify it with 7.3
- [x] 7.2 Run `cargo clippy --all-targets --tests -- -D warnings`, `cargo fmt --all -- --check` and `cargo test`; all pass
- [x] 7.3 Run `cargo test --features integration-rust -- --test-threads=1`; all pass
- [x] 7.4 Run `cargo test --features integration-typescript -- --test-threads=1`; all pass, confirming the TypeScript wait is unchanged
- [ ] 7.5 Manual cold-start check on a large multi-crate Rust workspace (one that takes well over 30 s to load) with no `.symtrace.toml` override: the first `find_implementations` either returns results or the indexing error, never an empty list; with `index_timeout_secs` raised above the load time, the first call returns correct results; the log shows the index-wait warning only in the first case and `symtrace-mcp stats` shows a startup time close to the load time. Record the outcome (without naming the workspace) in the PR description
