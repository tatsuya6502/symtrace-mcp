//! `symtrace-mcp setup`: install, inspect or remove the Claude Code integration.
//!
//! See `openspec/changes/setup-command/` for the specification. The four
//! items are: the prompt block in CLAUDE.md, the same block in the Explore
//! subagent file, PreToolUse hook entries in settings.json, and the MCP
//! server entry.

use std::path::Path;

use clap::Args;

#[derive(Args, Debug, Clone)]
pub struct SetupArgs {
    /// Target the project files in the current directory
    /// (./CLAUDE.md, ./.claude/agents/Explore.md, ./.claude/settings.json,
    /// ./.mcp.json) instead of the per-user files under $HOME
    #[arg(long)]
    pub local: bool,

    /// Report what would change without writing any file
    #[arg(long)]
    pub dry_run: bool,

    /// Replace a prompt block or MCP entry that was edited by the user
    #[arg(long)]
    pub force: bool,

    /// Do not touch CLAUDE.md
    #[arg(long)]
    pub skip_prompt: bool,

    /// Do not touch the Explore subagent file
    #[arg(long)]
    pub skip_explore: bool,

    /// Do not touch the hook entries in settings.json
    #[arg(long)]
    pub skip_hook: bool,

    /// Do not touch the MCP server entry
    #[arg(long)]
    pub skip_mcp: bool,

    /// Remove everything `setup` installed (only symtrace-mcp content)
    #[arg(long, conflicts_with = "status")]
    pub uninstall: bool,

    /// Report the installation state of each item without writing
    #[arg(long, conflicts_with = "uninstall")]
    pub status: bool,
}

/// Run the setup command. Returns the process exit code: 0 when no item
/// failed, non-zero when an item errored or needs `--force`.
pub fn run(args: SetupArgs, cwd: &Path) -> i32 {
    let _ = (args, cwd);
    eprintln!("symtrace-mcp setup: not implemented yet");
    2
}
