# symtrace-mcp Development Notes

## Configuration

The server reads `.symtrace.toml` from CWD at startup. If absent, runs in single-project mode (CWD as root, default rust-analyzer config). Config format:

```toml
[server.rust]
command = "rust-analyzer"

[[projects]]
root = "project-a"

[[projects]]
root = "project-b"
```

## PR Review Workflow

Use `gh pr-review review view --repo tatsuya6502/symtrace-mcp --pr <PR_NUMBER> --unresolved` to view unresolved review comments.

Requires: `gh extension install agynio/gh-pr-review`
