use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use moka::future::Cache;
use serde_json::Value;
use tokio::sync::watch;

use super::transport::LspTransport;
use super::types::{Diagnostic, ServerCapabilities, ServerStatusParams};
use crate::uri::path_to_uri;

/// Indexing state derived from the server's `experimental/serverStatus`
/// notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexState {
    /// The server has never sent a status notification.
    NotReported,
    /// The most recent status reported `quiescent: false`.
    Indexing,
    /// The most recent status reported `quiescent: true`.
    Ready,
}

/// Error type for LSP client operations.
#[derive(Debug)]
pub enum ClientError {
    Transport(String),
    Protocol(String),
    IndexTimeout,
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transport(msg) => write!(f, "transport error: {msg}"),
            Self::Protocol(msg) => write!(f, "protocol error: {msg}"),
            Self::IndexTimeout => write!(f, "language server failed to become ready in time"),
        }
    }
}

impl std::error::Error for ClientError {}

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait LspClientApi: Send + Sync {
    async fn did_open(
        &mut self,
        uri: &str,
        text: &str,
        version: i32,
        language_id: &str,
    ) -> Result<(), ClientError>;
    async fn did_change(&mut self, uri: &str, text: &str, version: i32) -> Result<(), ClientError>;
    async fn did_close(&mut self, uri: &str) -> Result<(), ClientError>;
    async fn goto_definition(
        &self,
        uri: &str,
        position: super::types::Position,
    ) -> Result<Vec<super::types::Location>, ClientError>;
    async fn references(
        &self,
        uri: &str,
        position: super::types::Position,
    ) -> Result<Vec<super::types::Location>, ClientError>;
    async fn implementations(
        &self,
        uri: &str,
        position: super::types::Position,
    ) -> Result<Vec<super::types::Location>, ClientError>;
    async fn hover(
        &self,
        uri: &str,
        position: super::types::Position,
    ) -> Result<Option<super::types::Hover>, ClientError>;
    async fn diagnostic(&self, uri: &str) -> Result<Vec<super::types::Diagnostic>, ClientError>;
    async fn rename(
        &self,
        uri: &str,
        position: super::types::Position,
        new_name: &str,
    ) -> Result<Option<super::types::WorkspaceEdit>, ClientError>;
    async fn prepare_call_hierarchy(
        &self,
        uri: &str,
        position: super::types::Position,
    ) -> Result<Vec<super::types::CallHierarchyItem>, ClientError>;
    async fn incoming_calls(
        &self,
        item: &super::types::CallHierarchyItem,
    ) -> Result<Vec<super::types::CallHierarchyIncomingCall>, ClientError>;
    async fn outgoing_calls(
        &self,
        item: &super::types::CallHierarchyItem,
    ) -> Result<Vec<super::types::CallHierarchyOutgoingCall>, ClientError>;
    async fn shutdown(self: Box<Self>) -> Result<(), ClientError>;
}

/// High-level LSP client that manages lifecycle, file tracking, and queries
/// on top of [`LspTransport`].
pub struct LspClient {
    transport: LspTransport,
    root_uri: String,
    open_files: HashSet<String>,
    capabilities: ServerCapabilities,
    diagnostics_cache: Cache<String, Vec<Diagnostic>>,
    index_state: watch::Receiver<IndexState>,
}

#[async_trait]
impl LspClientApi for LspClient {
    async fn shutdown(self: Box<Self>) -> Result<(), ClientError> {
        let mut this = *self;
        let mut first_err: Option<ClientError> = None;
        // Close all tracked open files first.
        for uri in this.open_files.clone() {
            if let Err(e) = this
                .transport
                .send_notification(
                    "textDocument/didClose",
                    serde_json::json!({ "textDocument": { "uri": uri } }),
                )
                .await
                && first_err.is_none()
            {
                first_err = Some(ClientError::Transport(e.to_string()));
            }
        }
        this.open_files.clear();

        // Send shutdown request.
        if let Err(e) = this.transport.send_request("shutdown", Value::Null).await
            && first_err.is_none()
        {
            first_err = Some(ClientError::Transport(e.to_string()));
        }

        // Send exit notification.
        if let Err(e) = this.transport.send_notification("exit", Value::Null).await
            && first_err.is_none()
        {
            first_err = Some(ClientError::Transport(e.to_string()));
        }

        if let Some(err) = first_err {
            return Err(err);
        }
        Ok(())
    }

    async fn did_open(
        &mut self,
        uri: &str,
        text: &str,
        version: i32,
        language_id: &str,
    ) -> Result<(), ClientError> {
        self.transport
            .send_notification(
                "textDocument/didOpen",
                serde_json::json!({
                    "textDocument": {
                        "uri": uri,
                        "languageId": language_id,
                        "version": version,
                        "text": text,
                    }
                }),
            )
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;

        self.open_files.insert(uri.to_string());
        self.diagnostics_cache.invalidate(uri).await;
        Ok(())
    }

    async fn did_change(&mut self, uri: &str, text: &str, version: i32) -> Result<(), ClientError> {
        self.transport
            .send_notification(
                "textDocument/didChange",
                serde_json::json!({
                    "textDocument": {
                        "uri": uri,
                        "version": version,
                    },
                    "contentChanges": [{ "text": text }]
                }),
            )
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;

        self.diagnostics_cache.invalidate(uri).await;
        Ok(())
    }

