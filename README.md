# symtrace-mcp

> **CAUTION:** This project is in early development. The API and architecture are not stable. Expect breaking changes.

A Rust MCP (Model Context Protocol) server that provides LSP-powered code intelligence to AI coding agents. It manages language server processes on behalf of AI tools, exposing operations like find-references, goto-definition, and call-hierarchy traversal as MCP tools over stdio.

## Architecture

```text
AI Agent  ──── stdio (JSON-RPC 2.0) ────┐
                                         │
                                    ┌──── ▼ ────┐
                                    │  MCP Server │   src/mcp/
                                    │ (protocol,  │
                                    │  tools)     │
                                    └──── ┬ ────┘
                                         │
                                    ┌──── ▼ ────┐
                                    │   Server    │   src/server/
                                    │  (lifecycle,│
                                    │   dispatch) │
                                    └──── ┬ ────┘
                                         │
                              ┌────────── ▼ ──────────┐
                              │    LSP Transport       │   src/lsp/
                              │  (JSON-RPC 2.0 over    │
                              │   stdio to child LS)   │
                              └────────── ▲ ──────────┘
                                         │
                              ┌────────── ┴ ──────────┐
                              │  Language Server       │
                              │  (rust-analyzer, etc.) │
                              └────────────────────────┘
```

The design follows a layered architecture:

- **MCP Protocol** (`src/mcp/`) — JSON-RPC 2.0 over stdio. Handles `initialize`, `tools/list`, and `tools/call`, dispatching tool invocations to registered handlers.
- **Server** (`src/server/`) — Manages language server lifecycle (start, shutdown, idle timeout) and routes tool calls to the appropriate LSP operation. *(stub — P1)*
- **LSP Transport** (`src/lsp/`) — Communicates with child language server processes using Content-Length–framed JSON-RPC 2.0. Routes responses by request ID via tokio oneshot channels.
- **Language** (`src/language/`) — Language-specific configuration and server discovery. *(stub — P3)*

JSON-RPC 2.0 is implemented from scratch using `serde_json::Value` — no external JSON-RPC or LSP type crates. This keeps the dependency tree minimal and gives full control over transport framing.

## Supported Languages

symtrace-mcp communicates with language servers via the Language Server Protocol, so it can support any language with an LSP-compliant server.

| Language | Language Server | Status |
|----------|----------------|--------|
| Rust | [rust-analyzer](https://rust-analyzer.github.io/) | Planned (P1) |
| TypeScript / JavaScript | [typescript-language-server](https://github.com/typescript-language-server/typescript-language-server) | Planned |
| Python | [pyright](https://github.com/microsoft/pyright) | Planned |

Language support is configured per-project. Adding a new language requires only a language server entry — no code changes needed (P3).

## Current Status

**P0 (Foundation)** — complete. The MCP server compiles, runs, and responds to `initialize` and `tools/list` over stdio. No functional tools yet.

## Build & Run

```bash
cargo build
cargo run
```

The server reads JSON-RPC 2.0 messages with `Content-Length` framing from stdin and writes responses to stdout.

## License

[MIT](LICENSE)
