//! Path resolution for the four integration items in global (`$HOME`) and
//! local (project) scope.

use std::path::{Path, PathBuf};

/// The four files `setup` touches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Targets {
    /// CLAUDE.md that receives the prompt block.
    pub claude_md: PathBuf,
    /// Explore subagent file that receives the same block.
    pub explore_md: PathBuf,
    /// settings.json that holds the PreToolUse hook entries.
    pub settings_json: PathBuf,
    /// File that holds `mcpServers.symtrace-mcp`.
    pub mcp_json: PathBuf,
}

impl Targets {
    /// Per-user files under `home` (`$HOME`).
    pub fn global(home: &Path) -> Self {
        let claude_dir = home.join(".claude");
        Targets {
            claude_md: claude_dir.join("CLAUDE.md"),
            explore_md: claude_dir.join("agents").join("Explore.md"),
            settings_json: claude_dir.join("settings.json"),
            mcp_json: home.join(".claude.json"),
        }
    }

    /// Project files under `cwd` (`--local`).
    pub fn local(cwd: &Path) -> Self {
        let claude_dir = cwd.join(".claude");
        Targets {
            claude_md: cwd.join("CLAUDE.md"),
            explore_md: claude_dir.join("agents").join("Explore.md"),
            settings_json: claude_dir.join("settings.json"),
            mcp_json: cwd.join(".mcp.json"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_paths() {
        let t = Targets::global(Path::new("/home/u"));
        assert_eq!(t.claude_md, PathBuf::from("/home/u/.claude/CLAUDE.md"));
        assert_eq!(
            t.explore_md,
            PathBuf::from("/home/u/.claude/agents/Explore.md")
        );
        assert_eq!(
            t.settings_json,
            PathBuf::from("/home/u/.claude/settings.json")
        );
        assert_eq!(t.mcp_json, PathBuf::from("/home/u/.claude.json"));
    }

    #[test]
    fn local_paths() {
        let t = Targets::local(Path::new("/work/proj"));
        assert_eq!(t.claude_md, PathBuf::from("/work/proj/CLAUDE.md"));
        assert_eq!(
            t.explore_md,
            PathBuf::from("/work/proj/.claude/agents/Explore.md")
        );
        assert_eq!(
            t.settings_json,
            PathBuf::from("/work/proj/.claude/settings.json")
        );
        assert_eq!(t.mcp_json, PathBuf::from("/work/proj/.mcp.json"));
    }
}
