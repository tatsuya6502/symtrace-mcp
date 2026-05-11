## Why

P0 established the MCP server framework and LSP transport layer, but the server has no functional tools. The three most critical LSP operations — find references, go to definition, and find implementations — are impossible with ast-outline's tree-sitter approach and require a live language server. P1 delivers these as working MCP tools backed by rust-analyzer.

## What Changes

- Add an `LspClient` layer on top of `LspTransport` that manages the full LSP lifecycle: `initialize` → `initialized` → queries → `shutdown` → `exit`
- Implement file management via `textDocument/didOpen`, `textDocument/didChange`, and `textDocument/didClose` so the language server has up-to-date file content
- Register three MCP tools (`find_references`, `goto_definition`, `find_implementations`) that dispatch to the corresponding LSP methods
- Implement a `LanguageServerManager` that lazily starts rust-analyzer on first tool invocation and provides a handle to the active client
- Implement `IdleMonitor` that automatically shuts down idle language servers after a configurable timeout
- Add `rust-analyzer`–specific initialization parameters (especially `rustfmt` and `cargo` settings for trait resolution)

## Capabilities

### New Capabilities
- `lsp-client`: LSP lifecycle management (initialize, shutdown, file management) on top of the P0 transport
- `server-manager`: Lazy language server startup, idle monitoring, and automatic shutdown
- `tools-definitions`: MCP tool schemas and dispatch for `find_references`, `goto_definition`, `find_implementations`

### Modified Capabilities
- `mcp-server` (P0): Fix stdio framing — MCP stdio uses newline-delimited JSON, not Content-Length headers (which are LSP-specific)

## Impact

- **New modules**: `src/lsp/client.rs`, `src/lsp/file_manager.rs`, `src/server/manager.rs`, `src/server/idle_monitor.rs`, `src/language/rust.rs`
- **Modified modules**: `src/mcp/tools.rs` (register real tools), `src/main.rs` (wire up server manager), `src/mcp/protocol.rs` (fix stdio framing)
- **Dependencies**: No new crate dependencies — uses only tokio, serde, serde_json from P0
- **Single language**: rust-analyzer only. TypeScript and Python support deferred to P3
