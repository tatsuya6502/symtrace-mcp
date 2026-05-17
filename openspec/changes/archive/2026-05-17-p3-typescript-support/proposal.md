## Why

symtrace-mcp currently supports only Rust. Teams working on mixed Rust + TypeScript projects need TypeScript language support to use symtrace-mcp across their entire codebase. TypeScript is the second language to add, establishing the pattern for future language additions.

## What Changes

- Add `Language::TypeScript` variant with `typescript-language-server` as the default LSP server
- Route `.ts`, `.tsx`, `.js`, `.jsx` files to the TypeScript language server
- Add client capabilities for TypeScript (without pull diagnostics, which `typescript-language-server` does not support)
- Add push diagnostics support: listen for `textDocument/publishDiagnostics` notifications from LSP servers and cache results in a `moka::future::Cache` with TTL
- Make `LspClient::diagnostic()` capability-aware: use pull diagnostics when the server supports it (rust-analyzer), read from cache otherwise (typescript-language-server)
- Allow `[server.typescript]` configuration in `.symtrace.toml`

## Capabilities

### New Capabilities
- `push-diagnostics`: Caches LSP push diagnostics (`textDocument/publishDiagnostics`) via moka `future::Cache`, with notification dispatch from the LSP transport layer. Provides capability-aware diagnostics that transparently use pull or cached push results.

### Modified Capabilities
- `server-manager`: Add `Language::TypeScript` variant and default server config for `typescript-language-server` (command, args, extensions, language_id)
- `config-file`: Support `[server.typescript]` section in `.symtrace.toml`, mapping the "typescript" key to `Language::TypeScript`
- `lsp-transport`: Dispatch server notifications (previously just logged) via an mpsc channel to enable push diagnostics and future notification handling

## Impact

- **Dependencies**: Add `moka` crate (async feature) for TTL-based diagnostics caching
- **Code**: `src/server/manager.rs` (Language enum, default configs), `src/language/typescript.rs` (new), `src/lsp/transport.rs` (notification dispatch), `src/lsp/client.rs` (push diagnostics cache, capability-aware `diagnostic()`), `src/project/registry.rs` (config mapping), `src/config.rs` (no structural change — already uses `HashMap<String, ServerConfig>`)
- **Config**: Users add `[server.typescript]` to `.symtrace.toml`; existing configs unaffected
- **MCP tools**: No changes to tool schemas or handler logic — diagnostics tool works transparently via pull or push path
