## Why

symtrace-mcp has 56 unit tests covering individual components, but no tests that validate the full end-to-end stack: MCP JSON-RPC framing, handler dispatch, LSP client, transport, and child process lifecycle. Protocol regressions (broken framing, missing tool registrations, malformed error envelopes) can slip through unit tests because each layer is tested in isolation.

## What Changes

- Add integration tests that spawn `symtrace-mcp` as a subprocess and send MCP JSON-RPC requests over stdin/stdout
- Create fixture projects (Rust and TypeScript) for testing all 8 tools against real language servers
- Add a shared test harness (`McpClient`) for subprocess management and JSON-RPC transport
- Gate integration tests behind per-language feature flags (`integration-rust`, `integration-typescript`)
- Add a separate GitHub Actions workflow (`integration.yml`) with a language matrix
- Pin `typescript-language-server` and `typescript` to specific versions; SHA-pin GHA actions via `pinact`

## Capabilities

### New Capabilities
- `integration-tests`: End-to-end MCP integration tests with real LSP servers, fixture projects, and CI workflow

### Modified Capabilities

_(No existing specs change at the requirement level — integration tests are additive.)_

## Impact

- **New files**: `tests/{common/mod.rs, integration_rust.rs, integration_ts.rs}`, `fixtures/{rust-project/, ts-project/}`, `.github/workflows/integration.yml`
- **Modified files**: `Cargo.toml` (feature flags), `.github/workflows/ci.yml` (pinact SHA-pinning)
- **CI**: New `integration.yml` workflow runs in parallel with existing `ci.yml`; adds ~2-3 minutes per matrix slot (LSP server install + indexing time)
- **Dependencies**: No new crate dependencies — tests use `tokio::process` and JSON parsing from existing deps
