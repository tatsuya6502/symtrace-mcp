//! Preflight check for `setup`: the hook and MCP entries name the bare
//! command `symtrace-mcp`, which Claude Code resolves through `PATH`. If
//! that resolves to nothing, or to a different (typically older) binary
//! than the one running `setup`, every hooked Bash/Grep call would fail or
//! be blocked, so warn before installing.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

const BINARY: &str = "symtrace-mcp";

/// First executable named `symtrace-mcp` on a `PATH`-style list.
pub fn find_on_path(path_var: &OsStr) -> Option<PathBuf> {
    std::env::split_paths(path_var)
        .map(|dir| dir.join(BINARY))
        .find(|p| p.is_file())
}

/// Warning text for the given lookup result, or `None` when the binary on
/// `PATH` is the one running `setup`.
pub fn warning_for(on_path: Option<&Path>, current_exe: Option<&Path>) -> Option<String> {
    let found = match on_path {
        None => {
            return Some(format!(
                "`{BINARY}` is not on PATH; Claude Code will not be able to run the hook \
                 or the MCP server. Install it first (e.g. `cargo install --path .`)."
            ));
        }
        Some(p) => p,
    };
    let same = match current_exe {
        Some(exe) => canonical(exe) == canonical(found),
        None => true,
    };
    if same {
        None
    } else {
        Some(format!(
            "`{BINARY}` on PATH is {} but setup is running from {}; Claude Code will run \
             the PATH binary, which may be an older version without the `hook` subcommand. \
             Reinstall it (e.g. `cargo install --path .`) before relying on the hook.",
            found.display(),
            current_exe
                .map(Path::display)
                .map_or(String::new(), |d| d.to_string()),
        ))
    }
}

/// Does an entry's `command` run this binary? True for the bare name (what
/// `setup` writes; `warning` covers where `PATH` resolves it) and for a path
/// that canonicalizes to the running executable.
pub fn is_running_binary(command: &str) -> bool {
    command == BINARY
        || std::env::current_exe().is_ok_and(|exe| canonical(&exe) == canonical(Path::new(command)))
}

fn canonical(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
}

/// Warning for the current process environment, if any.
pub fn warning() -> Option<String> {
    let on_path = std::env::var_os("PATH").and_then(|p| find_on_path(&p));
    let exe = std::env::current_exe().ok();
    warning_for(on_path.as_deref(), exe.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn find_on_path_picks_first_match() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        fs::write(b.join(BINARY), "").unwrap();
        let path_var = std::env::join_paths([&a, &b]).unwrap();
        assert_eq!(find_on_path(&path_var), Some(b.join(BINARY)));
        let empty = std::env::join_paths([&a]).unwrap();
        assert_eq!(find_on_path(&empty), None);
    }

    #[test]
    fn warning_cases() {
        let dir = tempfile::tempdir().unwrap();
        let installed = dir.path().join(BINARY);
        fs::write(&installed, "").unwrap();
        let other = dir.path().join("target").join(BINARY);

        assert!(
            warning_for(None, Some(&other))
                .unwrap()
                .contains("not on PATH")
        );
        assert_eq!(warning_for(Some(&installed), Some(&installed)), None);
        assert_eq!(warning_for(Some(&installed), None), None);
        let w = warning_for(Some(&installed), Some(&other)).unwrap();
        assert!(w.contains("older version"), "{w}");
    }
}
