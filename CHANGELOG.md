# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `index_timeout_secs` option in `[server.<lang>]` of `.symtrace.toml` (default 30, at most 86400): how long the first tool call waits for the language server to finish loading the workspace; `0` skips the wait. Raise it for large Rust workspaces.
- `symtrace-mcp --version` / `-V` print the version (#27).

### Changed

- The startup time that `symtrace-mcp stats` reports for rust-analyzer (`avg startup`) now measures the time until rust-analyzer reports that it has finished loading the workspace (or the index timeout), instead of about 2.5 seconds.

### Fixed

- `setup` reports `updated`, not `created`, when it adds its block or entry to a file that already existed (Explore.md written by another tool, an existing CLAUDE.md, settings.json or `.claude.json`) (#27).
- `setup --status` shows `installed v<version>` for the hook and MCP rows too, so all four rows read the same (#27).
- `incoming_calls` and `outgoing_calls` now work on TypeScript/JavaScript projects. symtrace-mcp now declares the `textDocument.callHierarchy` client capability, without which typescript-language-server did not advertise call hierarchy (#28).
- The first queries on a large Rust workspace no longer return empty results before rust-analyzer has finished loading it (#29). The index wait now follows rust-analyzer's `experimental/serverStatus` notification instead of giving up after 5 empty `workspace/symbol` polls, and a query that comes back empty while rust-analyzer is still indexing returns a "language server is still indexing the workspace; retry in a few seconds" error instead of an empty answer.
- `symtrace-mcp stats` always reported `uptime 0s total` for language servers because stop events were recorded without a duration. Each stop now records the time since the server was spawned; only stops recorded after upgrading count toward uptime (#25).
- An idle-timeout shutdown recorded an extra `manual` stop event alongside the `idle_timeout` one; each stop now records exactly one event (#25).
- The MCP server no longer exits at startup when the stats database cannot be opened, for example because another symtrace-mcp process in the same directory holds its lock. It now logs a warning with the cause and the database path, and runs with usage statistics disabled for that session.
- `goto_definition`, `find_references`, `hover` and `rename` now return project-wide results on the first query of a TypeScript/JavaScript project instead of file-local ones (e.g. the import line instead of the definition in another file). symtrace-mcp now disables typescript-language-server's syntax-only server (`tsserver.useSyntaxServer: "never"`), which answered requests while the project was still loading. The first query now waits for the project to load (#30).
- A `[server.<lang>]` section in `.symtrace.toml` no longer requires `command`. Omitting it keeps the default language server (`rust-analyzer` or `typescript-language-server --stdio`), so `index_timeout_secs` or `idle_timeout_secs` can be set on their own; before, such a file failed to parse with `missing field command` and the MCP server did not start.

## [0.6.0] — 2026-10-23

### Added

- `symtrace-mcp setup` subcommand that installs the Claude Code integration in one step: a prompt block in `~/.claude/CLAUDE.md` and `~/.claude/agents/Explore.md`, `PreToolUse` hook entries in `~/.claude/settings.json`, and the MCP server entry in `~/.claude.json`; with `--local`, `--dry-run`, `--force`, `--skip-*`, `--status`, `--uninstall`
- `symtrace-mcp hook` subcommand (Claude Code `PreToolUse` hook) that attaches a reminder to use symtrace-mcp tools when the model runs `sb callers`-style queries or identifier greps over source code
- Structured JSONL logging to `.symtrace/logs/` via tracing ecosystem (per-invocation files, configurable log level, 7-day cleanup) (#20)

### Fixed

- `hover`, `goto_definition`, `find_implementations`, `rename`, `incoming_calls` and `outgoing_calls` now retry on rust-analyzer's transient `ContentModified` / `ServerCancelled` errors, as `find_references` and `diagnostics` already did. Previously a request sent while rust-analyzer was still settling failed with "content modified".

### Changed

- JSON objects are now serialized in insertion order instead of sorted key order (`serde_json` `preserve_order`, needed so `setup` keeps the key order of Claude Code's config files). This also affects the `json: true` output of `hover`, `diagnostics` and `rename`; clients must not depend on key order.

## [0.5.1] — 2026-05-20

### Added

- Per-project environment variables in `.symtrace.toml` (`[[projects]]` now accepts `env` field) (#18)

## [0.5.0] — 2026-05-17

### Added

- `hover`, `diagnostics`, and `rename` MCP tools for type info, on-demand errors, and rename preview (#14)
- TypeScript/JavaScript support (`.ts`, `.tsx`, `.js`, `.jsx`) via typescript-language-server (#15)
- Push diagnostics caching with moka TTL for TypeScript servers that lack pull diagnostics (#15)
- Mockable `LspClientApi` trait with 16 new handler unit tests (#16)
- Integration tests with real LSP servers, fixture projects, and feature-gated CI workflow (#17)
### Changed

- Version bumped from 0.1.0 to 0.5.0 (#14)

## [0.1.0] — 2026-05-17

### Added

- MCP server framework with newline-delimited JSON-RPC over stdio (#1)
- OpenSpec workflow for proposing, applying, and archiving changes (#1)
- `find_references`, `goto_definition`, and `find_implementations` tools backed by rust-analyzer (#2)
- Lazy LSP server startup, file-state sync, idle monitoring, and auto-shutdown (#2)
- Multi-project support via `.symtrace.toml` config with longest-prefix routing (#4)
- `incoming_calls` and `outgoing_calls` call hierarchy tools (#8)
- Usage statistics with 30-day retention in `.symtrace/stats.db` (#9)
- `symtrace-mcp stats` CLI subcommand for 7-day usage summary (#9)
- Turso multiprocess WAL for concurrent CLI reads while server is running (#11)
- GitHub Actions CI workflow (#5)
- Japanese README translation (#6)
- README roadmap table, CI badges, and plugin conflict documentation (#12)
- Claude Code LSP plugins investigation document (#13)

[Unreleased]: https://github.com/tatsuya6502/symtrace-mcp/compare/v0.6.0...HEAD
[0.6.0]: https://github.com/tatsuya6502/symtrace-mcp/compare/v0.5.1...v0.6.0
[0.5.1]: https://github.com/tatsuya6502/symtrace-mcp/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/tatsuya6502/symtrace-mcp/compare/v0.1.0...v0.5.0
[0.1.0]: https://github.com/tatsuya6502/symtrace-mcp/releases/tag/v0.1.0
