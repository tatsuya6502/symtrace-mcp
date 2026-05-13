## Why

P1 provides three single-step LSP query tools (`find_references`, `goto_definition`, `find_implementations`). Call hierarchy — answering "who calls this function" and "what does this function call" — is the next most valuable LSP operation and requires a two-step protocol (`textDocument/prepareCallHierarchy` → `callHierarchy/incomingCalls` or `outgoingCalls`). The LSP types (`CallHierarchyItem`, `CallHierarchyIncomingCall`, `CallHierarchyOutgoingCall`) are already defined in `src/lsp/types.rs`, and `ServerCapabilities.call_hierarchy_provider` already exists. rust-analyzer supports this protocol.

## What Changes

- Add `incoming_calls` MCP tool — returns callers of a function/method at a given position
- Add `outgoing_calls` MCP tool — returns callees from a function/method at a given position
- Add `LspClient` methods for the two-step callHierarchy protocol with capability check
- `depth` parameter accepted but fixed at 1; `depth > 1` returns an error
- Text and JSON output formats consistent with existing P1 tools

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `lsp-client`: Add `prepare_call_hierarchy()`, `incoming_calls()`, `outgoing_calls()` methods with capability check
- `tools-definitions`: Add `incoming_calls` and `outgoing_calls` tool definitions and error handling

## Impact

- `src/lsp/client.rs` — 3 new methods
- `src/mcp/handlers.rs` — 2 new tool schemas + 2 new handler functions
- `src/mcp/tools.rs` — register 2 additional tools
- `src/lsp/types.rs` — no changes (types already present)
