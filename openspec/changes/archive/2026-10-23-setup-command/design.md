# Design

## Context

See proposal.md for motivation. Relevant current state:

- `src/main.rs` already uses clap with one subcommand (`stats`); the server runs when no subcommand is given. `stats` deliberately skips logging/stats initialization, which `setup` and `hook` must also do (`hook` runs on every Bash/Grep call and must stay fast and side-effect free).
- The four target files are owned by Claude Code and, in the case of `Explore.md`, shared with ast-bro's shadow file (`<!-- ast-bro:begin v=… -->` block). `settings.json` and `~/.claude.json` hold unrelated user data (permissions, OAuth state), so writes must be surgical.
- A bash prototype of the hook (`symtrace-remind.sh`) and the CLAUDE.md block text were validated on 2026-10-22; this change ports them to Rust.
- `serde_json` is already a dependency; `preserve_order` is not enabled, so a round-tripped file would currently have its keys re-sorted.

## Goals / Non-Goals

**Goals:**
- One command that a new user runs after `cargo install`, idempotent and safe to re-run on upgrade.
- Never damage user content: edits confined to our marker block / our JSON entries; refuse to clobber edited blocks without `--force`.
- Hook with zero runtime dependencies (no `jq`, no bash) and unit-testable decision logic.
- Coexist with ast-bro's install in the same files.

**Non-Goals:**
- Installing for other agents (Cursor, Codex, Gemini). The file layout is Claude Code specific.
- Blocking or rewriting tool calls (`deny` / `updatedInput`). The reminder is advisory; a block would strand sessions where the MCP server is not connected (second instance in the same directory).
- Windows path handling beyond what `std::env::var("HOME")` gives. If `HOME` is unset, `setup` exits with an error.
- A skill (`~/.claude/skills/…`). ast-bro ships one, but it is only invoked manually; it does not affect auto-mode tool selection.
- Counting consecutive calls before reminding. The prototype's counter was only useful with a threshold above 1; the default fires every time, so the state file and the `mcp__symtrace-mcp__*` reset matcher are dropped.

## Decisions

### Decision 1: Hook logic in Rust (`symtrace-mcp hook`), not a shipped shell script

**Alternatives:** embed the validated bash script with `include_str!` and write it to `~/.claude/hooks/`.

**Rationale:** the script needs `bash` and `jq` on PATH, cannot be unit-tested from `cargo test`, and would be a second copy of the logic to keep in sync. A Rust subcommand registers as a single string in settings.json (`symtrace-mcp hook`) and the decision function (`decide(event) -> Option<Reminder>`) is pure, so every spec scenario becomes a table-driven test. Regexes are hand-rolled over `split_whitespace` tokens rather than adding the `regex` crate: the grammar is small (command name, option tokens, one identifier token, path tokens).

### Decision 2: Prompt text is one embedded file, reused for CLAUDE.md and Explore.md

`src/setup/prompt.md` is embedded with `include_str!` and wrapped in the markers at write time, with the version taken from `env!("CARGO_PKG_VERSION")`. The Explore.md frontmatter is a separate constant (copy of the built-in Explore description, the same text ast-bro uses) and is only written when the file is created.

**Rationale:** one source of truth; the version in the marker lets `--status` tell `outdated` from `modified`.

### Decision 3: Block state machine

For a file and a template, classify the existing block as one of: `Absent`, `Current` (version == ours, body == template), `Outdated` (version != ours; body cannot be verified because older templates are not embedded), `Modified` (version == ours, body != template). Actions: `Absent → create`, `Current → unchanged`, `Outdated → update`, `Modified → refuse unless --force`.

**Alternative:** embed every past template to verify an outdated block's body. Rejected: the only cost of treating `Outdated` as replaceable is losing an edit the user made to an old block, and the marker makes the block visibly tool-owned.

### Decision 4: JSON files are parsed with `serde_json::Value` and `preserve_order`

Read → parse → mutate only our path (`hooks.PreToolUse[*]` entries with command `symtrace-mcp hook`; `mcpServers.symtrace-mcp`) → pretty-print with two-space indent → write to a temp file in the same directory → rename over the original.

**Alternatives:** textual edits (fragile), or typed structs (would drop unknown keys). `preserve_order` keeps diffs minimal; formatting of the rest of the file (e.g. 4-space indent) is not preserved, which matches how Claude Code itself rewrites these files.

**Ownership rule for hooks:** our entries are identified by `hooks[].command == "symtrace-mcp hook"`. Upsert adds a separate matcher entry per matcher (`Bash`, `Grep`) rather than merging into an existing `Bash` entry owned by another tool, so uninstall can remove exactly what we added.

### Decision 5: Local scope mirrors Claude Code's own project-scope files

`--local` writes `./CLAUDE.md`, `./.claude/agents/Explore.md`, `./.claude/settings.json`, `./.mcp.json`. These are the paths Claude Code reads for project-scope memory, agents, hooks and MCP servers, so no extra wiring is needed. Global scope is the default because the problem (auto-mode tool choice) is per-user, not per-project.

### Decision 6: Module layout

- `src/setup.rs`: clap args struct, orchestration, result reporting (`Outcome` enum → one line per item), exit code.
- `src/setup/marker_block.rs`: find/classify/insert/replace/remove a marker block in a string (pure, tested).
- `src/setup/json_file.rs`: load/save with preserve-order + atomic rename; hook and MCP upsert/remove helpers (pure functions over `Value`, tested).
- `src/setup/targets.rs`: path resolution for global/local scope.
- `src/hook.rs` + `src/hook/decide.rs`: stdin/stdout shell and the pure decision function.
Follows the project's `src/<module>.rs` + `src/<module>/sub.rs` convention.

## Risks / Trade-offs

- [`~/.claude.json` is also Claude Code's state file (OAuth, project list)] → Parse-mutate-write only when the entry actually changes; atomic rename; `--dry-run` and `--skip-mcp` available. Parse failure reports an error for that item and leaves the file alone.
- [Claude Code rewrites `settings.json` concurrently while a session runs] → Same as ast-bro: document that `setup` should run with no session open; the atomic rename limits damage to a lost update, never a truncated file.
- [Reminder fires on every matching call, adding ~400 bytes of context each time] → Only `sb` call-graph subcommands and identifier greps over source paths match; `sb map/digest/show`, regex greps, pipe filters and non-source paths are excluded. Users can set `SYMTRACE_HOOK_SRC_DIRS` to narrow further or `--skip-hook` at install time.
- [`--include=*.rs` style options precede the pattern] → Known false negative (option tokens with `=` are not modelled). Acceptable; a miss costs only a reminder.
- [Two tools shadow `Explore.md`] → We append outside ast-bro's markers and never create the file if it exists; `sb install --force` rewrites only its own block. Uninstall deletes the file only when nothing but our frontmatter remains.
- [`setup` registers the bare command name `symtrace-mcp`] → Requires the binary on PATH for Claude Code's hook and MCP spawn, same assumption as the existing MCP registration instructions in the README.
