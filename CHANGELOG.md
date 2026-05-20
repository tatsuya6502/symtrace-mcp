# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

[0.5.1]: https://github.com/tatsuya6502/symtrace-mcp/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/tatsuya6502/symtrace-mcp/compare/v0.1.0...v0.5.0
[0.1.0]: https://github.com/tatsuya6502/symtrace-mcp/releases/tag/v0.1.0
