## 1. Feature Flags and Build Setup

- [x] 1.1 Add `integration-rust`, `integration-typescript`, and `integration` feature flags to `Cargo.toml`
- [x] 1.2 Create `tests/common/mod.rs` with module structure (empty initially)

## 2. Test Harness

- [x] 2.1 Implement `McpClient::spawn(cwd)` — start `symtrace-mcp` subprocess with piped stdin/stdout
- [x] 2.2 Implement JSON-RPC write (newline-delimited JSON framing)
- [x] 2.3 Implement JSON-RPC read (parse lines, extract response body, match by ID)
- [x] 2.4 Implement `send_request(method, params)` — combines write + read with ID tracking
- [x] 2.5 Implement `wait_for_ready(query, timeout)` — poll with exponential backoff until LSP responds
- [x] 2.6 Implement `shutdown()` — terminate subprocess gracefully

## 3. Rust Fixture Project

- [x] 3.1 Create `fixtures/rust-project/Cargo.toml` (minimal lib project)
- [x] 3.2 Create `fixtures/rust-project/src/lib.rs` with struct, trait + impls, call graph, doc comments

## 4. TypeScript Fixture Project

- [x] 4.1 Create `fixtures/ts-project/package.json` and `tsconfig.json`
- [x] 4.2 Create `fixtures/ts-project/index.ts` with interface, classes, call graph

## 5. Rust Integration Tests

- [x] 5.1 Create `tests/integration_rust.rs` with `mod common;` and feature gate
- [x] 5.2 Test `tools/list` — verify all 8 tools registered with valid schemas
- [x] 5.3 Test `find_references` — verify returns multiple locations for a known symbol
- [x] 5.4 Test `goto_definition` — verify resolves function call to definition
- [x] 5.5 Test `find_implementations` — verify returns trait implementations
- [x] 5.6 Test `incoming_calls` — verify returns callers of a known function
- [x] 5.7 Test `outgoing_calls` — verify returns callees from a known function
- [x] 5.8 Test `hover` — verify returns hover content for a typed symbol
- [x] 5.9 Test `diagnostics` — verify returns successfully (may be empty)
- [x] 5.10 Test `rename` — verify returns updated locations

## 6. TypeScript Integration Tests

- [x] 6.1 Create `tests/integration_ts.rs` with `mod common;` and feature gate
- [x] 6.2 Test `tools/list` — verify all 8 tools registered with valid schemas
- [x] 6.3 Test `find_references` — verify returns multiple locations for a known symbol
- [x] 6.4 Test `goto_definition` — verify resolves function call to definition
- [x] 6.5 Test `find_implementations` — verify returns interface implementations
- [x] ~~6.6 Test `incoming_calls` — typescript-language-server does not support Call Hierarchy~~
- [x] ~~6.7 Test `outgoing_calls` — typescript-language-server does not support Call Hierarchy~~
- [x] 6.8 Test `hover` — verify returns hover content for a typed symbol
- [x] 6.9 Test `diagnostics` — verify returns successfully (may be empty)
- [x] 6.10 Test `rename` — verify returns updated locations

## 7. CI Workflow

- [x] 7.1 Create `.github/workflows/integration.yml` with language matrix
- [x] 7.2 Add Rust matrix slot: install rust-analyzer via rustup, run `cargo test --features integration-rust`
- [x] 7.3 Add TypeScript matrix slot: install pinned npm packages, run `cargo test --features integration-typescript`
- [x] 7.4 Set `fail-fast: false` so one language failure doesn't block the other
- [x] 7.5 Run `pinact` on both `rust.yml` and `integration.yml` to SHA-pin GHA actions
- [x] 7.6 Verify both workflows pass on a test push
