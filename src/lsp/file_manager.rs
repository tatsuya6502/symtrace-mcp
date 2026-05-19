use std::collections::HashMap;
use std::path::Path;
use std::time::SystemTime;

use super::client::{ClientError, LspClientApi};
use crate::uri::path_to_uri;

/// Metadata for a file currently tracked as open in the language server.
struct OpenFile {
    version: i32,
    modified_at: SystemTime,
}

/// Manages `textDocument/didOpen` / `didChange` / `didClose` lifecycle
/// for files that need to be visible to the language server.
///
/// Tracks files by URI. Compares file mtime on disk to detect changes
/// and avoids redundant notifications when the file hasn't been modified.
pub struct FileManager {
    open_files: HashMap<String, OpenFile>,
}

/// Error type for file manager operations.
#[derive(Debug)]
pub enum FileError {
    Io(String),
    Client(ClientError),
}

impl std::fmt::Display for FileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(msg) => write!(f, "file error: {msg}"),
            Self::Client(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for FileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Client(e) => Some(e),
            _ => None,
        }
    }
}

impl From<ClientError> for FileError {
    fn from(e: ClientError) -> Self {
        Self::Client(e)
    }
}

impl FileManager {
    pub fn new() -> Self {
        Self {
            open_files: HashMap::new(),
        }
    }

    /// Ensure a file is open and up-to-date in the language server.
    ///
    /// - If the file is not tracked, reads it from disk and sends `didOpen`.
    /// - If the file is tracked but modified on disk (mtime changed), reads
    ///   the new content and sends `didChange` with an incremented version.
    /// - If the file is tracked and unchanged, does nothing.
    pub async fn ensure_open(
        &mut self,
        client: &mut dyn LspClientApi,
        path: &Path,
        language_id: &str,
    ) -> Result<String, FileError> {
        let uri = path_to_uri(path);
        let metadata = path
            .metadata()
            .map_err(|e| FileError::Io(format!("failed to stat {}: {e}", path.display())))?;
        let mtime = metadata.modified().map_err(|e| {
            FileError::Io(format!("failed to get mtime for {}: {e}", path.display()))
        })?;

        if let Some(open) = self.open_files.get(&uri) {
            if open.modified_at == mtime {
                return Ok(uri);
            }
            // File changed on disk — send didChange.
            let text = read_file(path)?;
            let new_version = open.version + 1;
            client.did_change(&uri, &text, new_version).await?;

            self.open_files.insert(
                uri.clone(),
                OpenFile {
                    version: new_version,
                    modified_at: mtime,
                },
            );
            return Ok(uri);
        }

        // File not open — send didOpen.
        let text = read_file(path)?;
        let version = 1;
        client.did_open(&uri, &text, version, language_id).await?;

        self.open_files.insert(
            uri.clone(),
            OpenFile {
                version,
                modified_at: mtime,
            },
        );

        Ok(uri)
    }

    /// Close a file in the language server and remove it from tracking.
    #[allow(dead_code)]
    pub async fn close(
        &mut self,
        client: &mut dyn LspClientApi,
        uri: &str,
    ) -> Result<(), FileError> {
        if self.open_files.remove(uri).is_some() {
            client.did_close(uri).await?;
        }
        Ok(())
    }

    /// Close all tracked files. Called during server shutdown.
    pub async fn close_all(&mut self, client: &mut dyn LspClientApi) {
        let uris: Vec<String> = self.open_files.keys().cloned().collect();
        for uri in uris {
            let _ = client.did_close(&uri).await;
            self.open_files.remove(&uri);
        }
    }
}

fn read_file(path: &Path) -> Result<String, FileError> {
    std::fs::read_to_string(path)
        .map_err(|e| FileError::Io(format!("failed to read {}: {e}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lsp::client::MockLspClientApi;

    fn write_temp_file(dir: &tempfile::TempDir, name: &str, content: &str) -> std::path::PathBuf {
        let path = dir.path().join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    #[tokio::test]
    async fn ensure_open_sends_did_open_for_new_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_temp_file(&dir, "test.rs", "fn main() {}");
        let mut fm = FileManager::new();
        let mut mock = MockLspClientApi::new();

        // did_open should be called with the file content and language_id.
        mock.expect_did_open()
            .withf(|uri, text, version, language_id| {
                uri.contains("test.rs")
                    && text == "fn main() {}"
                    && *version == 1
                    && language_id == "rust"
            })
            .returning(|_, _, _, _| Ok(()));

        let uri = fm.ensure_open(&mut mock, &path, "rust").await.unwrap();
        assert!(uri.contains("test.rs"));
    }

    #[tokio::test]
    async fn ensure_open_sends_did_change_when_mtime_changed() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_temp_file(&dir, "test.rs", "fn main() {}");
        let mut fm = FileManager::new();
        let mut mock = MockLspClientApi::new();

        // First open.
        mock.expect_did_open().returning(|_, _, _, _| Ok(()));
        fm.ensure_open(&mut mock, &path, "rust").await.unwrap();

        // Modify the file and advance mtime deterministically.
        std::fs::write(&path, "fn main() { println!(); }").unwrap();
        filetime::set_file_mtime(&path, filetime::FileTime::from_unix_time(2_000_000_000, 0))
            .unwrap();

        // Re-open should detect mtime change and send didChange.
        mock.expect_did_change()
            .withf(|uri, text, version| {
                uri.contains("test.rs") && text == "fn main() { println!(); }" && *version == 2
            })
            .returning(|_, _, _| Ok(()));

        fm.ensure_open(&mut mock, &path, "rust").await.unwrap();
    }

    #[tokio::test]
    async fn ensure_open_is_noop_when_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_temp_file(&dir, "test.rs", "fn main() {}");
        let mut fm = FileManager::new();
        let mut mock = MockLspClientApi::new();

        // First open.
        mock.expect_did_open().returning(|_, _, _, _| Ok(()));
        fm.ensure_open(&mut mock, &path, "rust").await.unwrap();

        // Re-open without changes — no didChange expected.
        // (mock has no expectations, so any unexpected call would fail)
        fm.ensure_open(&mut mock, &path, "rust").await.unwrap();
    }

    #[tokio::test]
    async fn close_sends_did_close_and_removes_tracking() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_temp_file(&dir, "test.rs", "fn main() {}");
        let mut fm = FileManager::new();
        let mut mock = MockLspClientApi::new();

        // Open first.
        mock.expect_did_open().returning(|_, _, _, _| Ok(()));
        let uri = fm.ensure_open(&mut mock, &path, "rust").await.unwrap();

        // Close.
        mock.expect_did_close()
            .withf(|u| u.contains("test.rs"))
            .returning(|_| Ok(()));
        fm.close(&mut mock, &uri).await.unwrap();

        // Re-opening should trigger did_open again (file was closed).
        mock.expect_did_open().returning(|_, _, _, _| Ok(()));
        fm.ensure_open(&mut mock, &path, "rust").await.unwrap();
    }
}
