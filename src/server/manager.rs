use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use super::idle_monitor::IdleMonitor;
use crate::lsp::client::{ClientError, LspClient, LspClientApi};
use crate::lsp::file_manager::FileManager;
use crate::stats::StatsRecorder;

/// Supported language identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    Rust,
    TypeScript,
}

/// Configuration for a single language server.
#[derive(Clone)]
pub struct LanguageServerConfig {
    pub language: Language,
    pub command: String,
    pub args: Vec<String>,
    pub extensions: Vec<&'static str>,
    pub language_id: &'static str,
    pub idle_timeout_secs: u64,
}

impl LanguageServerConfig {
    /// Return the correct LSP `languageId` for a specific file extension.
    ///
    /// The default `language_id` field works as a fallback, but TypeScript-family
    /// servers require per-extension IDs: `javascript` for `.js`, `javascriptreact`
    /// for `.jsx`, `typescriptreact` for `.tsx`.
    pub fn language_id_for_extension(&self, ext: &str) -> &str {
        match self.language {
            Language::TypeScript => match ext {
                "js" => "javascript",
                "jsx" => "javascriptreact",
                "tsx" => "typescriptreact",
                _ => self.language_id,
            },
            _ => self.language_id,
        }
    }
}

/// Default configs for Rust and TypeScript.
fn default_configs() -> HashMap<Language, LanguageServerConfig> {
    let mut map = HashMap::new();
    map.insert(
        Language::Rust,
        LanguageServerConfig {
            language: Language::Rust,
            command: "rust-analyzer".to_string(),
            args: vec![],
            extensions: vec!["rs"],
            language_id: "rust",
            idle_timeout_secs: 600,
        },
    );
    map.insert(
        Language::TypeScript,
        LanguageServerConfig {
            language: Language::TypeScript,
            command: "typescript-language-server".to_string(),
            args: vec!["--stdio".to_string()],
            extensions: vec!["ts", "tsx", "js", "jsx"],
            language_id: "typescript",
            idle_timeout_secs: 600,
        },
    );
    map
}

/// Error type for server manager operations.
#[derive(Debug)]
pub enum ManagerError {
    UnsupportedLanguage(String),
    StartupFailed(String),
    ClientError(ClientError),
}

