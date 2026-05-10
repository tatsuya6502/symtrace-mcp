use std::path::{Path, PathBuf};

use url::Url;

/// Convert a filesystem path to an RFC-compliant `file://` URI.
pub fn path_to_uri(path: &Path) -> String {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    Url::from_file_path(&canonical)
        .map(|u| u.to_string())
        .unwrap_or_else(|_| format!("file://{}", canonical.display()))
}

/// Convert a `file://` URI to a filesystem path.
pub fn uri_to_path(uri: &str) -> PathBuf {
    Url::parse(uri)
        .ok()
        .and_then(|u| u.to_file_path().ok())
        .unwrap_or_else(|| uri.strip_prefix("file://").unwrap_or(uri).into())
}
