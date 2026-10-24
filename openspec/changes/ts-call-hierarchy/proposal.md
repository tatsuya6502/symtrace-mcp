# Proposal

## Why

On TypeScript projects, `incoming_calls` and `outgoing_calls` always fail with "language server does not support call hierarchy" (Issue #28). typescript-language-server has implemented LSP Call Hierarchy since v3.0.0, but it only advertises `callHierarchyProvider` when the client's `initialize` request declares `textDocument.callHierarchy`, and symtrace-mcp never declares it. Two of the eight MCP tools are therefore dead for every TypeScript/JavaScript project.

The main spec already requires the TypeScript `initialize` request to include call hierarchy (server-manager, "TypeScript client capabilities", since the p3-typescript-support change) and requires TypeScript integration tests for all 8 tools. The code never met either requirement, and a comment in the TypeScript integration tests wrongly claims the server lacks the feature.

## What Changes

- Declare `textDocument.callHierarchy` (`dynamicRegistration: false`) in the client capabilities sent to typescript-language-server, so it advertises `callHierarchyProvider` and the existing call hierarchy path works unchanged.
- Declare the same capability for rust-analyzer. rust-analyzer advertises call hierarchy regardless, so Rust behaviour does not change; the point is that every language declares the client capabilities the tools rely on.
- Add TypeScript integration tests for `incoming_calls` and `outgoing_calls`, and remove the incorrect comment saying they are tested only with rust-analyzer.
- No change to the "call hierarchy not supported" error: it still applies to servers that do not advertise the capability.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `server-manager`: ADD a requirement that every language's `initialize` request declares the call hierarchy client capability. Today only the TypeScript requirement mentions it (and the code ignores it); no requirement covers the Rust client capabilities at all. The existing "TypeScript client capabilities" requirement already lists call hierarchy and stays unchanged.
- `integration-tests`: MODIFY "Integration tests cover all MCP tools" to add explicit scenarios for `incoming_calls` and `outgoing_calls` returning results, so the TypeScript call hierarchy tests have a concrete spec target.

Not touched, because their behaviour does not change: `lsp-client` ("Call hierarchy capability check" still gates on `callHierarchyProvider`; TypeScript now passes it) and `tools-definitions` ("Call hierarchy unsupported error", `incoming_calls`/`outgoing_calls` tools are unchanged).

## Impact

- Code: `src/language/typescript.rs` and `src/language/rust.rs` (`client_capabilities()`), plus unit tests there.
- Tests: `tests/integration_ts.rs` (two new tests, comment removed); possibly `fixtures/ts-project/index.ts` (append-only) if the existing call graph does not give a non-empty outgoing result.
- Users: `incoming_calls` / `outgoing_calls` start working on TypeScript and JavaScript files with typescript-language-server >= 3.0.0 and TypeScript >= 3.8. No API or configuration change.
- Docs: `CHANGELOG.md` Fixed entry.
