## 1. TypeScript Language Support

- [ ] 1.1 Add `Language::TypeScript` variant to the `Language` enum in `src/server/manager.rs`
- [ ] 1.2 Add default TypeScript server config to `default_configs()` in `src/server/manager.rs` (command: `typescript-language-server`, args: `["--stdio"]`, extensions: `["ts", "tsx", "js", "jsx"]`, language_id: `"typescript"`)
- [ ] 1.3 Add default TypeScript server config to `default_server_configs()` in `src/project/registry.rs`
- [ ] 1.4 Handle `"typescript"` key in `build_server_configs()` in `src/project/registry.rs` (same pattern as `"rust"`)
- [ ] 1.5 Create `src/language/typescript.rs` with `client_capabilities()` (same as Rust minus `textDocument.diagnostic`)
- [ ] 1.6 Wire language-specific capabilities into `LspClient::start()` or `ServerManager` so TypeScript uses its own capabilities

## 2. Push Diagnostics Infrastructure

- [ ] 2.1 Add `moka` dependency with `future` feature to `Cargo.toml`
- [ ] 2.2 Add `mpsc::UnboundedSender<(String, Value)>` to `LspTransport` and return the receiver from `spawn()`
- [ ] 2.3 Update `reader_task()` to send server notifications through the mpsc channel instead of logging them
- [ ] 2.4 Handle dropped receiver gracefully (channel closed on client shutdown)

## 3. Diagnostics Cache on LspClient

- [ ] 3.1 Add `moka::future::Cache<String, Vec<Diagnostic>>` field to `LspClient` with configurable TTL (default 600s, matching idle timeout)
- [ ] 3.2 Add notification processing: spawn a task on `LspClient::start()` that reads from the notification receiver and updates the cache for `textDocument/publishDiagnostics`
- [ ] 3.3 Parse `publishDiagnostics` notification params (URI + diagnostics array) into cache entries
- [ ] 3.4 Invalidate cache entry for URI in `did_change(uri)` and `did_open(uri)` via `cache.invalidate(&uri)`

## 4. Capability-Aware Diagnostics

- [ ] 4.1 Update `LspClient::diagnostic()` to check `diagnosticProvider` capability: use pull path if supported, otherwise read from moka cache
- [ ] 4.2 Return empty `Vec<Diagnostic>` on cache miss (push-only server, no diagnostics received yet)

## 5. Tests

- [ ] 5.1 Unit test: TypeScript default config has correct command, args, extensions, language_id
- [ ] 5.2 Unit test: `.ts`/`.tsx`/`.js`/`.jsx` extensions resolve to `Language::TypeScript`
- [ ] 5.3 Unit test: TypeScript client capabilities do not include `textDocument.diagnostic`
- [ ] 5.4 Unit test: config with `[server.typescript]` produces correct `LanguageServerConfig`
- [ ] 5.5 Unit test: notification dispatch sends `(method, params)` through mpsc channel
- [ ] 5.6 Unit test: `diagnostic()` reads from moka cache when pull diagnostics not supported
- [ ] 5.7 Unit test: `diagnostic()` returns empty vec on cache miss
- [ ] 5.8 Unit test: `diagnostic()` uses pull path when server supports it (existing behavior preserved)
- [ ] 5.9 Unit test: cache entry invalidated on `did_change` for the changed URI only
- [ ] 5.10 Unit test: cache entry invalidated on `did_open`

## 6. Lint and Verify

- [ ] 6.1 Run `cargo clippy --all-targets --tests -- -D warnings` with no warnings
- [ ] 6.2 Run `cargo fmt --all -- --check` with no issues
- [ ] 6.3 Run `cargo test` with all tests passing

## 7. Documentation

- [ ] 7.1 Update README.md with TypeScript support (supported languages, config example, tool compatibility)
- [ ] 7.2 Update CLAUDE.md if needed (new dependencies, config changes)
