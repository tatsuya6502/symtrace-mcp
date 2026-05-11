# symtrace-mcp

> **CAUTION:** This project is in early development. The API and architecture are not stable. Expect breaking changes.

A Rust MCP (Model Context Protocol) server that provides LSP-powered code intelligence to AI coding agents. It manages language server processes on behalf of AI tools, exposing operations like find-references, goto-definition, and call-hierarchy traversal as MCP tools over stdio.

It is designed to complement, not replace, existing code analysis tools like `ast-outline`.

## When to Use symtrace-mcp vs ast-outline

`ast-outline` covers most code exploration needs and should be the first tool you reach for. `symtrace-mcp` handles the operations that require a live language server:

| Task | Tool |
|------|------|
| Structure overview, signatures, symbol bodies | `ast-outline outline` / `show` |
| Implementation search (tree-sitter) | `ast-outline implements` |
| File-level dependency graph | `ast-outline deps` / `reverse-deps` |
| Semantic and BM25 search | `ast-outline search` |
| **Symbol-level reference search** | **`symtrace-mcp find_references`** |
| **Rust trait implementation resolution** | **`symtrace-mcp find_implementations`** |
| **Jump to definition (type-resolved)** | **`symtrace-mcp goto_definition`** |
| **Call hierarchy** | **`symtrace-mcp incoming_calls`** *(Planned P2)* |
| **Type information / hover** | **`symtrace-mcp hover`** *(Planned P4)* |

The first tool call to `symtrace-mcp` starts the language server in the background. Subsequent calls reuse the running server. The server shuts down automatically after 10 minutes of inactivity.

## Architecture

```text
AI Agent  ──── stdio (JSON-RPC 2.0) ──────┐
                                          │
                                    ┌──── ▼ ─────┐
                                    │ MCP Server │   src/mcp/
                                    │ (protocol, │
                                    │ tools)     │
                                    └──── ┬ ─────┘
                                          │
                                    ┌──── ▼ ─────┐
                                    │  Server    │   src/server/
                                    │ (lifecycle,│
                                    │  dispatch) │
                                    └──── ┬ ─────┘
                                          │
                              ┌────────── ▼ ──────────┐
                              │    LSP Transport      │   src/lsp/
                              │  (JSON-RPC 2.0 over   │
                              │   stdio to child LS)  │
                              └────────── ▲ ──────────┘
                                          │
                              ┌────────── ┴ ──────────┐
                              │ Language Server       │
                              │ (rust-analyzer, etc.) │
                              └───────────────────────┘
```

The design follows a layered architecture:

- **MCP Protocol** (`src/mcp/`) — JSON-RPC 2.0 over stdio. Handles `initialize`, `tools/list`, and `tools/call`, dispatching tool invocations to registered handlers.
- **Server** (`src/server/`) — Manages language server lifecycle (lazy start, idle shutdown) and routes tool calls to the appropriate LSP operation via `LspClient`.
- **LSP Transport** (`src/lsp/`) — Communicates with child language server processes using Content-Length–framed JSON-RPC 2.0. Routes responses by request ID via tokio oneshot channels.
- **Language** (`src/language/`) — Language-specific configuration and server discovery. *(stub — P3)*

JSON-RPC 2.0 is implemented from scratch using `serde_json::Value` — no external JSON-RPC or LSP type crates. This keeps the dependency tree minimal and gives full control over transport framing.

## Supported Languages

symtrace-mcp communicates with language servers via the Language Server Protocol, so it can support any language with an LSP-compliant server.

| Language | Language Server | Status |
|----------|----------------|--------|
| Rust | [rust-analyzer](https://rust-analyzer.github.io/) | Supported (P1) |
| TypeScript / JavaScript | [typescript-language-server](https://github.com/typescript-language-server/typescript-language-server) | Planned |
| Python | [pyright](https://github.com/microsoft/pyright) | Planned |

Language support is configured per-project. Adding a new language requires only a language server entry — no code changes needed (P3).

## Current Status

**P1 (Minimal Features)** — complete. Three MCP tools are available: `find_references`, `goto_definition`, and `find_implementations`. The server lazily starts rust-analyzer, manages open files with mtime tracking, and shuts down idle servers automatically.

**P0 (Foundation)** — complete.

**Planned phases:**

| Phase | Scope |
|-------|-------|
| **P2: Call Hierarchy** | `incoming_calls`, `outgoing_calls` via the callHierarchy protocol |
| **P3: Multi-language** | TypeScript and Python support; configuration file (`.symtrace.toml`) |
| **P4: Advanced Features** | `hover`, `diagnostics`, `rename` |

## Installation

If you analyze Rust projects, you need to install `rust-analyzer` and ensure it's in your `PATH`.

```bash
rustup component add rust-analyzer
rustup component add rust-src
```

Clone the repository and install the server:

```bash
cargo install --path .
```

Add `symtrace-mcp` to your AI agent's tool configuration, specifying the path to the executable and any necessary arguments.

```bash
## Claude Code
claude mcp add --scope user symtrace-mcp -- symtrace-mcp
```

The server reads newline-delimited JSON-RPC 2.0 messages from stdin and writes responses to stdout.

## License

[MIT](LICENSE)
