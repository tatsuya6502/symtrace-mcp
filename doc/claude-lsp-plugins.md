# Claude Code Built-in LSP Plugins — Investigation Results

Date: 2026-05-16

## Overview

The official Anthropic plugin marketplace (`claude-plugins-official`) includes **11 code intelligence plugins** that provide LSP-based code intelligence:

| Plugin | LSP Server | Languages |
|--------|-----------|-----------|
| `clangd-lsp` | `clangd` | C/C++ |
| `csharp-lsp` | `csharp-ls` | C# |
| `gopls-lsp` | `gopls` | Go |
| `jdtls-lsp` | `jdtls` | Java |
| `kotlin-lsp` | `kotlin-language-server` | Kotlin |
| `lua-lsp` | `lua-language-server` | Lua |
| `php-lsp` | `intelephense` | PHP |
| `pyright-lsp` | `pyright-langserver` | Python (`.py`, `.pyi`) |
| `rust-analyzer-lsp` | `rust-analyzer` | Rust (`.rs`) |
| `swift-lsp` | `sourcekit-lsp` | Swift |
| `typescript-lsp` | `typescript-language-server` | TypeScript (`.ts`, `.tsx`), JavaScript (`.js`, `.jsx`) |

(Source: [Claude Code docs — Discover plugins](https://code.claude.com/docs/en/discover-plugins#code-intelligence))

These are **not** MCP servers. They are declarative JSON configurations that tell the Claude Code CLI how to spawn and communicate with a language server. The actual LSP client logic lives inside the closed-source Claude Code binary.

## Plugin Configurations

Each plugin is a JSON file in the `anthropics/claude-plugins-official` repository containing only a manifest — no source code.

### rust-analyzer-lsp

```json
{
  "command": "rust-analyzer",
  "extensionToLanguage": { ".rs": "rust" }
}
```

### typescript-lsp

```json
{
  "command": "typescript-language-server",
  "args": ["--stdio"],
  "extensionToLanguage": {
    ".ts": "typescript",
    ".tsx": "typescriptreact",
    ".js": "javascript",
    ".jsx": "javascriptreact"
  }
}
```

### pyright-lsp

```json
{
  "command": "pyright-langserver",
  "args": ["--stdio"],
  "extensionToLanguage": {
    ".py": "python",
    ".pyi": "python"
  }
}
```

### Full Plugin Schema

Beyond the fields shown above, the plugin manifest supports:

| Field | Purpose |
|-------|---------|
| `command` | Executable name (resolved via `$PATH`) |
| `args` | Arguments passed to the server |
| `extensionToLanguage` | Maps file extensions to LSP language IDs |
| `transport` | Communication transport (default: stdio) |
| `env` | Environment variables for the server process |
| `initializationOptions` | LSP `initialize` request options |
| `settings` | LSP workspace configuration |
| `workspaceFolder` | Explicit workspace root |
| `startupTimeout` | Time to wait for server readiness |
| `shutdownTimeout` | Time to wait for graceful shutdown |
| `restartOnCrash` | Auto-restart on unexpected exit |
| `maxRestarts` | Cap on crash restarts |

None of the three built-in plugins use `restartOnCrash`, `maxRestarts`, or custom `settings`.

## Capabilities

The built-in LSP plugins provide two categories of functionality:

### 1. Automatic Diagnostics

After every file edit, Claude Code queries the LSP server for diagnostics (errors, warnings) and incorporates them into its reasoning. This is automatic — no explicit tool call needed.

### 2. Code Navigation

When a plugin is enabled, Claude Code exposes the following tools for code navigation:

| Operation | LSP Method |
|-----------|-----------|
| Go to definition | `textDocument/definition` |
| Find references | `textDocument/references` |
| Type definition | `textDocument/typeDefinition` |
| Hover / type info | `textDocument/hover` |
| Document symbols | `textDocument/documentSymbol` |
| Find implementations | `textDocument/implementation` |
| Call hierarchy (incoming) | `callHierarchy/incomingCalls` |
| Call hierarchy (outgoing) | `callHierarchy/outgoingCalls` |

## Process Model and Lifecycle

### Architecture

```
┌─────────────────────┐
│   Claude Code CLI   │
│  (LSP client logic) │
│         │           │
│    stdio transport  │
│         │           │
│  ┌──────▼──────┐    │
│  │ LSP Server  │    │
│  │ (separate   │    │
│  │  process)   │    │
│  └─────────────┘    │
└─────────────────────┘
```

Key facts:
- **LSP servers run as separate OS processes**, spawned by the Claude Code CLI
- Communication is over **stdio** (stdin/stdout pipes)
- The Claude Code CLI acts as the LSP client — protocol negotiation, request/response handling, lifecycle management
- No source code exists in the plugin itself — it's purely declarative

### Lazy Loading

Based on official documentation, Claude Code activates LSP plugins based on the files present in the workspace. If a `.rs` file exists, `rust-analyzer-lsp` is activated; if a `.ts` file exists, `typescript-lsp` is activated, etc. This is extension-driven, not project-type-driven.

### Idle Behavior and Resource Consumption

**What is known:**
- The official docs explicitly warn: *"language servers like rust-analyzer and pyright can consume significant memory on large projects. If you experience memory issues, disable the plugin"* ([source](https://code.claude.com/docs/en/discover-plugins#code-intelligence))
- No idle eviction policy is exposed in the plugin manifest
- No `maxIdleTime`, `ttl`, or similar field exists in the schema

**What is unknown (closed-source CLI internals):**
- Whether the CLI shuts down idle LSP servers after a period of inactivity
- Whether there's a memory threshold that triggers server shutdown
- ~~Whether the CLI reuses a running server across sessions or starts fresh each time~~ **Partially answered (2026-05-17):** Servers survive `/clear` — they are tied to the CLI process, not the conversation. See "Actual Testing Results".
- Exact timing of server startup relative to first tool call vs. session start

**Likely behavior (inferred):** Given the docs warn about memory consumption, the servers likely stay alive for the duration of the CLI process rather than being spawned/destroyed per-request or per-conversation. rust-analyzer in particular builds a full project index on startup and would be expensive to restart repeatedly.

## Comparison: Claude Code LSP Plugins vs symtrace-mcp

### Feature Matrix

| Capability | Claude Code LSP Plugins | symtrace-mcp |
|-----------|------------------------|--------------|
| **Architecture** | Declarative config → CLI acts as LSP client | MCP server with own LSP client |
| **Protocol** | LSP over stdio (CLI as client) | MCP over stdio (symtrace-mcp as client), LSP over stdio to server |
| **Auto-diagnostics** | Yes (after every edit) | No |
| **Go to definition** | Yes | Yes |
| **Find references** | Yes | Yes |
| **Find implementations** | Yes | Yes |
| **Incoming calls** | Yes | Yes |
| **Outgoing calls** | Yes | Yes |
| **Hover / type info** | Yes | No (planned: P4) |
| **Document symbols** | Yes | No |
| **Type definition** | Yes | No |
| **Rename** | No | No (planned: P4) |
| **Multi-project** | No (one LSP per workspace) | Yes (via `.symtrace.toml`) |
| **Usage stats** | No | Yes (`symtrace-mcp stats`) |
| **Language support** | Rust, TS/JS, Python | Rust only (TS/Python planned: P3) |
| **Per-language config** | Via plugin settings | Via `.symtrace.toml` |

### When They Conflict

If both `rust-analyzer-lsp` and symtrace-mcp are active in the same session, Claude Code launches **two separate `rust-analyzer` processes** — one managed by the built-in LSP plugin, one managed by symtrace-mcp. This causes:

1. **Double memory usage** — each `rust-analyzer` instance independently indexes the project (easily 500 MB–2 GB per instance on large projects)
2. **Conflicting tool names** — Claude Code's built-in LSP tools may shadow or coexist with symtrace-mcp's MCP tools
3. **Duplicate results** — two separate tools can return similar but independently computed results

Recommendation: **Disable `rust-analyzer-lsp` when using symtrace-mcp** for Rust projects. Add to project or user settings:

```json
{
  "permissions": {
    "disabledPlugins": ["rust-analyzer-lsp"]
  }
}
```

### Key Differentiators

**symtrace-mcp advantages:**
- Multi-project workspace support (`.symtrace.toml`)
- Usage analytics (`symtrace-mcp stats`)
- Designed for MCP-first workflows — works with any MCP-compatible AI tool, not just Claude Code
- Active development roadmap (multi-language, hover, diagnostics, rename)

**Claude Code LSP plugin advantages:**
- Zero configuration — works out of the box when enabled
- Auto-diagnostics after every edit
- Hover/type information and document symbols
- Broader language support today (Rust + TypeScript + Python)
- Deeply integrated into Claude Code's reasoning loop

## Actual Testing Results (2026-05-16)

Tested with `rust-analyzer-lsp` enabled alongside symtrace-mcp on the symtrace-mcp repository (small Rust project).

### Startup Timing

The plugin's `rust-analyzer` does **not** start at session launch. No rust-analyzer process was visible after restarting Claude Code with the plugin enabled. It spawns lazily on the first LSP tool call.

symtrace-mcp's `rust-analyzer` also spawns on first MCP tool call (`mcp__symtrace-mcp__goto_definition`).

### Concurrent Operation — Two Separate rust-analyzer Instances

When both are active, **two independent rust-analyzer processes** run simultaneously:

| Instance | Parent | RSS (peak) | RSS (steady) | Source |
|----------|--------|------------|--------------|--------|
| PID 9437 | `claude -c` | 167 MB | 42 MB | Built-in LSP plugin |
| PID 34494 | symtrace-mcp | 1.34 GB | 322 MB | symtrace-mcp |

Each also spawns a `rust-analyzer-proc-macro-srv` child (~45–57 MB).

**Combined peak memory: ~1.5 GB** for this small project. On large monorepos, this would be significantly higher.

Both sets of tools coexist in the tool list — Claude Code exposes its built-in `LSP` tool alongside symtrace-mcp's `mcp__symtrace-mcp__*` tools. Both work correctly and return similar (but independently computed) results.

### Idle Behavior

The plugin's rust-analyzer **stayed alive** throughout the session with no sign of idle shutdown. After initial indexing, its RSS dropped but the process remained. No eviction was observed.

### Extended Idle Test (9 hours)

After 9 hours of complete inactivity (session left open overnight):

| PID | Process | RSS | ELAPSED |
|-----|---------|-----|---------|
| 74091 | rust-analyzer | **3.9 MB** | 08:51:38 |
| 74278 | rust-analyzer-proc-macro-srv | **1.0 MB** | 08:51:38 |

**Findings:**
- No idle eviction — processes persisted for the entire session duration
- The OS reclaimed virtually all memory pages (from ~167 MB peak to 3.9 MB), but the processes never exited
- Memory would ramp back up immediately on the next LSP query as rust-analyzer re-indexes
- Conclusion: **plugin-managed LSP servers are session-scoped with no idle timeout**

`goToDefinition` on `run_server` at `src/main.rs:38:27`:

| Tool | Results | Format |
|------|---------|--------|
| Built-in `LSP` (goToDefinition) | 4 definitions | File paths with line:col |
| `mcp__symtrace-mcp__goto_definition` | 4 definitions | File paths with line:col + context lines |

Both returned the same 4 definitions (project source, tokio runtime builder, core::result::Result, tokio runtime). symtrace-mcp's output includes surrounding source lines as context.

### `/clear` Command Behavior (2026-05-17)

After running `/clear` (which resets conversation context but keeps the CLI process alive):

| PID | Process | Status |
|-----|---------|--------|
| 74091 | rust-analyzer | Still running (same PID from extended idle test) |
| 74278 | rust-analyzer-proc-macro-srv | Still running (same PID from extended idle test) |
| 64220 | symtrace-mcp | Still running |

**Findings:**
- `/clear` does **not** kill LSP server processes — they are tied to the CLI process lifetime, not the conversation
- The built-in plugin's `rust-analyzer` survives across conversation resets (confirmed: same PIDs before and after `/clear`)
- symtrace-mcp (as an MCP server) also survives `/clear` — the MCP connection persists at the process level
- In the new session after `/clear`, Claude Code reconnects to the already-running servers rather than spawning new ones

### Still Unknown

~~- Whether the plugin's rust-analyzer eventually shuts down after extended idle time~~ **Answered: No.** Survived 9 hours idle. Processes persist for the session lifetime.
~~- Whether the CLI reuses a running server across sessions or starts fresh each time~~ **Answered: Servers survive `/clear`.** They are tied to the CLI process, not the conversation.
- How the plugin handles multi-root workspaces (multiple Cargo projects)
- Whether auto-diagnostics can be disabled independently from code navigation

## Sources

- Official plugin discovery docs: https://code.claude.com/docs/en/discover-plugins#code-intelligence
- Plugin source repository: https://github.com/anthropics/claude-plugins-official
- Troubleshooting docs (partially accessible, 403 on some pages): https://code.claude.com/docs/en/code-intelligence-troubleshooting
