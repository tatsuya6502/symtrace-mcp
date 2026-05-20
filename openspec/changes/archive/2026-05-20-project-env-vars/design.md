## Context

symtrace-mcp spawns LSP servers (rust-analyzer, typescript-language-server) as child processes. These inherit the parent environment. Language servers like rust-analyzer read environment variables at initialization (e.g., `DATABASE_URL` for SQLx compile-time checking). Currently there is no mechanism to set per-project environment variables.

Each project root already has its own `LanguageServerManager`, which lazily spawns one LSP server per language. The env vars need to flow from config through to the `Command::new()` call in `LspTransport::spawn()`.

## Goals / Non-Goals

**Goals:**
- Allow per-project env vars in `.symtrace.toml` via `env` field on `[[projects]]`
- Augment parent environment (inherit + add/override), not replace
- Backward compatible — `env` is optional

**Non-Goals:**
- Unsetting inherited env vars (can be added later if needed)
- Per-language env vars within a project (env applies to all LSP servers for the project)
- Env vars for single-project mode (no `[[projects]]` section) — inherits parent env as before

## Decisions

### 1. Env on `[[projects]]`, not `[server.*]`

**Decision**: `env` field lives on `ProjectEntry`, not `ServerConfig`.

**Rationale**: Environment variables like `DATABASE_URL` are properties of the project/codebase, not the analysis tool. A single project may have both rust-analyzer and typescript-language-server, and both should see the same project-level env.

### 2. Augment semantics

**Decision**: Configured env vars are added on top of the inherited parent environment using `Command::envs()`. Duplicate keys override parent values.

**Rationale**: Replacing the full environment would require re-specifying `PATH`, `HOME`, etc. Augmenting is simpler and matches user expectations.

### 3. Storage on `LanguageServerManager`

**Decision**: The env `HashMap` is stored on `LanguageServerManager` and passed through to `LspClient::start()` → `LspTransport::spawn()` when a server is lazily started.

**Rationale**: The manager already owns `root` (per-project state). Adding `env` alongside it keeps the per-project context together. The env is only needed at spawn time, which the manager controls.

### 4. TOML syntax: inline table

```toml
[[projects]]
root = "my-app"
env = { DATABASE_URL = "postgres://localhost/mydb" }
```

**Rationale**: TOML `[[projects]]` is an array of tables. Using an inline table for `env` avoids ambiguity with array-of-tables syntax.

## Risks / Trade-offs

- **Env vars may contain secrets**: Users should be aware `.symtrace.toml` with secrets should not be committed. Not a new risk — `.env` files have the same issue. → Mitigation: document this in config reference.
- **Single-project mode has no env support**: When no `[[projects]]` section exists (implicit single-project), there's nowhere to put `env`. → Acceptable for now; can add top-level `[env]` later if needed.
