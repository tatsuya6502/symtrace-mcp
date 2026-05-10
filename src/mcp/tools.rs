//! MCP server framework — tool registry and request dispatch.

use serde_json::Value;
use std::collections::HashMap;
use tokio::io::{BufReader, stdin, stdout};

use super::protocol;

type ToolHandler = Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync>;

struct Tool {
    description: String,
    handler: ToolHandler,
}

/// MCP server that reads JSON-RPC requests from stdin and writes responses to stdout.
pub struct McpServer {
    tools: HashMap<String, Tool>,
}

impl McpServer {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool handler by name.
    pub fn register_tool(
        &mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        handler: ToolHandler,
    ) {
        self.tools.insert(
            name.into(),
            Tool {
                description: description.into(),
                handler,
            },
        );
    }

    /// Run the MCP server event loop. Reads from stdin, writes to stdout.
    pub async fn run(&mut self) -> std::io::Result<()> {
        let mut reader = BufReader::new(stdin());
        let mut writer = stdout();

        loop {
            let message = match protocol::read_message(&mut reader).await {
                Ok(msg) => msg,
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::UnexpectedEof {
                        break;
                    }
                    // -32700 Parse error
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
                "tools/call" if !is_notification => Some(self.handle_tools_call(&id, &params)),
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
                })
            })
            .collect();
        protocol::success_response(id, serde_json::json!({ "tools": tools }))
    }

    fn handle_tools_call(&self, id: &Value, params: &Option<Value>) -> Value {
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
                match (tool.handler)(args) {
                    Ok(result) => protocol::success_response(id, result),
                    Err(e) => protocol::error_response(id.clone(), protocol::INTERNAL_ERROR, &e),
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
