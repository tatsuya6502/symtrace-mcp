# Spec Delta

## Purpose

Installs, reports and removes the Claude Code integration for symtrace-mcp with a single command, so that auto-mode sessions route semantic code questions to the symtrace-mcp tools.

## ADDED Requirements

### Requirement: setup subcommand installs four integration items
The binary SHALL accept a `setup` subcommand. By default it SHALL install, in the global scope, (1) the symtrace-mcp prompt block in `$HOME/.claude/CLAUDE.md`, (2) the same block in `$HOME/.claude/agents/Explore.md`, (3) `PreToolUse` hook entries in `$HOME/.claude/settings.json`, and (4) an `mcpServers.symtrace-mcp` entry in `$HOME/.claude.json`. It SHALL print one result line per item (`created`, `updated`, `unchanged`, `skipped`, or an error) and exit 0 when no item failed.

#### Scenario: Fresh global install
- **WHEN** none of the four files contain symtrace-mcp content and the user runs `symtrace-mcp setup`
- **THEN** all four items are created, each is reported as `created`, and the exit code is 0

#### Scenario: Re-running setup is a no-op
- **WHEN** all four items are already installed at the current version and the user runs `symtrace-mcp setup`
- **THEN** every item is reported as `unchanged` and no file is modified

#### Scenario: Missing parent directories
- **WHEN** `$HOME/.claude/agents/` does not exist
- **THEN** `setup` creates the directory before writing `Explore.md`

### Requirement: Local scope targets project files
With `--local`, `setup` SHALL target the project files instead: `./CLAUDE.md`, `./.claude/agents/Explore.md`, `./.claude/settings.json`, and `./.mcp.json`, relative to the current working directory.

#### Scenario: Local install
- **WHEN** the user runs `symtrace-mcp setup --local` in a project directory
- **THEN** the four project-scope files are created or updated and no file under `$HOME` is touched

### Requirement: Items can be skipped individually
`setup` SHALL accept `--skip-prompt`, `--skip-explore`, `--skip-hook` and `--skip-mcp`. A skipped item SHALL be reported as `skipped` and its file SHALL NOT be read or written.

#### Scenario: Skip MCP registration
- **WHEN** the user runs `symtrace-mcp setup --skip-mcp`
- **THEN** `$HOME/.claude.json` is not modified and the MCP item is reported as `skipped`

### Requirement: Prompt block is delimited by version markers
The prompt block written to CLAUDE.md and Explore.md SHALL begin with `<!-- symtrace-mcp:begin v=<crate version> -->` and end with `<!-- symtrace-mcp:end -->`. Content outside the markers SHALL be preserved byte for byte. In CLAUDE.md the block SHALL be appended at the end of the file; in Explore.md it SHALL be appended after any existing content, which keeps it outside another tool's marker block.

#### Scenario: Existing CLAUDE.md content is preserved
- **WHEN** `CLAUDE.md` already contains user text and another tool's marker block
- **THEN** after `setup` the file equals the original content followed by the symtrace-mcp block

#### Scenario: Older block is upgraded
- **WHEN** the file contains a block whose marker version differs from the running binary's version and whose body matches that older template
- **THEN** `setup` replaces the block in place and reports `updated`

### Requirement: User edits inside the block are protected
If the body between the markers differs from the template for the version named in the begin marker, or the version is unknown to the binary, `setup` SHALL leave the block unchanged, report the item as `modified (use --force)`, and exit with a non-zero code. With `--force` it SHALL replace the block and report `updated`.

#### Scenario: Edited block without --force
- **WHEN** the user has changed text inside the symtrace-mcp block and runs `symtrace-mcp setup`
- **THEN** the block is not rewritten, the item is reported as `modified (use --force)`, and the exit code is non-zero

#### Scenario: Edited block with --force
- **WHEN** the same user runs `symtrace-mcp setup --force`
- **THEN** the block is replaced with the current template and reported as `updated`

### Requirement: Explore.md is created with the built-in frontmatter when absent
When the Explore.md target does not exist, `setup` SHALL create it with YAML frontmatter containing `name: Explore` and the built-in Explore subagent description, followed by the prompt block. When it exists, `setup` SHALL only add or update the prompt block.

