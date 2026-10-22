//! Atomic file replacement shared by the JSON and markdown items: the bytes
//! go to a temp file in the target's directory, which is then renamed over
//! the target, so a crash mid-write never leaves a truncated CLAUDE.md or
//! settings.json behind.

use std::fs;
use std::io::Write;
use std::path::Path;

use super::SetupError;

/// Write `bytes` to `path` atomically. Parent directories are created. A
/// symlinked target is followed so the rename replaces the real file, not
/// the link.
///
/// Permissions: an existing target keeps its exact mode (copied onto the
/// temp file before any byte is written). A new file is created with
/// `default_mode` (masked by the umask, like any other new file), so a
/// `0o600` default never exposes the contents through a wider umask.
pub fn write_atomic(path: &Path, bytes: &[u8], default_mode: u32) -> Result<(), SetupError> {
    let resolved = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let path = resolved.as_path();
    let io = |e: std::io::Error| SetupError::Io(path.to_path_buf(), e);
    if let Some(dir) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(dir).map_err(io)?;
    }
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".into());
    let tmp = path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()));
    let existing = fs::metadata(path).ok().map(|m| m.permissions());

    let result = (|| {
        // A stale temp file from a crashed run (same PID reused) must not
        // make `create_new` fail.
        let _ = fs::remove_file(&tmp);
        let mut f = create_temp(&tmp, existing.as_ref(), default_mode)?;
        if let Some(perm) = &existing {
            // Exact copy: the open(2) mode above was masked by the umask.
            f.set_permissions(perm.clone())?;
        }
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, path)
    })();
    if let Err(e) = result {
        let _ = fs::remove_file(&tmp);
        return Err(io(e));
    }
    Ok(())
}

#[cfg(unix)]
fn create_temp(
    tmp: &Path,
    existing: Option<&fs::Permissions>,
    default_mode: u32,
) -> std::io::Result<fs::File> {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let mode = existing.map_or(default_mode, |p| p.mode() & 0o7777);
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(tmp)
}

#[cfg(not(unix))]
fn create_temp(
    tmp: &Path,
    _existing: Option<&fs::Permissions>,
    _default_mode: u32,
) -> std::io::Result<fs::File> {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(tmp)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(dir: &Path) -> Vec<std::ffi::OsString> {
        let mut v: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        v.sort();
        v
    }

    #[test]
    fn writes_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("CLAUDE.md");
        write_atomic(&path, b"# hi\n", 0o644).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "# hi\n");
        write_atomic(&path, b"# again\n", 0o644).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "# again\n");
        assert_eq!(names(path.parent().unwrap()), ["CLAUDE.md"]);
    }

    #[cfg(unix)]
    #[test]
    fn new_file_gets_default_mode() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let json = dir.path().join(".claude.json");
        write_atomic(&json, b"{}\n", 0o600).unwrap();
        assert_eq!(
            fs::metadata(&json).unwrap().permissions().mode() & 0o777,
            0o600
        );

        // 0o644 may be narrowed by the umask; owner rw, no exec, is fixed.
        let md = dir.path().join("CLAUDE.md");
        write_atomic(&md, b"x\n", 0o644).unwrap();
        let mode = fs::metadata(&md).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode & 0o600, 0o600, "{mode:o}");
        assert_eq!(mode & 0o133, 0, "{mode:o}");
    }

    #[cfg(unix)]
    #[test]
    fn existing_file_keeps_exact_mode() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        for mode in [0o600, 0o644, 0o664] {
            let path = dir.path().join(format!("f{mode:o}"));
            fs::write(&path, "old").unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
            write_atomic(&path, b"new", 0o600).unwrap();
            assert_eq!(fs::read_to_string(&path).unwrap(), "new");
            let got = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(got, mode, "{mode:o}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn stale_temp_file_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.json");
        let tmp = dir
            .path()
            .join(format!(".s.json.{}.tmp", std::process::id()));
        fs::write(&tmp, "stale").unwrap();
        write_atomic(&path, b"{}", 0o600).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "{}");
        assert_eq!(names(dir.path()), ["s.json"]);
    }
}
