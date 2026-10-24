# Tasks

## 1. `--version` / `-V` (stats-cli: Version flag)

- [ ] 1.1 In `src/main.rs:22`, change `#[command(name = "symtrace-mcp")]` to `#[command(name = "symtrace-mcp", version)]` (design D1); verify `cargo run -- --version` and `cargo run -- -V` both print `symtrace-mcp <Cargo.toml version>` and exit 0
- [ ] 1.2 Add a `#[cfg(test)] mod tests` at the end of `src/main.rs` (`Cli` is private; import `clap::CommandFactory`): assert `Cli::command().get_version() == Some(env!("CARGO_PKG_VERSION"))`; assert `Cli::try_parse_from(["symtrace-mcp", "--version"])` and `["symtrace-mcp", "-V"]` both return an error whose `kind()` is `clap::error::ErrorKind::DisplayVersion` and whose `render().to_string()` is `format!("symtrace-mcp {}\n", env!("CARGO_PKG_VERSION"))`; assert `Cli::try_parse_from(["symtrace-mcp"])` still yields `command == None`; verify with `cargo test --bin symtrace-mcp tests::`
- [ ] 1.3 Add to `CHANGELOG.md` `## [Unreleased]` a new `### Added` section (above the existing `### Fixed`): "`symtrace-mcp --version` / `-V` print the version (#27)."; verify the section renders in order Added, Fixed

## 2. `created` / `updated` mean file creation vs. file change (claude-code-setup: Result words describe the target file)

- [ ] 2.1 In `src/setup.rs` `markdown_item`, `Mode::Install`, `State::Absent` arm: change the `_ =>` fallback (existing non-blank content, no block; used by both prompt and explore) from `"created"` to `"updated"`; keep `"created"` for the `text.trim().is_empty()` arm and `"updated"` for the frontmatter-prepend arm (design D2). Note: when `frontmatter` is `None` (CLAUDE.md) and the file is blank, the fallback is also hit, so split it by `text.trim().is_empty()` rather than by the frontmatter match; verify with task 2.4
- [ ] 2.2 In `src/setup/json_file.rs`, expose whether the file was missing or blank, reusing `load`'s rules at L26-35 (either change `load` to return `(Value, bool)` and update its callers and the `load_edge_cases` / `load_and_save_round_trip` tests, or add `pub fn is_missing_or_blank(path) -> Result<bool, SetupError>`); verify `cargo test setup::json_file` passes with a new assertion for missing, empty, whitespace-only and non-blank files
- [ ] 2.3 In `src/setup.rs` `hook_item` Install arm, return `"created"` only when the file was missing/blank, otherwise `"updated"` (replacing `if any { "updated" } else { "created" }`); in `mcp_item` Install arm, map `McpChange::Created` to `"created"` only when the file was missing/blank, otherwise `"updated"`; verify with task 2.4
- [ ] 2.4 Update `src/setup/tests.rs` expectations from `created` to `updated`: `existing_claude_md_content_is_preserved` (L267), `explore_shadowed_by_other_tool_gets_appended_block` (L283), `crlf_claude_md_stays_crlf_and_unchanged_on_rerun` (L347), `settings_with_unrelated_hook_and_keys` (L480), `claude_json_other_keys_kept` (L564), `explore_md_without_tool_restriction_warns` (L674). Confirm unchanged: `fresh_install_creates_four_items`, `dry_run_on_fresh_writes_nothing`, `empty_explore_md_gets_frontmatter_and_is_deleted_on_uninstall` (blank seeds stay `created`), `invalid_settings_json_errors_only_hook_item` (other files absent), `local_scope_writes_under_cwd_only`. Add a test that a whitespace-only `settings.json` and `.claude.json` report `created`. Verify `cargo test setup::` passes
- [ ] 2.5 Add to `CHANGELOG.md` `## [Unreleased]` `### Fixed`: "`setup` reports `updated`, not `created`, when it adds its block or entry to a file that already existed (Explore.md written by another tool, an existing CLAUDE.md, settings.json or `.claude.json`) (#27)."; verify the entry is present

## 3. Uniform `--status` rows (claude-code-setup: Status reports each item)

- [ ] 3.1 In `src/setup.rs` `hook_item` and `mcp_item` `Mode::Status` arms, return `format!("installed v{VERSION}")` instead of `"installed"` (design D3); leave `modified` / `absent` bare; verify with task 3.2
- [ ] 3.2 Extend `status_on_partial_install` in `src/setup/tests.rs` (L770-832), or add `status_after_full_install`, to run a full `execute(&args(), ...)` and then `--status`, asserting all four rows equal `installed v{VERSION}`; keep the existing `("mcp", "modified")` and `("hook", "absent")` assertions unchanged; verify `cargo test setup::` passes
- [ ] 3.3 Add to `CHANGELOG.md` `## [Unreleased]` `### Fixed`: "`setup --status` shows `installed v<version>` for the hook and MCP rows too, so all four rows read the same (#27)."; verify the entry is present

## 4. Idle-monitor fallback (code fix, no spec delta)

- [ ] 4.1 In `src/server/idle_monitor.rs:46-49`, replace `.map(...).unwrap_or(Duration::from_secs(300))` with a `let Some(cfg) = manager.config_for(language) else { tracing::warn!(language = ?language, "no server config; untracking"); last_used.remove(&language); continue; };` and compute `Duration::from_secs(cfg.idle_timeout_secs)` (design D4); verify `grep -n 300 src/server/idle_monitor.rs` finds nothing
- [ ] 4.2 Add a `#[cfg(test)] mod tests` to `src/server/idle_monitor.rs`: build `LanguageServerManager::with_configs` with an empty config map and a temp `StatsRecorder` (as in `src/server/manager.rs` tests), `touch(Language::Rust)`, call `check_and_shutdown_idle`, and assert the Rust entry was removed from `last_used` without panicking (with the old fallback it would remain); verify `cargo test server::idle_monitor` passes

## 5. Final checks

- [ ] 5.1 Run `cargo clippy --all-targets --tests -- -D warnings` and verify it reports no warnings
- [ ] 5.2 Run `cargo fmt --all -- --check` and verify it reports no diff
- [ ] 5.3 Run `cargo test` and verify all tests pass (no integration features needed)
- [ ] 5.4 Smoke-test against a temp HOME with the built binary (not `cargo run`, which would resolve `CARGO_HOME` under the temp HOME): `cargo build`, then `HOME=$(mktemp -d) target/debug/symtrace-mcp setup` prints four `created` lines and `setup --status` in that HOME shows `installed v<version>` on all four rows; in a second temp HOME seed `.claude/agents/Explore.md` with foreign frontmatter and verify `explore  updated`
