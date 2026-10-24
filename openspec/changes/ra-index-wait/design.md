# Design

## Context

See proposal.md (Why) for the problem. The code facts that shape the approach:

- `LanguageServerManager::start_server_internal` (`src/server/manager.rs`) calls `LspClient::start`, then `client.wait_for_index(30 s)`, logs a warning on `Err`, boxes the client into `ServerEntry` and records the `started` stats event. All of this runs while the manager's `servers` mutex is held (`get_client_for_file`), so every tool call for that project, in any language, waits for the startup.
- `LspClient::start` (`src/lsp/client.rs`) sends `initialized` and then spawns a task that drains the transport's unbounded notification channel. Today it handles only `textDocument/publishDiagnostics`. Because the channel is unbounded, a notification that arrives before the task starts is buffered, not lost.
- `wait_for_index` returns `Ok(())` without logging after 5 consecutive empty or failed `workspace/symbol` polls. That exit exists for typescript-language-server, which answers "No Project" until a file is opened.
- Handlers in `src/mcp/handlers.rs` reach the client through `ServerEntry.client: Box<dyn LspClientApi>` and turn empty results into "No ... found" text. Tool failures become JSON-RPC error responses built from `ToolError { code, message }` (`src/mcp/tools.rs`).
- `src/server/manager.rs` tests already build a `ServerEntry` around `MockLspClientApi` (`make_entry`), which gives the new check a test seam.

Probe (a throwaway Python script, not committed, that speaks LSP to rust-analyzer 1.99.1 on this repository with client capability `experimental.serverStatusNotification: true` and polls `workspace/symbol("")` every 500 ms; run on 2026-10-24):