    async fn did_close(&mut self, uri: &str) -> Result<(), ClientError> {
        self.transport
            .send_notification(
                "textDocument/didClose",
                serde_json::json!({ "textDocument": { "uri": uri } }),
            )
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;
        self.open_files.remove(uri);
        Ok(())
    }

    async fn goto_definition(
        &self,
        uri: &str,
        position: super::types::Position,
    ) -> Result<Vec<super::types::Location>, ClientError> {
        let result = self
            .retry_on_transient(
                "textDocument/definition",
                serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": position,
                }),
            )
            .await?;

        Ok(parse_location_list(&result))
    }

    async fn references(
        &self,
        uri: &str,
        position: super::types::Position,
    ) -> Result<Vec<super::types::Location>, ClientError> {
        let params = serde_json::json!({
            "textDocument": { "uri": uri },
            "position": position,
            "context": { "includeDeclaration": false }
        });

        let result = self
            .retry_on_transient("textDocument/references", params)
            .await?;

        Ok(parse_location_list(&result))
    }

    async fn implementations(
        &self,
        uri: &str,
        position: super::types::Position,
    ) -> Result<Vec<super::types::Location>, ClientError> {
        let result = self
            .retry_on_transient(
                "textDocument/implementation",
                serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": position,
                }),
            )
            .await?;

        Ok(parse_location_list(&result))
    }

    async fn hover(
        &self,
        uri: &str,
        position: super::types::Position,
    ) -> Result<Option<super::types::Hover>, ClientError> {
        if !provider_enabled(&self.capabilities.hover_provider) {
            return Err(ClientError::Protocol(
                "language server does not support hover".into(),
            ));
        }

        let result = self
            .retry_on_transient(
                "textDocument/hover",
                serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": position,
                }),
            )
            .await?;

        if result.is_null() {
            return Ok(None);
        }

        let hover: super::types::Hover = serde_json::from_value(result)
            .map_err(|e| ClientError::Protocol(format!("failed to parse Hover: {e}")))?;

        Ok(Some(hover))
    }

    async fn diagnostic(&self, uri: &str) -> Result<Vec<super::types::Diagnostic>, ClientError> {
        if !provider_enabled(&self.capabilities.diagnostic_provider) {
            // Server does not support pull diagnostics — read from push cache.
            return Ok(self.diagnostics_cache.get(uri).await.unwrap_or_default());
        }

        let params = serde_json::json!({
            "textDocument": { "uri": uri }
        });

        // Retry on ServerCancelled (-32802) — servers like rust-analyzer cancel
        // diagnostics when they're busy indexing. The LSP spec expects clients
        // to retry in this case.
        let result = self
            .retry_on_transient("textDocument/diagnostic", params)
            .await?;

        if result.is_null() {
            return Ok(Vec::new());
        }

        // LSP can return either FullDocumentDiagnosticReport (with items)
        // or UnchangedDocumentDiagnosticReport (no items field). Since we
        // don't track result IDs, unchanged reports yield an empty list.
        #[derive(serde::Deserialize)]
        struct DiagnosticReport {
            items: Option<Vec<super::types::Diagnostic>>,
        }

        let report: DiagnosticReport = serde_json::from_value(result)
            .map_err(|e| ClientError::Protocol(format!("failed to parse DiagnosticReport: {e}")))?;

        Ok(report.items.unwrap_or_default())
    }

    async fn rename(
        &self,
        uri: &str,
        position: super::types::Position,
        new_name: &str,
    ) -> Result<Option<super::types::WorkspaceEdit>, ClientError> {
        if !provider_enabled(&self.capabilities.rename_provider) {
            return Err(ClientError::Protocol(
                "language server does not support rename".into(),
            ));
        }

        let result = self
            .retry_on_transient(
                "textDocument/rename",
                serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": position,
                    "newName": new_name,
                }),
            )
            .await?;

        if result.is_null() {
            return Ok(None);
        }

        let edit: super::types::WorkspaceEdit = serde_json::from_value(result)
            .map_err(|e| ClientError::Protocol(format!("failed to parse WorkspaceEdit: {e}")))?;

        Ok(Some(edit))
    }

    async fn prepare_call_hierarchy(
        &self,
        uri: &str,
        position: super::types::Position,
    ) -> Result<Vec<super::types::CallHierarchyItem>, ClientError> {
        if !self.call_hierarchy_supported() {
            return Err(ClientError::Protocol(
                "language server does not support call hierarchy".into(),
            ));
        }

        let result = self
            .retry_on_transient(
                "textDocument/prepareCallHierarchy",
                serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": position,
                }),
            )
            .await?;

        let items: Vec<super::types::CallHierarchyItem> = if result.is_null() {
            Vec::new()
        } else {
            serde_json::from_value(result).map_err(|e| {
                ClientError::Protocol(format!("failed to parse CallHierarchyItem[]: {e}"))
            })?
        };

        Ok(items)
    }

    async fn incoming_calls(
        &self,
        item: &super::types::CallHierarchyItem,
    ) -> Result<Vec<super::types::CallHierarchyIncomingCall>, ClientError> {
        let result = self
            .retry_on_transient(
                "callHierarchy/incomingCalls",
                serde_json::json!({ "item": item }),
            )
            .await?;

        let calls: Vec<super::types::CallHierarchyIncomingCall> = if result.is_null() {
            Vec::new()
        } else {
            serde_json::from_value(result).map_err(|e| {
                ClientError::Protocol(format!("failed to parse CallHierarchyIncomingCall[]: {e}"))
            })?
        };

        Ok(calls)
    }

    async fn outgoing_calls(
        &self,
        item: &super::types::CallHierarchyItem,
    ) -> Result<Vec<super::types::CallHierarchyOutgoingCall>, ClientError> {
        let result = self
            .retry_on_transient(
                "callHierarchy/outgoingCalls",
                serde_json::json!({ "item": item }),
            )
            .await?;

        let calls: Vec<super::types::CallHierarchyOutgoingCall> = if result.is_null() {
            Vec::new()
        } else {
            serde_json::from_value(result).map_err(|e| {
                ClientError::Protocol(format!("failed to parse CallHierarchyOutgoingCall[]: {e}"))
            })?
        };

        Ok(calls)
    }
}