impl std::fmt::Display for ManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedLanguage(ext) => {
                write!(f, "no language server configured for .{ext} files")
            }
            Self::StartupFailed(msg) => write!(f, "language server startup failed: {msg}"),
            Self::ClientError(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ManagerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ClientError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<ClientError> for ManagerError {
    fn from(e: ClientError) -> Self {
        Self::ClientError(e)
    }
}

/// State for a running language server: the client and its file manager.
pub(crate) struct ServerEntry {
    pub client: Box<dyn LspClientApi>,
    pub file_manager: FileManager,
}

/// Manages language server lifecycle: lazy startup, lookup, and shutdown.
///
/// The manager is shared between MCP tool handlers and the idle monitor
/// via `Arc<Mutex<...>>`. The tokio `Mutex` allows holding the lock
/// across `.await` points during server startup.
pub struct LanguageServerManager {
    configs: HashMap<Language, LanguageServerConfig>,
    servers: Mutex<HashMap<Language, ServerEntry>>,
    root: PathBuf,
    env: HashMap<String, String>,
    monitor: Arc<IdleMonitor>,
    stats: Arc<StatsRecorder>,
}

impl LanguageServerManager {
    #[expect(dead_code)]
    pub fn new(root: PathBuf, stats: Arc<StatsRecorder>) -> Self {
        Self::with_configs(root, default_configs(), HashMap::new(), stats)
    }

    pub fn with_configs(
        root: PathBuf,
        configs: HashMap<Language, LanguageServerConfig>,
        env: HashMap<String, String>,
        stats: Arc<StatsRecorder>,
    ) -> Self {
        Self {
            configs,
            servers: Mutex::new(HashMap::new()),
            root,
            env,
            monitor: Arc::new(IdleMonitor::new(stats.clone())),
            stats,
        }
    }

    pub fn monitor(&self) -> &Arc<IdleMonitor> {
        &self.monitor
    }

    pub fn start_idle_monitor(self: Arc<Self>) -> JoinHandle<()> {
        let monitor = self.monitor.clone();
        let manager = self.clone();
        tokio::spawn(async move { monitor.run(manager).await })
    }

    /// Resolve a file path to its language, based on the file extension.
    fn language_for_file(&self, path: &Path) -> Option<Language> {
        let ext = path.extension()?.to_str()?;
        self.configs
            .values()
            .find(|cfg| cfg.extensions.contains(&ext))
            .map(|cfg| cfg.language)
    }

    /// Get a reference to the config for a language.
    pub fn config_for(&self, language: Language) -> Option<&LanguageServerConfig> {
        self.configs.get(&language)
    }

    /// Get (or lazily start) the language server for a file, returning
    /// a guarded reference to the client and file manager.
    ///
    /// If multiple tool calls arrive concurrently for the same language,
    /// the server is started only once — subsequent callers wait for the
    /// startup to complete and then reuse the same entry.
    pub async fn get_client_for_file(
        &self,
        path: &Path,
    ) -> Result<
        (
            Language,
            tokio::sync::MutexGuard<'_, HashMap<Language, ServerEntry>>,
        ),
        ManagerError,
    > {
        let language = self.language_for_file(path).ok_or_else(|| {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("???");
            ManagerError::UnsupportedLanguage(ext.to_string())
        })?;

        let mut servers = self.servers.lock().await;

        if !servers.contains_key(&language) {
            self.start_server_internal(&mut servers, language).await?;
        }

        Ok((language, servers))
    }

    /// Start a language server and store it in the map.
    /// Must be called while holding the servers lock.
    async fn start_server_internal(
        &self,
        servers: &mut HashMap<Language, ServerEntry>,
        language: Language,
    ) -> Result<(), ManagerError> {
        let cfg = self
            .configs
            .get(&language)
            .expect("config must exist for language");

        let capabilities = match language {
            Language::Rust => crate::language::rust::client_capabilities(),
            Language::TypeScript => crate::language::typescript::client_capabilities(),
        };

        let start = std::time::Instant::now();
        let lang_str = format!("{language:?}");

        let client_result = LspClient::start(
            &cfg.command,
            &args_from(cfg),
            &self.root,
            capabilities,
            &self.env,
        )
        .await;

        match client_result {
            Ok(client) => {
                // Wait for the server to finish indexing (30s timeout).
                let timeout = std::time::Duration::from_secs(30);
                if let Err(e) = client.wait_for_index(timeout).await {
                    eprintln!("[manager] index wait warning: {e}");
                }

                let duration_ms = start.elapsed().as_millis() as u64;
                servers.insert(
                    language,
                    ServerEntry {
                        client: Box::new(client),
                        file_manager: FileManager::new(),
                    },
                );

                if let Err(e) = self
                    .stats
                    .record_server_event(&lang_str, "started", Some(duration_ms), None)
                    .await
                {
                    eprintln!("stats recording failed: {e}");
                }

                Ok(())
            }
            Err(e) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                let err_msg = e.to_string();
                if let Err(se) = self
                    .stats
                    .record_server_event(
                        &lang_str,
                        "startup_failed",
                        Some(duration_ms),
                        Some(&err_msg),
                    )
                    .await
                {
                    eprintln!("stats recording failed: {se}");
                }
                Err(ManagerError::StartupFailed(err_msg))
            }
        }
    }

    /// Stop a specific language server.
    pub async fn stop_server(&self, language: Language) -> Result<(), ManagerError> {
        let mut servers = self.servers.lock().await;
        if let Some(entry) = servers.remove(&language) {
            let mut client = entry.client;
            let mut fm = entry.file_manager;
            fm.close_all(&mut *client).await;
            client.shutdown().await.map_err(ManagerError::from)?;

            let lang_str = format!("{language:?}");
            if let Err(e) = self
                .stats
                .record_server_event(&lang_str, "stopped", None, Some("manual"))
                .await
            {
                eprintln!("stats recording failed: {e}");
            }
        }
        Ok(())
    }

    /// Shut down all running language servers.
    pub async fn shutdown_all(&self) {
        let mut servers = self.servers.lock().await;
        let languages: Vec<Language> = servers.keys().copied().collect();
        for language in languages {
            if let Some(entry) = servers.remove(&language) {
                let mut client = entry.client;
                let mut fm = entry.file_manager;
                fm.close_all(&mut *client).await;
                let _ = client.shutdown().await;

                let lang_str = format!("{language:?}");
                if let Err(e) = self
                    .stats
                    .record_server_event(&lang_str, "stopped", None, Some("session_end"))
                    .await
                {
                    eprintln!("stats recording failed: {e}");
                }
            }
        }
    }
}