| Time | Event |
|------|-------|
| 0.01 s | `initialize` result. `capabilities.experimental` does not list `serverStatusNotification`; `serverInfo.name` is `rust-analyzer` |
| 0.01 s | `serverStatus {health: ok, quiescent: false, message: null}` |
| 0.5–2.0 s | `workspace/symbol("")` returns `null` four times |
| 2.67 s | `serverStatus {health: warning, quiescent: false, message: "invalid config value ..."}`; `workspace/symbol` returns `[]` (fifth empty poll: today's wait would end here) |
| 13.9 s | `workspace/symbol` returns 128 symbols |
| 23.25 s | `serverStatus {health: warning, quiescent: true, ...}` |

So `health` can be `warning` while the server is perfectly usable, the first status arrives within milliseconds, and `workspace/symbol` turns non-empty well before quiescence.

## Goals / Non-Goals

**Goals:**

- One source of truth for "is this server indexing", shared by the startup wait and the tool handlers.
- Zero behaviour change for servers that never send `experimental/serverStatus`.
- A check that handlers apply in one line each, unit-testable without a language server.

**Non-Goals:**

- Tracking `$/progress` work-done notifications. `serverStatus` already summarises them for rust-analyzer, and typescript-language-server's progress does not tell us when a project is loaded.
- Releasing the manager mutex during startup or running the wait in the background. That would change the concurrency model of `get_client_for_file`; it is a separate change if the longer wait proves painful.
- Detecting results that are non-empty but incomplete (between 13.9 s and 23.25 s in the probe). The maintainer decided to return non-empty results as they are.
- A new MCP error code.

## Decisions

### D1. Status state: a `watch` channel holding a tri-state

`LspClient` owns a `tokio::sync::watch` channel of

```rust
pub enum IndexState { NotReported, Indexing, Ready }
```

initialised to `NotReported`. The notification task gets the `Sender`. On `experimental/serverStatus` it deserialises the params into a new `ServerStatusParams { health: String, quiescent: bool, message: Option<String> }` in `src/lsp/types.rs`; on failure it logs at debug and keeps the old state. Otherwise it sends `Indexing` or `Ready` from `quiescent` alone (`send_if_modified`, so unchanged statuses do not wake waiters). Every status change is logged at debug. When `health` is not `ok` and differs from the last seen `health` or `message`, the task logs a warning with the message, so a degraded workspace (for example a failing build script) is visible in `.symtrace/logs/` without spamming on every notification.

Alternatives: an `Arc<Mutex<Option<ServerStatusParams>>>` (needs its own notify mechanism for the wait), or an `AtomicU8` (same problem). `watch` gives both "read the latest value" and "await a predicate" (`wait_for`) for free, and `tokio` already has the `sync` feature.

### D2. Telling status-reporting servers from the rest: "has any status arrived"

The client treats a server as status-reporting from the moment its first `serverStatus` notification is processed, and permanently for that client. The wait is:

1. If the state is not `NotReported`, await `wait_for(|s| *s == Ready)` bounded by the remaining timeout.
2. Otherwise, if the server lacks `workspaceSymbolProvider`, return `Ok` (today's skip).
3. Otherwise run one `workspace/symbol` poll, raced in a `select!` against `changed()` on the watch receiver. A status change abandons the poll and goes to step 1.
4. A non-empty result returns `Ok`. After the fifth consecutive empty or failed poll, the state is checked once more (step 1 if a status arrived). If no status arrived, the wait logs at info that it ended after 5 empty `workspace/symbol` polls and returns `Ok`.

Why this over the alternatives:

- **Key on the declared client capability (Rust only).** This would need a Rust-specific branch and would misbehave when `[server.rust] command` points at a wrapper or an old rust-analyzer that never sends the notification: the wait would always run to the timeout. With D2 such a server falls back to the poll, as today.
- **Key on the server's advertised capabilities or `serverInfo.name`.** The probe shows rust-analyzer does not advertise `serverStatusNotification` in its `initialize` result, and matching on a name is brittle.
- **A fixed grace period before deciding.** The probe shows the first status within 10 ms, but nothing in the protocol guarantees it, and any number would be a guess. D2 needs no number: the 5-poll exit takes at least about 2 seconds, and a status that arrives at any point during that window switches the mode.

The remaining gap: a server that reports status but has no `workspaceSymbolProvider`, and whose first status is processed after step 2, skips the wait. No known server does this, and D4 still protects its results.

### D3. Where handlers read the state: a `ServerEntry` field, not a trait method

`LspClient` gets an inherent `index_state(&self) -> watch::Receiver<IndexState>`. `start_server_internal` takes the receiver before boxing the client and stores it in a new `ServerEntry.index_state` field. Handlers read `entry.index_state.borrow()`.

Alternative: add `fn index_state(&self) -> IndexState` to `LspClientApi`. Rejected because nothing needs the state through `dyn LspClientApi` except the handlers, which already hold the `ServerEntry`. The trait change would also make every future handler-level mock test set an `expect_index_state` and would need an `lsp-client-api` delta. With D3, `MockLspClientApi` is untouched. `make_entry` in the manager tests gains a `watch::Receiver` argument (or builds a channel), and tests drive the state through the matching `Sender`.

### D4. The indexing check: one helper in `handlers.rs`, sampled before and after

```rust
fn reject_empty_while_indexing(before: IndexState, after: IndexState, empty: bool) -> Result<(), ToolError>
```

returns `Err(ToolError::indexing())` when `empty` is true and either sample is `Indexing`, otherwise `Ok(())`. Each handler copies the state just before sending its LSP request and again after the response, and calls the helper before formatting:

- `execute_query` (references, definition, implementations): `locations.is_empty()`.
- `execute_call_hierarchy`: after `prepare_call_hierarchy` (`items.is_empty()`, before the "No callable symbol" early return) and after `incoming_calls` / `outgoing_calls` (`calls.is_empty()`). The "before" sample is the one taken before `prepare_call_hierarchy`.
- `hover`: `result.is_none()`. `rename`: `result.is_none()`. `diagnostics`: `diagnostics.is_empty()`.

Sampling both ends covers a server that leaves quiescence mid-query (spec scenario "Server becomes indexing during the query") and one that reaches quiescence just after sending an answer computed on the half-loaded crate graph.

`ToolError::indexing()` in `src/mcp/tools.rs` uses code `-32603` (`INTERNAL_ERROR`) and the fixed message `language server is still indexing the workspace; retry in a few seconds`. The fixed prefix makes these errors easy to count in the `tool_calls.error_msg` stats column.

Alternatives:

- **A server-defined code in -32000..-32099.** MCP clients show the message, not the code, and MCP already uses part of that range (for example -32002), so a new code buys nothing and risks a clash. The `Tool error handling` requirement already uses -32603 for all server-side failures.
- **A successful result with a "still indexing" text.** The maintainer chose a tool error so the agent cannot mistake it for an answer.
- **Do the check in `LspClient` and return `ClientError::Indexing`.** The client would have to judge emptiness per method, and the `lsp-client` scenarios "Query returns empty result ... SHALL return an empty list (not an error)" would have to change. The handler is where "empty" turns into user-visible text, so the check belongs there.

### D5. The check applies whenever the server reports `quiescent: false`, not only during the first startup

rust-analyzer goes back to `quiescent: false` when it reloads the workspace (for example after `Cargo.toml` or build-script changes) or re-runs proc-macro loading. Empty answers in that window are as unreliable as during the first load. With the `watch` state, "always" is also the simpler implementation: there is no "initial indexing finished" flag to keep. The cost is that a short reload can turn a genuinely empty answer into a retryable error; non-empty answers still pass, and the message tells the agent to retry.

### D6. Config plumbing

- `src/config.rs`: `ServerConfig` gains `#[serde(default = "default_index_timeout")] pub index_timeout_secs: u64` with `default_index_timeout() -> 30`. `ServerConfig` does not deny unknown fields, so an older binary reading a new config ignores the field.
- `src/server/manager.rs`: `LanguageServerConfig` gains `index_timeout_secs: u64`; `default_configs()` sets 30 for both languages. `start_server_internal` skips `wait_for_index` when it is 0, otherwise passes `Duration::from_secs(cfg.index_timeout_secs)`. The timeout warning names the language and the timeout.
- `src/project/registry.rs`: `default_server_configs()` sets 30 for both languages and `build_server_configs` copies `cfg.index_timeout_secs`. The two default tables already duplicate each other; this change keeps them in step and does not merge them.

### D7. Rust capabilities and the sibling change

`src/language/rust.rs::client_capabilities()` gains `"experimental": { "serverStatusNotification": true }` and its first unit test, which checks that key. `src/language/typescript.rs` stays unchanged and gets a test that asserts the key is absent. The sibling change `ts-call-hierarchy` (Issue #28) also edits `rust.rs::client_capabilities()`. Both add JSON keys to the same literal, so expect a textual conflict. Whichever change lands second rebases and keeps both keys. If that change also adds a requirement to `server-manager` about Rust capabilities, the second change to archive must merge it with "Rust server status opt-in" instead of adding a duplicate.

### Cold start of a large workspace with this design

1. The first tool call for a `.rs` file takes the manager mutex and spawns rust-analyzer with the opt-in capability.
2. `initialize` / `initialized` complete. The notification task starts and processes `serverStatus {quiescent: false}`: the state becomes `Indexing`.
3. `wait_for_index` sees a reported status (at once, or after the first poll's `select!` wakes on `changed()`) and awaits `Ready`.
4a. rust-analyzer reports `quiescent: true` within `index_timeout_secs`. The state becomes `Ready`, the server is registered, `started` is recorded with the real time to quiescence, and the tool call runs on a fully loaded crate graph.
4b. The timeout expires first. A warning is logged and the server is registered anyway. The tool call runs; if it comes back empty while the state is still `Indexing`, the agent gets "language server is still indexing the workspace; retry in a few seconds". Non-empty answers pass through.
5. Later calls reuse the server. When it reaches `Ready`, empty answers are reported as "No ... found" again. If a reload sends it back to `Indexing`, step 4b's rule applies again.

typescript-language-server never sends `serverStatus`, so its state stays `NotReported`: the poll and its 5-poll exit run as today, and the handler check never fires.

### Integration tests

The harness's `wait_for_ready` (`tests/common/mod.rs`) already retries on any JSON-RPC error and returns on any success, so the indexing error turns it into a real readiness probe: before, a cold "No definition found" counted as ready. One harness detail does need a change: each probe is bounded by the remaining budget and a probe that outlasts it ends the poll without a retry. The first probe now blocks inside the server for up to `index_timeout_secs` (30 s), equal to the Rust test's 30 s budget, so that budget is raised to 60 s, as the TypeScript test already uses. One new Rust integration test sends a position query before `wait_for_ready` and asserts that the answer is either non-empty or the indexing error, never a plain "No ... found". No `integration-tests` requirement changes.

## Risks / Trade-offs

- [The 30 s default is shorter than the time to quiescence on large workspaces (this repository already took 23 s on the dev VM), so the timeout path will be common there] → D4 makes the timeout path safe; README documents `index_timeout_secs` for large workspaces.
- [The first tool call now blocks up to `index_timeout_secs` with the manager mutex held, which also blocks calls for other languages in the same project; MCP clients have their own tool-call timeouts] → Default stays 30 s, `index_timeout_secs = 0` disables the wait and leaves protection to D4. Releasing the mutex is a non-goal here.
- [A rust-analyzer version that sends a different status shape, or none] → Lenient parsing: a notification without a boolean `quiescent` is ignored. A server that never sends a valid status stays `NotReported` and falls back to today's poll, so results are no worse than today.
- [A server stuck at `quiescent: false` (for example a hung proc-macro server) turns every empty answer into the indexing error for as long as it is stuck] → Non-empty answers still pass, and the error says what is going on instead of returning silent empties. The status `message` is logged when `health` is not `ok`.
- [Short `quiescent: false` windows during reloads produce retryable errors for genuinely empty answers] → Accepted (D5); the window is short and the agent is told to retry.
- [Non-empty but incomplete results before quiescence after a timeout] → Accepted by maintainer decision; out of scope.
- [The `started` stats duration jumps from about 2.3 s to the real indexing time] → Intended: "time from spawn to ready" now means ready. Mention in CHANGELOG.
- [Merge conflict with `ts-call-hierarchy` in `rust.rs`] → See D7.

## Migration Plan

No data or config migration. Existing `.symtrace.toml` files keep working with the 30 s default. Rollback is a revert; a config that sets `index_timeout_secs` is still accepted by older binaries because unknown fields are ignored.