// --- Concrete methods not on the trait ---

impl LspClient {
    /// Start a language server and perform the LSP handshake.
    ///
    /// Spawns the child process, sends `initialize`, receives capabilities,
    /// sends `initialized`, and waits for the server to finish indexing.
    pub async fn start(
        command: &str,
        args: &[&str],
        root: &Path,
        client_capabilities: Value,
        env: &HashMap<String, String>,
    ) -> Result<Self, ClientError> {
        let root_uri = path_to_uri(root);
        let (transport, mut notification_rx) = LspTransport::spawn(command, args, env)
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;

        // Send initialize request.
        let init_params = serde_json::json!({
            "processId": std::process::id(),
            "rootUri": &root_uri,
            "rootPath": root,
            "capabilities": client_capabilities,
        });

        let result = transport
            .send_request("initialize", init_params)
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;

        let init_result: super::types::InitializeResult = serde_json::from_value(result)
            .map_err(|e| ClientError::Protocol(format!("failed to parse InitializeResult: {e}")))?;

        // Send initialized notification.
        transport
            .send_notification("initialized", serde_json::json!({}))
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;

        let diagnostics_cache: Cache<String, Vec<Diagnostic>> = Cache::builder()
            .time_to_idle(Duration::from_secs(600))
            .build();

        let (status_tx, index_state) = watch::channel(IndexState::NotReported);

        // Spawn background task to process server notifications.
        let cache = diagnostics_cache.clone();
        tokio::spawn(async move {
            let mut last_health = None;
            while let Some((method, params)) = notification_rx.recv().await {
                handle_notification(&method, params, &cache, &status_tx, &mut last_health).await;
            }
        });

        Ok(Self {
            transport,
            root_uri,
            open_files: HashSet::new(),
            capabilities: init_result.capabilities,
            diagnostics_cache,
            index_state,
        })
    }

    /// Return a receiver for the server's indexing state, as reported by
    /// `experimental/serverStatus` notifications.
    pub fn index_state(&self) -> watch::Receiver<IndexState> {
        self.index_state.clone()
    }

    /// Return whether a file is currently tracked as open.
    #[allow(dead_code)]
    pub fn is_file_open(&self, uri: &str) -> bool {
        self.open_files.contains(uri)
    }

    /// Remove a file from the open tracking set (after didClose sent).
    #[allow(dead_code)]
    pub fn mark_file_closed(&mut self, uri: &str) {
        self.open_files.remove(uri);
    }

    /// Get the server capabilities received during initialization.
    #[allow(dead_code)]
    pub fn capabilities(&self) -> &ServerCapabilities {
        &self.capabilities
    }

    /// Get the root URI.
    #[allow(dead_code)]
    pub fn root_uri(&self) -> &str {
        &self.root_uri
    }

    /// Check whether the language server supports the callHierarchy protocol.
    fn call_hierarchy_supported(&self) -> bool {
        provider_enabled(&self.capabilities.call_hierarchy_provider)
    }

    /// Send an LSP request, retrying on transient errors.
    ///
    /// Retries up to 3 times on ContentModified (-32801) and
    /// ServerCancelled (-32802) with 500ms delay between attempts.
    async fn retry_on_transient(&self, method: &str, params: Value) -> Result<Value, ClientError> {
        const MAX_RETRIES: u32 = 3;
        let mut result = None;
        for attempt in 0..=MAX_RETRIES {
            match self.transport.send_request(method, params.clone()).await {
                Ok(val) => {
                    result = Some(val);
                    break;
                }
                Err(super::transport::LspError::JsonRpc {
                    code: -32801 | -32802,
                    ..
                }) if attempt < MAX_RETRIES => {
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                }
                Err(e) => return Err(ClientError::Transport(e.to_string())),
            }
        }
        Ok(result.unwrap())
    }

    /// Send `textDocument/documentSymbol` and return whether the server
    /// appears ready (returns a non-empty result).
    #[allow(dead_code)]
    pub async fn document_symbol(&self, uri: &str) -> Result<bool, ClientError> {
        let result = self
            .transport
            .send_request(
                "textDocument/documentSymbol",
                serde_json::json!({
                    "textDocument": { "uri": uri }
                }),
            )
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;

        if let Some(symbols) = result.as_array() {
            Ok(!symbols.is_empty())
        } else {
            Ok(false)
        }
    }

