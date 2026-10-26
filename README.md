# symtrace-mcp

[![DeepWiki][deepwiki-badge]][deepwiki]
[![GitHub Actions][gh-actions-badge]][gh-actions]

`symtrace-mcp` is an MCP (Model Context Protocol) server that acts as a bridge between AI coding agents and the Language Server Protocol. It manages language server processes on behalf of AI tools, exposing operations like find-references, goto-definition, and call-hierarchy traversal as MCP tools over stdio.

**Lazy startup** and **automatic idle shutdown** ensure that heavy language server processes only consume resources when the AI agent specifically requests deep code analysis.

It is designed to complement, not replace, existing lightweight code analysis tools like [`ast-bro`][ast-bro].

## When to Use symtrace-mcp vs ast-bro

[`ast-bro`][ast-bro] (`sb`) covers most code exploration needs and should be the first tool you reach for. `symtrace-mcp` handles the operations that require a live language server:

| Task | Tool |
|------|------|
| Structure overview, signatures, symbol bodies | `sb map` / `digest` / `show` |
| Implementation search (tree-sitter) | `sb implements` |
| File-level dependency graph | `sb deps` / `reverse-deps` |
| Call graph (tree-sitter heuristic, no type resolution) | `sb callers` / `callees` / `impact` |
| Semantic and BM25 search | `sb search` |
| **Symbol-level reference search** | **`symtrace-mcp find_references`** |
| **Rust trait implementation resolution** | **`symtrace-mcp find_implementations`** |
| **Jump to definition (type-resolved)** | **`symtrace-mcp goto_definition`** |
| **Call hierarchy (type-resolved)** | **`symtrace-mcp incoming_calls`** / **`outgoing_calls`** |
| **Type information / hover** | **`symtrace-mcp hover`** |
| **Pull diagnostics** | **`symtrace-mcp diagnostics`** |
| **Rename preview** | **`symtrace-mcp rename`** |

The first tool call to `symtrace-mcp` starts the language server in the background. Subsequent calls reuse the running server. The server shuts down automatically after 10 minutes of inactivity.

[ast-bro]: https://github.com/aeroxy/ast-bro

## Supported Languages

symtrace-mcp communicates with language servers via the Language Server Protocol, so it can support any language with an LSP-compliant server.

