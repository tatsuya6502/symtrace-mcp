//! `symtrace-mcp hook`: Claude Code PreToolUse hook.
//!
//! Reads one event from stdin and, when the call is a text/tree-sitter
//! heuristic for a semantic question (`sb callers`, identifier grep over
//! source code, `Grep` tool with a bare identifier), prints a JSON response
//! that lets the call proceed but attaches a reminder to use the
//! symtrace-mcp tools. See `openspec/changes/setup-command/specs/tool-reminder-hook/`.

/// Run the hook. Always returns 0: a hook failure must never block a tool call.
pub fn run() -> i32 {
    0
}
