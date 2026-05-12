## Why

symtrace-mcp currently assumes a single project root (CWD at startup), running one LSP server per language. This limits the tool to monolithic repositories. Monorepo setups with multiple independent Rust projects (each with their own `Cargo.toml`) need separate rust-analyzer instances, each rooted at the correct project directory, to provide accurate code intelligence.

## What Changes

- Add `.symtrace.toml` config file support at the Claude Code launch directory
- Introduce a `ProjectRegistry` layer that owns multiple `LanguageServerManager` instances, one per configured project
- Route file-based tool calls to the correct manager using longest-prefix-match on project roots
- Per-project `IdleMonitor` instances spawned from `McpServer::run()`
- Maintain backward compatibility: no config file = implicit single-project mode (CWD as root, default server configs)

## Capabilities

### New Capabilities
- `project-registry`: Multi-project routing layer that maps file paths to project-scoped language server managers via longest-prefix-match
- `config-file`: `.symtrace.toml` parsing and project/server configuration loading

### Modified Capabilities
- `server-manager`: `LanguageServerManager` construction changes from single-root to per-project instantiation; `McpServer` ownership shifts from direct `Arc<LanguageServerManager>` to `Arc<ProjectRegistry>`
- `mcp-server`: Tool handlers route through `ProjectRegistry` instead of directly calling `LanguageServerManager`; idle monitor spawning iterates all project managers

## Impact

- **Code**: New `ProjectRegistry` struct; new config module for `.symtrace.toml` parsing; modified `McpServer` initialization and tool handler closures; `IdleMonitor` lifecycle changes
- **Dependencies**: Add `toml` crate (config parsing); `serde` already in use for LSP types
- **API**: No MCP protocol changes; tool call signatures remain the same
- **Backward compat**: Fully preserved — absence of `.symtrace.toml` produces identical behavior to current single-project mode
