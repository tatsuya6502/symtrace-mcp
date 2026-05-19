## Context

symtrace-mcp has 56 unit tests covering individual components (LSP transport, client API, handlers, config parsing). These mock the LSP layer via the `LspClientApi` trait. No tests validate the full path from MCP JSON-RPC request through handler dispatch to real LSP server interaction.

The existing CI (`ci.yml`) runs `cargo build && cargo test` — fast, unit-only. Integration tests need real language servers installed, adding 30-60s of setup per language.

## Goals / Non-Goals

**Goals:**
- Validate full end-to-end MCP protocol: JSON-RPC framing, tool registration, tool execution
- Test all 8 tools against real `rust-analyzer` and `typescript-language-server`
- CI runs integration tests in parallel with unit tests via a separate workflow
- Per-language feature flags allow running a single language's tests in isolation
- Supply chain safety: pinned TypeScript dependencies, SHA-pinned GHA actions

**Non-Goals:**
- Testing multi-project configuration through integration tests (already covered by unit tests)
- Testing error diagnostics on deliberately broken code (unit tests cover parsing logic)
- Performance or load testing
- Testing with languages beyond Rust and TypeScript (Python, Kotlin are future work)

## Decisions

### D1: Test file structure — separate files per language

Two test files: `tests/integration_rust.rs` and `tests/integration_ts.rs`, with shared code in `tests/common/mod.rs`.

**Why not a single file:** Separate files compile as separate crates, giving natural isolation. Maps cleanly to the GHA matrix (one file per matrix slot). Avoids installing both LSP servers in a single CI job.

**Why not parameterized tests:** Parameterizing by language would require both servers in one job or complex conditional logic. Separate files are simpler and more debuggable.

### D2: Gating — per-language feature flags

```toml
[features]
integration-rust = []
integration-typescript = []
integration = ["integration-rust", "integration-typescript"]
```

**Why not a single `integration` flag:** With a GHA matrix, each slot only installs one LSP server. A single flag would compile both test files in every slot, causing failures for the missing server. Per-language flags ensure each slot only compiles and runs its own tests.

**Why not `#[ignore]`:** Feature flags are more explicit and composable. `--features integration-rust` is self-documenting; `-- --ignored` requires knowing which tests are ignored and why.

### D3: Test harness — `McpClient` struct

A shared `McpClient` in `tests/common/mod.rs` handles:
- Spawning `symtrace-mcp` as a subprocess with piped stdin/stdout
- JSON-RPC message framing (Content-Length headers)
- Request/response matching by ID (skip notifications)
- Readiness polling with timeout
- Graceful shutdown

**Why not use an existing MCP client library:** Adds a dependency for test-only code. The MCP protocol is simple enough (JSON-RPC over stdio) that a lightweight harness is cleaner.

### D4: Fixture projects — implicit config via CWD

Each test sets the subprocess's CWD to its fixture directory (e.g., `fixtures/rust-project/`). symtrace-mcp's `SymtraceConfig::implicit(cwd)` creates a single-project config from CWD — no `.symtrace.toml` needed.

**Why not write a temporary config:** Simpler, fewer moving parts. Implicit config is the default path for single-language projects.

### D5: Readiness detection — poll with backoff

Send a `goto_definition` request on a known symbol, retry with exponential backoff (100ms initial, 2x cap), fail after 30s timeout.

**Why not fixed sleep:** Self-calibrating — fast on powerful machines, patient on slow CI runners. A fixed 5s sleep wastes time locally and may be too short on overloaded runners.

**Why not parse logs:** Couples tests to internal log format. Polling is a black-box approach that tests the same path real clients use.

### D6: CI — separate workflow with language matrix

```yaml
# .github/workflows/integration.yml
strategy:
  fail-fast: false
  matrix:
    include:
      - language: rust
        feature: integration-rust
      - language: typescript
        feature: integration-typescript
```

Each slot installs its LSP server, then runs `cargo test --features ${{ matrix.feature }}`.

**Why separate from `ci.yml`:** Integration tests are slower (LSP install + indexing). Separating them keeps unit test feedback fast. If integration tests fail, the signal is clear — it's a protocol or LSP issue, not a code issue.

### D7: Supply chain — pinned versions and SHA-pinned actions

- `typescript-language-server@<exact>` and `typescript@<exact>` pinned in workflow env vars
- All GHA actions SHA-pinned via `pinact`
- `rust-analyzer` via `rustup` — trusted via Rust release process

## Risks / Trade-offs

**CI flakiness from LSP startup time** → 30s poll timeout is generous. If tests flake, increase timeout or add retry logic per-test.

**rust-analyzer behavior changes between versions** → Tests assert on structure (response has `locations` array with >0 items), not exact text. Loose assertions reduce false positives from minor LSP output changes.

**Fixture code drift** → Fixtures are small (~40-50 lines each) and intentionally simple. Line/column assertions use named constants, not magic numbers.

**Feature flag proliferation** → Each new language adds one flag. Acceptable for the foreseeable future (Python, Kotlin). If it grows unwieldy, refactor to a single flag with runtime filtering.

**No testing of diagnostics with real errors** → Deliberate trade-off. Unit tests cover diagnostics parsing. Could add error-fixture tests in a future iteration.
