## Context

symtrace-mcp is a Rust MCP server that manages LSP language servers on behalf of AI coding agents. The design spec (`doc/spec-2026-05-10.md`) defines a layered architecture: MCP protocol handler at the top, tool dispatcher in the middle, and LSP client/transport at the bottom.

The codebase is currently empty (`src/main.rs` prints "Hello, world!"). This change (P0) builds the three foundational layers so that P1 can add concrete tool implementations (find_references, goto_definition, etc.).

Reference projects: serena (Python, LSP client), serena-fork (lazy language server startup), ast-outline (Rust, MCP server mode).

## Goals / Non-Goals

**Goals:**
- Establish the tokio-based async runtime and module skeleton
- Implement LSP transport that can send/receive JSON-RPC 2.0 messages with Content-Length framing over stdio to child processes
- Define all core LSP protocol types needed for initialize, shutdown, textDocument/* methods, and callHierarchy
- Build an MCP server that accepts JSON-RPC 2.0 over stdio, handles `initialize`/`initialized`, `tools/list`, and `tools/call`, and dispatches tool calls to handler functions
- The MCP server SHALL compile, run, and respond to `initialize` and `tools/list` (with an empty tool list) by end of P0

**Non-Goals:**
- No LSP client lifecycle management (start/shutdown/idle) — that's `src/server/` in P1
- No file management (didOpen/didClose) — P1
- No tool implementations — P1+
- No configuration file parsing — P3
- No multi-language support — P3

## Decisions

### D1: JSON-RPC 2.0 implemented from scratch (no crate)

**Choice**: Hand-roll JSON-RPC 2.0 for both LSP and MCP layers using `serde_json::Value`.

**Alternatives considered**:
- `jsonrpc-core` crate: adds dependency for trivial parsing; our needs are simple (parse method + params from requests, send responses/errors)
- `lsp-types` crate: provides full LSP type coverage but we only need ~20 types; pulls in many transitive deps; version churn with LSP spec updates

**Rationale**: Both protocols are JSON-RPC 2.0 with identical framing. The message shapes are simple (request/response/notification). A thin hand-rolled layer keeps the dependency tree minimal and gives full control over the transport-level details (Content-Length header, etc.).

### D2: LSP transport uses tokio channels for async message routing

**Choice**: `LspTransport` spawns a background reader task that reads LSP responses and routes them through `tokio::sync::oneshot` channels keyed by request ID.

**Rationale**: The MCP server is async (tokio). LSP responses arrive asynchronously and may be interleaved with notifications. A background reader + channel-per-pending-request pattern handles this cleanly without blocking the MCP request handler.

### D3: MCP server reads from stdin line-by-line (Content-Length framing)

**Choice**: Same Content-Length framing as LSP — read `Content-Length: N\r\n\r\n` then N bytes.

**Rationale**: MCP over stdio uses JSON-RPC 2.0, identical framing to LSP. Reusing the same framing parser avoids duplication.

### D4: Module structure matches spec §9.3

**Choice**: Follow the directory layout from the design spec exactly:
```
src/
├── main.rs
├── mcp/          (protocol.rs, tools.rs)
├── lsp/          (transport.rs, types.rs)
├── server/       (stub mod.rs)
└── language/     (stub mod.rs)
```

**Rationale**: The spec was researched thoroughly against serena/serena-fork/codegraph. Following it now avoids restructuring later. Stub modules (server/, language/) establish the module tree for P1+.

### D5: `edition = "2024"` in Cargo.toml

**Choice**: Keep the existing `edition = "2024"` setting.

**Rationale**: Already set. Uses latest Rust edition features (gen blocks, let chains in if/while, etc.).

## Risks / Trade-offs

- **[Risk] LSP response ordering** — LSP servers may send responses out of order or interleave notifications. → Mitigated by the channel-per-request-ID pattern; each pending request gets a oneshot channel, the reader task routes by ID.
- **[Risk] Content-Length parser edge cases** — Some language servers may send extra headers. → Mitigated by parsing headers line-by-line and only extracting Content-Length; ignore unknown headers.
- **[Trade-off] No `lsp-types` crate** — We must define and maintain our own LSP type structs. → Acceptable because we only need ~20 types for P0/P1, and it avoids a heavy dependency tree.
- **[Trade-off] Stub modules for server/ and language/** — Empty modules that do nothing in P0. → Acceptable because they establish the module tree and will be filled in P1+.
