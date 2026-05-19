## Context

symtrace-mcp bridges LSP servers to AI coding assistants via MCP. The core data flow is:

```
MCP request → handler → ProjectRegistry → LanguageServerManager → LspClient → LspTransport → child process
```

`LspClient` is a concrete struct that owns `LspTransport` (which owns a `tokio::process::Child`). There is no way to create an `LspClient` without spawning a real language server process. This makes unit testing impossible for:

- Handler query dispatch (14 tests exist, all test formatting/parsing — none test the LSP call path)
- FileManager logic (mtime-based re-open, version tracking — zero tests)
- Client-side capability gating (hover null handling, diagnostic provider fallback — zero tests)

The codebase has 40 inline tests across 7 modules. The untested layers (`transport.rs` 304 lines, `protocol.rs` 119 lines, `file_manager.rs` 138 lines, `tools.rs` 351 lines) represent ~900 lines of production code with zero coverage.

## Goals / Non-Goals

**Goals:**
- Extract an `LspClientApi` trait so handlers can be tested with mock LSP responses
- Enable unit tests for all handler query paths without a real language server
- Enable unit tests for FileManager logic (didOpen/didChange/didClose lifecycle)
- Enable unit tests for LspClient capability gating (hover null, diagnostic fallback)

**Non-Goals:**
- Integration tests with a real language server (separate change)
- Changing the MCP tool interface or user-visible behavior
- Adding a transport-level trait (LspTransport mocking)
- Test coverage for transport.rs, protocol.rs, or tools.rs protocol dispatch

## Decisions

### D1: Trait on LspClient (not transport)

**Decision:** Define `LspClientApi` trait covering ~12 methods on LspClient.

**Alternatives considered:**
- *Transport trait (2-3 methods):* Narrower surface, but still requires a real LspClient to test handlers. FileManager needs real didOpen/didChange. Two seams instead of one.
- *Both transport + client traits:* More flexible but more complexity. The client trait alone is sufficient for handler-level isolation.

**Rationale:** The client trait gives full handler isolation with a single seam. mockall auto-generates the mock, so the 12-method surface area has zero maintenance cost (add method to trait → mock gains it automatically). Handlers access the client through `ServerEntry`, so the trait replaces the concrete type at the storage layer.

### D2: `async-trait` crate (not native async fn in traits)

**Decision:** Use the `async-trait` crate for the trait definition.

**Rationale:** Native `async fn` in traits (stable since Rust 1.75) does not support `dyn` dispatch. Since handlers use `Box<dyn LspClientApi>` for mock injection, `async-trait` is required. Its desugaring (`fn method(self: &Self) -> Pin<Box<dyn Future>>`) is the standard pattern for dyn-compatible async traits.

**Dependency cost:** `async-trait` is a widely-used, zero-cost abstraction (the macro generates the same code you'd write by hand). It's a dev + prod dependency since the trait is in production code.

### D3: `mockall` for mock generation

**Decision:** Use `mockall` with `#[cfg_attr(test, automock)]` on the trait.

**Rationale:** mockall is the de-facto Rust mocking library (128M+ downloads, actively maintained). With the correct macro ordering (`automock` before `async_trait`), the `.returning()` closures in tests are synchronous — no Future boilerplate. The generated `MockLspClientApi` provides `.expect_*()` methods for setting per-test expectations.

**Dependency cost:** `mockall` is dev-only. Only needed in test code.

### D4: `Box<dyn LspClientApi>` in ServerEntry

**Decision:** `ServerEntry { client: Box<dyn LspClientApi>, file_manager: FileManager }`.

**Alternatives considered:**
- *Generic `ServerEntry<C: LspClientApi>`:* Would propagate generics through `LanguageServerManager`, `ProjectRegistry`, and handlers. Template explosion with no benefit — there's exactly one production type and one test type.
- *Erase via enum:* Would require manually dispatching every method. No benefit over trait objects.

**Rationale:** A trait object is the standard Rust pattern for "one type in production, another in tests." The vtable overhead is negligible (LSP requests are ms-scale). The boxing happens once per server startup, not per request.

### D5: Trait method selection

**Decision:** 12 methods on the trait + `shutdown`.

Methods on trait:
- File lifecycle: `did_open`, `did_change`, `did_close` (called by FileManager)
- Query methods: `goto_definition`, `references`, `implementations`, `hover`, `diagnostic`, `rename`, `prepare_call_hierarchy`, `incoming_calls`, `outgoing_calls`
- Lifecycle: `shutdown(self)` (for clean teardown in stop_server/shutdown_all)

Methods NOT on trait (stay on concrete `LspClient`):
- `start()` — factory that creates the concrete type and spawns a child process
- `wait_for_index()`, `workspace_symbol()`, `document_symbol()` — only called during startup, before boxing
- `call_hierarchy_supported()` — private helper
- `capabilities()`, `root_uri()`, `is_file_open()`, `mark_file_closed()` — internal state accessors used only by LspClient/FileManager internals

**Rationale:** The trait covers exactly the methods that handlers and FileManager call through the `ServerEntry`. Startup methods are called on the concrete type before boxing. Internal accessors are implementation details.

### D6: FileManager parameter change

**Decision:** `FileManager::ensure_open`, `close`, `close_all` accept `&mut dyn LspClientApi` instead of `&mut LspClient`.

**Rationale:** FileManager calls `did_open`, `did_change`, `did_close` on the client — all trait methods. With the trait parameter, FileManager can be tested with a mock client (e.g., testing mtime-based re-open logic without file I/O or a real server).

### D7: Test structure

**Decision:** Inline `#[cfg(test)]` modules, same pattern as existing tests.

Tests to add:
- `lsp/client.rs`: Capability gating tests (hover null, diagnostic provider fallback, rename disabled)
- `lsp/file_manager.rs`: didOpen/didChange lifecycle tests with MockLspClientApi
- `mcp/handlers.rs`: Query dispatch tests with MockLspClientApi (each query kind, error mapping, capability check)
- No new test files — follow existing project convention

## Risks / Trade-offs

**[Vtable dispatch on every Lsp method call]** → Negligible. LSP requests take 1-50ms over stdio; vtable dispatch is ~1ns. Measured impact: zero.

**[async-trait proc macro adds compile time]** → Small. `async-trait` is a lightweight macro. symtrace-mcp is a small crate (under 4000 LOC). Acceptable.

**[Box allocation per server startup]** → Negligible. Boxing happens once when a language server starts (seconds-scale operation). No per-request allocation.

**[Trait surface area maintenance]** → Mitigated by mockall. Adding a new method to LspClient means adding one line to the trait. The mock is auto-generated. The cost is proportional to the benefit (each new method is immediately testable).

**[shutdown(self) on trait requires careful handling]** → `async_trait` desugars `async fn shutdown(self)` to `fn shutdown(self: Box<Self>)`. Since ServerEntry holds `Box<dyn LspClientApi>`, the entry is consumed naturally when removed from the HashMap. Works correctly.