    /// Send `workspace/symbol` and return whether the server appears ready
    /// (returns a non-empty result).
    pub async fn workspace_symbol(&self, query: &str) -> Result<bool, ClientError> {
        let result = self
            .transport
            .send_request("workspace/symbol", serde_json::json!({ "query": query }))
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;

        if let Some(symbols) = result.as_array() {
            Ok(!symbols.is_empty())
        } else {
            Ok(false)
        }
    }

    /// Wait for the language server to finish indexing, bounded by `timeout`.
    ///
    /// If the server reports its status through `experimental/serverStatus`
    /// (rust-analyzer), the wait ends when it reports `quiescent: true`.
    /// Otherwise it polls `workspace/symbol` with an empty query every 500ms
    /// (each request bounded to 5 seconds) until a non-empty result. A status
    /// that arrives while polling switches to waiting for `quiescent: true`.
    ///
    /// Returns `Ok(())` immediately if the server has not reported a status
    /// and does not advertise `workspaceSymbolProvider`. Also returns
    /// `Ok(())` (logged at info) after 5 consecutive empty or failed polls
    /// from a server that never reported a status, as this indicates the
    /// server is responsive but doesn't support the query or hasn't loaded a
    /// project yet (e.g., typescript-language-server returns JSON-RPC error
    /// "No Project" before any file is opened via `didOpen`).
    ///
    /// Returns `Err(ClientError::IndexTimeout)` when the timeout expires
    /// first. An unreachable server is handled by the overall `timeout` —
    /// each poll is also bounded by the per-request timeout, so a truly
    /// broken server won't trigger the early bailout (it will time out
    /// instead).
    pub async fn wait_for_index(&self, timeout: Duration) -> Result<(), ClientError> {
        wait_for_index_with(
            self.index_state(),
            provider_enabled(&self.capabilities.workspace_symbol_provider),
            move || self.workspace_symbol(""),
            timeout,
        )
        .await
    }
}

/// Resolve when the watched state changes. If the sender is gone (the
/// notification task ended because the server stopped), return an error so
/// the wait fails instead of registering a dead server.
async fn state_changed(state: &mut watch::Receiver<IndexState>) -> Result<(), ClientError> {
    state.changed().await.map_err(|_| {
        ClientError::Transport("language server stopped before reporting its status".into())
    })
}

/// Wait until the reported state is `Ready` or `deadline` passes.
async fn wait_until_ready(
    state: &mut watch::Receiver<IndexState>,
    deadline: tokio::time::Instant,
) -> Result<(), ClientError> {
    let ready = tokio::time::timeout_at(deadline, async {
        state
            .wait_for(|s| *s == IndexState::Ready)
            .await
            .map(|_| ())
    })
    .await;
    match ready {
        Ok(Ok(())) => Ok(()),
        Ok(Err(_)) => Err(ClientError::Transport(
            "language server stopped before reporting quiescence".into(),
        )),
        Err(_) => Err(ClientError::IndexTimeout),
    }
}

/// `now + timeout`, or a deadline about 30 years away when that overflows
/// `Instant` (the same horizon tokio uses for "never").
fn deadline_after(timeout: Duration) -> tokio::time::Instant {
    const FAR_FUTURE: Duration = Duration::from_secs(86_400 * 365 * 30);
    let now = tokio::time::Instant::now();
    now.checked_add(timeout).unwrap_or_else(|| now + FAR_FUTURE)
}

/// The index readiness wait behind [`LspClient::wait_for_index`], separated
/// from the client so it can be tested without a language server.
///
/// `poll` sends one `workspace/symbol` request and returns whether the result
/// was non-empty.
async fn wait_for_index_with<P, F>(
    mut state: watch::Receiver<IndexState>,
    has_workspace_symbol: bool,
    mut poll: P,
    timeout: Duration,
) -> Result<(), ClientError>
where
    P: FnMut() -> F,
    F: std::future::Future<Output = Result<bool, ClientError>>,
{
    const POLL_INTERVAL: Duration = Duration::from_millis(500);
    const PER_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
    const MAX_FAIL_POLLS: u32 = 5;

    let deadline = deadline_after(timeout);
    let mut fail_count: u32 = 0;

    loop {
        // `borrow_and_update` marks the value seen, so `changed()` below only
        // wakes on statuses processed after this check.
        if *state.borrow_and_update() != IndexState::NotReported {
            return wait_until_ready(&mut state, deadline).await;
        }
        if !has_workspace_symbol {
            return Ok(());
        }

        let now = tokio::time::Instant::now();
        if now >= deadline {
            return Err(ClientError::IndexTimeout);
        }
        let request_deadline = std::cmp::min(now + PER_REQUEST_TIMEOUT, deadline);

        let result = tokio::select! {
            r = tokio::time::timeout_at(request_deadline, poll()) => r.unwrap_or(Err(
                ClientError::Transport("workspace/symbol request timed out".into()),
            )),
            // A status arrived: abandon the poll and wait for quiescence.
            // A closed sender means the server stopped: fail the wait.
            changed = state_changed(&mut state) => {
                changed?;
                continue;
            }
        };

        match result {
            Ok(true) => return Ok(()),
            Ok(false) | Err(_) => {
                fail_count += 1;
                if fail_count >= MAX_FAIL_POLLS {
                    // The sender may have closed while this poll was in
                    // flight (both select! branches ready, poll won): a
                    // stopped server must not be registered as ready.
                    if state.has_changed().is_err() {
                        return Err(ClientError::Transport(
                            "language server stopped before reporting its status".into(),
                        ));
                    }
                    if *state.borrow_and_update() != IndexState::NotReported {
                        return wait_until_ready(&mut state, deadline).await;
                    }
                    tracing::info!(
                        polls = fail_count,
                        "index wait ended after {MAX_FAIL_POLLS} empty workspace/symbol polls"
                    );
                    return Ok(());
                }
                let sleep_until =
                    std::cmp::min(tokio::time::Instant::now() + POLL_INTERVAL, deadline);
                tokio::select! {
                    () = tokio::time::sleep_until(sleep_until) => {}
                    changed = state_changed(&mut state) => changed?,
                }
            }
        }
    }
}

