# Design

## Context

See proposal.md - Why. Relevant current state:

- `Cli` in `src/main.rs:21-26` is `#[command(name = "symtrace-mcp")]` with no `version`.
- `setup` result words come from three handlers in `src/setup.rs`. `markdown_item` (shared by the prompt and explore items) returns `created` in the `State::Absent` arm for a blank file and also for the `_` fallback (existing content, no block), and `updated` only for the frontmatter-prepend case. `hook_item` returns `created` when none of our matchers were present, `updated` when some were. `mcp_item` maps `McpChange::Created` (no entry) to `created`. So today the words are item-level ("our content is new"), and the main spec's "Fresh global install" scenario ("none of the four files *contain symtrace-mcp content*") codifies that. Tests assert it: `existing_claude_md_content_is_preserved`, `explore_shadowed_by_other_tool_gets_appended_block`, `crlf_claude_md_stays_crlf_and_unchanged_on_rerun`, `settings_with_unrelated_hook_and_keys`, `claude_json_other_keys_kept`, `explore_md_without_tool_restriction_warns`.
- `--status` for markdown prints `installed v{VERSION}` where `VERSION` is the crate constant; `State::Current` only holds when the marker equals it. Hook/MCP print a bare `installed`. `ItemLine` formats the status in a `{:<9}` column, which `installed v0.6.0` and `modified (use --force)` already overflow.
- `IdleMonitor::check_and_shutdown_idle` (`src/server/idle_monitor.rs:45-49`) uses `unwrap_or(Duration::from_secs(300))` when `manager.config_for(language)` is `None`. `touch` is only called from `src/mcp/handlers.rs` after a client was obtained, which requires a config, and configs never change after construction, so the branch is dead.

## Goals / Non-Goals

**Goals:**
- One definition of `created` / `updated` that holds for all four items.
- Four `--status` rows that read the same when everything is installed.

**Non-Goals:**
- Changing uninstall words (`removed`, `absent`) or the `modified (use --force)` path.
- Fixing the stale "the `stats` subcommand is the only additional subcommand" sentence in `stats-cli` (separate spec-reconciliation work).

## Decisions

### D1: `--version` via clap's `version` attribute
`#[command(name = "symtrace-mcp", version)]` makes clap read `CARGO_PKG_VERSION` and handle `--version` / `-V` before `main` dispatches, printing `symtrace-mcp <version>` and exiting 0. No custom subcommand or flag handling. Alternative `long_version` with git hash: rejected, needs a build script and nobody asked for it.

### D2: `created` means the file was created; `updated` everywhere else
Chosen word for "file existed, our block appended": `updated`.
- It is already in the spec's result vocabulary and already used for the closest case (frontmatter-less Explore.md, where we prepend frontmatter and append the block).
- The result line prints the file path, so `created  ~/.claude/agents/Explore.md` reads as a file creation; for a file another tool wrote that is false.
- Alternative `appended`: rejected. New word outside the spec's list, and wrong for JSON upserts where nothing is appended.
- Alternative "fix Explore.md only": rejected. The `_` arm is shared with CLAUDE.md, so it would need a `frontmatter.is_some()` special case and CLAUDE.md and Explore.md would disagree in the same situation. Hook and MCP follow the same item-level rule, so the fix is applied to all four. **This widens Issue #27's wording (Explore.md only)**; the extra rows are the same misreport.
- Rule: `created` iff the target file was missing or empty/whitespace-only before the run (matching how `marker_block` and `json_file::load` already treat blank files as absent). For markdown, the `State::Absent` arm already has `text` and `existing`; return `created` when `text.trim().is_empty()`, else `updated` (the frontmatter-prepend sub-arm stays `updated`). For JSON, `json_file::load` returns `{}` for both missing and blank files, so the handler needs one more bit: have `load` also report whether it read non-blank content (e.g. return `(Value, bool)`, or a separate `json_file::is_blank_or_missing(path)` helper that reuses `load`'s rules). `hook_item` returns `created` when the file was blank/missing, else `updated`; `mcp_item` maps `McpChange::Created` the same way. `McpChange::Updated` stays `updated`.

### D3: Hook and MCP status print `installed v{VERSION}`
For markdown, `installed v<X>` already means "matches what this binary would install", with X equal to the binary's version. For hook/MCP, `installed` means exactly that too (the entry equals or is equivalent to what this binary writes), so printing `installed v{VERSION}` from the `hook_item` / `mcp_item` Status arms is accurate, provided the entry runs this binary. `is_our_hook_command` / `is_our_binary` also accept an absolute path to any file named `symtrace-mcp` (e.g. an older install), so the version is appended only when every command is the bare name or canonicalizes to `std::env::current_exe()` (`preflight::is_running_binary`); otherwise the status is a bare `installed`. `modified` and `absent` stay bare; `outdated` cannot occur for JSON items.
- Alternative "widen / compute the status column" instead: rejected as a substitute, since it aligns paths but leaves the vocabulary gap. It is done in addition: the column is widened from 9 to 22 (`modified (use --force)`, the longest fixed status), because `installed v<version>` would otherwise misalign mixed reports.
- Alternative "`installed (no version)`": rejected, noisier and says nothing useful.
- The existing scenario "Entry registered with claude mcp add ... `--status` reports `installed`" is left as is: it names the status class, which `installed v<version>` still is.

### D4: Drop the idle-timeout fallback instead of changing 300 to 600
Replace `.map(...).unwrap_or(Duration::from_secs(300))` with `let Some(cfg) = manager.config_for(language) else { tracing::warn!(...); last_used.remove(&language); continue; };`. A language with no config cannot have a running server, so it is untracked rather than given a guessed timeout. Removing the entry means the warning logs once, not every check interval. Alternative `300 → 600`: rejected, it adds a fourth copy of the default (already in `config.rs:28` and `manager.rs:61,72`) for a branch that never runs. No spec delta: `server-manager` already says 600 s.

## Risks / Trade-offs

- [Scripts that match `created` on existing files see `updated`] → Pre-1.0 CLI, the output is human-oriented, and the change is listed under `### Fixed` in CHANGELOG.
- [`installed v<binary version>` for hook/MCP could be read as "the entry records this version"] → The entry has no version; the spec text says the version is the running binary's. The hook command runs whatever `symtrace-mcp` is on `PATH`, which the existing PATH preflight warning already covers; a command-local `PATH=...` assignment (or a wrapper word such as `env`) escapes that warning, so such a command gets a bare `installed`.
- [A blank-but-existing JSON file reported as `created`] → Deliberate: nothing of the user's is preserved, same as a blank Explore.md today.
