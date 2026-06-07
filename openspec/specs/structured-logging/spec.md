## Requirements

### Requirement: Logging initialization
The system SHALL initialize a `tracing` subscriber at server startup that writes JSONL-formatted log events to a per-invocation log file. The log file SHALL be named `symtrace-mcp.YYYY-MM-DD_HHmmss.PID.log` where `YYYY-MM-DD_HHmmss` is the local time at startup and `PID` is the process ID. The log directory SHALL be `<project_root>/.symtrace/logs/` by default, overridable by the `SYMTRACE_LOG_DIR` environment variable.

#### Scenario: Default initialization
- **WHEN** the MCP server starts with no logging-related environment variables or config
- **THEN** a log file SHALL be created at `<project_root>/.symtrace/logs/symtrace-mcp.<timestamp>.<pid>.log` with log level `info`

#### Scenario: Custom log directory via environment variable
- **WHEN** `SYMTRACE_LOG_DIR` is set to `/tmp/symtrace-logs`
- **THEN** log files SHALL be written to `/tmp/symtrace-logs/`

#### Scenario: Logging disabled
- **WHEN** `SYMTRACE_LOG` is set to `off`
- **THEN** no log subscriber SHALL be initialized and no log files SHALL be created

#### Scenario: Custom log level
- **WHEN** `SYMTRACE_LOG` is set to `debug`
- **THEN** the subscriber SHALL emit events at `debug` level and above

### Requirement: Non-blocking file writes
The log subscriber SHALL use `tracing_appender::non_blocking` to wrap the log file handle. The `WorkerGuard` SHALL be held in `main()` scope so that it flushes on drop when the process exits.

#### Scenario: Flush on normal exit
- **WHEN** the server shuts down normally (stdin closed)
- **THEN** the `WorkerGuard` SHALL be dropped, flushing all buffered log events to disk

#### Scenario: No blocking on tool calls
- **WHEN** a tool call triggers a log event while the file is being written
- **THEN** the log write SHALL not block the async runtime (it buffers internally)

### Requirement: Log file format
Each log line SHALL be a self-contained JSON object (JSONL format) produced by `tracing-subscriber`'s JSON formatter. The JSON object SHALL include at minimum: `timestamp` (ISO 8601), `level`, `target`, and `fields` containing the message and any structured data.

#### Scenario: Log line structure
- **WHEN** `info!("Server started", cwd = %path)` is called
- **THEN** a JSON line SHALL be written containing `"level":"INFO"`, `"target":"symtrace_mcp"`, `"fields":{"message":"Server started","cwd":"<path>"}`

### Requirement: Log file cleanup
The system SHALL delete log files older than 7 days from the log directory on startup. Only files matching the pattern `symtrace-mcp.*.log` SHALL be considered for deletion.

#### Scenario: Cleanup on startup
- **WHEN** the server starts and the log directory contains files older than 7 days
- **THEN** those files SHALL be deleted before the new log file is created

#### Scenario: No old files
- **WHEN** the server starts and no files are older than 7 days
- **THEN** no files SHALL be deleted

### Requirement: Log level control
The system SHALL determine the log level from the following sources in priority order: `SYMTRACE_LOG` environment variable (highest) → `[logging]` config section `level` field → hardcoded default `info`. The `SYMTRACE_LOG` value SHALL accept: `off`, `error`, `warn`, `info`, `debug`, `trace`, or `tracing-subscriber::filter::Targets` filter syntax.

#### Scenario: Environment variable overrides config
- **WHEN** `.symtrace.toml` has `[logging] level = "warn"` and `SYMTRACE_LOG=debug`
- **THEN** the log level SHALL be `debug`

#### Scenario: Config file default
- **WHEN** `SYMTRACE_LOG` is not set and `.symtrace.toml` has `[logging] level = "debug"`
- **THEN** the log level SHALL be `debug`

#### Scenario: Hardcoded default
- **WHEN** neither `SYMTRACE_LOG` nor `[logging]` config is set
- **THEN** the log level SHALL be `info`

#### Scenario: Targets filter syntax
- **WHEN** `SYMTRACE_LOG=symtrace_mcp=debug,turso=warn`
- **THEN** the subscriber SHALL use `tracing-subscriber::filter::Targets` parsed from the value

### Requirement: Replace eprintln with tracing macros
All `eprintln!` calls in server code (not the `stats` CLI subcommand) SHALL be replaced with appropriate `tracing` macros: `error!` for fatal/error conditions, `warn!` for non-critical failures, `info!` for lifecycle events.

#### Scenario: Error condition
- **WHEN** stats recording fails during a tool call
- **THEN** `warn!("stats recording failed"; "error" => %e)` SHALL be used instead of `eprintln!`

#### Scenario: Fatal startup error
- **WHEN** config loading fails
- **THEN** `error!("error loading config"; "path" => %p, "error" => %e)` SHALL be used before `process::exit(1)`

### Requirement: No logging for stats CLI
The `symtrace-mcp stats` subcommand SHALL NOT initialize a log subscriber. `tracing` macros in shared code SHALL be no-ops (the default behavior when no subscriber is set).

#### Scenario: Stats command runs without logging
- **WHEN** `symtrace-mcp stats` is invoked
- **THEN** no log subscriber SHALL be initialized and no log files SHALL be created

### Requirement: Startup log event
The system SHALL emit an `info!` event at server startup containing the current working directory and process ID.

#### Scenario: Server start logged
- **WHEN** the MCP server starts successfully
- **THEN** a log event SHALL be written with message "Server started" containing `cwd` and `pid` fields
