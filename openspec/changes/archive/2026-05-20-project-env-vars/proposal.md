## Why

Language servers like rust-analyzer read environment variables from the parent process (e.g., `DATABASE_URL` for SQLx projects). Currently, symtrace-mcp spawns LSP servers with inherited parent environment with no way to customize per project. Users working on multiple projects with different environment needs cannot configure per-project env vars.

## What Changes

- Add optional `env` field (inline TOML table) to `[[projects]]` entries in `.symtrace.toml`
- Environment variables are augmented (inherited from parent + overridden/added from config)
- Env vars apply to all LSP servers spawned for that project root

Example config:
```toml
[[projects]]
root = "my-app"
env = { DATABASE_URL = "postgres://localhost/mydb" }
```

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `config-file`: Add `env` field to project entries
- `project-registry`: Pass env from project config to language server manager
- `lsp-transport`: Accept env vars when spawning LSP server process
- `server-manager`: Store and forward project env to LSP client on spawn

## Impact

- Config: `ProjectEntry` gains optional `env` field
- Internal: env flows through registry → manager → client → transport
- Backward compatible: `env` is optional, existing configs unchanged
- No new dependencies