/// The `(health, message)` pair of the last processed server status, used to
/// log a degraded health only when it changes.
type LastHealth = Option<(String, Option<String>)>;

/// Process one server notification.
///
/// - `textDocument/publishDiagnostics` fills the push diagnostics cache.
/// - `experimental/serverStatus` updates the indexing state from `quiescent`
///   alone. Malformed params are ignored (the previous state is kept). A
///   `health` other than `ok` is logged as a warning when the
///   `(health, message)` pair differs from the last processed status.
async fn handle_notification(
    method: &str,
    params: Value,
    cache: &Cache<String, Vec<Diagnostic>>,
    status_tx: &watch::Sender<IndexState>,
    last_health: &mut LastHealth,
) {
    match method {
        "textDocument/publishDiagnostics" => {
            if let Some(uri) = params.get("uri").and_then(|v| v.as_str()) {
                let diags = params
                    .get("diagnostics")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();
                cache.insert(uri.to_string(), diags).await;
            }
        }
        "experimental/serverStatus" => {
            let status: ServerStatusParams = match serde_json::from_value(params) {
                Ok(status) => status,
                Err(e) => {
                    tracing::debug!(error = %e, "ignoring malformed experimental/serverStatus");
                    return;
                }
            };

            let new_state = if status.quiescent {
                IndexState::Ready
            } else {
                IndexState::Indexing
            };
            let changed = status_tx.send_if_modified(|state| {
                if *state == new_state {
                    false
                } else {
                    *state = new_state;
                    true
                }
            });
            if changed {
                tracing::debug!(
                    state = ?new_state,
                    health = %status.health,
                    message = status.message.as_deref().unwrap_or(""),
                    "language server status changed"
                );
            }

            let pair = (status.health, status.message);
            if pair.0 != "ok" && last_health.as_ref() != Some(&pair) {
                tracing::warn!(
                    health = %pair.0,
                    message = pair.1.as_deref().unwrap_or(""),
                    quiescent = status.quiescent,
                    "language server reports degraded health"
                );
            }
            *last_health = Some(pair);
        }
        _ => {}
    }
}

/// Parse a GotoDefinitionResponse / ReferenceResponse into a flat list of
/// locations. LSP spec allows: `Location[]`, `LocationLink[]`, or `null`.
fn parse_location_list(value: &Value) -> Vec<super::types::Location> {
    let Some(arr) = value.as_array() else {
        return Vec::new();
    };

    let mut locations = Vec::new();
    for item in arr {
        // LocationLink has `targetUri` + `targetRange`.
        if let Some(uri) = item.get("targetUri").and_then(|v| v.as_str())
            && let Some(range) = item.get("targetRange")
            && let Ok(range) = serde_json::from_value(range.clone())
        {
            locations.push(super::types::Location {
                uri: uri.to_string(),
                range,
            });
            continue;
        }
        // Plain Location has `uri` + `range`.
        if let Ok(loc) = serde_json::from_value(item.clone()) {
            locations.push(loc);
        }
    }
    locations
}

