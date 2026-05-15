## Why

symtrace-mcp currently provides no visibility into how it's being used. There's no way to know which tools are called most often, how long they take, whether they're failing, or how often language servers are restarting due to idle timeouts. Adding usage tracking gives the developer insight to prioritize improvements and debug issues.

## What Changes

- Add per-call tracking for every MCP tool invocation: tool name, target file path, duration, success/error, and timestamp
- Add language server lifecycle tracking: startup count, startup duration, shutdown reason (idle timeout vs session end), total uptime
- Store tracking data in a Turso (SQLite-compatible) database per project at `.symtrace/stats.db`
- Add a `symtrace-mcp stats` CLI subcommand that prints a human-readable summary (last 7 days) including tool usage breakdown, top files by call count, and server lifecycle stats

## Capabilities

### New Capabilities
- `stats-storage`: Per-project Turso database for persisting tool call logs and server lifecycle events. Covers DB schema, open/write/close pattern, and retention policy (rolling 30-day window).
- `stats-instrumentation`: Hooks in MCP tool dispatch and server lifecycle to record events. Covers what is measured, where hooks are placed, and how data flows to storage.
- `stats-cli`: `symtrace-mcp stats` subcommand that queries the stats DB and prints a formatted summary. Covers CLI interface, output format, and time window.

### Modified Capabilities
- `mcp-server`: Binary gains a second mode (subcommand dispatch) via `clap`. The `run` path remains the MCP server; the `stats` path queries the DB directly.
- `tools-definitions`: Tool handlers gain instrumentation wrappers (no change to tool schemas or behavior).

## Impact

- **New dependency**: `turso` crate (pure Rust, async SQLite-compatible DB), `clap` (CLI arg parser)
- **New directory**: `.symtrace/stats.db` created automatically per project on first tool call
- **Code changes**: `src/mcp/tools.rs` (instrumentation in `handle_tools_call`), `src/server/manager.rs` (lifecycle hooks), `src/main.rs` (subcommand dispatch), new `src/stats/` module
- **No breaking changes**: Existing MCP tool behavior is unchanged; stats are recorded as a side effect
