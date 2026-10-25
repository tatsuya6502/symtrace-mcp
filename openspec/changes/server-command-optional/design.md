# Design

## Context

See proposal.md - Why. `ServerConfig` (`src/config.rs`) is the serde shape of a `[server.<lang>]` section, with `command: String` required and both timeouts defaulted by serde. `ProjectRegistry::build_server_configs` (`src/project/registry.rs`) starts from `default_server_configs()` and, for each recognized language key, overwrites `command`, `idle_timeout_secs` and `index_timeout_secs`. `args` are never taken from the config file.

## Goals / Non-Goals

**Goals:**
- A `[server.<lang>]` section may set only timeouts; the language's default command is kept.

**Non-Goals:**
- Configurable `args` in `.symtrace.toml`.
- Warning about unknown language keys in `[server]` (still silently ignored).

## Decisions

### D1: `Option<String>` on the config struct, resolved in `build_server_configs`
`ServerConfig.command` becomes `Option<String>` (serde treats a missing `Option` field as `None`), and `build_server_configs` replaces the default command only when it is `Some`. Alternative: a serde `default` function for `command`. Rejected: the default depends on the language key (`rust` vs `typescript`), which a field-level serde default cannot see, and `default_server_configs()` already holds the per-language defaults in one place.

### D2: `args` are untouched either way
When `command` is omitted, the language keeps its default `args` (`--stdio` for TypeScript, none for Rust). When `command` is given, `args` also stay at the default, as today. No behaviour change for existing configs.

## Risks / Trade-offs

- None significant. Every file that parsed before still parses to the same effective configuration.
- A section with an unknown language key (e.g. a typo like `[server.rsut]`) is still silently ignored, and is now even easier to write without `command`. Out of scope for this change.
