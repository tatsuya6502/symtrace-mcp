## 1. Config

- [x] 1.1 Add `env: Option<HashMap<String, String>>` field to `ProjectEntry` in `src/config.rs`
- [x] 1.2 Add unit test for parsing `[[projects]]` with `env` field
- [x] 1.3 Add unit test for parsing `[[projects]]` without `env` field (backward compat)

## 2. Transport

- [x] 2.1 Add `env: &HashMap<String, String>` parameter to `LspTransport::spawn()` in `src/lsp/transport.rs`
- [x] 2.2 Apply `.envs()` on the `Command` before `.spawn()`
- [x] 2.3 Update transport tests to pass empty `HashMap`

## 3. Client

- [x] 3.1 Add `env: &HashMap<String, String>` parameter to `LspClient::start()` in `src/lsp/client.rs`
- [x] 3.2 Forward `env` to `LspTransport::spawn()`
- [x] 3.3 Update `LspClientApi` trait if `start()` is part of the trait

## 4. Server Manager

- [x] 4.1 Add `env: HashMap<String, String>` field to `LanguageServerManager` in `src/server/manager.rs`
- [x] 4.2 Thread `env` from manager through `start_server_internal()` → `LspClient::start()`
- [x] 4.3 Update `with_configs()` and `new()` to accept env parameter
- [x] 4.4 Update manager tests to pass empty `HashMap`

## 5. Project Registry

- [x] 5.1 In `src/project/registry.rs`, pass `ProjectEntry::env` to `LanguageServerManager::with_configs()`
- [x] 5.2 Handle implicit single-project mode (no `[[projects]]`) — use empty env

## 6. Documentation

- [x] 6.1 Update `README.md` — add `env` to `[[projects]]` config example and description
- [x] 6.2 Update `README.ja.md` — same changes as README.md in Japanese
- [x] 6.3 Update `CLAUDE.md` — add `env` to config format example

## 7. Lint & Test

- [x] 7.1 Run `cargo clippy --all-targets --tests -- -D warnings`
- [x] 7.2 Run `cargo fmt --all -- --check`
- [x] 7.3 Run `cargo test`
