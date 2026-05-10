## Why

symtrace-mcp currently has no implementation — `src/main.rs` is a placeholder `Hello, world!`. Before any LSP-backed tools (find_references, goto_definition, etc.) can be built, we need the foundational layers: LSP transport to communicate with language servers, LSP protocol types, and the MCP server framework that exposes tools to AI agents via JSON-RPC over stdio.

## What Changes

- **Add async runtime and dependencies**: tokio, serde, serde_json, toml to `Cargo.toml`
- **LSP transport layer** (`src/lsp/transport.rs`): JSON-RPC 2.0 over stdio with `Content-Length` framing for communicating with child language server processes
- **LSP protocol types** (`src/lsp/types.rs`): Core LSP types — Position, Location, Range, TextDocumentIdentifier, InitializeParams/Result, ServerCapabilities, and related structs
- **MCP server framework** (`src/mcp/`): JSON-RPC 2.0 server over stdio that handles `initialize`, `tools/list`, and `tools/call` requests; includes tool dispatch infrastructure
- **Module structure**: Create the full directory skeleton (`src/lsp/`, `src/mcp/`, `src/server/`, `src/language/`) with `mod.rs` files

## Capabilities

### New Capabilities
- `lsp-transport`: JSON-RPC 2.0 transport with Content-Length framing for stdio-based communication with language server child processes
- `lsp-types`: Core LSP protocol type definitions (Position, Location, Range, TextDocumentIdentifier, InitializeParams/Result, ServerCapabilities)
- `mcp-server`: MCP server framework — JSON-RPC 2.0 over stdio with initialize, tools/list, tools/call handlers and tool dispatch

### Modified Capabilities
<!-- No existing capabilities to modify — this is the first implementation. -->

## Impact

- **Dependencies**: Adds tokio (async runtime), serde/serde_json (serialization), toml (config parsing) — all specified in the design spec §9.2
- **Source layout**: Creates 4 new modules under `src/` (lsp, mcp, server, language) with initial files
- **Entry point**: `src/main.rs` changes from placeholder to MCP server startup
- **No external API yet**: The MCP server will be runnable but exposes no functional tools until P1 (tool implementations)
