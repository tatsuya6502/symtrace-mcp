# Spec Delta

## Purpose

Attaches a reminder to Claude Code tool calls that answer semantic code questions with text or tree-sitter heuristics, pointing the model at the symtrace-mcp tool that resolves them through the language server.

## ADDED Requirements

### Requirement: hook subcommand reads one PreToolUse event
The binary SHALL accept a `hook` subcommand that reads a single JSON object from stdin in Claude Code's `PreToolUse` input format (`tool_name`, `tool_input`). It SHALL exit 0 in every case, SHALL write nothing to stdout when no reminder applies, and SHALL NOT initialize logging, the stats database, or any language server.

#### Scenario: Unrelated tool call
- **WHEN** stdin is `{"tool_name":"Edit","tool_input":{...}}`
- **THEN** the command prints nothing and exits 0

#### Scenario: Malformed input
- **WHEN** stdin is empty or not valid JSON
- **THEN** the command prints nothing and exits 0

### Requirement: Reminder response format
When a reminder applies, the command SHALL print exactly one JSON object: `{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"allow","additionalContext":"<reminder>"}}`. The command SHALL never emit `deny`, `ask`, or a non-zero exit code.

#### Scenario: Reminder on sb callers
- **WHEN** stdin is a Bash event whose command is `sb callers stop_server`
- **THEN** the output is a single JSON object with `hookEventName` `PreToolUse` and `permissionDecision` `allow`

### Requirement: Reminder text names the detection and the symtrace-mcp tools
The `additionalContext` text SHALL name what was detected (for example `sb callers` or `grep for identifier 'stop_server'`) and list the six symtrace-mcp tools (`incoming_calls`, `outgoing_calls`, `find_implementations`, `goto_definition`, `find_references`, `hover`) with the instruction to prefer them for the final answer.

#### Scenario: Detection named in the text
- **WHEN** the Bash command is `grep -rn stop_server src/`
- **THEN** `additionalContext` contains `grep for identifier 'stop_server'` and `mcp__symtrace-mcp__incoming_calls`

### Requirement: Bash sb call-graph queries trigger the reminder
A `Bash` event SHALL trigger the reminder when any command segment (split on `|`, `||`, `&&`, `;`) invokes `sb` or `ast-bro` with one of the subcommands `callers`, `callees`, `implements`, `impact`, `trace`.

#### Scenario: sb inside a pipeline
- **WHEN** the Bash command is `cd src && sb trace a b | head`
- **THEN** the reminder is emitted

#### Scenario: sb structural command
- **WHEN** the Bash command is `sb map src/main.rs` or `sb digest src/`
- **THEN** nothing is printed

### Requirement: Bash identifier grep over source code triggers the reminder
A `Bash` event SHALL trigger the reminder when a command segment invokes `grep`, `rg` or `ugrep` whose pattern is a whole bare identifier (`[A-Za-z_][A-Za-z0-9_]*`, optionally single- or double-quoted) and the same segment targets source code as defined by the source-target rule below.

#### Scenario: Identifier grep under src/
- **WHEN** the Bash command is `grep -rn stop_server src/`
- **THEN** the reminder is emitted

#### Scenario: Quoted identifier in a later segment
- **WHEN** the Bash command is `sed -n 1,3p x; grep -rn "stop_server" src/ tests/ 2>/dev/null`
- **THEN** the reminder is emitted

#### Scenario: Regex pattern
- **WHEN** the Bash command is `grep -rn "fn [a-z_]*(" src/`
- **THEN** nothing is printed

#### Scenario: Non-source target
- **WHEN** the Bash command is `grep -c worktrees /proc/self/mountinfo` or `grep -n Install README.md`
- **THEN** nothing is printed

#### Scenario: Pipe filter
- **WHEN** the Bash command is `cargo test 2>&1 | grep FAILED`
- **THEN** nothing is printed

#### Scenario: Source extension without a source directory
- **WHEN** the Bash command is `grep -n LspClientApi client.rs`
- **THEN** the reminder is emitted

### Requirement: Source-target rule for grep segments
A grep segment targets source code when it contains a path with one of the source directories, a file with a source extension, or the bare path `.`. The default source directories are `src`, `crates`, `lib`, `tests`, `test`, `app`, `apps`, `packages`, `pkg`, `internal`, `cmd`; the environment variable `SYMTRACE_HOOK_SRC_DIRS` (a `|`-separated list) SHALL replace the default list.

#### Scenario: Default directory list
- **WHEN** `SYMTRACE_HOOK_SRC_DIRS` is unset and the Bash command is `rg -n Config crates/foo`
- **THEN** the reminder is emitted

#### Scenario: Overridden directory list
- **WHEN** `SYMTRACE_HOOK_SRC_DIRS=src` and the Bash command is `rg -n Config crates/foo`
- **THEN** nothing is printed

### Requirement: Grep tool with a bare identifier triggers the reminder
A `Grep` event SHALL trigger the reminder when `tool_input.pattern` is a whole bare identifier.

#### Scenario: Grep tool identifier
- **WHEN** stdin is `{"tool_name":"Grep","tool_input":{"pattern":"LspClientApi"}}`
- **THEN** the reminder is emitted

#### Scenario: Grep tool regex
- **WHEN** `tool_input.pattern` is `fn \w+\(`
- **THEN** nothing is printed
