## Semantic queries: use symtrace-mcp tools, not `sb` or `grep`

For the questions below, use the symtrace-mcp MCP tool (load it with
`ToolSearch("select:mcp__symtrace-mcp__<tool>")` if deferred).
`sb callers` / `sb callees` / `sb implements` / `sb impact` / `sb trace`
are tree-sitter heuristics: they do not resolve types, traits, generics,
or re-exports and report `Inferred` / `Ambiguous` edges, so use `sb`
only for structural overview (`map`, `digest`, `surface`, `deps`).

| Question | Call this, not `sb`/`grep` |
|---|---|
| Who calls this function? | `mcp__symtrace-mcp__incoming_calls` |
| What does this function call? | `mcp__symtrace-mcp__outgoing_calls` |
| What implements this trait/type? | `mcp__symtrace-mcp__find_implementations` |
| Where is this defined? | `mcp__symtrace-mcp__goto_definition` |
| Where is this used? | `mcp__symtrace-mcp__find_references` |
| What type is this? | `mcp__symtrace-mcp__hover` |

The MCP call is read-only and cheap.
