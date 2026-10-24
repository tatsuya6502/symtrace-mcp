# Proposal

## Why

The first query on a TypeScript project returns wrong, file-local answers (Issue #30). typescript-language-server runs a syntax-only tsserver next to the semantic one (`useSyntaxServer: "auto"`) and, while the semantic project is still loading, routes definition, references, hover, rename and similar requests to the syntax server, which knows only the open file. tsserver starts loading the project only at the first `textDocument/didOpen`, and symtrace-mcp sends `didOpen` immediately followed by the request, so the first query always lands in that window (0.7–0.9 s on the fixture, seconds on large projects). Observed on `fixtures/ts-project/caller.ts`: `goto_definition` returns the import line instead of `index.ts:21`, `find_references` returns 2 same-file hits instead of 6 across both files, and `hover` shows `import greetEntity` without a signature. By the same routing, a first `rename` would edit only the current file. The startup `wait_for_index` cannot help because no file is open at startup and tsls reports no readiness status.

Fixes #30.

## What Changes

- symtrace-mcp sends `initializationOptions: {"tsserver": {"useSyntaxServer": "never"}}` in the `initialize` request to typescript-language-server, so every request is answered by the semantic tsserver.
- rust-analyzer keeps receiving no `initializationOptions` (it treats that field as its own configuration).
- The LSP client accepts optional, per-language initialization options and passes them through to `initialize`.
- New TypeScript integration tests whose very first tool call is a cross-file `goto_definition` / `find_references` from `caller.ts`, so the syntax-server window is actually exercised in CI.
- **Trade-off**: the first TypeScript query now waits until the semantic project has loaded instead of answering fast and wrong. No configuration option and no output-format change.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `server-manager`: TypeScript servers are initialized in semantic-server-only mode; Rust servers receive no initialization options.
- `lsp-client`: `initialize` carries optional initialization options supplied by the caller, omitted entirely when none are given.
- `integration-tests`: TypeScript tests cover cross-file results on the first query of a fresh server.

## Impact

- Code: `src/language/typescript.rs`, `src/language/rust.rs` (new per-language initialization options), `src/lsp/client.rs` (`LspClient::start` signature and `initialize` params), `src/server/manager.rs` (`start_server_internal` passes the options).
- Tests: unit tests in the language modules; `tests/integration_ts.rs` gains first-call tests.
- Behaviour: first TypeScript query latency rises to the project-load time; results become correct for `goto_definition`, `find_references`, `hover` and `rename`.
- No config, MCP schema, output format or dependency changes. `CHANGELOG.md` gets an Unreleased Fixed entry.
