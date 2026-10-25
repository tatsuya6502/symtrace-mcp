# Tasks

## 1. Optional `command` in the config struct (config-file: Configuration loaded from .symtrace.toml)

- [x] 1.1 In `src/config.rs`, change `ServerConfig.command` from `String` to `Option<String>` (design D1); verify `cargo build` succeeds after task 2.1
- [x] 1.2 Update the existing tests in `src/config.rs` (`load_multi_project`, `load_server_only`, `load_typescript_server`) to compare `.command.as_deref()` against `Some("...")`; verify `load_server_only` still asserts `Some("/custom/rust-analyzer")`
- [x] 1.3 Add `load_server_without_command` in `src/config.rs`: `[server.rust]` with only `index_timeout_secs = 600` parses with `command == None`, `index_timeout_secs == 600`, `idle_timeout_secs == 600`; verify `cargo test config::` passes

## 2. Default command kept when omitted (config-file: Server configuration with global defaults)

- [x] 2.1 In `src/project/registry.rs` `build_server_configs`, replace `lang_cfg.command = cfg.command.clone();` with `if let Some(command) = &cfg.command { lang_cfg.command = command.clone(); }`; keep both timeout assignments unconditional and leave `args` untouched (design D2); verify with task 2.2
- [x] 2.2 Add tests next to `index_timeout_from_server_section`: (a) `[server.rust]` with only `index_timeout_secs = 600` keeps command `rust-analyzer`, sets Rust `index_timeout_secs` to 600 and leaves TypeScript at 30; (b) `[server.typescript]` with only `idle_timeout_secs = 120` keeps command `typescript-language-server`, `args == ["--stdio"]`, and sets `idle_timeout_secs` to 120; verify `cargo test project::registry` passes
- [x] 2.3 Confirm no other use of `ServerConfig.command` remains (`grep -rn 'ServerConfig\|\.command' src`); `LanguageServerConfig.command` in `src/server/manager.rs` stays `String`; verify the grep shows only the updated sites

## 3. Docs

- [x] 3.1 In `README.md` "Multi-Project Support", change the `[server.rust]` snippet to `index_timeout_secs = 120` without `command`, with a comment that the command defaults to rust-analyzer; in the `[server.rust]` and `[server.typescript]` bullets say `command` is optional inside the section (omitted → the default command); in the `index_timeout_secs` bullet say it can be set on its own; verify the idle/index timeout sentences and the vtsls/deno note are unchanged
- [x] 3.2 Append to `CHANGELOG.md` `## [Unreleased]` `### Fixed` as the last bullet: "A `[server.<lang>]` section in `.symtrace.toml` no longer requires `command`. ..."; verify the entry is present and last

## 4. Final checks

- [x] 4.1 Run `cargo fmt --all -- --check` and verify it reports no diff
- [x] 4.2 Run `cargo clippy --all-targets --tests -- -D warnings` and verify it reports no warnings
- [x] 4.3 Run `cargo test` and verify all tests pass (no integration features needed)
