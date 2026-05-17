## Context

symtrace-mcp is a single-language (Rust) MCP server that bridges LSP capabilities to AI coding assistants. The LSP transport layer (`LspTransport`) already receives all server notifications in `reader_task()` but currently only logs them. The `LspClient::diagnostic()` method only supports pull diagnostics (`textDocument/diagnostic`), which `typescript-language-server` does not implement — it uses push diagnostics via `textDocument/publishDiagnostics` notifications instead.

The config system already uses `HashMap<String, ServerConfig>` — arbitrary language names are accepted at the TOML level. The gap is in `build_server_configs()` which only maps "rust" today.

## Goals / Non-Goals

**Goals:**
- Add TypeScript as a second language with minimal architecture changes
- Support all existing MCP tools for TypeScript files (goto definition, references, implementations, hover, rename, call hierarchy, document/workspace symbol, diagnostics)
- Establish the push diagnostics pattern (moka cache + notification dispatch) that future languages can reuse
- Keep the MCP handler layer unchanged — diagnostics tool works transparently regardless of pull vs push

**Non-Goals:**
- Python or other language support (future change)
- TypeScript-specific initialization options or tsconfig configuration (can be added later)
- `.js`/`.jsx` files using a separate language server (e.g., `vscode-eslint`) — all JS/TS extensions go to `typescript-language-server`
- Waiting for push diagnostics to arrive before responding to the MCP tool (cache miss returns "no diagnostics yet")

## Decisions

### D1: Notification dispatch via mpsc channel

**Decision:** Add `mpsc::UnboundedSender<(String, Value)>` to `LspTransport` and a corresponding receiver in `LspClient`. The `reader_task()` sends all server notifications through this channel instead of logging them.

**Alternatives considered:**
- Callback map in `LspTransport`: tighter coupling, harder to test, requires `Box<dyn Fn>` management
- Shared `Arc<NotificationBus>`: more flexible but overkill for a single notification type

**Rationale:** mpsc is the idiomatic tokio pattern. Unbounded channel avoids backpressure complexity (notification volume is low). The client can filter and dispatch in its own task. Future notification types (e.g., `textDocument/publishDiagnostics` for other languages) are handled without transport changes.

### D2: moka `future::Cache` for diagnostics storage

**Decision:** Use `moka::future::Cache<String, Vec<Diagnostic>>` keyed by file URI with a configurable TTL (default 600 seconds, matching idle timeout).

**Alternatives considered:**
- `moka::sync::Cache`: simpler but risks deadlocking when both the reader task and async handlers access the cache — the user (moka's author) recommended `future::Cache` for this reason
- `DashMap` with manual TTL: more code, reinventing what moka provides
- No cache (wait for push on each request): adds latency, complex synchronization

**Rationale:** `future::Cache` provides async-aware locking, TTL-based eviction (stale diagnostics auto-expire), and a simple API. The cache lives on `LspClient` — it naturally clears when the server goes idle and restarts, which is desired behavior (stale diagnostics from a previous server instance should not persist). Per-URI invalidation on `did_change`/`did_open` is the primary freshness mechanism; the 600s TTL is a safety net bounded by the idle timeout — the cache is destroyed on server shutdown regardless.

### D3: Capability-aware `diagnostic()` method with per-URI invalidation

**Decision:** `LspClient::diagnostic(uri)` checks the server's `diagnosticProvider` capability. If pull is supported, send `textDocument/diagnostic` (current behavior). If not, read from the moka cache. If the cache is empty (no push received yet), return an empty `Vec<Diagnostic>`.

When `did_change(uri)` or `did_open(uri)` is called, the cache entry for that URI is invalidated via `cache.invalidate(&uri)`. This ensures that after a file edit, the MCP tool won't return stale pre-edit diagnostics — instead it returns "No diagnostics found" until the server pushes fresh results. The URI is available at the call site, so no iteration or `invalidate_all` is needed.

**Alternatives considered:**
- Separate `pull_diagnostic()` and `push_diagnostic()` methods: leaks transport details to callers
- Always use cache, populate cache from pull results: conflates two different data sources
- `invalidate_all` on any file change: too aggressive — invalidates diagnostics for unrelated open files
- No invalidation (TTL only): stale pre-edit diagnostics could be returned for up to 60 seconds after a file change

**Rationale:** The MCP handler layer should not know about pull vs push. A single method with capability-based dispatch keeps the abstraction clean. Returning an empty vec on cache miss is simple and honest — the MCP tool formats it as "No diagnostics found", which is accurate. Per-URI invalidation on `did_change`/`did_open` prevents stale data without over-invalidating.

### D4: TypeScript default config with `--stdio`

**Decision:** Default TypeScript config: command `typescript-language-server`, args `["--stdio"]`, extensions `["ts", "tsx", "js", "jsx"]`, language_id `"typescript"`.

**Rationale:** `--stdio` is required for LSP communication over stdin/stdout. Including `.js`/`.jsx` is practical — `typescript-language-server` handles JavaScript files and most TS projects have JS interop. Users can override via config if they prefer a different setup.

### D5: `Language` enum with static dispatch

**Decision:** Add `TypeScript` variant to the existing `Language` enum. Use a `match` in `build_server_configs()` to map "typescript" → default config (same pattern as "rust").

**Alternatives considered:**
- Dynamic language registry (string-based): more flexible but loses compile-time exhaustiveness checking
- Generics/trait objects: over-engineering for two languages

**Rationale:** Match exhaustiveness ensures we handle all languages when new variants are added. The enum pattern is already established and works well for a small, known set of languages.

## Risks / Trade-offs

- **Cache miss on first diagnostics request** → The MCP tool returns "No diagnostics found" if the server hasn't pushed diagnostics yet. Users can retry. Mitigation: `typescript-language-server` pushes diagnostics quickly after `textDocument/didOpen`, so cache misses should be rare in practice.

- **Stale diagnostics from TTL** → Mitigated by per-URI invalidation on `did_change`/`did_open`. The 600s TTL is a safety net for edge cases (e.g., external file changes not routed through `ensure_open`). Between `did_change` and the server's `publishDiagnostics` push, the tool returns "No diagnostics found" instead of stale data. The TTL matches the idle timeout — the cache is destroyed on server shutdown regardless, so it never outlives the server.

- **Unbounded notification channel** → If the server floods notifications, the channel grows. Mitigation: LSP servers send notifications at reasonable rates. If this becomes an issue, switch to a bounded channel with drop-oldest policy.

- **`.js`/`.jsx` routed to TypeScript server** → Projects using plain JS without TypeScript might not have `typescript-language-server` installed. Mitigation: the server will fail to start with a clear error. Users can exclude JS extensions via config (future enhancement).
