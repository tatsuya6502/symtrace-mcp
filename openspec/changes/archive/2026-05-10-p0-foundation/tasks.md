## 1. Project Setup

- [x] 1.1 Add tokio, serde, serde_json, toml dependencies to `Cargo.toml`
  - Use `cargo add` command to ensure the latest version and valid features are added.
- [x] 1.2 Create module directory skeleton: `src/lsp/`, `src/mcp/`, `src/server/`, `src/language/` with `mod.rs` files
- [x] 1.3 Update `src/main.rs` to initialize tokio runtime and declare all modules

## 2. LSP Protocol Types

- [x] 2.1 Define core types in `src/lsp/types.rs`: `Position`, `Range`, `Location`
- [x] 2.2 Define document types: `TextDocumentIdentifier`, `VersionedTextDocumentIdentifier`, `TextDocumentContentChangeEvent`
- [x] 2.3 Define initialize types: `InitializeParams`, `InitializeResult`, `ClientCapabilities`, `ServerCapabilities`
- [x] 2.4 Define query-related types: `Hover`, `Diagnostic`, `WorkspaceEdit`, `TextEdit`
- [x] 2.5 Define call hierarchy types: `CallHierarchyItem`, `CallHierarchyIncomingCall`, `CallHierarchyOutgoingCall`

## 3. LSP Transport

- [x] 3.1 Implement Content-Length framing: write messages with `Content-Length: N\r\n\r\n<body>` header in `src/lsp/transport.rs`
- [x] 3.2 Implement Content-Length parser: read header, extract length, read body bytes
- [x] 3.3 Implement `LspTransport` struct with child process management (stdin/stdout handles)
- [x] 3.4 Implement `send_request` — serialize request, write to stdin, register pending oneshot channel
- [x] 3.5 Implement `send_notification` — serialize notification, write to stdin (no pending channel)
- [x] 3.6 Implement background reader task — continuously read responses, route by request ID to pending channels
- [x] 3.7 Handle child process exit — error all pending channels when process terminates

## 4. MCP Protocol Layer

- [x] 4.1 Implement JSON-RPC 2.0 message types in `src/mcp/protocol.rs`: `Request`, `Response`, `Error`, `Notification`
- [x] 4.2 Implement Content-Length framing for MCP (reuse same pattern as LSP transport) over stdin/stdout
- [x] 4.3 Implement MCP message reader — read and parse incoming requests from stdin
- [x] 4.4 Implement MCP message writer — serialize and write responses to stdout

## 5. MCP Server Framework

- [x] 5.1 Implement tool registry in `src/mcp/tools.rs` — register handlers by name, list registered tools
- [x] 5.2 Implement `initialize` handler — respond with server capabilities (tools capability)
- [x] 5.3 Implement `tools/list` handler — return registered tools (empty in P0)
- [x] 5.4 Implement `tools/call` handler — dispatch to registered handler or return error -32601
- [x] 5.5 Implement error handling for malformed requests (-32700, -32600)

## 6. Integration and Smoke Test

- [x] 6.1 Wire `main.rs` to start MCP server on tokio runtime
- [x] 6.2 Verify `cargo build` succeeds
- [x] 6.3 Send `initialize` request via stdin and verify correct response
- [x] 6.4 Send `tools/list` request and verify empty tool list response