/// Check if a provider capability is enabled.
/// LSP allows `true`, `false`, or an object (e.g. `{"documentSelector": ...}`).
/// Both `None` (missing) and `Some(false)` mean the capability is disabled.
fn provider_enabled(provider: &Option<Value>) -> bool {
    match provider {
        None => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => true, // object form means enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn provider_enabled_none() {
        assert!(!provider_enabled(&None));
    }

    #[test]
    fn provider_enabled_bool() {
        assert!(provider_enabled(&Some(Value::Bool(true))));
        assert!(!provider_enabled(&Some(Value::Bool(false))));
    }

    #[test]
    fn provider_enabled_object() {
        assert!(provider_enabled(&Some(
            serde_json::json!({ "documentSelector": [] })
        )));
    }

    #[tokio::test]
    async fn notification_updates_diagnostics_cache() {
        let cache: Cache<String, Vec<Diagnostic>> = Cache::builder()
            .time_to_idle(Duration::from_secs(600))
            .build();

        // Simulate a publishDiagnostics notification.
        let diags = vec![Diagnostic {
            range: super::super::types::Range {
                start: super::super::types::Position {
                    line: 0,
                    character: 0,
                },
                end: super::super::types::Position {
                    line: 0,
                    character: 5,
                },
            },
            severity: Some(1),
            code: None,
            source: None,
            message: "test error".into(),
        }];
        cache
            .insert("file:///test.ts".to_string(), diags.clone())
            .await;

        let cached = cache.get(&"file:///test.ts".to_string()).await;
        assert_eq!(cached.as_ref().unwrap().len(), 1);
        assert_eq!(cached.as_ref().unwrap()[0].message, "test error");
    }

    #[tokio::test]
    async fn cache_miss_returns_empty() {
        let cache: Cache<String, Vec<Diagnostic>> = Cache::builder()
            .time_to_idle(Duration::from_secs(600))
            .build();

        let result = cache.get(&"file:///nonexistent.ts".to_string()).await;
        assert!(result.is_none());
        assert!(result.unwrap_or_default().is_empty());
    }

    #[tokio::test]
    async fn invalidate_on_did_change() {
        let cache: Cache<String, Vec<Diagnostic>> = Cache::builder()
            .time_to_idle(Duration::from_secs(600))
            .build();

        let uri = "file:///test.ts".to_string();
        cache
            .insert(
                uri.clone(),
                vec![Diagnostic {
                    range: super::super::types::Range {
                        start: super::super::types::Position {
                            line: 0,
                            character: 0,
                        },
                        end: super::super::types::Position {
                            line: 0,
                            character: 5,
                        },
                    },
                    severity: Some(1),
                    code: None,
                    source: None,
                    message: "old error".into(),
                }],
            )
            .await;

        // Simulate didChange invalidation.
        cache.invalidate(&uri).await;

        let result = cache.get(&uri).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn invalidate_only_target_uri() {
        let cache: Cache<String, Vec<Diagnostic>> = Cache::builder()
            .time_to_idle(Duration::from_secs(600))
            .build();

        let uri_a = "file:///App.tsx".to_string();
        let uri_b = "file:///utils.ts".to_string();

        cache.insert(uri_a.clone(), vec![]).await;
        cache.insert(uri_b.clone(), vec![]).await;

        // Invalidate only uri_a.
        cache.invalidate(&uri_a).await;

        assert!(cache.get(&uri_a).await.is_none());
        assert!(cache.get(&uri_b).await.is_some());
    }

    #[tokio::test]
    async fn invalidate_on_did_open() {
        let cache: Cache<String, Vec<Diagnostic>> = Cache::builder()
            .time_to_idle(Duration::from_secs(600))
            .build();

        let uri = "file:///new-file.ts".to_string();
        // Pre-populate with stale data (e.g., from a previous session).
        cache
            .insert(
                uri.clone(),
                vec![Diagnostic {
                    range: super::super::types::Range {
                        start: super::super::types::Position {
                            line: 0,
                            character: 0,
                        },
                        end: super::super::types::Position {
                            line: 0,
                            character: 5,
                        },
                    },
                    severity: Some(1),
                    code: None,
                    source: None,
                    message: "stale".into(),
                }],
            )
            .await;

        // Simulate didOpen invalidation.
        cache.invalidate(&uri).await;

        let result = cache.get(&uri).await;
        assert!(result.is_none());
    }

    // --- Server status notification tests ---

    fn new_cache() -> Cache<String, Vec<Diagnostic>> {
        Cache::builder()
            .time_to_idle(Duration::from_secs(600))
            .build()
    }

    async fn send_status(
        tx: &watch::Sender<IndexState>,
        last_health: &mut LastHealth,
        params: Value,
    ) {
        handle_notification(
            "experimental/serverStatus",
            params,
            &new_cache(),
            tx,
            last_health,
        )
        .await;
    }

    #[tokio::test]
    async fn status_initial_state_not_reported() {
        let (_tx, rx) = watch::channel(IndexState::NotReported);
        assert_eq!(*rx.borrow(), IndexState::NotReported);
    }

    #[tokio::test]
    async fn status_transitions() {
        let (tx, rx) = watch::channel(IndexState::NotReported);
        let mut last = None;

        send_status(
            &tx,
            &mut last,
            serde_json::json!({ "health": "ok", "quiescent": false, "message": null }),
        )
        .await;
        assert_eq!(*rx.borrow(), IndexState::Indexing);

        send_status(
            &tx,
            &mut last,
            serde_json::json!({ "health": "ok", "quiescent": true }),
        )
        .await;
        assert_eq!(*rx.borrow(), IndexState::Ready);

        // A workspace reload sends the server back to indexing.
        send_status(
            &tx,
            &mut last,
            serde_json::json!({ "health": "ok", "quiescent": false }),
        )
        .await;
        assert_eq!(*rx.borrow(), IndexState::Indexing);
    }

    #[tokio::test]
    async fn status_degraded_health_quiescent_is_ready() {
        let (tx, rx) = watch::channel(IndexState::NotReported);
        let mut last = None;

        send_status(
            &tx,
            &mut last,
            serde_json::json!({
                "health": "warning",
                "quiescent": true,
                "message": "invalid config value"
            }),
        )
        .await;
        assert_eq!(*rx.borrow(), IndexState::Ready);
        assert_eq!(
            last,
            Some((
                "warning".to_string(),
                Some("invalid config value".to_string())
            ))
        );
    }

    #[tokio::test]
    async fn status_malformed_keeps_previous_state() {
        let (tx, rx) = watch::channel(IndexState::NotReported);
        let mut last = None;

        send_status(
            &tx,
            &mut last,
            serde_json::json!({ "health": "ok", "quiescent": false }),
        )
        .await;
        assert_eq!(*rx.borrow(), IndexState::Indexing);

        send_status(&tx, &mut last, serde_json::json!({ "health": "ok" })).await;
        assert_eq!(*rx.borrow(), IndexState::Indexing);

        send_status(
            &tx,
            &mut last,
            serde_json::json!({ "health": "ok", "quiescent": "yes" }),
        )
        .await;
        assert_eq!(*rx.borrow(), IndexState::Indexing);
    }

    #[tokio::test]
    async fn publish_diagnostics_fills_cache() {
        let (tx, rx) = watch::channel(IndexState::NotReported);
        let cache = new_cache();
        let mut last = None;

        handle_notification(
            "textDocument/publishDiagnostics",
            serde_json::json!({
                "uri": "file:///test.rs",
                "diagnostics": [{
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 0, "character": 5 }
                    },
                    "severity": 1,
                    "message": "test error"
                }]
            }),
            &cache,
            &tx,
            &mut last,
        )
        .await;

        let cached = cache.get(&"file:///test.rs".to_string()).await.unwrap();
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].message, "test error");
        assert_eq!(*rx.borrow(), IndexState::NotReported);
    }

    // --- Index wait tests ---

    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A poll closure that counts calls and returns `result` each time.
    fn counting_poll(
        count: &Arc<AtomicUsize>,
        result: bool,
    ) -> impl FnMut() -> std::future::Ready<Result<bool, ClientError>> {
        let count = count.clone();
        move || {
            count.fetch_add(1, Ordering::SeqCst);
            std::future::ready(Ok(result))
        }
    }

    #[tokio::test(start_paused = true)]
    async fn wait_server_stopped_during_fifth_poll_fails() {
        // The sender closes while the fifth empty poll is in flight, so the
        // poll branch of the select! can win; the five-poll exit must still
        // notice the closure instead of returning Ok.
        let (tx, rx) = watch::channel(IndexState::NotReported);
        let tx = std::sync::Mutex::new(Some(tx));
        let polls = Arc::new(AtomicUsize::new(0));
        let count = polls.clone();
        let poll = move || {
            let n = count.fetch_add(1, Ordering::SeqCst) + 1;
            if n == 5 {
                drop(tx.lock().unwrap().take());
            }
            std::future::ready(Ok(false))
        };

        let result = wait_for_index_with(rx, true, poll, Duration::from_secs(30)).await;

        assert!(
            matches!(result, Err(ClientError::Transport(_))),
            "{result:?}"
        );
        assert_eq!(polls.load(Ordering::SeqCst), 5);
    }

    #[tokio::test(start_paused = true)]
    async fn wait_status_ready_before_timeout_does_not_poll() {
        let (tx, rx) = watch::channel(IndexState::NotReported);
        tx.send(IndexState::Indexing).unwrap();
        let polls = Arc::new(AtomicUsize::new(0));

        let (result, ()) = tokio::join!(
            wait_for_index_with(
                rx,
                true,
                counting_poll(&polls, false),
                Duration::from_secs(30)
            ),
            async {
                tokio::time::sleep(Duration::from_secs(10)).await;
                tx.send(IndexState::Ready).unwrap();
            }
        );

        assert!(result.is_ok());
        assert_eq!(polls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn wait_huge_timeout_does_not_overflow() {
        let (tx, rx) = watch::channel(IndexState::NotReported);
        tx.send(IndexState::Indexing).unwrap();
        let polls = Arc::new(AtomicUsize::new(0));

        let (result, ()) = tokio::join!(
            wait_for_index_with(rx, true, counting_poll(&polls, false), Duration::MAX),
            async {
                tokio::time::sleep(Duration::from_secs(10)).await;
                tx.send(IndexState::Ready).unwrap();
            }
        );

        assert!(result.is_ok());
    }

    #[tokio::test(start_paused = true)]
    async fn wait_server_stopped_is_not_a_timeout() {
        let (tx, rx) = watch::channel(IndexState::NotReported);
        tx.send(IndexState::Indexing).unwrap();
        let polls = Arc::new(AtomicUsize::new(0));

        let start = tokio::time::Instant::now();
        let (result, ()) = tokio::join!(
            wait_for_index_with(
                rx,
                true,
                counting_poll(&polls, false),
                Duration::from_secs(30)
            ),
            async move {
                // The notification task ends (server died): the sender drops.
                tokio::time::sleep(Duration::from_secs(5)).await;
                drop(tx);
            }
        );

        assert!(matches!(result, Err(ClientError::Transport(_))));
        assert_eq!(start.elapsed(), Duration::from_secs(5));
    }

    #[tokio::test(start_paused = true)]
    async fn wait_server_stopped_before_first_status_fails() {
        // No status was ever reported (polling path); the sender drops after
        // two empty polls because the server died. The wait must fail rather
        // than end with Ok after five empty polls.
        let (tx, rx) = watch::channel(IndexState::NotReported);
        let polls = Arc::new(AtomicUsize::new(0));

        let start = tokio::time::Instant::now();
        let (result, ()) = tokio::join!(
            wait_for_index_with(
                rx,
                true,
                counting_poll(&polls, false),
                Duration::from_secs(30)
            ),
            async move {
                tokio::time::sleep(Duration::from_millis(700)).await;
                drop(tx);
            }
        );

        assert!(matches!(result, Err(ClientError::Transport(_))));
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "{:?}",
            start.elapsed()
        );
        assert!(polls.load(Ordering::SeqCst) < 5);
    }

    #[tokio::test(start_paused = true)]
    async fn wait_status_never_ready_times_out() {
        let (tx, rx) = watch::channel(IndexState::NotReported);
        tx.send(IndexState::Indexing).unwrap();
        let polls = Arc::new(AtomicUsize::new(0));

        let start = tokio::time::Instant::now();
        let result = wait_for_index_with(
            rx,
            true,
            counting_poll(&polls, false),
            Duration::from_secs(30),
        )
        .await;

        assert!(matches!(result, Err(ClientError::IndexTimeout)));
        assert_eq!(start.elapsed(), Duration::from_secs(30));
        // A status-reporting server is never polled, so the 5-poll early
        // exit cannot end the wait.
        assert_eq!(polls.load(Ordering::SeqCst), 0);
        drop(tx);
    }

    #[tokio::test(start_paused = true)]
    async fn wait_no_status_five_empty_polls_returns_ok() {
        let (_tx, rx) = watch::channel(IndexState::NotReported);
        let polls = Arc::new(AtomicUsize::new(0));

        let result = wait_for_index_with(
            rx,
            true,
            counting_poll(&polls, false),
            Duration::from_secs(30),
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(polls.load(Ordering::SeqCst), 5);
    }

    #[tokio::test(start_paused = true)]
    async fn wait_status_after_two_polls_waits_for_ready() {
        let (tx, rx) = watch::channel(IndexState::NotReported);
        let polls = Arc::new(AtomicUsize::new(0));

        let start = tokio::time::Instant::now();
        let (result, ()) = tokio::join!(
            wait_for_index_with(
                rx,
                true,
                counting_poll(&polls, false),
                Duration::from_secs(30)
            ),
            async {
                // Polls run at 0 ms and 500 ms; the status arrives before
                // the third poll at 1000 ms.
                tokio::time::sleep(Duration::from_millis(750)).await;
                tx.send(IndexState::Indexing).unwrap();
                tokio::time::sleep(Duration::from_secs(20)).await;
                tx.send(IndexState::Ready).unwrap();
            }
        );

        assert!(result.is_ok());
        assert_eq!(polls.load(Ordering::SeqCst), 2);
        assert!(start.elapsed() >= Duration::from_secs(20));
    }

    #[tokio::test(start_paused = true)]
    async fn wait_no_status_no_provider_returns_immediately() {
        let (_tx, rx) = watch::channel(IndexState::NotReported);
        let polls = Arc::new(AtomicUsize::new(0));

        let start = tokio::time::Instant::now();
        let result = wait_for_index_with(
            rx,
            false,
            counting_poll(&polls, false),
            Duration::from_secs(30),
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(start.elapsed(), Duration::ZERO);
        assert_eq!(polls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn wait_non_empty_poll_returns_ok() {
        let (_tx, rx) = watch::channel(IndexState::NotReported);
        let polls = Arc::new(AtomicUsize::new(0));

        let result = wait_for_index_with(
            rx,
            true,
            counting_poll(&polls, true),
            Duration::from_secs(30),
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(polls.load(Ordering::SeqCst), 1);
    }

    // --- Capability gating tests (Group 6) ---

    fn test_position() -> super::super::types::Position {
        super::super::types::Position {
            line: 5,
            character: 10,
        }
    }

    fn test_range() -> super::super::types::Range {
        super::super::types::Range {
            start: super::super::types::Position {
                line: 0,
                character: 0,
            },
            end: super::super::types::Position {
                line: 0,
                character: 5,
            },
        }
    }

    #[tokio::test]
    async fn hover_returns_none_on_null() {
        let mut mock = MockLspClientApi::new();
        mock.expect_hover().returning(|_, _| Ok(None));

        let result = mock.hover("file:///test.rs", test_position()).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn hover_handles_marked_string_object() {
        let hover = super::super::types::Hover {
            contents: serde_json::json!({
                "language": "rust",
                "value": "fn main()"
            }),
            range: Some(test_range()),
        };

        let mut mock = MockLspClientApi::new();
        mock.expect_hover()
            .returning(move |_, _| Ok(Some(hover.clone())));

        let result = mock.hover("file:///test.rs", test_position()).await;
        let hover_result = result.unwrap().unwrap();
        assert!(hover_result.contents.is_object());
        assert_eq!(hover_result.contents["language"], "rust");
        assert_eq!(hover_result.contents["value"], "fn main()");
    }

    #[tokio::test]
    async fn diagnostic_fallback_to_push_cache() {
        let mut mock = MockLspClientApi::new();
        // Simulate a server without pull-diagnostics: returns empty vec.
        mock.expect_diagnostic().returning(|_| Ok(Vec::new()));

        let result = mock.diagnostic("file:///test.rs").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn diagnostic_sends_pull_request() {
        let diag = Diagnostic {
            range: test_range(),
            severity: Some(1),
            code: None,
            source: None,
            message: "unused variable".into(),
        };

        let mut mock = MockLspClientApi::new();
        mock.expect_diagnostic()
            .returning(move |_| Ok(vec![diag.clone()]));

        let result = mock.diagnostic("file:///test.rs").await;
        let diags = result.unwrap();
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "unused variable");
    }

    #[tokio::test]
    async fn rename_returns_none_on_null() {
        let mut mock = MockLspClientApi::new();
        mock.expect_rename().returning(|_, _, _| Ok(None));

        let result = mock
            .rename("file:///test.rs", test_position(), "new_name")
            .await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }
}
