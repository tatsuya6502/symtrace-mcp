## 1. Config

- [ ] 1.1 Add `env: Option<HashMap<String, String>>` field to `ProjectEntry` in `src/config.rs`
- [ ] 1.2 Add unit test for parsing `[[projects]]` with `env` field
- [ ] 1.3 Add unit test for parsing `[[projects]]` without `env` field (backward compat)

## 2. Transport

- [ ] 2.1 Add `env: &HashMap<String, String>` parameter to `LspTransport::spawn()` in `src/lsp/transport.rs`
- [ ] 2.2 Apply `.envs()` on the `Command` before `.spawn()`
- [ ] 2.3 Update transport tests to pass empty `HashMap`

## 3. Client

- [ ] 3.1 Add `env: &HashMap<String, String>` parameter to `LspClient::start()` in `src/lsp/client.rs`
- [ ] 3.2 Forward `env` to `LspTransport::spawn()`
- [ ] 3.3 Update `LspClientApi` trait if `start()` is part of the trait

## 4. Server Manager

- [ ] 4.1 Add `env: HashMap<String, String>` field to `LanguageServerManager` in `src/server/manager.rs`
- [ ] 4.2 Thread `env` from manager through `start_server_internal()` → `LspClient::start()`
- [ ] 4.3 Update `with_configs()` and `new()` to accept env parameter
- [ ] 4.4 Update manager tests to pass empty `HashMap`

## 5. Project Registry

- [ ] 5.1 In `src/project/registry.rs`, pass `ProjectEntry::env` to `LanguageServerManager::with_configs()`
- [ ] 5.2 Handle implicit single-project mode (no `[[projects]]`) — use empty env

## 6. Documentation

- [ ] 6.1 Update `README.md` — add `env` to `[[projects]]` config example and description
- [ ] 6.2 Update `README.ja.md` — same changes as README.md in Japanese
- [ ] 6.3 Update `CLAUDE.md` — add `env` to config format example

## 7. Lint & Test

- [ ] 7.1 Run `cargo clippy --all-targets --tests -- -D warnings`
- [ ] 7.2 Run `cargo fmt --all -- --check`
- [ ] 7.3 Run `cargo test`
