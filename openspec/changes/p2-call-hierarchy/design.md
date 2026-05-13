## Context

symtrace-mcp currently provides three P1 tools (`find_references`, `goto_definition`, `find_implementations`) that use single-step LSP requests. P2 adds call hierarchy support via a two-step LSP protocol.

The callHierarchy protocol differs architecturally from P1 tools:
1. `textDocument/prepareCallHierarchy` (file + position → `CallHierarchyItem[]`)
2. `callHierarchy/incomingCalls` or `callHierarchy/outgoingCalls` (item → call results)

The LSP types (`CallHierarchyItem`, `CallHierarchyIncomingCall`, `CallHierarchyOutgoingCall`) and the `ServerCapabilities.call_hierarchy_provider` field already exist in `src/lsp/types.rs`.

## Goals / Non-Goals

**Goals:**
- `incoming_calls` and `outgoing_calls` MCP tools with `depth: 1`
- Capability check: reject calls when the language server does not support callHierarchy
- Output formats consistent with P1 tools (text + JSON)

**Non-Goals:**
- `depth > 1` recursive call chains
- New language support
- `hover`, `diagnostics`, `rename` (P4)

## Decisions

### D1: Two-step handler, not `execute_query` reuse

The existing `execute_query` helper in `src/mcp/handlers.rs` handles single-step LSP queries. Call hierarchy requires two sequential LSP calls with the first feeding into the second. A separate handler function per tool is cleaner than bolting multi-step logic onto `execute_query`.

**Alternative considered:** Extend `execute_query` with a `QueryKind::IncomingCalls` variant. Rejected because the two-step protocol needs different parameter construction and result parsing, making the shared abstraction leaky.

### D2: Capability check in `LspClient`

Check `ServerCapabilities.call_hierarchy_provider` at call time. If `None`, return a `ClientError::Protocol` with a clear message ("language server does not support call hierarchy"). This avoids unnecessary `prepareCallHierarchy` round trips and provides a useful error message for future P3 languages that may not support callHierarchy (e.g., pyright).

### D3: `depth` parameter accepted but restricted

The `depth` parameter is accepted in the tool schema for forward compatibility. Values other than `1` return an MCP error (`-32602` Invalid Params). This keeps the door open for future recursion without implementing it now.

### D4: Output format

Text output uses directional arrows for visual clarity:
```
incoming_calls: my_function
  ← src/bar.rs:128:10  process_data()
  (2 callers)

outgoing_calls: my_function
  → src/utils.rs:42:5   helper()
  (2 callees)
```

JSON output mirrors P1 format: `[{ file_path, line, column, line_text }]` plus a `name` field for the caller/callee symbol name.

## Risks / Trade-offs

- **[prepareCallHierarchy returns multiple items]** → Use the first item. Rust-analyzer typically returns one item per position. If multiple items are returned, the first is the most relevant match. Document this behavior.
- **[Unsupported capability returns unclear error]** → Capability check in `LspClient` before protocol calls provides a clear, actionable error message.
