# Proposal

## Why

Issue #27 collects three rough edges in the v0.6.0 CLI that make it hard to verify an install: there is no `--version`, `setup` says `created` for files that already existed, and `setup --status` shows a version on two rows but not on the other two. A fourth inconsistency was found later: the idle monitor falls back to a 300 s timeout while every real default is 600 s.

## What Changes

- Add `symtrace-mcp --version` / `-V`, printing `symtrace-mcp <crate version>` and exiting 0 without starting the MCP server.
- Redefine the `setup` result words at file level for all four items: `created` only when the target file did not exist or was blank; `updated` when the file had content and setup added or replaced symtrace-mcp content. This fixes Explore.md (the issue's case) and, because the code path is shared or follows the same rule, also CLAUDE.md, settings.json and the MCP config file, which today also report `created` when only our block or entry is added to an existing file. The printed line names the file path, so `created <path>` must mean the file was created.
- `setup --status` prints `installed v<binary version>` for the hook and MCP items too, so all four rows read the same. For the markdown items `installed` already means "marker equals the running binary's version" and the printed version is the binary's, so the same wording is accurate for the JSON items.
- Remove the dead 300 s fallback in the idle monitor (code fix only; the spec already says 600 s).

## Capabilities

### New Capabilities

None. `--version` belongs to `stats-cli`, the capability that already owns the binary's top-level CLI surface ("Default subcommand preserves MCP server behavior"); a new capability for one flag would be a near-empty catch-all.

### Modified Capabilities

- `stats-cli`: ADDED requirement for the `--version` / `-V` flag.
- `claude-code-setup`: MODIFIED "setup subcommand installs four integration items" (file-level meaning of `created` / `updated`), "Existing Explore.md keeps its frontmatter" (scenario for an existing Explore.md with frontmatter reporting `updated`), and "Status reports each item" (hook and MCP rows carry the version).

## Impact

- `src/main.rs`: `#[command(name = "symtrace-mcp", version)]` on `Cli`, plus a unit test module.
- `src/setup.rs`: `markdown_item` `State::Absent` arm, `hook_item` and `mcp_item` Install arms (file-was-missing-or-blank signal), `hook_item` / `mcp_item` Status arms. `src/setup/json_file.rs` may need to report whether `load` read any content.
- `src/setup/tests.rs`: expected words flip in `existing_claude_md_content_is_preserved`, `explore_shadowed_by_other_tool_gets_appended_block`, `crlf_claude_md_stays_crlf_and_unchanged_on_rerun`, `settings_with_unrelated_hook_and_keys`, `claude_json_other_keys_kept`, `explore_md_without_tool_restriction_warns`; status test extended.
- `src/server/idle_monitor.rs:46-49`: the `unwrap_or(Duration::from_secs(300))` fallback is removed (no spec delta; `server-manager` already specifies 600 s).
- User-visible output change: scripts that grep `setup` output for `created` on an existing file will now see `updated`. No API, dependency or config change.
- `CHANGELOG.md` `## [Unreleased]`.
