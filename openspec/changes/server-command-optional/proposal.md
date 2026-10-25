# Proposal

## Why

During the v0.7.0 release check on a large multi-crate Rust workspace, a `.symtrace.toml` containing only

```toml
[server.rust]
index_timeout_secs = 600
```

made the MCP server refuse to start with ``config parse error: ... missing field `command` ``. `ServerConfig.command` is a required `String`, so any `[server.<lang>]` section has to repeat the default command even when the user only wants to change a timeout. `index_timeout_secs` is new in this release and the README documents it as usable "in any `[server.<lang>]` section", which makes omitting `command` the natural thing to write. The failure is startup-fatal: no tool works until the file is fixed.

## What Changes

- `command` becomes optional inside a `[server.<lang>]` section. When it is omitted, the language keeps its default command (`rust-analyzer`, or `typescript-language-server` with its default `--stdio` args); `idle_timeout_secs` and `index_timeout_secs` from the section still apply.
- A section that does set `command` behaves exactly as before.
- README and CHANGELOG document the change.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `config-file`: MODIFIED "Configuration loaded from .symtrace.toml" (`command` is optional; an omitted `command` keeps the language's default) and "Server configuration with global defaults" (scenario for a section without `command`).

## Impact

- `src/config.rs`: `ServerConfig.command` becomes `Option<String>`; existing tests compare via `.as_deref()`; new test for a section without `command`.
- `src/project/registry.rs`: `build_server_configs` overrides the default command only when `command` is `Some`; new tests for Rust and TypeScript sections without `command`.
- `README.md` "Multi-Project Support" snippet and bullets; `CHANGELOG.md` `## [Unreleased]` `### Fixed`.
- No API or dependency change. Every config file that parsed before parses to the same result.
