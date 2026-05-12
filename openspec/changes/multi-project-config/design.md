## Context

symtrace-mcp is an MCP server that provides code exploration tools (find references, go to definition, find implementations) by driving LSP servers like rust-analyzer. Currently it assumes a single project root (CWD at startup) and runs one LSP server per language. The key data structure is `LanguageServerManager` with `servers: HashMap<Language, ServerEntry>` — one server instance per language, keyed solely by `Language`.

The server identity is `(Language,)` when it needs to become `(Project, Language)` to support monorepo setups with multiple independent Rust projects.

## Goals / Non-Goals

**Goals:**
- Support multiple independent projects in a single repository, each with its own LSP server instance
- Configuration via `.symtrace.toml` at the Claude Code launch directory (CWD)
- Longest-prefix-match routing from file paths to the correct project manager
- Per-project idle monitoring and server lifecycle
- Full backward compatibility when no config file exists

**Non-Goals:**
- Auto-detection of project roots (explicit configuration only)
- Per-project language server command overrides (deferred; global `[server]` section only for now)
- Runtime config reload or dynamic project addition/removal
- Multi-root LSP workspace protocol support
- Supporting languages other than Rust (architecture should not preclude it, but only Rust is configured)

## Decisions

### D1: Manager-per-project architecture (Approach B)

**Decision:** Introduce a `ProjectRegistry` layer that owns `HashMap<PathBuf, Arc<LanguageServerManager>>`, one manager per configured project root. Existing `LanguageServerManager` code remains largely unchanged.

**Alternatives considered:**
- **Approach A (flat HashMap with composite key):** Change `servers: HashMap<Language, ServerEntry>` to `HashMap<(PathBuf, Language), ServerEntry>`. More invasive — requires modifying `LanguageServerManager`, `IdleMonitor`, and all handlers. Single MutexGuard would serialize requests across all projects.
- **Approach B (manager per project):** New `ProjectRegistry` as a routing layer; each manager operates independently. Minimal changes to existing code. MutexGuard serialization is scoped per-project, enabling concurrent access across projects.

**Rationale:** Approach B has lower blast radius on existing tested code, natural concurrency boundaries per project, and follows the principle of composition over modification.

### D2: Immutable registry after construction

**Decision:** `ProjectRegistry` is constructed once at startup from the config file and never mutated. The project-to-manager mapping is read-only at runtime.

**Implementation:** `Arc<HashMap<PathBuf, Arc<LanguageServerManager>>>`. No RwLock or concurrent map needed. If runtime updates become needed later, `papaya` (a concurrent hash map) can replace the `Arc<HashMap>`.

### D3: Config file location and format

**Decision:** `.symtrace.toml` at CWD (the directory where Claude Code was launched). TOML format with `serde` deserialization.

```toml
[server.rust]
command = "rust-analyzer"
idle_timeout_secs = 600

[[projects]]
root = "project-a"

[[projects]]
root = "project-b"
```

- `[[projects]]` is optional — if absent, implicit single project at CWD with defaults
- `[server.<lang>]` is optional — if absent, hardcoded defaults per language
- `root` paths are relative to the config file's parent directory (CWD), canonicalized at load time

**Alternatives considered:**
- YAML/JSON config: TOML is the Rust ecosystem standard; `toml` crate is lightweight.
- Auto-detection by walking up to `Cargo.toml`: Rejected per requirements (explicit config only).
- Config at repo root vs CWD: CWD matches where Claude Code launches and where the MCP server stdin/stdout connects.

### D4: Longest-prefix-match for file routing

**Decision:** Sort project roots by path length (descending). For a given file path, return the first project root that is a prefix of the file path.

**Edge case handling:**
- Overlapping roots (e.g., `project-a/` and `project-a/sub-crate/`): longest prefix wins, routing to the more specific project.
- No match: return an error to the MCP client with a descriptive message.

### D5: IdleMonitor per manager

**Decision:** Each `LanguageServerManager` owns its own `IdleMonitor`. `McpServer::run()` spawns a background task for each manager's monitor.

**Current state:** `McpServer` directly owns `Arc<IdleMonitor>`. This changes to `ProjectRegistry` iterating all managers and spawning their monitors.

### D6: Implicit single-project mode (backward compatibility)

**Decision:** When `.symtrace.toml` does not exist, generate an implicit config:
- `projects = [ { root: CWD } ]`
- `servers = default_configs()` (hardcoded rust-analyzer)

This means all code paths always go through `ProjectRegistry`. No separate "legacy" code path needed.

## Risks / Trade-offs

- **[Config not found at CWD]** → The tool may be launched from an unexpected directory. Error message should clearly state which directory was searched for `.symtrace.toml`.
- **[Canonicalized paths diverge from user expectation]** → Symlinks or relative paths may resolve differently. Canonicalize all paths at load time and use canonicalized paths for prefix matching.
- **[TOML parsing errors are silent]** → Log parse errors at startup and fall back to single-project mode with a warning, rather than crashing.
- **[IdleMonitor lifecycle]** → Each monitor's background task must be properly cleaned up if `McpServer` shuts down. Use `tokio::JoinHandle` and cancellation via `CancellationToken` or drop.
