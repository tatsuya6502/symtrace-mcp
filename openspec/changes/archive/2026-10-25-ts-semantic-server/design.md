# Design

## Context

See proposal.md - Why. Current code:

- `src/language/{rust,typescript}.rs` each expose `client_capabilities() -> Value`; `LanguageServerManager::start_server_internal` (`src/server/manager.rs`) picks one by `Language` and calls `LspClient::start(command, args, root, capabilities, env)`.
- `LspClient::start` (`src/lsp/client.rs`) builds the `initialize` params with `json!` from `processId`, `rootUri`, `rootPath` and `capabilities`; no `initializationOptions` is sent today. `start_server_internal` is its only caller; no test or mock constructs `LspClient` through `start`.
- `execute_query` (`src/mcp/handlers.rs`) calls `ensure_open` (sends `didOpen`) and then the request at once, so the first TypeScript query always falls inside the project-loading window in which typescript-language-server (tsls) routes definition, references, hover, rename etc. to its syntax-only tsserver.
- The fix was verified end to end in a patched scratch copy with typescript-language-server 5.2.0 / TypeScript 5.8.3 (the versions pinned in `.github/workflows/integration.yml`): with `useSyntaxServer: "never"`, the first `goto_definition` returns `index.ts:21`, `find_references` returns 6 locations and `hover` returns the full signature.

## Goals / Non-Goals

**Goals:**
- Correct cross-file answers on the first TypeScript query, for every request kind tsls would otherwise route to the syntax server.
- A per-language place to put initialization options, with Rust sending none.
- A CI test that fails without the fix.

**Non-Goals:**
- A readiness gate for TypeScript (see D2).
- User-facing configuration of initialization options (see D3).
- Changes to the startup `wait_for_index` logic, including its early exit after 5 failed/empty `workspace/symbol` polls.
- Re-issuing definition on import lines, preferring `LocationLink.targetUri`, `textDocument/declaration` (tsls answers -32601) or `typeDefinition` (different semantics). None of these fix the problem: the first three still hit the syntax server or the same file, the last answers a different question.

## Decisions

### D1: Per-language `initialization_options()` plumbed through `LspClient::start`

Add `pub fn initialization_options() -> Option<serde_json::Value>` next to `client_capabilities()` in each language module:

- `typescript::initialization_options()` returns `Some(json!({"tsserver": {"useSyntaxServer": "never"}}))`.
- `rust::initialization_options()` returns `None`.

`start_server_internal` selects it with the same `match language` as the capabilities and passes it to `LspClient::start`, which gains an `initialization_options: Option<Value>` parameter. `LspClient::start` builds the base params as today and inserts `"initializationOptions"` into the params object only when the value is `Some`. Writing `json!({ "initializationOptions": opts })` with `opts = None` would serialize `null`, which rust-analyzer would read as its configuration and which the spec forbids.

Alternatives considered:
- Return a full "init params extension" object per language: more general than needed; only one field is involved.
- Hard-code the option inside `LspClient::start` keyed on the command name: couples the generic client to tsls and breaks when users configure a wrapper command.
- Send the option to every server: rust-analyzer treats `initializationOptions` as its own configuration, so an unknown `tsserver` key is at best ignored and at worst a config error. Rejected.

### D2: `useSyntaxServer: "never"` rather than a progress-based readiness gate

With `"never"`, tsls runs only the semantic tsserver and queues requests until the project has loaded, so correctness needs no client-side state. A readiness gate would instead drive a TypeScript `IndexState` from `$/progress` ("Initializing JS/TS language features…") after the first `didOpen`, and hold queries until the progress ends. That requires the transport to answer server-initiated `window/workDoneProgress/create` requests and to track progress tokens, which is a larger change and still relies on tsls's progress wording. It is recorded as a follow-up idea, out of scope here. The cost of `"never"` is the loss of fast syntax-only answers during loading, which were wrong for cross-file questions anyway.

### D3: No user-facing option for now

The value is fixed in code. If a user ever needs the syntax server back (e.g. very large projects where a fast, file-local answer is acceptable), the option can later be exposed under `[server.typescript]` in `.symtrace.toml`. No config, README or CLAUDE.md change is needed in this change.

### D4: First-call integration tests use a cold client

The existing `spawn_client()` helper in `tests/integration_ts.rs` calls `wait_for_ready` with a `goto_definition` on `index.ts`; that call opens a file, starts the project load and is answered by the syntax server, so any later query in the same session may already be outside the loading window. The new tests therefore create the client directly (`common::require_command("typescript-language-server")` + `McpClient::spawn(&cwd)`, e.g. via a new `spawn_cold_client()` helper) and make the cross-file query the first `tools/call` of the session. One fresh client per test, so each test hits the window independently. Existing tests keep using `spawn_client()`.

## Risks / Trade-offs

- [First TypeScript query is slower: it waits for the semantic project to load, which can take several seconds on large projects] → Accepted; a correct answer after a delay is better than a fast wrong one. The per-request handling is unchanged, so a slow load shows up as latency, not as an error.
- [With the syntax server disabled, the startup `wait_for_index` `workspace/symbol` polls go to the semantic tsserver. Before any `didOpen` it has no project and is expected to return an error or empty result at once, so the existing early exit after 5 failed/empty polls still ends the wait in ~2.5 s. If tsls held these requests instead, each poll is bounded at 5 s and startup could stall up to the 30 s overall timeout] → Task 2 verifies startup time with logging; the early-exit logic itself stays as is.
- [The first-call tests depend on timing: they fail without the fix only if the query lands inside the loading window] → The window opens at the first `didOpen`, which the first query itself sends immediately before its request, so the race is deterministic in practice. Task 3 checks that the new tests fail without the fix.
- [`rename` behaviour on the first query is covered only by the tsls routing analysis, not by a test] → The fix removes the routing for all request kinds at once; a dedicated first-call rename test can be added later if needed.
- [Future tsls versions might rename or drop `useSyntaxServer`] → An unknown option is ignored by tsls, so the failure mode is a return to today's behaviour, which the new integration tests would catch when the pinned versions are bumped.
