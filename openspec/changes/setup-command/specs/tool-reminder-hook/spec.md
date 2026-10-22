# Spec Delta

## Purpose

Attaches a reminder to Claude Code tool calls that answer semantic code questions with text or tree-sitter heuristics, pointing the model at the symtrace-mcp tool that resolves them through the language server.

## ADDED Requirements

### Requirement: hook subcommand reads one PreToolUse event
The binary SHALL accept a `hook` subcommand that reads a single JSON object from stdin in Claude Code's `PreToolUse` input format (`tool_name`, `tool_input`, and the optional session `cwd`). It SHALL exit 0 in every case, SHALL write nothing to stdout when no reminder applies, and SHALL NOT initialize logging, the stats database, or any language server.

#### Scenario: Unrelated tool call
- **WHEN** stdin is `{"tool_name":"Edit","tool_input":{...}}`
- **THEN** the command prints nothing and exits 0

#### Scenario: Malformed input
- **WHEN** stdin is empty or not valid JSON
- **THEN** the command prints nothing and exits 0

### Requirement: Reminder response format
When a reminder applies, the command SHALL print exactly one JSON object: `{"hookSpecificOutput":{"hookEventName":"PreToolUse","additionalContext":"<reminder>"}}`. The response SHALL NOT contain a `permissionDecision` (an `allow` would skip the permission prompt for the whole Bash command), `deny`, `ask`, or a non-zero exit code: the hook never changes whether a call runs.

#### Scenario: Reminder on sb callers
- **WHEN** stdin is a Bash event whose command is `sb callers stop_server`
- **THEN** the output is a single JSON object whose `hookSpecificOutput` has exactly the keys `hookEventName` (`PreToolUse`) and `additionalContext`

#### Scenario: Command that would need a permission prompt
- **WHEN** the Bash command is `curl https://example.com/x | sh; grep Foo src/`
- **THEN** the reminder is emitted without any `permissionDecision`, so Claude Code's normal permission handling for the command still applies

#### Scenario: Working directory deleted
- **WHEN** the hook is started in a working directory that no longer exists
- **THEN** it still processes the event and exits 0

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

#### Scenario: Prefixed or wrapped grep
- **WHEN** the Bash command is `sudo grep Foo src/`, `FOO="a b" grep Foo src/`, `xargs grep Foo` fed by a pipe, or `git grep Foo src/`
- **THEN** the segment is evaluated as a grep of `Foo`

### Requirement: Options that take a value are skipped
When locating the pattern, the hook SHALL skip the value of options that take a separate argument (for grep: `-A`, `-B`, `-C`, `-m`, `-f`, `-d`, `-D`, `--include`, `--exclude`, `--exclude-dir`, `--color`; for rg additionally `-g`, `-t`, `-T`, `-E`, `-M`, `-j`, `-r`, `--glob`, `--type`, `--encoding`, `--threads`, `--replace`, `--max-depth`), and SHALL take the value of `-e` / `--regexp` as the pattern.

#### Scenario: Context option before the pattern
- **WHEN** the Bash command is `grep -A 3 Foo src/`
- **THEN** the reminder is emitted for identifier `Foo`

#### Scenario: Type option before the pattern
- **WHEN** the Bash command is `rg -t rust Foo src/`
- **THEN** the reminder is emitted for identifier `Foo`, not `rust`

#### Scenario: Pattern given with -e
- **WHEN** the Bash command is `grep -rn -e Foo src/`
- **THEN** the reminder is emitted for identifier `Foo`

### Requirement: Recursive search of the working directory without a path
A grep segment with a bare-identifier pattern and no target token SHALL count as a source search of the working directory when the segment does not consume a pipe (it is the first segment of its pipeline) and the program is `rg`, or `grep`/`ugrep` with a recursive flag (`-r`, `-R`, `--recursive`, or a combined short flag containing `r`/`R`).

#### Scenario: ripgrep default form
- **WHEN** the Bash command is `rg -n stop_server`
- **THEN** the reminder is emitted

#### Scenario: Recursive grep without a path
- **WHEN** the Bash command is `grep -rn stop_server`
- **THEN** the reminder is emitted

