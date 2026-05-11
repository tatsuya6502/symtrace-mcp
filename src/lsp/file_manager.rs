use std::collections::HashMap;
use std::path::Path;
use std::time::SystemTime;

use super::client::{ClientError, LspClient};
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
        client: &mut LspClient,
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
    pub async fn close(&mut self, client: &mut LspClient, uri: &str) -> Result<(), FileError> {
        if self.open_files.remove(uri).is_some() {
            client.did_close(uri).await?;
        }
        Ok(())
    }

    /// Close all tracked files. Called during server shutdown.
    pub async fn close_all(&mut self, client: &mut LspClient) {
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
