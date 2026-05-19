## ADDED Requirements

### Requirement: MCP subprocess test harness
The integration test harness SHALL spawn `symtrace-mcp` as a subprocess with piped stdin/stdout and provide JSON-RPC message transport (Content-Length framing, request/response ID matching).

#### Scenario: Spawn and initialize
- **WHEN** a test creates an `McpClient` with a fixture project directory
- **THEN** `symtrace-mcp` subprocess SHALL be spawned with CWD set to the fixture directory
- **AND** the harness SHALL be able to send MCP requests and receive responses

#### Scenario: Graceful shutdown
- **WHEN** a test calls shutdown on the `McpClient`
- **THEN** the subprocess SHALL be terminated without hanging

### Requirement: LSP readiness polling
The test harness SHALL poll the LSP server for readiness by sending a known query (e.g., `goto_definition`) with exponential backoff (100ms initial, 2x cap) until success or a configurable timeout (default 30 seconds).

#### Scenario: Server becomes ready
- **WHEN** the LSP server finishes indexing
- **THEN** the readiness poll SHALL return success

#### Scenario: Server fails to start
- **WHEN** the LSP server does not become ready within the timeout
- **THEN** the readiness poll SHALL return an error with elapsed time information

### Requirement: Per-language feature flags
`Cargo.toml` SHALL define feature flags `integration-rust` and `integration-typescript`, plus an umbrella `integration` flag that enables both.

#### Scenario: Run Rust integration tests only
- **WHEN** `cargo test --features integration-rust` is executed
- **THEN** only Rust integration tests SHALL compile and run

#### Scenario: Run all integration tests
- **WHEN** `cargo test --features integration` is executed
- **THEN** both Rust and TypeScript integration tests SHALL compile and run

### Requirement: Rust fixture project
A minimal Rust library project at `fixtures/rust-project/` SHALL contain enough code to test all 8 MCP tools: struct, trait with multiple impls, functions with call relationships, and doc comments.

#### Scenario: find_references returns multiple locations
- **WHEN** `find_references` is called on the `User` struct
- **THEN** the response SHALL include locations in the struct definition, impl block, function parameter, and usage sites

#### Scenario: find_implementations returns trait impls
- **WHEN** `find_implementations` is called on the trait
- **THEN** the response SHALL include all struct implementations of that trait

### Requirement: TypeScript fixture project
A minimal TypeScript project at `fixtures/ts-project/` SHALL contain enough code to test all 8 MCP tools: interface, classes implementing the interface, functions with call relationships.

#### Scenario: find_references returns multiple locations
- **WHEN** `find_references` is called on a class name
- **THEN** the response SHALL include the class definition, constructor usage, and call sites

#### Scenario: find_implementations returns interface impls
- **WHEN** `find_implementations` is called on the interface
- **THEN** the response SHALL include all classes implementing that interface

### Requirement: Integration tests cover all MCP tools
Integration tests for each language SHALL test all 8 tools: `find_references`, `goto_definition`, `find_implementations`, `incoming_calls`, `outgoing_calls`, `hover`, `diagnostics`, and `rename`.

#### Scenario: tools/list returns all registered tools
- **WHEN** a `tools/list` request is sent
- **THEN** the response SHALL include all 8 tools with valid JSON schemas

#### Scenario: goto_definition resolves to correct location
- **WHEN** `goto_definition` is called on a function call site
- **THEN** the response SHALL contain a location pointing to the function definition

#### Scenario: hover returns type information
- **WHEN** `hover` is called on a typed symbol
- **THEN** the response SHALL contain hover content

#### Scenario: diagnostics returns successfully
- **WHEN** `diagnostics` is called on a file with no errors
- **THEN** the response SHALL indicate success (may return empty diagnostics)

### Requirement: Separate GitHub Actions integration workflow
A `.github/workflows/integration.yml` file SHALL run integration tests independently from the unit test workflow, using a matrix strategy with one slot per language.

#### Scenario: Rust matrix slot
- **WHEN** the Rust matrix slot runs
- **THEN** it SHALL install `rust-analyzer` via `rustup`, build the project, and run `cargo test --features integration-rust`

#### Scenario: TypeScript matrix slot
- **WHEN** the TypeScript matrix slot runs
- **THEN** it SHALL install pinned versions of `typescript-language-server` and `typescript` via npm, build the project, and run `cargo test --features integration-typescript`

#### Scenario: One language failure does not block the other
- **WHEN** the Rust integration tests fail
- **THEN** the TypeScript integration tests SHALL still run to completion (`fail-fast: false`)

### Requirement: Pinned dependency versions in CI
The integration workflow SHALL pin `typescript-language-server` and `typescript` to specific versions via environment variables. All GHA actions SHALL be SHA-pinned.

#### Scenario: TypeScript versions are pinned
- **WHEN** the TypeScript matrix slot installs npm packages
- **THEN** it SHALL use exact versions from env vars (no `latest`, no caret ranges)

#### Scenario: GHA actions are SHA-pinned
- **WHEN** the workflow uses external actions (e.g., `actions/checkout`)
- **THEN** the action reference SHALL be a commit SHA, not a tag