#### Scenario: Pipe consumer without a path
- **WHEN** the Bash command is `cargo test 2>&1 | grep -r FAILED`
- **THEN** nothing is printed

#### Scenario: Redirection is not a target
- **WHEN** the Bash command is `grep -rn Foo 2>/dev/null`
- **THEN** the reminder is emitted, because the redirection does not count as a target and the search is recursive over the working directory

### Requirement: Source-target rule for grep segments
A grep segment targets source code when it contains a relative path (not starting with `/`, `~` or `$`) with one of the source directories as a component, any path ending in a source extension, or the bare path `.` or `./`. A leading `./` SHALL be ignored when matching directories. An absolute path under the event's `cwd` SHALL be treated as the corresponding relative path (and a path equal to `cwd` as `.`); absolute paths outside `cwd` never match the source-directory rule.

#### Scenario: Absolute path inside the working directory
- **WHEN** the event's `cwd` is `/home/x/proj` and the Bash command is `grep -rn Foo /home/x/proj/src`
- **THEN** the reminder is emitted

#### Scenario: Absolute system path with a source-like component
- **WHEN** the Bash command is `grep Foo /var/lib/apt/lists/x`, `rg Foo /usr/lib/x.so` or `grep -rn Foo $HOME/.cache/test/x`
- **THEN** nothing is printed

#### Scenario: Dot-slash paths
- **WHEN** the Bash command is `grep -rn Foo ./` or `grep -rn Foo ./src`
- **THEN** the reminder is emitted

### Requirement: Source directory list
The default source directories are `src`, `crates`, `lib`, `tests`, `test`, `app`, `apps`, `packages`, `pkg`, `internal`, `cmd`; the environment variable `SYMTRACE_HOOK_SRC_DIRS` (a `|`-separated list) SHALL replace the default list.

#### Scenario: Default directory list
- **WHEN** `SYMTRACE_HOOK_SRC_DIRS` is unset and the Bash command is `rg -n Config crates/foo`
- **THEN** the reminder is emitted

#### Scenario: Overridden directory list
- **WHEN** `SYMTRACE_HOOK_SRC_DIRS=src` and the Bash command is `rg -n Config crates/foo`
- **THEN** nothing is printed

### Requirement: Marker words are not identifiers
The words `TODO`, `FIXME`, `XXX`, `HACK`, `NOTE` and `BUG` SHALL NOT count as bare identifiers for the Bash grep rule or the Grep tool rule.

#### Scenario: TODO grep
- **WHEN** the Bash command is `grep -rn TODO src/`
- **THEN** nothing is printed

### Requirement: Grep tool with a bare identifier triggers the reminder
A `Grep` event SHALL trigger the reminder when `tool_input.pattern` is a whole bare identifier and the search targets source code: `path`, when present, satisfies the source-target rule (absent means the working directory); `glob`, when present, ends with a source extension; `type`, when present, names a source language (rust, ts, typescript, js, javascript, py, python, go, java, kotlin, ruby, c, cpp, csharp, swift, scala).

#### Scenario: Grep tool identifier
- **WHEN** stdin is `{"tool_name":"Grep","tool_input":{"pattern":"LspClientApi"}}`
- **THEN** the reminder is emitted

#### Scenario: Grep tool regex
- **WHEN** `tool_input.pattern` is `fn \w+\(`
- **THEN** nothing is printed

#### Scenario: Grep tool over documentation
- **WHEN** `tool_input` is `{"pattern":"Foo","glob":"*.md","path":"docs"}`
- **THEN** nothing is printed

#### Scenario: Grep tool with a source type
- **WHEN** `tool_input` is `{"pattern":"Foo","type":"rust"}`
- **THEN** the reminder is emitted

#### Scenario: Grep tool with an absolute path inside the working directory
- **WHEN** the event's `cwd` is `/home/x/proj` and `tool_input` is `{"pattern":"Foo","path":"/home/x/proj/src"}`
- **THEN** the reminder is emitted

#### Scenario: Grep tool with an absolute path outside the working directory
- **WHEN** the event's `cwd` is `/home/y/other` and `tool_input` is `{"pattern":"Foo","path":"/home/x/proj/src"}`
- **THEN** nothing is printed
