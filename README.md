# symtrace-mcp

> [!CAUTION]
> This project is in early development. Expect breaking changes.

An MCP (Model Context Protocol) server that acts as a bridge between AI coding agents and the Language Server Protocol. It manages language server processes on behalf of AI tools, exposing operations like find-references, goto-definition, and call-hierarchy traversal as MCP tools over stdio.

**Lazy startup** and **automatic idle shutdown** ensure that heavy language server processes only consume resources when the AI agent specifically requests deep code analysis.

It is designed to complement, not replace, existing code analysis tools like `ast-outline`.

## When to Use symtrace-mcp vs ast-outline

[`ast-outline`][ast-outline] covers most code exploration needs and should be the first tool you reach for. `symtrace-mcp` handles the operations that require a live language server:

| Task | Tool |
|------|------|
| Structure overview, signatures, symbol bodies | `ast-outline outline` / `show` |
| Implementation search (tree-sitter) | `ast-outline implements` |
| File-level dependency graph | `ast-outline deps` / `reverse-deps` |
| Semantic and BM25 search | `ast-outline search` |
| **Symbol-level reference search** | **`symtrace-mcp find_references`** |
| **Rust trait implementation resolution** | **`symtrace-mcp find_implementations`** |
| **Jump to definition (type-resolved)** | **`symtrace-mcp goto_definition`** |
| **Call hierarchy** | **`symtrace-mcp incoming_calls`** / **`outgoing_calls`** |
| **Type information / hover** | **`symtrace-mcp hover`** *(Planned)* |

The first tool call to `symtrace-mcp` starts the language server in the background. Subsequent calls reuse the running server. The server shuts down automatically after 10 minutes of inactivity.

[ast-outline]: https://github.com/aeroxy/ast-outline

## Supported Languages

symtrace-mcp communicates with language servers via the Language Server Protocol, so it can support any language with an LSP-compliant server.

| Language | Language Server | Status |
|----------|----------------|--------|
| Rust | [rust-analyzer](https://rust-analyzer.github.io/) | Supported |
| TypeScript / JavaScript | [typescript-language-server](https://github.com/typescript-language-server/typescript-language-server) | Planned |
| Python | [pyright](https://github.com/microsoft/pyright) | Planned |

Language support is configured per-project. Adding a new language requires only a language server entry — no code changes needed.

## Multi-Project Support

symtrace-mcp can manage multiple independent projects in a single repository, each with its own language server instance. Create a `.symtrace.toml` file in the directory where Claude Code is launched:

```toml
[server.rust]
command = "rust-analyzer"

[[projects]]
root = "project-a"

[[projects]]
root = "project-b"
```

- `[[projects]]` — List of project root directories (relative to the config file). Each gets its own language server.
- `[server.rust]` — Global language server configuration. Optional; defaults to `rust-analyzer` with a 600s idle timeout.
- If `.symtrace.toml` is absent, the server runs in single-project mode using the current directory as the project root.

Tool calls are automatically routed to the correct project's language server based on the file path (longest-prefix match).

## Current Status

**Phase 2 (Call Hierarchy)** — complete. Two MCP tools for call hierarchy traversal: `incoming_calls` (callers) and `outgoing_calls` (callees) via the callHierarchy protocol.

**Phase 1 (Minimal Features)** — complete. Three MCP tools are available: `find_references`, `goto_definition`, and `find_implementations`. The server lazily starts rust-analyzer, manages open files with mtime tracking, and shuts down idle servers automatically. Multi-project support via `.symtrace.toml`.

**Phase 0 (Foundation)** — complete.

**Planned phases:**

| Phase | Scope |
|-------|-------|
| **Phase 3: Multi-language** | TypeScript and Python support |
| **Phase 4: Advanced Features** | `hover`, `diagnostics`, `rename` |

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
