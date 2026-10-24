# Spec Delta

## ADDED Requirements

### Requirement: Result words describe the target file
In install mode, an item SHALL be reported as `created` only when its target file did not exist or was empty or whitespace-only before `setup` ran. When the file had other content and `setup` added or replaced symtrace-mcp content (a prompt block, frontmatter, hook entries, or the MCP entry), the item SHALL be reported as `updated`. This applies to all four items.

#### Scenario: Block appended to an existing CLAUDE.md
- **WHEN** `CLAUDE.md` exists with user text and no symtrace-mcp block, and the user runs `symtrace-mcp setup`
- **THEN** the block is appended and the prompt item is reported as `updated`

#### Scenario: Hook entries added to an existing settings.json
- **WHEN** `settings.json` exists with unrelated keys and no `symtrace-mcp hook` entry
- **THEN** the hook entries are added and the hook item is reported as `updated`

#### Scenario: MCP entry added to an existing config file
- **WHEN** `$HOME/.claude.json` exists with other keys and no `symtrace-mcp` server
- **THEN** the entry is added and the MCP item is reported as `updated`

#### Scenario: Blank file counts as new
- **WHEN** a target file exists but contains only whitespace
- **THEN** the item is reported as `created`

## MODIFIED Requirements

### Requirement: setup subcommand installs four integration items
The binary SHALL accept a `setup` subcommand. By default it SHALL install, in the global scope, (1) the symtrace-mcp prompt block in `$HOME/.claude/CLAUDE.md`, (2) the same block in `$HOME/.claude/agents/Explore.md`, (3) `PreToolUse` hook entries in `$HOME/.claude/settings.json`, and (4) an `mcpServers.symtrace-mcp` entry in `$HOME/.claude.json`. It SHALL print one result line per item (`created`, `updated`, `unchanged`, `skipped`, or an error) and exit 0 when no item failed.

#### Scenario: Fresh global install
- **WHEN** none of the four files exist and the user runs `symtrace-mcp setup`
- **THEN** all four items are created, each is reported as `created`, and the exit code is 0

#### Scenario: Re-running setup is a no-op
- **WHEN** all four items are already installed at the current version and the user runs `symtrace-mcp setup`
- **THEN** every item is reported as `unchanged` and no file is modified

#### Scenario: Missing parent directories
- **WHEN** `$HOME/.claude/agents/` does not exist
- **THEN** `setup` creates the directory before writing `Explore.md`

### Requirement: Existing Explore.md keeps its frontmatter
When Explore.md has content that starts with frontmatter, `setup` SHALL only add or update the prompt block. When its content does not start with frontmatter, `setup` SHALL prepend the frontmatter and append the block so the file is a valid agent definition.

#### Scenario: Frontmatter-less Explore.md
- **WHEN** `Explore.md` contains text that does not start with `---`
- **THEN** `setup` prepends the frontmatter, keeps the text, appends the block, and reports `updated`

#### Scenario: Explore.md with another tool's frontmatter
- **WHEN** `Explore.md` already exists with its own frontmatter (for example written by another tool) and no symtrace-mcp block
- **THEN** `setup` leaves the frontmatter and existing content unchanged, appends the block, and reports `updated`, not `created`

### Requirement: Status reports each item
With `--status`, `setup` SHALL print one line per item with one of `installed`, `modified`, `outdated`, or `absent`, SHALL NOT modify any file, and SHALL exit 0. For the two markdown items an `installed` or `outdated` status SHALL be followed by the begin marker's version (`installed v<version>`, `outdated v<version>`). The hook and MCP items record no version, so when installed they SHALL show the running binary's version (`installed v<version>`) only when every command the entries run is the bare `symtrace-mcp` or a path that resolves (after canonicalization) to the running binary; otherwise they SHALL show a bare `installed`.

#### Scenario: Partially installed
- **WHEN** the block is in CLAUDE.md but the hook and MCP entries are missing
- **THEN** `--status` reports `installed` for the prompt item and `absent` for the hook and MCP items

#### Scenario: Fully installed rows read uniformly
- **WHEN** all four items are installed by a binary of crate version 0.6.1, the hook and MCP entries run the bare command `symtrace-mcp` or a path that resolves to the running binary, and the user runs `symtrace-mcp setup --status` with that binary
- **THEN** each of the prompt, explore, hook and mcp lines shows the status `installed v0.6.1`

#### Scenario: Entry points at a different binary
- **WHEN** the MCP entry's command (or a hook command's binary) is an absolute path to a file named `symtrace-mcp` that is not the running binary, and the user runs `symtrace-mcp setup --status`
- **THEN** that item's line shows a bare `installed`, without a version
