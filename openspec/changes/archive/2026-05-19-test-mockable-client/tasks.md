## 1. Dependencies

- [x] 1.1 Add `async-trait = "0.1"` to `[dependencies]` in Cargo.toml
- [x] 1.2 Add `mockall = "0.14"` to `[dev-dependencies]` in Cargo.toml

## 2. LspClientApi Trait Extraction

- [x] 2.1 Define `LspClientApi` trait in `src/lsp/client.rs` with `#[cfg_attr(test, automock)]` and `#[async_trait]`, declaring all 13 methods (3 file lifecycle + 9 query + shutdown) with `Send + Sync` bounds
- [x] 2.2 Implement `LspClientApi` for `LspClient` by moving existing method bodies to the trait impl block
- [x] 2.3 Keep `start`, `wait_for_index`, `workspace_symbol`, `document_symbol`, `call_hierarchy_supported`, `capabilities`, `root_uri`, `is_file_open`, `mark_file_closed` as concrete methods on `LspClient` (not on the trait)

## 3. FileManager Parameter Update

- [x] 3.1 Change `FileManager::ensure_open` parameter from `&mut LspClient` to `&mut dyn LspClientApi`
- [x] 3.2 Change `FileManager::close` parameter from `&mut LspClient` to `&mut dyn LspClientApi`
- [x] 3.3 Change `FileManager::close_all` parameter from `&mut LspClient` to `&mut dyn LspClientApi`
- [x] 3.4 Update `FileError::Client` to reference `ClientError` (unchanged) and verify `From<ClientError>` still works

## 4. ServerEntry Boxing

- [x] 4.1 Change `ServerEntry::client` field from `LspClient` to `Box<dyn LspClientApi>`
- [x] 4.2 Update `start_server_internal` to box `LspClient` after startup and indexing: `Box::new(client) as Box<dyn LspClientApi>`
- [x] 4.3 Update `stop_server` and `shutdown_all` to work with the boxed trait object (destructure entry, call `fm.close_all(&mut *client)`, call `client.shutdown()`)

## 5. Compilation Verification

- [x] 5.1 Run `cargo clippy --all-targets --tests -- -D warnings` and fix any issues
- [x] 5.2 Run `cargo fmt --all -- --check` and fix any issues
- [x] 5.3 Run `cargo test` and verify all 40 existing tests still pass

## 6. Unit Tests — LspClient Capability Gating

- [x] 6.1 Add test: `hover` returns `None` when server returns null response
- [x] 6.2 Add test: `hover` handles `MarkedString` object `{ language, value }` format
- [x] 6.3 Add test: `diagnostic` falls back to push-diagnostics cache when server lacks pull-diagnostic provider
- [x] 6.4 Add test: `diagnostic` sends `textDocument/diagnostic` when pull-diagnostics provider is enabled
- [x] 6.5 Add test: `rename` returns `None` when server returns null

## 7. Unit Tests — FileManager

- [x] 7.1 Add test: `ensure_open` sends `didOpen` for a new file with correct language_id
- [x] 7.2 Add test: `ensure_open` sends `didChange` when file mtime has changed
- [x] 7.3 Add test: `ensure_open` is a no-op when file is unchanged
- [x] 7.4 Add test: `close` sends `didClose` and removes tracking

## 8. Unit Tests — Handler Query Dispatch

- [x] 8.1 Add test helper: construct `ServerEntry` with `MockLspClientApi` and real `FileManager` backed by temp files
- [x] 8.2 Add test: `find_references` handler with mock returning locations, verify text output format
- [x] 8.3 Add test: `find_references` handler with mock returning empty, verify "No references found"
- [x] 8.4 Add test: `goto_definition` handler with mock returning locations, verify JSON output format
- [x] 8.5 Add test: `hover` handler with mock returning hover content, verify formatted output
- [x] 8.6 Add test: `diagnostics` handler with mock returning diagnostics, verify severity names
- [x] 8.7 Add test: `rename` handler with mock returning workspace edits, verify text and JSON output
- [x] 8.8 Add test: handler receives `ClientError::Transport` from mock, verify tool error response

## 9. Final Verification

- [x] 9.1 Run `cargo clippy --all-targets --tests -- -D warnings`
- [x] 9.2 Run `cargo fmt --all -- --check`
- [x] 9.3 Run `cargo test` and verify all tests pass (old + new)
