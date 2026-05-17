## Context

symtrace-mcp already has five MCP tools for code navigation (find_references, goto_definition, find_implementations, incoming_calls, outgoing_calls). The LSP types for hover (`Hover`), diagnostics (`Diagnostic`), and rename (`TextEdit`, `WorkspaceEdit`) were defined in `lsp/types.rs` during P0/P2 but never wired into tools. `ServerCapabilities` already parses `hover_provider`, `diagnostic_provider`, and `rename_provider`. This change adds three new MCP tools following the established pattern.

## Goals / Non-Goals

**Goals:**
- Add `hover`, `diagnostics`, and `rename` (preview-only) MCP tools
- All three follow the same request/response pattern as existing tools
- Capability gating for each tool (clear error when server lacks support)

**Non-Goals:**
- Push diagnostics (caching `textDocument/publishDiagnostics` notifications)
- Applying rename edits — symtrace-mcp returns previews only; Claude applies changes via its own Write/Edit tools
- Rename validation or workspace edit application logic

## Decisions

### D1: Diagnostics uses pull model (`textDocument/diagnostic`)

LSP 3.17 introduced pull diagnostics as a request/response protocol. This matches our existing tool pattern (send request, get response, format output). rust-analyzer supports it. Push diagnostics (`textDocument/publishDiagnostics`) would require a caching layer and a different architectural pattern — out of scope.

**Alternative considered:** Subscribe to `textDocument/publishDiagnostics` and cache results. Rejected because it adds complexity (notification handler, cache invalidation) and deviates from the established tool pattern.

### D2: Rename returns preview only

The `textDocument/rename` response contains a `WorkspaceEdit` listing every location that would change. symtrace-mcp formats this as a human-readable preview. Claude can then apply changes using its own editing tools. This keeps symtrace-mcp read-only and gives Claude full control over the edit.

**Alternative considered:** symtrace-mcp applies the `WorkspaceEdit` directly. Rejected because (1) it makes an MCP tool that writes files — surprising for a code intelligence server, (2) blast radius is large for multi-file renames, (3) Claude already has Write/Edit tools with undo support.

### D3: Hover contents normalization

The LSP spec allows `Hover.contents` to be a `string`, `MarkedString`, `MarkupContent`, or an array of these. Since the type is stored as `Value`, the tool handler normalizes to a single string for text output mode: extracts `value` from `MarkupContent`, joins arrays with newlines, and passes through plain strings. JSON mode returns the raw `Value`.

## Risks / Trade-offs

- **Pull diagnostics not universally supported** → Capability gate returns a clear error message. rust-analyzer and pyright support it. `typescript-language-server` does **not** and has no plans to (tsserver API makes it infeasible — see [#972](https://github.com/typescript-language-server/typescript-language-server/issues/972)). P3 (TypeScript support) may need a push diagnostics fallback for the `diagnostics` tool to work with TypeScript projects.
- **Hover contents format varies across LSP servers** → Normalize to string for text output; raw JSON available via `json: true` for edge cases.
- **Rename preview can be large for workspace-wide renames** → Same as find_references which already handles large result sets.
