use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

use moka::future::Cache;
use serde_json::Value;

use super::transport::LspTransport;
use super::types::{Diagnostic, ServerCapabilities};
use crate::uri::path_to_uri;

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

/// High-level LSP client that manages lifecycle, file tracking, and queries
/// on top of [`LspTransport`].
pub struct LspClient {
    transport: LspTransport,
    root_uri: String,
    open_files: HashSet<String>,
    capabilities: ServerCapabilities,
    diagnostics_cache: Cache<String, Vec<Diagnostic>>,
}

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
    ) -> Result<Self, ClientError> {
        let root_uri = path_to_uri(root);
        let (transport, mut notification_rx) = LspTransport::spawn(command, args)
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

        // Spawn background task to process server notifications.
        let cache = diagnostics_cache.clone();
        tokio::spawn(async move {
            while let Some((method, params)) = notification_rx.recv().await {
                if method == "textDocument/publishDiagnostics"
                    && let Some(uri) = params.get("uri").and_then(|v| v.as_str())
                {
                    let diags = params
                        .get("diagnostics")
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                        .unwrap_or_default();
                    cache.insert(uri.to_string(), diags).await;
                }
            }
        });

        Ok(Self {
            transport,
            root_uri,
            open_files: HashSet::new(),
            capabilities: init_result.capabilities,
            diagnostics_cache,
        })
    }

    /// Shut down the language server gracefully.
    ///
    /// Sends `shutdown`, waits for the response, then sends `exit`.
    /// Also closes any files still tracked as open.
    pub async fn shutdown(mut self) -> Result<(), ClientError> {
        // Close all tracked open files first.
        for uri in self.open_files.clone() {
            let _ = self
                .transport
                .send_notification(
                    "textDocument/didClose",
                    serde_json::json!({ "textDocument": { "uri": uri } }),
                )
                .await;
        }
        self.open_files.clear();

        // Send shutdown request.
        let _ = self.transport.send_request("shutdown", Value::Null).await;

        // Send exit notification.
        let _ = self.transport.send_notification("exit", Value::Null).await;
        Ok(())
    }

    /// Ensure a file is open in the language server.
    ///
    /// Sends `textDocument/didOpen` if the file is not currently tracked.
    /// Does NOT handle `didChange` — that is `FileManager`'s responsibility.
    pub async fn did_open(
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

    /// Send `textDocument/didChange` for an already-open file.
    pub async fn did_change(
        &mut self,
        uri: &str,
        text: &str,
        version: i32,
    ) -> Result<(), ClientError> {
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

    /// Send `textDocument/didClose` for a file.
    pub async fn did_close(&mut self, uri: &str) -> Result<(), ClientError> {
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

    // --- LSP query methods ---

    /// Send `textDocument/definition` and return locations.
    pub async fn goto_definition(
        &self,
        uri: &str,
        position: super::types::Position,
    ) -> Result<Vec<super::types::Location>, ClientError> {
        let result = self
            .transport
            .send_request(
                "textDocument/definition",
                serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": position,
                }),
            )
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;

        Ok(parse_location_list(&result))
    }

    /// Send `textDocument/references` and return locations.
    pub async fn references(
        &self,
        uri: &str,
        position: super::types::Position,
    ) -> Result<Vec<super::types::Location>, ClientError> {
        let result = self
            .transport
            .send_request(
                "textDocument/references",
                serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": position,
                    "context": { "includeDeclaration": false }
                }),
            )
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;

        Ok(parse_location_list(&result))
    }

    /// Send `textDocument/implementation` and return locations.
    pub async fn implementations(
        &self,
        uri: &str,
        position: super::types::Position,
    ) -> Result<Vec<super::types::Location>, ClientError> {
        let result = self
            .transport
            .send_request(
                "textDocument/implementation",
                serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": position,
                }),
            )
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;

        Ok(parse_location_list(&result))
    }

    pub async fn hover(
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
            .transport
            .send_request(
                "textDocument/hover",
                serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": position,
                }),
            )
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;

        if result.is_null() {
            return Ok(None);
        }

        let hover: super::types::Hover = serde_json::from_value(result)
            .map_err(|e| ClientError::Protocol(format!("failed to parse Hover: {e}")))?;

        Ok(Some(hover))
    }

    pub async fn diagnostic(
        &self,
        uri: &str,
    ) -> Result<Vec<super::types::Diagnostic>, ClientError> {
        if !provider_enabled(&self.capabilities.diagnostic_provider) {
            // Server does not support pull diagnostics — read from push cache.
            return Ok(self.diagnostics_cache.get(uri).await.unwrap_or_default());
        }

        let result = self
            .transport
            .send_request(
                "textDocument/diagnostic",
                serde_json::json!({
                    "textDocument": { "uri": uri }
                }),
            )
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;

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

    pub async fn rename(
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
            .transport
            .send_request(
                "textDocument/rename",
                serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": position,
                    "newName": new_name,
                }),
            )
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;

        if result.is_null() {
            return Ok(None);
        }

        let edit: super::types::WorkspaceEdit = serde_json::from_value(result)
            .map_err(|e| ClientError::Protocol(format!("failed to parse WorkspaceEdit: {e}")))?;

        Ok(Some(edit))
    }

    /// Check whether the language server supports the callHierarchy protocol.
    fn call_hierarchy_supported(&self) -> bool {
        provider_enabled(&self.capabilities.call_hierarchy_provider)
    }

    /// Send `textDocument/prepareCallHierarchy` and return prepared items.
    pub async fn prepare_call_hierarchy(
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
            .transport
            .send_request(
                "textDocument/prepareCallHierarchy",
                serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": position,
                }),
            )
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;

        let items: Vec<super::types::CallHierarchyItem> = if result.is_null() {
            Vec::new()
        } else {
            serde_json::from_value(result).map_err(|e| {
                ClientError::Protocol(format!("failed to parse CallHierarchyItem[]: {e}"))
            })?
        };

        Ok(items)
    }

    /// Send `callHierarchy/incomingCalls` and return callers.
    pub async fn incoming_calls(
        &self,
        item: &super::types::CallHierarchyItem,
    ) -> Result<Vec<super::types::CallHierarchyIncomingCall>, ClientError> {
        let result = self
            .transport
            .send_request(
                "callHierarchy/incomingCalls",
                serde_json::json!({ "item": item }),
            )
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;

        let calls: Vec<super::types::CallHierarchyIncomingCall> = if result.is_null() {
            Vec::new()
        } else {
            serde_json::from_value(result).map_err(|e| {
                ClientError::Protocol(format!("failed to parse CallHierarchyIncomingCall[]: {e}"))
            })?
        };

        Ok(calls)
    }

    /// Send `callHierarchy/outgoingCalls` and return callees.
    pub async fn outgoing_calls(
        &self,
        item: &super::types::CallHierarchyItem,
    ) -> Result<Vec<super::types::CallHierarchyOutgoingCall>, ClientError> {
        let result = self
            .transport
            .send_request(
                "callHierarchy/outgoingCalls",
                serde_json::json!({ "item": item }),
            )
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;

        let calls: Vec<super::types::CallHierarchyOutgoingCall> = if result.is_null() {
            Vec::new()
        } else {
            serde_json::from_value(result).map_err(|e| {
                ClientError::Protocol(format!("failed to parse CallHierarchyOutgoingCall[]: {e}"))
            })?
        };

        Ok(calls)
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

    /// Wait for the language server to finish indexing by polling
    /// `workspace/symbol` with an empty query until a non-empty result arrives.
    ///
    /// Polls every 500ms with the given overall timeout.
    pub async fn wait_for_index(&self, timeout: std::time::Duration) -> Result<(), ClientError> {
        let start = std::time::Instant::now();
        let poll_interval = std::time::Duration::from_millis(500);

        loop {
            match self.workspace_symbol("").await {
                Ok(true) => return Ok(()),
                Ok(false) | Err(_) => {
                    if start.elapsed() >= timeout {
                        return Err(ClientError::IndexTimeout);
                    }
                    tokio::time::sleep(poll_interval).await;
                }
            }
        }
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
}
