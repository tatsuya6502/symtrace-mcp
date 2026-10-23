# Proposal

## Why

In Claude Code auto mode the model is told to prefer Bash, so for semantic questions ("who calls X", "what implements Y") it reaches for `sb callers` or `grep` and never picks the symtrace-mcp tools, even when the MCP server is connected (measured 2026-10-22: 0 of 7 headless auto-mode runs used symtrace-mcp). A CLAUDE.md block that names `sb` explicitly and routes those questions to symtrace-mcp fixes it (6 of 6 runs across Opus 5.5 and Fable 5.1), and a `PreToolUse` hook that attaches a reminder to `sb`/`grep` calls reinforces it. Today a user has to hand-edit four files to get there; `ast-bro install` shows that a one-command installer is the expected shape for this kind of tool.

## What Changes

- Add a `symtrace-mcp setup` subcommand that installs the Claude Code integration in one step:
  - a marker block (`<!-- symtrace-mcp:begin v=<version> -->` … `<!-- symtrace-mcp:end -->`) in `~/.claude/CLAUDE.md` telling the model to use symtrace-mcp tools before `sb`/`grep` for semantic questions
  - the same block in `~/.claude/agents/Explore.md` (created with the built-in Explore frontmatter when absent), because the built-in Explore subagent never sees CLAUDE.md
  - `PreToolUse` hook entries for the `Bash` and `Grep` matchers in `~/.claude/settings.json`, running `symtrace-mcp hook`
  - an `mcpServers.symtrace-mcp` entry in `~/.claude.json`
- `setup` flags: `--local` (project files: `./CLAUDE.md`, `./.claude/agents/Explore.md`, `./.claude/settings.json`, `./.mcp.json`), `--dry-run`, `--force` (replace a block the user has edited), `--skip-prompt` / `--skip-explore` / `--skip-hook` / `--skip-mcp`, `--uninstall`, `--status`.
- Add a `symtrace-mcp hook` subcommand: reads one Claude Code `PreToolUse` event from stdin and, when the call is a Bash `sb callers|callees|implements|impact|trace`, a Bash bare-identifier `grep`/`rg`/`ugrep` aimed at source code, or a `Grep` tool call with a bare-identifier pattern, prints a JSON response with `permissionDecision: "allow"` and an `additionalContext` reminder naming the matching symtrace-mcp tool. Otherwise it prints nothing. It never blocks a call.
- The installer is idempotent: re-running `setup` is a no-op when everything is current, upgrades a block from an older version, refuses to overwrite a user-edited block without `--force`, and never touches keys it does not own in the JSON files.

## Capabilities

### New Capabilities
- `claude-code-setup`: One-command install, status and removal of the Claude Code integration (CLAUDE.md block, Explore.md block, hook registration, MCP registration), in global or project scope.
- `tool-reminder-hook`: The `hook` subcommand's PreToolUse behavior: which tool calls trigger a reminder, what the reminder says, and the output contract with Claude Code.

### Modified Capabilities
<!-- none: the existing `stats` subcommand and server behavior are unchanged -->

## Impact

- **Code**: `src/main.rs` (two new clap subcommands), new `src/setup.rs` + `src/setup/{marker_block,json_file,targets}.rs`, new `src/hook.rs` (+ `src/hook/decide.rs`), a prompt text file embedded with `include_str!`.
- **Dependencies**: `serde_json` gains the `preserve_order` feature so rewritten JSON files keep their key order. No new crates.
- **User files written (global scope)**: `~/.claude/CLAUDE.md`, `~/.claude/agents/Explore.md`, `~/.claude/settings.json`, `~/.claude.json`. All edits are confined to the marker block or to entries whose `command` is `symtrace-mcp hook` / key `symtrace-mcp`.
- **Interaction with ast-bro**: Explore.md is shared with ast-bro's shadow file; the symtrace block is placed outside ast-bro's marker block so either tool can rewrite its own block.
- **Docs**: README gains an "Installation → Claude Code integration" section; CHANGELOG entry under Added.
