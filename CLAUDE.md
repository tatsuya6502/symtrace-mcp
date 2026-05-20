# symtrace-mcp Development Notes

## Code Conventions

- Prefer `src/<module>.rs` + `src/<module>/sub.rs` over `src/<module>/mod.rs`. This project follows the Rust 2018+ module style.

## Test & Lint

```bash
cargo clippy --all-targets --tests -- -D warnings
cargo fmt --all -- --check
cargo test
```

### Debugging test failures with rtk

`rtk` (Rust Token Killer) filters output to save tokens, which can hide useful diagnostics like compiler warnings or stderr from subprocesses (e.g., rust-analyzer shutdown messages). Use `rtk proxy` to see the full output:

```bash
rtk proxy cargo test -- --nocapture
rtk proxy cargo test --features integration-rust -- --test-threads=1 --nocapture
```

## Integration Tests

Integration tests spawn `symtrace-mcp` + a real LSP server, so they are slow (~60s for Rust, ~250s for TypeScript). Run them only as the final step of an OpenSpec change, not after every edit.

```bash
# Per-language (install the corresponding LSP server first)
cargo test --features integration-rust -- --test-threads=1
cargo test --features integration-typescript -- --test-threads=1

# Both (requires both rust-analyzer and typescript-language-server)
cargo test --features integration -- --test-threads=1
```

CI runs integration tests separately in `.github/workflows/integration.yml`.

## CLI

```bash
symtrace-mcp              # Run as MCP server (stdio)
symtrace-mcp stats        # Show usage stats for last 7 days
```

## Configuration

The server reads `.symtrace.toml` from CWD at startup. If absent, runs in single-project mode (CWD as root, default Rust and TypeScript server configs). Config format:

```toml
[server.rust]
command = "rust-analyzer"

[server.typescript]
command = "typescript-language-server"

[[projects]]
root = "project-a"

[[projects]]
root = "project-b"
env = { DATABASE_URL = "postgres://localhost/mydb" }
```

## OpenSpec

Feature development follows an OpenSpec workflow. Specs live in `openspec/specs/` and changes in `openspec/changes/`. Each change has a `change.md` (scope + task list) and per-component spec deltas. Completed changes are archived to `openspec/changes/archive/`.

## PR Review Workflow

Use `gh pr-review review view --repo tatsuya6502/symtrace-mcp --pr <PR_NUMBER> --unresolved` to view unresolved review comments.

Requires: `gh extension install agynio/gh-pr-review`
