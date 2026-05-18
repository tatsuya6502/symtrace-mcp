## 1. Dependencies

- [ ] 1.1 Add `async-trait = "0.1"` to `[dependencies]` in Cargo.toml
- [ ] 1.2 Add `mockall = "0.14"` to `[dev-dependencies]` in Cargo.toml

## 2. LspClientApi Trait Extraction

- [ ] 2.1 Define `LspClientApi` trait in `src/lsp/client.rs` with `#[cfg_attr(test, automock)]` and `#[async_trait]`, declaring all 13 methods (3 file lifecycle + 9 query + shutdown) with `Send + Sync` bounds
- [ ] 2.2 Implement `LspClientApi` for `LspClient` by moving existing method bodies to the trait impl block
- [ ] 2.3 Keep `start`, `wait_for_index`, `workspace_symbol`, `document_symbol`, `call_hierarchy_supported`, `capabilities`, `root_uri`, `is_file_open`, `mark_file_closed` as concrete methods on `LspClient` (not on the trait)

## 3. FileManager Parameter Update

- [ ] 3.1 Change `FileManager::ensure_open` parameter from `&mut LspClient` to `&mut dyn LspClientApi`
- [ ] 3.2 Change `FileManager::close` parameter from `&mut LspClient` to `&mut dyn LspClientApi`
- [ ] 3.3 Change `FileManager::close_all` parameter from `&mut LspClient` to `&mut dyn LspClientApi`
- [ ] 3.4 Update `FileError::Client` to reference `ClientError` (unchanged) and verify `From<ClientError>` still works

## 4. ServerEntry Boxing

- [ ] 4.1 Change `ServerEntry::client` field from `LspClient` to `Box<dyn LspClientApi>`
- [ ] 4.2 Update `start_server_internal` to box `LspClient` after startup and indexing: `Box::new(client) as Box<dyn LspClientApi>`
- [ ] 4.3 Update `stop_server` and `shutdown_all` to work with the boxed trait object (destructure entry, call `fm.close_all(&mut *client)`, call `client.shutdown()`)

## 5. Compilation Verification

- [ ] 5.1 Run `cargo clippy --all-targets --tests -- -D warnings` and fix any issues
- [ ] 5.2 Run `cargo fmt --all -- --check` and fix any issues
- [ ] 5.3 Run `cargo test` and verify all 40 existing tests still pass

## 6. Unit Tests — LspClient Capability Gating

- [ ] 6.1 Add test: `hover` returns `None` when server returns null response
- [ ] 6.2 Add test: `hover` handles `MarkedString` object `{ language, value }` format
- [ ] 6.3 Add test: `diagnostic` falls back to push-diagnostics cache when server lacks pull-diagnostic provider
- [ ] 6.4 Add test: `diagnostic` sends `textDocument/diagnostic` when pull-diagnostics provider is enabled
- [ ] 6.5 Add test: `rename` returns `None` when server returns null

## 7. Unit Tests — FileManager

- [ ] 7.1 Add test: `ensure_open` sends `didOpen` for a new file with correct language_id
- [ ] 7.2 Add test: `ensure_open` sends `didChange` when file mtime has changed
- [ ] 7.3 Add test: `ensure_open` is a no-op when file is unchanged
- [ ] 7.4 Add test: `close` sends `didClose` and removes tracking

## 8. Unit Tests — Handler Query Dispatch

- [ ] 8.1 Add test helper: construct `ServerEntry` with `MockLspClientApi` and real `FileManager` backed by temp files
- [ ] 8.2 Add test: `find_references` handler with mock returning locations, verify text output format
- [ ] 8.3 Add test: `find_references` handler with mock returning empty, verify "No references found"
- [ ] 8.4 Add test: `goto_definition` handler with mock returning locations, verify JSON output format
- [ ] 8.5 Add test: `hover` handler with mock returning hover content, verify formatted output
- [ ] 8.6 Add test: `diagnostics` handler with mock returning diagnostics, verify severity names
- [ ] 8.7 Add test: `rename` handler with mock returning workspace edits, verify text and JSON output
- [ ] 8.8 Add test: handler receives `ClientError::Transport` from mock, verify tool error response

## 9. Final Verification

- [ ] 9.1 Run `cargo clippy --all-targets --tests -- -D warnings`
- [ ] 9.2 Run `cargo fmt --all -- --check`
- [ ] 9.3 Run `cargo test` and verify all tests pass (old + new)
