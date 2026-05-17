# symtrace-mcp Development Notes

## Code Conventions

- Prefer `src/<module>.rs` + `src/<module>/sub.rs` over `src/<module>/mod.rs`. This project follows the Rust 2018+ module style.

## Test & Lint

```bash
cargo clippy --all-targets --tests -- -D warnings
cargo fmt --all -- --check
cargo test
```

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
```

## OpenSpec

Feature development follows an OpenSpec workflow. Specs live in `openspec/specs/` and changes in `openspec/changes/`. Each change has a `change.md` (scope + task list) and per-component spec deltas. Completed changes are archived to `openspec/changes/archive/`.

## PR Review Workflow

Use `gh pr-review review view --repo tatsuya6502/symtrace-mcp --pr <PR_NUMBER> --unresolved` to view unresolved review comments.

Requires: `gh extension install agynio/gh-pr-review`
