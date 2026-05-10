//! MCP server framework — tool registry and request dispatch.

use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use tokio::io::{BufReader, stdin, stdout};

use super::handlers;
use super::protocol;
use crate::server::idle_monitor::IdleMonitor;
use crate::server::manager::LanguageServerManager;

/// Error returned by tool handlers, carrying an MCP error code.
pub struct ToolError {
    pub code: i64,
    pub message: String,
}

impl ToolError {
    pub fn invalid_params(msg: impl Into<String>) -> Self {
        Self {
            code: protocol::INVALID_PARAMS,
            message: msg.into(),
        }
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self {
            code: protocol::INTERNAL_ERROR,
            message: msg.into(),
        }
    }
}

type ToolHandler = Box<
    dyn Fn(Value) -> Pin<Box<dyn Future<Output = Result<Value, ToolError>> + Send>>
        + Send
        + Sync,
>;

struct Tool {
    description: String,
    input_schema: Value,
    handler: ToolHandler,
}

/// MCP server that reads JSON-RPC requests from stdin and writes responses to stdout.
pub struct McpServer {
    tools: HashMap<String, Tool>,
    manager: Arc<LanguageServerManager>,
    monitor: Arc<IdleMonitor>,
}

/// Wrap an async handler into a boxed closure that clones the shared state per call.
macro_rules! tool_handler {
    ($manager:expr, $monitor:expr, $handler:path) => {{
        let manager = $manager.clone();
        let monitor = $monitor.clone();
        Box::new(move |args: Value| {
            let m = manager.clone();
            let mon = monitor.clone();
            Box::pin(async move { $handler(&m, &mon, args).await })
                as Pin<Box<dyn Future<Output = Result<Value, ToolError>> + Send>>
        })
    }};
}

impl McpServer {
    pub fn new(root: PathBuf) -> Self {
        let manager = Arc::new(LanguageServerManager::new(root));
        let monitor = Arc::new(IdleMonitor::new(manager.clone()));

        let mut server = Self {
            tools: HashMap::new(),
            manager: manager.clone(),
            monitor: monitor.clone(),
        };

        // Register the three MCP tools (6.2).
        server.register_tool(
            "find_references",
            "Find all references to the symbol at the given position.",
            handlers::find_references_schema(),
            tool_handler!(manager, monitor, handlers::find_references),
        );
        server.register_tool(
            "goto_definition",
            "Go to the definition of the symbol at the given position.",
            handlers::goto_definition_schema(),
            tool_handler!(manager, monitor, handlers::goto_definition),
        );
        server.register_tool(
            "find_implementations",
            "Find implementations of the trait or type at the given position.",
            handlers::find_implementations_schema(),
            tool_handler!(manager, monitor, handlers::find_implementations),
        );

        server
    }

    fn register_tool(
        &mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: Value,
        handler: ToolHandler,
    ) {
        self.tools.insert(
            name.into(),
            Tool {
                description: description.into(),
                input_schema,
                handler,
            },
        );
    }

    /// Run the MCP server event loop. Reads from stdin, writes to stdout.
    pub async fn run(&mut self) -> std::io::Result<()> {
        // Spawn idle monitor as a background task (6.3).
        let monitor_handle = tokio::spawn(self.monitor.clone().run());

        let mut reader = BufReader::new(stdin());
        let mut writer = stdout();

        loop {
            let message = match protocol::read_message(&mut reader).await {
                Ok(msg) => msg,
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::UnexpectedEof {
                        break;
                    }
                    let response = protocol::error_response(
                        Value::Null,
                        protocol::PARSE_ERROR,
                        &e.to_string(),
                    );
                    protocol::write_message(&mut writer, &response).await?;
                    continue;
                }
            };

            let id_opt = message.get("id").cloned();
            let is_notification = id_opt.is_none();
            let id = id_opt.unwrap_or(Value::Null);
            let method = message.get("method").and_then(|m| m.as_str());
            let params = message.get("params").cloned();

            let Some(method) = method else {
                if !is_notification {
                    let response = protocol::error_response(
                        id,
                        protocol::INVALID_REQUEST,
                        "missing method field",
                    );
                    protocol::write_message(&mut writer, &response).await?;
                }
                continue;
            };

            let response = match method {
                "initialize" if !is_notification => self.handle_initialize(&id, &params),
                "initialize" => None,
                "notifications/initialized" => None,
                "tools/list" if !is_notification => Some(self.handle_tools_list(&id)),
                "tools/list" => None,
                "tools/call" if !is_notification => {
                    Some(self.handle_tools_call(&id, &params).await)
                }
                "tools/call" => None,
                _ if is_notification => None,
                _ => Some(protocol::error_response(
                    id,
                    protocol::METHOD_NOT_FOUND,
                    &format!("unknown method: {method}"),
                )),
            };

            if let Some(response) = response {
                protocol::write_message(&mut writer, &response).await?;
            }
        }

        // Graceful shutdown (6.4).
        monitor_handle.abort();
        self.manager.shutdown_all().await;

        Ok(())
    }

    fn handle_initialize(&self, id: &Value, _params: &Option<Value>) -> Option<Value> {
        let result = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "symtrace-mcp",
                "version": env!("CARGO_PKG_VERSION"),
            }
        });
        Some(protocol::success_response(id, result))
    }

    fn handle_tools_list(&self, id: &Value) -> Value {
        let tools: Vec<Value> = self
            .tools
            .iter()
            .map(|(name, tool)| {
                serde_json::json!({
                    "name": name,
                    "description": tool.description,
                    "inputSchema": tool.input_schema,
                })
            })
            .collect();
        protocol::success_response(id, serde_json::json!({ "tools": tools }))
    }

    async fn handle_tools_call(&self, id: &Value, params: &Option<Value>) -> Value {
        let params = match params {
            Some(p) => p,
            None => {
                return protocol::error_response(
                    id.clone(),
                    protocol::INVALID_REQUEST,
                    "missing params",
                );
            }
        };

        let Some(tool_name) = params.get("name").and_then(|n| n.as_str()) else {
            return protocol::error_response(
                id.clone(),
                protocol::INVALID_REQUEST,
                "missing or invalid tools/call.params.name",
            );
        };

        match self.tools.get(tool_name) {
            Some(tool) => {
                let args = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or(Value::Object(Default::default()));
                match (tool.handler)(args).await {
                    Ok(result) => protocol::success_response(id, result),
                    Err(e) => protocol::error_response(id.clone(), e.code, &e.message),
                }
            }
            None => protocol::error_response(
                id.clone(),
                protocol::INVALID_PARAMS,
                &format!("unknown tool: {tool_name}"),
            ),
        }
    }
}