| Language | Language Server | Status |
|----------|----------------|--------|
| Rust | [rust-analyzer](https://rust-analyzer.github.io/) | Supported |
| TypeScript / JavaScript | [typescript-language-server](https://github.com/typescript-language-server/typescript-language-server) | Supported |
| Python | [pyright](https://github.com/microsoft/pyright) | Planned |

Each language server is configured in its own `[server.<lang>]` section of `.symtrace.toml` (see [Configuration](#configuration)). The goal is for adding a new language to require only a language server entry, with no code changes.

## Configuration

symtrace-mcp reads `.symtrace.toml` from the directory where the AI agent is launched. The file is optional: without it the server runs in single-project mode with the current directory as the project root and the default Rust and TypeScript server settings.

```toml
[server.rust]
# command defaults to rust-analyzer when omitted
index_timeout_secs = 120  # raise for large workspaces

[server.typescript]
command = "typescript-language-server"
idle_timeout_secs = 300

[logging]
level = "debug"

[[projects]]
root = "project-a"

[[projects]]
root = "project-b"
env = { DATABASE_URL = "postgres://localhost/mydb" }
```

| Key | Default | Description |
|-----|---------|-------------|
| `[server.<lang>]` | Rust and TypeScript defaults | Per-language server settings. `<lang>` is `rust` or `typescript`. Every key inside is optional, so a section may contain just one of them |
| `command` | `rust-analyzer` / `typescript-language-server --stdio` | Language server command. The TypeScript defaults assume typescript-language-server: other servers such as vtsls or deno ignore the `tsserver.useSyntaxServer = "never"` initialization option and answer the first query from a partially loaded project |
| `idle_timeout_secs` | `600` | Shut the language server down after this many seconds without a tool call. The next call starts it again |
| `index_timeout_secs` | `30` | How long the first tool call waits for the language server to finish loading the workspace. `0` skips the wait; at most `86400` |
| `[logging] level` | `info` | Log level: `off`, `error`, `warn`, `info`, `debug`, `trace`, or a [`tracing-subscriber` Targets filter](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.Targets.html) such as `symtrace_mcp::lsp=debug` |
| `[[projects]]` `root` | current directory | Project root, relative to the config file. Each project gets its own language server instance per language |
| `[[projects]]` `env` | none | Environment variables for the language servers of this project, added to the parent process environment |

### Multi-project support

With several `[[projects]]` entries, symtrace-mcp manages independent projects in one repository, each with its own language server instance. Tool calls are routed to the right project by file path (longest-prefix match).

### Index timeout

On timeout the server is used anyway. While rust-analyzer is still indexing, an empty answer comes back as a "language server is still indexing" error so the agent can retry. With `index_timeout_secs = 0` the first queries may run before the language server has reported its indexing status, so their results can be incomplete, because that error relies on the status.

### Logs

Each run writes a JSONL log file to `.symtrace/logs/` (named `symtrace-mcp.<timestamp>.<pid>.log`). Files older than 7 days are deleted at startup. Two environment variables adjust logging without touching the config file:

- `SYMTRACE_LOG`: log level; overrides `[logging] level`. `SYMTRACE_LOG=off` disables file logging
- `SYMTRACE_LOG_DIR`: log directory, instead of `.symtrace/logs/`

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

### Prerequisites

Install the language servers for the languages you work with:

```bash
# Rust
rustup component add rust-analyzer
rustup component add rust-src

# TypeScript / JavaScript
npm install -g typescript-language-server typescript

# Claude Code: disable the built-in rust-analyzer plugin so that
# two rust-analyzer instances do not run side by side
claude plugin disable rust-analyzer-lsp@claude-plugins-official
```

The last step can also be done by adding `"rust-analyzer-lsp@claude-plugins-official": false` under `enabledPlugins` in `~/.claude/settings.json`.

### Install the server

Install the latest release with Cargo straight from GitHub:

```bash
cargo install --locked --git https://github.com/tatsuya6502/symtrace-mcp.git --tag v0.7.0
```

Omit `--tag` to build the `main` branch instead. Check which version is installed with `symtrace-mcp --version` (or `-V`).

For AI agents other than Claude Code, add `symtrace-mcp` to the agent's MCP server configuration as a stdio server with the `symtrace-mcp` command and no arguments.

### Claude Code integration

Run `setup` once with no Claude Code session open:

```bash
symtrace-mcp setup
```

Options:

- `--local`: target the project files instead (`./CLAUDE.md`, `./.claude/agents/Explore.md`, `./.claude/settings.json`, `./.mcp.json`)
- `--dry-run`: print what would change without writing
- `--skip-prompt` / `--skip-explore` / `--skip-hook` / `--skip-mcp`: leave that item alone
- `--force`: replace a block or MCP entry you edited by hand (otherwise `setup` refuses and exits non-zero)
- `--status`: show `installed v<version>` / `outdated v<version>` / `modified` / `absent` per item. The hook and MCP entries record no version, so they show the running binary's version when their command is the bare `symtrace-mcp` or a path to the running binary, and a bare `installed` when it points at another install
- `--uninstall`: remove only what `setup` added; other content in those files is preserved

`setup` installs four things in your per-user Claude Code files:

| Item | File | What it does |
|------|------|--------------|
| prompt | `~/.claude/CLAUDE.md` | A marker block (`<!-- symtrace-mcp:begin v=<version> -->` … `<!-- symtrace-mcp:end -->`) telling the model to use symtrace-mcp tools before `sb`/`grep` for semantic questions (callers, implementations, definitions, references, types) |
| explore | `~/.claude/agents/Explore.md` | The same block, because the built-in Explore subagent never sees `CLAUDE.md`. Created with the built-in Explore frontmatter plus `disallowedTools: Write, Edit, NotebookEdit` when absent; if the file exists (for example ast-bro's copy), only the block is added, and `setup` warns when that file's frontmatter has no `tools:`/`disallowedTools:` line, because a user agent named Explore replaces the built-in read-only one and would inherit every tool |
| hook | `~/.claude/settings.json` | `PreToolUse` entries for the `Bash` and `Grep` matchers running `symtrace-mcp hook` |
| mcp | `~/.claude.json` | The `mcpServers.symtrace-mcp` entry |

Why: in Claude Code auto mode the model prefers Bash, so for "who calls X" it reaches for `sb callers` or `grep` and never picks an MCP tool unless an instruction names `sb` explicitly. The block fixes that; the hook reinforces it.

The hook reads each `Bash`/`Grep` call and, when it is `sb callers|callees|implements|impact|trace`, a bare-identifier `grep`/`rg` over source code, or a `Grep` tool call with a bare identifier, lets the call run and attaches a reminder naming the matching symtrace-mcp tool. It never blocks a call. Set `SYMTRACE_HOOK_SRC_DIRS="src|crates"` to change which directories count as source code.

Re-running `setup` after upgrading symtrace-mcp updates the block to the new version. The hook and MCP entries use the bare command name `symtrace-mcp`, so the binary must be on `PATH` for Claude Code: run the `cargo install` command above before `setup`. If `PATH` resolves to a different binary than the one running `setup` (for example an older install while you run `target/debug/symtrace-mcp`), `setup` prints a warning; an old binary without the `hook` subcommand would make Claude Code block every `Bash` and `Grep` call.

### Why symtrace-mcp over built-in LSP plugins? *(as of May 2026)*

As of May 2026, Claude Code includes [built-in LSP plugins for 11 languages](https://code.claude.com/docs/en/discover-plugins#code-intelligence) that provide code navigation and auto-diagnostics. symtrace-mcp adds features the built-in plugins lack:

| | Built-in LSP Plugins | symtrace-mcp |
|--|---------------------|--------------|
| Multi-project | One LSP per workspace | Multiple projects via `.symtrace.toml` |
| Idle shutdown | None — servers stay alive for the entire CLI process | Automatic after 10 min inactivity |
| Usage analytics | No | `symtrace-mcp stats` |
| Works with | Claude Code only | Any MCP-compatible AI tool |
| Auto-diagnostics | Yes | Pull and push diagnostics (`diagnostics` tool) |
| Languages | 11 (C/C++, C#, Go, Java, Kotlin, Lua, PHP, Python, Rust, Swift, TS/JS) | Rust, TypeScript/JS (Python planned) |

If you work on a single Rust project in Claude Code, the built-in plugin may be enough. symtrace-mcp is worth using when you need multi-project support, idle resource management, or usage tracking — or when you use AI tools beyond Claude Code.

### MCP Protocol

The server reads newline-delimited JSON-RPC 2.0 messages from stdin and writes responses to stdout.

## Roadmap

| Item | Scope | Status |
|------|-------|--------|
| Foundation | MCP protocol, LSP transport, LSP process management | Complete |
| Minimal Features | `find_references`, `goto_definition`, `find_implementations` | Complete |
| Multi-Project Config | `.symtrace.toml`, per-project language servers | Complete |
| Call Hierarchy | `incoming_calls`, `outgoing_calls` | Complete |
| Usage Stats | Tool call tracking, stats CLI, SQLite-compatible storage | Complete |
| Multi-language | TypeScript support | Complete |
| Multi-language | Python support | Planned |
| Multi-language | Kotlin support | Planned |
| Advanced Features | `hover`, `diagnostics`, `rename` | Complete |
| Installers & Upgrade | `curl \| sh` installer, Homebrew tap, `symtrace-mcp upgrade` | Planned |
| Doctor Command | `symtrace-mcp doctor` — environment checks and prerequisite validation | Planned |
| Stats Per Language | Group usage stats by language, schema migration | Planned |

## License

[MIT](LICENSE)

[deepwiki-badge]: https://deepwiki.com/badge.svg
[gh-actions-badge]: https://github.com/tatsuya6502/symtrace-mcp/workflows/Test/badge.svg

[deepwiki]: https://deepwiki.com/tatsuya6502/symtrace-mcp
[gh-actions]: https://github.com/tatsuya6502/symtrace-mcp/actions?query=workflow%3ATest
