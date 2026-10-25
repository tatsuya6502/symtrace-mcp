# Spec Delta

## ADDED Requirements

### Requirement: Version flag
The binary SHALL accept `--version` and `-V`. Either flag SHALL print `symtrace-mcp <crate version>` followed by a newline to stdout and exit with code 0, without starting the MCP server, opening the stats database, or touching any file.

#### Scenario: Long version flag
- **WHEN** the user runs `symtrace-mcp --version` with a binary built from crate version 0.6.1
- **THEN** stdout is `symtrace-mcp 0.6.1` and the exit code is 0

#### Scenario: Short version flag
- **WHEN** the user runs `symtrace-mcp -V`
- **THEN** the output is identical to `symtrace-mcp --version`

#### Scenario: Version flag does not start the server
- **WHEN** the user runs `symtrace-mcp --version`
- **THEN** the process exits immediately without reading stdin or spawning a language server
