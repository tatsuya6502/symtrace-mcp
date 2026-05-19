## 1. Feature Flags and Build Setup

- [ ] 1.1 Add `integration-rust`, `integration-typescript`, and `integration` feature flags to `Cargo.toml`
- [ ] 1.2 Create `tests/common/mod.rs` with module structure (empty initially)

## 2. Test Harness

- [ ] 2.1 Implement `McpClient::spawn(cwd)` — start `symtrace-mcp` subprocess with piped stdin/stdout
- [ ] 2.2 Implement JSON-RPC write (Content-Length framing)
- [ ] 2.3 Implement JSON-RPC read (parse headers, extract response body, match by ID)
- [ ] 2.4 Implement `send_request(method, params)` — combines write + read with ID tracking
- [ ] 2.5 Implement `wait_for_ready(query, timeout)` — poll with exponential backoff until LSP responds
- [ ] 2.6 Implement `shutdown()` — terminate subprocess gracefully

## 3. Rust Fixture Project

- [ ] 3.1 Create `fixtures/rust-project/Cargo.toml` (minimal lib project)
- [ ] 3.2 Create `fixtures/rust-project/src/lib.rs` with struct, trait + impls, call graph, doc comments

## 4. TypeScript Fixture Project

- [ ] 4.1 Create `fixtures/ts-project/package.json` and `tsconfig.json`
- [ ] 4.2 Create `fixtures/ts-project/index.ts` with interface, classes, call graph

## 5. Rust Integration Tests

- [ ] 5.1 Create `tests/integration_rust.rs` with `mod common;` and feature gate
- [ ] 5.2 Test `tools/list` — verify all 8 tools registered with valid schemas
- [ ] 5.3 Test `find_references` — verify returns multiple locations for a known symbol
- [ ] 5.4 Test `goto_definition` — verify resolves function call to definition
- [ ] 5.5 Test `find_implementations` — verify returns trait implementations
- [ ] 5.6 Test `incoming_calls` — verify returns callers of a known function
- [ ] 5.7 Test `outgoing_calls` — verify returns callees from a known function
- [ ] 5.8 Test `hover` — verify returns hover content for a typed symbol
- [ ] 5.9 Test `diagnostics` — verify returns successfully (may be empty)
- [ ] 5.10 Test `rename` — verify returns updated locations

## 6. TypeScript Integration Tests

- [ ] 6.1 Create `tests/integration_ts.rs` with `mod common;` and feature gate
- [ ] 6.2 Test `tools/list` — verify all 8 tools registered with valid schemas
- [ ] 6.3 Test `find_references` — verify returns multiple locations for a known symbol
- [ ] 6.4 Test `goto_definition` — verify resolves function call to definition
- [ ] 6.5 Test `find_implementations` — verify returns interface implementations
- [ ] 6.6 Test `incoming_calls` — verify returns callers of a known function
- [ ] 6.7 Test `outgoing_calls` — verify returns callees from a known function
- [ ] 6.8 Test `hover` — verify returns hover content for a typed symbol
- [ ] 6.9 Test `diagnostics` — verify returns successfully (may be empty)
- [ ] 6.10 Test `rename` — verify returns updated locations

## 7. CI Workflow

- [ ] 7.1 Create `.github/workflows/integration.yml` with language matrix
- [ ] 7.2 Add Rust matrix slot: install rust-analyzer via rustup, run `cargo test --features integration-rust`
- [ ] 7.3 Add TypeScript matrix slot: install pinned npm packages, run `cargo test --features integration-typescript`
- [ ] 7.4 Set `fail-fast: false` so one language failure doesn't block the other
- [ ] 7.5 Run `pinact` on both `ci.yml` and `integration.yml` to SHA-pin GHA actions
- [ ] 7.6 Verify both workflows pass on a test push
