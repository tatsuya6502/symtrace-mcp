## ADDED Requirements

### Requirement: stats subcommand
The binary SHALL accept a `stats` subcommand via clap. When invoked as `symtrace-mcp stats`, it SHALL open `.symtrace/stats.db` in CWD (or the project root if determinable), query the last 7 days of data, and print a human-readable summary to stdout, then exit.

#### Scenario: Run stats subcommand
- **WHEN** the user runs `symtrace-mcp stats`
- **THEN** the system prints a formatted summary and exits with code 0

#### Scenario: No stats database exists
- **WHEN** `symtrace-mcp stats` is run and `.symtrace/stats.db` does not exist
- **THEN** the system prints "No stats data found" and exits with code 0

### Requirement: Stats output format — tool usage
The stats output SHALL include a "Tool Usage" section showing, for each tool: call count, average duration in milliseconds, and error count. Tools SHALL be listed in descending order by call count.

#### Scenario: Tool usage with data
- **WHEN** tool calls exist in the last 7 days
- **THEN** the output includes lines like `  goto_definition      32 calls   89ms avg   2 errors`

#### Scenario: No tool calls in window
- **WHEN** no tool calls exist in the last 7 days
- **THEN** the "Tool Usage" section shows "(no data)"

### Requirement: Stats output format — top files
The stats output SHALL include a "Top Files" section showing the top 10 files by tool call count, with call counts. File paths SHALL be displayed as relative paths from the project root (the directory containing `.symtrace/`).

#### Scenario: Files with multiple calls
- **WHEN** tool calls reference different files
- **THEN** the output lists up to 10 files sorted by call count descending, with relative paths (e.g., `src/stats.rs` instead of `/full/path/src/stats.rs`)

#### Scenario: File outside project root
- **WHEN** a file path does not start with the project root prefix
- **THEN** the full path is displayed as-is

### Requirement: Stats output format — language servers
The stats output SHALL include a "Language Servers" section showing, for each language: number of startups, average startup duration, and total uptime.

#### Scenario: Server lifecycle data
- **WHEN** server events exist in the last 7 days
- **THEN** the output includes lines like `  rust  started 5×  avg startup 2.3s  uptime 4h 12m total`

### Requirement: Default subcommand preserves MCP server behavior
Running `symtrace-mcp` with no subcommand SHALL start the MCP server (current behavior). The `stats` subcommand is the only additional subcommand.

#### Scenario: Run with no arguments
- **WHEN** the user runs `symtrace-mcp` with no arguments
- **THEN** the MCP server starts normally (identical to current behavior)
