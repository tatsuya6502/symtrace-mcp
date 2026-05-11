## Context

P0 built the foundational layers: `LspTransport` (JSON-RPC 2.0 over stdio to child processes), LSP types, and an `McpServer` skeleton that handles `initialize` and `tools/list` with an empty tool registry. The server compiles and responds to MCP requests but has no functional tools.

During P1 testing, a framing bug was discovered in `src/mcp/protocol.rs`: the MCP stdio layer was using `Content-Length` header framing (which is correct for LSP but not for MCP). Claude Code's MCP client sends newline-delimited JSON with no headers. Fixed by replacing the header-parsing loop with a single `read_line` call and replacing the `Content-Length` write with a `body + "\n"` write.

P1 fills in the middle and lower layers: an `LspClient` that manages the LSP lifecycle on top of `LspTransport`, a `LanguageServerManager` that lazily starts rust-analyzer, and three concrete MCP tools (`find_references`, `goto_definition`, `find_implementations`).

The design follows the layered architecture from `doc/spec-2026-05-10.md` sections 4–6.

## Goals / Non-Goals

**Goals:**
- Implement `LspClient` that can start a language server (`initialize` + `initialized`), manage open files, run queries, and shut down cleanly
- Implement `FileManager` that sends `textDocument/didOpen`/`didChange`/`didClose` with version tracking and mtime checks
- Implement `LanguageServerManager` with lazy startup (start on first tool invocation, wait for readiness)
- Implement `IdleMonitor` that shuts down idle servers after a configurable timeout (default 600s)
- Register three MCP tools that dispatch to `LspClient` query methods
- Output human-readable text by default (spec §6.5), with optional `json: true` for structured output
- Index readiness wait via `textDocument/documentSymbol` polling (spec §11.1)

**Non-Goals:**
- TypeScript and Python support (P3)
- `incoming_calls`, `outgoing_calls`, `hover`, `diagnostics`, `rename` (P2, P4)
- Configuration file (`.symtrace.toml`) parsing (P3)
- Multi-project support (future)
- Persistent file synchronization (only open files needed for queries)

## Decisions

### D1: LspClient wraps LspTransport with lifecycle methods

**Choice**: `LspClient` owns an `LspTransport` and exposes high-level async methods: `start`, `shutdown`, `ensure_file_open`, `close_file`, `goto_definition`, `references`, `implementations`.

**Rationale**: Matches the spec §4.2 design exactly. `LspTransport` handles framing; `LspClient` handles protocol semantics (which methods to call, what params to send, how to parse responses).

### D2: FileManager tracks open files with mtime-based staleness checks

**Choice**: `FileManager` maintains a `HashMap<Uri, OpenFile>` where `OpenFile` stores `version` and `modified_at` (file mtime). `ensure_open` reads the file from disk, compares mtime, and sends `didChange` if the file was modified since last open.

**Rationale**: Spec §4.3 requires reading file contents from disk each time. Mtime checks avoid redundant `didChange` notifications when the file hasn't changed. Version counters ensure the language server tracks content updates correctly.

### D3: LanguageServerManager uses tokio::sync::Mutex for interior mutability

**Choice**: `LanguageServerManager` holds `clients: Mutex<HashMap<Language, LspClient>>` and `file_managers: Mutex<HashMap<Language, FileManager>>`. The `get_client_for_file` method acquires the lock, starts the server if needed (lazy), and returns a `MutexGuard<LspClient>`.

**Rationale**: The manager is shared between the MCP tool handlers (async) and the idle monitor (background task). A tokio `Mutex` allows holding the lock across `.await` points during server startup. Returning a `MutexGuard` gives callers direct access without additional wrapping.

### D4: IdleMonitor as a spawned background task with touch-based tracking

**Choice**: `IdleMonitor` runs as a tokio task, checking every 60s (configurable) whether each active server's last-used timestamp exceeds the idle timeout. Tool handlers call `touch(language)` on each invocation. When a server is idle, the monitor calls `stop_server` on the manager.

**Rationale**: Matches spec §5.2. The monitor must run concurrently with tool handlers, so a background task is natural. Using `Instant` timestamps with periodic checks is simpler than per-server timers and handles the multi-server case cleanly.

### D5: Single language (Rust) with hardcoded config

**Choice**: Hardcode a `LanguageServerConfig` for rust-analyzer with known defaults (command: `rust-analyzer`, args: `[]`, extensions: `["rs"]`, idle_timeout: 600s). No config file parsing.

**Rationale**: P1 is single-language. Hardcoding avoids the config file parsing work (P3) while still going through the `LanguageServerManager` abstraction so adding languages later requires no structural changes.

### D6: Human-readable output by default, optional JSON

**Choice**: Tool handlers return text output (spec §6.5 format) by default. When the `json` parameter is `true`, return a structured JSON result.

**Rationale**: AI agents consume text more easily than structured JSON for most use cases. The `json` flag provides a machine-readable escape hatch.

### D7: Index readiness via documentSymbol polling

**Choice**: After `initialize`, poll `textDocument/documentSymbol` on a known file (or `workspaceSymbol`) with a short interval until a non-empty result arrives, with a timeout fallback.

**Rationale**: Spec §11.1. rust-analyzer needs time to index the workspace. Queries before indexing completes return empty results. Polling `documentSymbol` is a concrete signal that the server is ready.

## Risks / Trade-offs

- **[Risk] rust-analyzer startup time** — Large workspaces may take 30–60s to index. The tool call will block until ready. → Mitigated by the documentSymbol polling approach; the first tool call waits but subsequent calls are instant. The idle timeout prevents keeping the server running unnecessarily.
- **[Risk] LspClient not thread-safe** — `LspClient` holds `LspTransport` which has `RefCell`-like semantics (only one mutable borrow at a time via `MutexGuard`). → Acceptable because `LanguageServerManager` serializes access through a single `Mutex`. Only one MCP tool handler can use a given language client at a time.
- **[Trade-off] No file watching** — We don't forward `didChangeWatchedFiles` from the OS to the language server. → Acceptable for P1. rust-analyzer has its own file watcher. If files change between tool calls, the next `ensure_open` call will detect the mtime change and send `didChange`.
- **[Trade-off] tokio Mutex vs std Mutex** — tokio Mutex is slightly slower but allows holding across `.await` during lazy server startup. → Acceptable because tool calls are infrequent (user-driven, not hot-path).