#### Scenario: Explore.md absent
- **WHEN** `$HOME/.claude/agents/Explore.md` does not exist
- **THEN** `setup` creates it with `name: Explore` frontmatter and the block

#### Scenario: Explore.md shadowed by another tool
- **WHEN** `Explore.md` exists and contains another tool's marker block
- **THEN** `setup` appends the symtrace-mcp block after the existing content and leaves the other block intact

### Requirement: Hook entries are upserted in settings.json
`setup` SHALL ensure `hooks.PreToolUse` in the settings file contains, for each of the matchers `Bash` and `Grep`, an entry whose `hooks` array has one element `{"type": "command", "command": "symtrace-mcp hook"}`. Entries for other matchers and other commands SHALL be preserved. When the file does not exist it SHALL be created with only the hook entries. The rewritten file SHALL keep the original key order and use two-space indentation.

#### Scenario: settings.json has unrelated hooks
- **WHEN** `settings.json` already has a `PreToolUse` entry for `Read` from another tool and an unrelated `statusLine` key
- **THEN** after `setup` both are unchanged and two new entries for `Bash` and `Grep` exist

#### Scenario: Hook entry already present
- **WHEN** an entry with matcher `Bash` and command `symtrace-mcp hook` already exists
- **THEN** `setup` does not add a duplicate and reports the hook item as `unchanged`

#### Scenario: settings.json is not valid JSON
- **WHEN** the settings file exists but cannot be parsed
- **THEN** `setup` reports an error for the hook item, does not write the file, and continues with the other items

### Requirement: MCP server entry is upserted
`setup` SHALL ensure the MCP config file contains `mcpServers.symtrace-mcp` equal to `{"command": "symtrace-mcp", "args": []}` (global: `$HOME/.claude.json`; local: `./.mcp.json`). Other keys in the file SHALL be preserved. An existing `symtrace-mcp` entry with a different `command` or `args` SHALL be left as is and reported as `modified (use --force)`; with `--force` it SHALL be replaced.

#### Scenario: Fresh MCP registration
- **WHEN** `$HOME/.claude.json` exists without a `symtrace-mcp` server
- **THEN** `setup` adds the entry and all other keys remain identical

#### Scenario: User-customized MCP entry
- **WHEN** `mcpServers.symtrace-mcp` exists with an extra `env` field
- **THEN** `setup` without `--force` leaves it and reports `modified (use --force)`

### Requirement: Dry run reports without writing
With `--dry-run`, `setup` (including `--uninstall`) SHALL compute and print the result line for every item exactly as a real run would, prefixed with `dry-run`, and SHALL NOT create, modify or delete any file.

#### Scenario: Dry run on a fresh system
- **WHEN** the user runs `symtrace-mcp setup --dry-run` with nothing installed
- **THEN** four `created` lines are printed and no file exists afterwards

### Requirement: Uninstall removes only symtrace-mcp content
With `--uninstall`, `setup` SHALL remove the prompt block from CLAUDE.md and Explore.md, remove the hook entries whose command is `symtrace-mcp hook` (dropping a `PreToolUse` matcher entry only when its `hooks` array becomes empty), and remove `mcpServers.symtrace-mcp`. Explore.md SHALL be deleted only when its remaining content is exactly the frontmatter that `setup` wrote. Every other byte SHALL be preserved. Items that were not installed SHALL be reported as `absent`.

#### Scenario: Uninstall after a full install
- **WHEN** the user runs `symtrace-mcp setup --uninstall` after `setup` on a fresh system
- **THEN** CLAUDE.md has no symtrace-mcp block, Explore.md is deleted, settings.json has no `symtrace-mcp hook` entries, and the MCP entry is gone

#### Scenario: Uninstall with a shared Explore.md
- **WHEN** Explore.md also contains another tool's block
- **THEN** only the symtrace-mcp block is removed and the file is kept

### Requirement: Status reports each item
With `--status`, `setup` SHALL print one line per item with one of `installed v<version>`, `modified`, `outdated v<version>`, or `absent`, SHALL NOT modify any file, and SHALL exit 0.

#### Scenario: Partially installed
- **WHEN** the block is in CLAUDE.md but the hook and MCP entries are missing
- **THEN** `--status` reports `installed` for the prompt item and `absent` for the hook and MCP items
