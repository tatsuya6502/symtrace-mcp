## Why

symtrace-mcp currently provides five code navigation tools (find_references, goto_definition, find_implementations, incoming_calls, outgoing_calls) but lacks hover information, diagnostics, and rename support — three LSP features that would give AI agents much richer code intelligence. The LSP types and server capability fields for all three are already defined from prior work; we just need to wire them into MCP tools.

## What Changes

- Add `hover` MCP tool: returns type information, documentation, and signature for the symbol at a given position via `textDocument/hover`
- Add `diagnostics` MCP tool: returns errors and warnings for a file on demand via `textDocument/diagnostic` (pull diagnostics, LSP 3.17+)
- Add `rename` MCP tool: returns a preview of all locations that would change if a symbol were renamed, via `textDocument/rename` — preview only, symtrace-mcp does not apply edits

## Capabilities

### New Capabilities

(none — all three tools follow the existing tool pattern and are captured as requirements in tools-definitions)

### Modified Capabilities

- `lsp-client`: add `hover()`, `diagnostic()`, and `rename()` methods to LspClient
- `tools-definitions`: add tool schemas, handlers, and output format specs for hover, diagnostics, and rename

## Impact

- `src/lsp/client.rs` — three new methods
- `src/lsp/types.rs` — no changes (Hover, Diagnostic, TextEdit, WorkspaceEdit already defined)
- `src/mcp/tools.rs` — three new tool registrations with handlers and schemas
- `src/server/manager.rs` — no changes needed (capability gating handled in tool handlers)
- Affected specs: `lsp-client`, `tools-definitions`
