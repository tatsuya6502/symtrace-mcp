# Tasks

## 1. Scaffolding

- [x] 1.1 Enable the `preserve_order` feature on `serde_json` in `Cargo.toml`; verify `cargo build` succeeds and a `serde_json::Map` round-trip test keeps key order
- [x] 1.2 Add `setup` and `hook` subcommands to the clap `Commands` enum in `src/main.rs` with the flags from the spec (`--local`, `--dry-run`, `--force`, `--skip-prompt/explore/hook/mcp`, `--uninstall`, `--status`); verify `symtrace-mcp setup --help` and `symtrace-mcp hook --help` print them and neither path calls `init_logging` or `StatsRecorder`

## 2. Hook decision logic

- [ ] 2.1 Create `src/hook/decide.rs` with `decide(tool_name, tool_input) -> Option<Reminder>`: segment splitting on `|`, `||`, `&&`, `;`; `sb|ast-bro` + `callers|callees|implements|impact|trace`; bare-identifier `grep|rg|ugrep` with the source-target rule (`SYMTRACE_HOOK_SRC_DIRS` override, source extensions, bare `.`); `Grep` tool bare-identifier pattern. Verify with table-driven unit tests covering every scenario in `specs/tool-reminder-hook/spec.md` (including the four negative cases)
- [ ] 2.2 Create `src/hook.rs`: read stdin, parse JSON leniently (malformed → print nothing, exit 0), build the reminder text naming the detected call and the six symtrace-mcp tools, print the `hookSpecificOutput` JSON on one line. Verify with a unit test on the JSON shape and a manual run: `echo '{"tool_name":"Bash","tool_input":{"command":"sb callers x"}}' | cargo run -- hook` prints the object, and `echo '' | cargo run -- hook` prints nothing with exit 0

## 3. Marker block and prompt text

- [ ] 3.1 Add `src/setup/prompt.md` (the validated "Semantic queries: use symtrace-mcp tools first" block with the question→tool table, no markers) and the Explore frontmatter constant; verify `include_str!` compiles and the text contains all six tool names
- [ ] 3.2 Create `src/setup/marker_block.rs` with find/classify (`Absent`/`Current`/`Outdated`/`Modified`), insert-at-end, replace-in-place and remove; verify unit tests for each state transition, preservation of surrounding bytes, and coexistence with an `ast-bro:begin/end` block in the same string

## 4. JSON file editing

- [ ] 4.1 Create `src/setup/json_file.rs` with load (missing file → empty object, parse error → item error), atomic save (temp file + rename, two-space pretty print), `upsert_hook(value, matcher)`, `remove_hooks(value)`, `upsert_mcp(value, force)`, `remove_mcp(value)`; verify unit tests: unrelated hooks/keys preserved, no duplicate entry on re-upsert, matcher entry dropped only when its `hooks` array empties, customized `symtrace-mcp` MCP entry reported as modified without `--force` and replaced with it

## 5. Setup orchestration

- [ ] 5.1 Create `src/setup/targets.rs` resolving the four paths for global (`$HOME`) and `--local` (cwd) scope, erroring when `HOME` is unset in global scope; verify unit tests with an overridden home directory
- [ ] 5.2 Create `src/setup.rs` running the four items (prompt, explore, hook, mcp) honoring `--skip-*`, `--dry-run`, `--force`, creating parent directories, creating `Explore.md` with frontmatter only when absent, printing one result line per item, exiting non-zero when any item is `modified (use --force)` or errored; verify integration tests in a `tempfile` home covering fresh install, idempotent re-run, outdated upgrade, modified-without-force refusal, and `--dry-run` leaving no files
- [ ] 5.3 Implement `--uninstall` (block removal from both markdown files, hook entry removal, MCP entry removal, `Explore.md` deleted only when only our frontmatter remains) and `--status` (`installed v…`/`modified`/`outdated v…`/`absent` per item, no writes); verify integration tests for uninstall after full install, uninstall with a shared `Explore.md`, and status on a partial install

## 6. Documentation and verification

- [ ] 6.1 Add a "Claude Code integration" subsection to README `Installation` describing `symtrace-mcp setup`, its flags, what files it touches, the hook's behavior, and the note to run it with no Claude Code session open; add a CHANGELOG `Unreleased` → `Added` entry for `setup` and `hook`; verify the README commands run as written
- [ ] 6.2 Run `cargo clippy --all-targets --tests -- -D warnings`, `cargo fmt --all -- --check`, and `cargo test`; verify all pass
- [ ] 6.3 Manual end-to-end check in a throwaway `HOME`: `setup --dry-run`, `setup`, `setup` again (all `unchanged`), `setup --status`, `setup --uninstall`; verify the four files match the spec scenarios at each step

## Workflow follow-up

- Archive the change with `openspec archive setup-command` after review.
