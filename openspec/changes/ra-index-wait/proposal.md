# Proposal

## Why

On a large Rust workspace the first `find_implementations`, `incoming_calls` and `hover` calls return empty results, and the same calls about 30 seconds later are correct (Issue #29). The index wait polls `workspace/symbol("")` and gives up after 5 empty polls. rust-analyzer answers `null` or `[]` while it loads the workspace, so the wait ends after about 2.5 seconds without any log line, and queries run against a half-loaded crate graph. A local probe against rust-analyzer 1.99.1 on this repository showed all three effects: the poll gave up at 2.7 s, `workspace/symbol` turned non-empty at 13.9 s, and rust-analyzer reported `quiescent: true` only at 23.3 s. So even a poll without the 5-poll cut-off would declare readiness about 9 seconds early.

## What Changes

- The Rust client opts in to rust-analyzer's `experimental/serverStatus` notification and keeps the latest reported status for the life of the server.
- The index readiness wait for a server that reports status ends when the server reports `quiescent: true`, or when the timeout expires. A server that never reports status (typescript-language-server) keeps the current `workspace/symbol` poll, including the early exit after 5 empty polls, so TypeScript behaviour does not change. The early exit is now logged.
- While a status-reporting server says it is not quiescent, a position query that comes back empty returns a "language server is still indexing; retry in a few seconds" tool error instead of an empty answer. Non-empty results are returned as before. This applies whenever the server reports that it is not quiescent, including later workspace reloads, not only the first startup.
- New optional `index_timeout_secs` field on `[server.<lang>]` in `.symtrace.toml` (default 30) replaces the hard-coded 30-second index wait. `0` skips the wait. On timeout the server is still registered with a warning, as today.
- Side effect: the `started` duration recorded in the stats database, and therefore `avg startup` in `symtrace-mcp stats`, will now measure time to quiescence (or the timeout) for rust-analyzer instead of about 2.3 s.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `lsp-client`: the index readiness wait is gated on the server's reported status when one is reported, uses the configured timeout and logs the poll's early exit; a new requirement tracks the server status notification.
- `lsp-types`: a new type for the server status notification payload.
- `tools-definitions`: a new requirement that turns an empty result into an "indexing" tool error while the server reports it is not quiescent.
- `config-file`: `[server.<lang>]` accepts `index_timeout_secs` with a default of 30.
- `server-manager`: the index wait timeout comes from the language's configuration; the Rust client capabilities declare the server status opt-in.

## Impact

- Code: `src/lsp/client.rs` (notification task, status state, `wait_for_index`), `src/lsp/types.rs`, `src/language/rust.rs`, `src/server/manager.rs` (`LanguageServerConfig`, defaults, `ServerEntry`, `start_server_internal`), `src/project/registry.rs` (defaults and config application), `src/config.rs` (`ServerConfig`), `src/mcp/tools.rs` (new `ToolError` constructor), `src/mcp/handlers.rs` (the indexing check in every position-query handler).
- Behaviour: the first tool call on a cold rust-analyzer now blocks until quiescence or the timeout (default 30 s) instead of about 2.5 s. MCP clients may receive a retryable error where they used to receive a silent empty result.
- Config: backward compatible; existing `.symtrace.toml` files keep working.
- Docs: README configuration section, project `CLAUDE.md` config example, `CHANGELOG.md`.
- Sibling change `ts-call-hierarchy` also edits `src/language/rust.rs::client_capabilities()`; expect a merge conflict there.
- No new dependencies (`tokio` already has the `sync` feature).
