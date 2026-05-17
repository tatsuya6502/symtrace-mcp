## 1. TypeScript Language Support

- [x] 1.1 Add `Language::TypeScript` variant to the `Language` enum in `src/server/manager.rs`
- [x] 1.2 Add default TypeScript server config to `default_configs()` in `src/server/manager.rs` (command: `typescript-language-server`, args: `["--stdio"]`, extensions: `["ts", "tsx", "js", "jsx"]`, language_id: `"typescript"`)
- [x] 1.3 Add default TypeScript server config to `default_server_configs()` in `src/project/registry.rs`
- [x] 1.4 Handle `"typescript"` key in `build_server_configs()` in `src/project/registry.rs` (same pattern as `"rust"`)
- [x] 1.5 Create `src/language/typescript.rs` with `client_capabilities()` (same as Rust minus `textDocument.diagnostic`)
- [x] 1.6 Wire language-specific capabilities into `LspClient::start()` or `ServerManager` so TypeScript uses its own capabilities

## 2. Push Diagnostics Infrastructure

- [x] 2.1 Add `moka` dependency with `future` feature to `Cargo.toml`
- [x] 2.2 Add `mpsc::UnboundedSender<(String, Value)>` to `LspTransport` and return the receiver from `spawn()`
- [x] 2.3 Update `reader_task()` to send server notifications through the mpsc channel instead of logging them
- [x] 2.4 Handle dropped receiver gracefully (channel closed on client shutdown)

## 3. Diagnostics Cache on LspClient

- [x] 3.1 Add `moka::future::Cache<String, Vec<Diagnostic>>` field to `LspClient` with configurable TTL (default 600s, matching idle timeout)
- [x] 3.2 Add notification processing: spawn a task on `LspClient::start()` that reads from the notification receiver and updates the cache for `textDocument/publishDiagnostics`
- [x] 3.3 Parse `publishDiagnostics` notification params (URI + diagnostics array) into cache entries
- [x] 3.4 Invalidate cache entry for URI in `did_change(uri)` and `did_open(uri)` via `cache.invalidate(&uri)`

## 4. Capability-Aware Diagnostics

- [x] 4.1 Update `LspClient::diagnostic()` to check `diagnosticProvider` capability: use pull path if supported, otherwise read from moka cache
- [x] 4.2 Return empty `Vec<Diagnostic>` on cache miss (push-only server, no diagnostics received yet)

## 5. Tests

- [x] 5.1 Unit test: TypeScript default config has correct command, args, extensions, language_id
- [x] 5.2 Unit test: `.ts`/`.tsx`/`.js`/`.jsx` extensions resolve to `Language::TypeScript`
- [x] 5.3 Unit test: TypeScript client capabilities do not include `textDocument.diagnostic`
- [x] 5.4 Unit test: config with `[server.typescript]` produces correct `LanguageServerConfig`
- [x] 5.5 Unit test: notification dispatch sends `(method, params)` through mpsc channel
- [x] 5.6 Unit test: `diagnostic()` reads from moka cache when pull diagnostics not supported
- [x] 5.7 Unit test: `diagnostic()` returns empty vec on cache miss
- [x] 5.8 Unit test: `diagnostic()` uses pull path when server supports it (existing behavior preserved)
- [x] 5.9 Unit test: cache entry invalidated on `did_change` for the changed URI only
- [x] 5.10 Unit test: cache entry invalidated on `did_open`

## 6. Lint and Verify

- [x] 6.1 Run `cargo clippy --all-targets --tests -- -D warnings` with no warnings
- [x] 6.2 Run `cargo fmt --all -- --check` with no issues
- [x] 6.3 Run `cargo test` with all tests passing

## 7. Documentation

- [x] 7.1 Update README.md with TypeScript support (supported languages, config example, tool compatibility)
- [x] 7.2 Update CLAUDE.md if needed (new dependencies, config changes)
