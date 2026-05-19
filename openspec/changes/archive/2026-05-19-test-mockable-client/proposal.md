## Why

LspClient is a concrete struct that owns `LspTransport` (which owns a child process). There is no way to create an LspClient without spawning a real language server. This means the handler logic that dispatches LSP queries — the core of what symtrace-mcp does — has zero unit test coverage. Only pure formatting helpers are tested today. As the codebase grows (more LSP methods, more languages), the gap between "code that runs" and "code that's tested" will widen.

Extracting a trait from LspClient unlocks mock-based unit tests for handlers, FileManager, and client-side capability gating — without needing a real language server.

## What Changes

- Extract `LspClientApi` trait from `LspClient` covering file lifecycle methods (`did_open`, `did_change`, `did_close`) and all query methods (`goto_definition`, `references`, `implementations`, `hover`, `diagnostic`, `rename`, `prepare_call_hierarchy`, `incoming_calls`, `outgoing_calls`, `shutdown`)
- Add `async-trait` dependency (required for `dyn` dispatch with async methods)
- Add `mockall` dev-dependency for auto-generated mock implementations
- Change `ServerEntry` to hold `Box<dyn LspClientApi>` instead of `LspClient`
- Change `FileManager` methods to accept `&mut dyn LspClientApi` instead of `&mut LspClient`
- Add unit tests using `MockLspClientApi` for handler query paths, FileManager logic, and LspClient capability gating

## Capabilities

### New Capabilities
- `lsp-client-api`: Trait definition for `LspClientApi` and its requirements (method signatures, error types, `Send + Sync` bounds)

### Modified Capabilities
- `lsp-client`: LspClient now implements `LspClientApi`. Constructor and lifecycle methods (`start`, `wait_for_index`) remain on the concrete type. Internal state accessors (`capabilities`, `root_uri`, `is_file_open`, `mark_file_closed`) remain concrete.
- `server-manager`: `ServerEntry` holds `Box<dyn LspClientApi>`. `start_server_internal` boxes the client after initialization.
- `tools-definitions`: Handler functions dispatch through trait methods on `dyn LspClientApi` (no signature changes, dispatch is transparent via trait).

## Impact

- **Dependencies**: New `async-trait` (prod), `mockall` (dev)
- **Code**: Changes to `lsp/client.rs` (trait extraction), `lsp/file_manager.rs` (dyn parameter), `server/manager.rs` (boxed client). Handler files (`mcp/handlers.rs`) unchanged at call sites.
- **Performance**: Vtable dispatch on every LspClient method call. Negligible — LSP requests are ms-scale, vtable overhead is ns-scale.
- **No breaking API changes**: MCP tool interface is unchanged. Config format unchanged.