fn args_from(cfg: &LanguageServerConfig) -> Vec<&str> {
    cfg.args.iter().map(|s| s.as_str()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_stats() -> (tempfile::TempDir, Arc<StatsRecorder>) {
        let dir = tempfile::tempdir().unwrap();
        let recorder = StatsRecorder::new(dir.path()).await.unwrap();
        (dir, Arc::new(recorder))
    }

    #[test]
    fn typescript_default_config() {
        let configs = default_configs();
        let ts = configs
            .get(&Language::TypeScript)
            .expect("TypeScript config must exist");
        assert_eq!(ts.command, "typescript-language-server");
        assert_eq!(ts.args, vec!["--stdio"]);
        assert_eq!(ts.extensions, vec!["ts", "tsx", "js", "jsx"]);
        assert_eq!(ts.language_id, "typescript");
        assert_eq!(ts.idle_timeout_secs, 600);
    }

    #[tokio::test]
    async fn ts_file_resolves_to_typescript() {
        let configs = default_configs();
        let manager = LanguageServerManager::with_configs(
            PathBuf::from("/tmp/test"),
            configs,
            HashMap::new(),
            test_stats().await.1,
        );
        assert_eq!(
            manager.language_for_file(Path::new("app.ts")),
            Some(Language::TypeScript)
        );
        assert_eq!(
            manager.language_for_file(Path::new("App.tsx")),
            Some(Language::TypeScript)
        );
        assert_eq!(
            manager.language_for_file(Path::new("utils.js")),
            Some(Language::TypeScript)
        );
        assert_eq!(
            manager.language_for_file(Path::new("Component.jsx")),
            Some(Language::TypeScript)
        );
    }

    // --- Handler query dispatch tests (Group 8) ---

    use crate::lsp::client::{ClientError, MockLspClientApi};
    use crate::lsp::types::{Diagnostic, Location, Position, Range, TextEdit, WorkspaceEdit};

    fn test_pos() -> Position {
        Position {
            line: 0,
            character: 0,
        }
    }

    fn test_range() -> Range {
        Range {
            start: test_pos(),
            end: Position {
                line: 0,
                character: 5,
            },
        }
    }

    fn test_location(uri: &str) -> Location {
        Location {
            uri: uri.to_string(),
            range: test_range(),
        }
    }

    fn write_temp_file(dir: &tempfile::TempDir, name: &str, content: &str) -> std::path::PathBuf {
        let path = dir.path().join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    fn make_entry(mock: MockLspClientApi) -> ServerEntry {
        ServerEntry {
            client: Box::new(mock),
            file_manager: FileManager::new(),
        }
    }

    async fn open_file(entry: &mut ServerEntry, path: &Path, language_id: &str) -> String {
        entry
            .file_manager
            .ensure_open(&mut *entry.client, path, language_id)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn find_references_returns_locations() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_temp_file(&dir, "test.rs", "fn foo() {}");
        let mut mock = MockLspClientApi::new();
        mock.expect_did_open().returning(|_, _, _, _| Ok(()));
        mock.expect_references()
            .returning(|_, _| Ok(vec![test_location("file:///test.rs")]));

        let mut entry = make_entry(mock);
        let uri = open_file(&mut entry, &path, "rust").await;

        let results = entry.client.references(&uri, test_pos()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].uri, "file:///test.rs");
        assert_eq!(results[0].range.start.line, 0);
    }

    #[tokio::test]
    async fn find_references_empty_returns_no_results() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_temp_file(&dir, "test.rs", "fn foo() {}");
        let mut mock = MockLspClientApi::new();
        mock.expect_did_open().returning(|_, _, _, _| Ok(()));
        mock.expect_references().returning(|_, _| Ok(Vec::new()));

        let mut entry = make_entry(mock);
        let uri = open_file(&mut entry, &path, "rust").await;

        let results = entry.client.references(&uri, test_pos()).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn goto_definition_returns_locations_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_temp_file(&dir, "test.rs", "fn foo() {}");
        let mut mock = MockLspClientApi::new();
        mock.expect_did_open().returning(|_, _, _, _| Ok(()));
        mock.expect_goto_definition()
            .returning(|_, _| Ok(vec![test_location("file:///lib.rs")]));

        let mut entry = make_entry(mock);
        let uri = open_file(&mut entry, &path, "rust").await;

        let results = entry
            .client
            .goto_definition(&uri, test_pos())
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].uri, "file:///lib.rs");
    }

    #[tokio::test]
    async fn hover_returns_formatted_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_temp_file(&dir, "test.rs", "fn foo() {}");
        let hover = crate::lsp::types::Hover {
            contents: serde_json::json!("pub fn foo() -> i32"),
            range: Some(test_range()),
        };
        let mut mock = MockLspClientApi::new();
        mock.expect_did_open().returning(|_, _, _, _| Ok(()));
        mock.expect_hover()
            .returning(move |_, _| Ok(Some(hover.clone())));

        let mut entry = make_entry(mock);
        let uri = open_file(&mut entry, &path, "rust").await;

        let result = entry.client.hover(&uri, test_pos()).await.unwrap();
        assert!(result.is_some());
        let h = result.unwrap();
        assert!(h.contents.is_string());
    }

    #[tokio::test]
    async fn diagnostics_returns_severity_names() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_temp_file(&dir, "test.rs", "fn foo() {}");
        let diag = Diagnostic {
            range: test_range(),
            severity: Some(1), // Error
            code: None,
            source: None,
            message: "expected `;`".into(),
        };
        let mut mock = MockLspClientApi::new();
        mock.expect_did_open().returning(|_, _, _, _| Ok(()));
        mock.expect_diagnostic()
            .returning(move |_| Ok(vec![diag.clone()]));

        let mut entry = make_entry(mock);
        let uri = open_file(&mut entry, &path, "rust").await;

        let results = entry.client.diagnostic(&uri).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].severity, Some(1));
        assert_eq!(results[0].message, "expected `;`");
    }

    #[tokio::test]
    async fn rename_returns_workspace_edits() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_temp_file(&dir, "test.rs", "fn foo() {}");
        let edit = WorkspaceEdit {
            changes: Some({
                let mut map = std::collections::HashMap::new();
                map.insert(
                    "file:///test.rs".to_string(),
                    vec![TextEdit {
                        range: test_range(),
                        new_text: "bar".into(),
                    }],
                );
                map
            }),
            document_changes: None,
        };
        let mut mock = MockLspClientApi::new();
        mock.expect_did_open().returning(|_, _, _, _| Ok(()));
        mock.expect_rename()
            .returning(move |_, _, _| Ok(Some(edit.clone())));

        let mut entry = make_entry(mock);
        let uri = open_file(&mut entry, &path, "rust").await;

        let result = entry.client.rename(&uri, test_pos(), "bar").await.unwrap();
        assert!(result.is_some());
        let ws = result.unwrap();
        let changes = ws.changes.unwrap();
        assert!(changes.contains_key("file:///test.rs"));
    }

    #[tokio::test]
    async fn client_error_from_mock_returns_transport_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_temp_file(&dir, "test.rs", "fn foo() {}");
        let mut mock = MockLspClientApi::new();
        mock.expect_did_open().returning(|_, _, _, _| Ok(()));
        mock.expect_goto_definition()
            .returning(|_, _| Err(ClientError::Transport("connection lost".into())));

        let mut entry = make_entry(mock);
        let uri = open_file(&mut entry, &path, "rust").await;

        let result = entry.client.goto_definition(&uri, test_pos()).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ClientError::Transport(msg) if msg == "connection lost"));
    }
}
