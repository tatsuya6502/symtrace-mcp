# symtrace-mcp

[![DeepWiki][deepwiki-badge]][deepwiki]
[![GitHub Actions][gh-actions-badge]][gh-actions]

`symtrace-mcp` is an MCP (Model Context Protocol) server that acts as a bridge between AI coding agents and the Language Server Protocol. It manages language server processes on behalf of AI tools, exposing operations like find-references, goto-definition, and call-hierarchy traversal as MCP tools over stdio.

**Lazy startup** and **automatic idle shutdown** ensure that heavy language server processes only consume resources when the AI agent specifically requests deep code analysis.

It is designed to complement, not replace, existing lightweight code analysis tools like `ast-outline`.

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

Language support is configured per-project. The goal is for adding a new language to require only a language server entry, with no code changes.

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

## Usage Statistics

`symtrace-mcp` records tool call and language server lifecycle events in a per-project [Turso](https://github.com/tursodatabase/turso) database (SQLite-compatible) at `.symtrace/stats.db`. Data is automatically deleted after 30 days.

View a summary of the last 7 days:

```bash
symtrace-mcp stats
```

Example output:

```text
Usage Stats (last 7 days)

Tool Usage:
  goto_definition            32 calls   89ms avg    2 errors
  find_references            18 calls   45ms avg    0 errors
  find_implementations        8 calls  120ms avg    1 errors
  incoming_calls              5 calls   67ms avg    0 errors
  outgoing_calls              3 calls   52ms avg    0 errors

Top Files:
  src/mcp/tools.rs                              28 calls
  src/server/manager.rs                         15 calls
  src/main.rs                                    8 calls

Language Servers:
  rust        started  3×  avg startup  2.3s  uptime 4h 12m total
```

If no data has been collected yet:

```text
No stats data found.
```

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

<detail>
  <summary>Claude Code example</summary>

```bash
claude mcp add --scope user symtrace-mcp -- symtrace-mcp
```

Disable the built-in `rust-analyzer-lsp` plugin to avoid running duplicate language server instances:

```bash
claude plugin disable rust-analyzer-lsp@claude-plugins-official
```

Or add the following to `~/.claude/settings.json`:

```json
{
  "enabledPlugins": {
    "rust-analyzer-lsp@claude-plugins-official": false
  }
}
```

</detail>

**MCP Protocol**

The server reads newline-delimited JSON-RPC 2.0 messages from stdin and writes responses to stdout.

## Roadmap

| Item | Scope | Status |
|------|-------|--------|
| Foundation | MCP protocol, LSP transport, LSP process management | Complete |
| Minimal Features | `find_references`, `goto_definition`, `find_implementations` | Complete |
| Multi-Project Config | `.symtrace.toml`, per-project language servers | Complete |
| Call Hierarchy | `incoming_calls`, `outgoing_calls` | Complete |
| Usage Stats | Tool call tracking, stats CLI, SQLite storage | Complete |
| Multi-language | TypeScript and Python support | Planned |
| Advanced Features | `hover`, `diagnostics`, `rename` | Planned |
| Installers & Upgrade | `curl \| sh` installer, Homebrew tap, `symtrace-mcp upgrade` | Planned |
| Doctor Command | `symtrace-mcp doctor` — environment checks and prerequisite validation | Planned |
| Stats Per Language | Group usage stats by language, schema migration | Planned |

## License

[MIT](LICENSE)

[deepwiki-badge]: https://deepwiki.com/badge.svg
[gh-actions-badge]: https://github.com/tatsuya6502/symtrace-mcp/workflows/Test/badge.svg

[deepwiki]: https://deepwiki.com/tatsuya6502/symtrace-mcp
[gh-actions]: https://github.com/tatsuya6502/symtrace-mcp/actions?query=workflow%3ATest
