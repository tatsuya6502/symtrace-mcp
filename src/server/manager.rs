use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use super::idle_monitor::IdleMonitor;
use crate::lsp::client::{ClientError, LspClient};
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
    pub client: LspClient,
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
    monitor: Arc<IdleMonitor>,
    stats: Arc<StatsRecorder>,
}

impl LanguageServerManager {
    #[expect(dead_code)]
    pub fn new(root: PathBuf, stats: Arc<StatsRecorder>) -> Self {
        Self::with_configs(root, default_configs(), stats)
    }

    pub fn with_configs(
        root: PathBuf,
        configs: HashMap<Language, LanguageServerConfig>,
        stats: Arc<StatsRecorder>,
    ) -> Self {
        Self {
            configs,
            servers: Mutex::new(HashMap::new()),
            root,
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

        let client_result =
            LspClient::start(&cfg.command, &args_from(cfg), &self.root, capabilities).await;

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
                        client,
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
            fm.close_all(&mut client).await;
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
                fm.close_all(&mut client).await;
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
}
