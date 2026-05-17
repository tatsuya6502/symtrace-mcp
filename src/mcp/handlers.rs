//! MCP tool handlers — schemas, query dispatch, output formatting, and error handling.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;

use super::tools::ToolError;
use crate::lsp::types::{CallHierarchyItem, Location, Position};
use crate::lsp::types::{Diagnostic, TextEdit, WorkspaceEdit};
use crate::project::registry::ProjectRegistry;
use crate::server::manager::ManagerError;
use crate::uri::uri_to_path;

// ---------------------------------------------------------------------------
// Tool schemas (5.1)
// ---------------------------------------------------------------------------

pub fn find_references_schema() -> Value {
    query_schema("Find all references to the symbol at the given position.")
}

pub fn goto_definition_schema() -> Value {
    query_schema("Go to the definition of the symbol at the given position.")
}

pub fn find_implementations_schema() -> Value {
    query_schema("Find implementations of the trait or type at the given position.")
}

pub fn incoming_calls_schema() -> Value {
    call_hierarchy_schema(
        "Find callers (incoming calls) of the function or method at the given position.",
    )
}

pub fn outgoing_calls_schema() -> Value {
    call_hierarchy_schema(
        "Find callees (outgoing calls) from the function or method at the given position.",
    )
}

pub fn hover_schema() -> Value {
    query_schema(
        "Show type information, documentation, and signature for the symbol at the given position.",
    )
}

pub fn diagnostics_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "file_path": {
                "type": "string",
                "description": "Absolute path to the source file"
            },
            "json": {
                "type": "boolean",
                "default": false,
                "description": "Return JSON output instead of human-readable text"
            }
        },
        "required": ["file_path"]
    })
}

pub fn rename_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "file_path": {
                "type": "string",
                "description": "Absolute path to the source file"
            },
            "line": {
                "type": "integer",
                "description": "Line number (1-based)"
            },
            "column": {
                "type": "integer",
                "description": "Column number (1-based)"
            },
            "new_name": {
                "type": "string",
                "description": "The new name for the symbol"
            },
            "json": {
                "type": "boolean",
                "default": false,
                "description": "Return JSON output instead of human-readable text"
            }
        },
        "required": ["file_path", "line", "column", "new_name"]
    })
}

fn query_schema(_description: &str) -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "file_path": {
                "type": "string",
                "description": "Absolute path to the source file"
            },
            "line": {
                "type": "integer",
                "description": "Line number (1-based)"
            },
            "column": {
                "type": "integer",
                "description": "Column number (1-based)"
            },
            "json": {
                "type": "boolean",
                "default": false,
                "description": "Return JSON output instead of human-readable text"
            }
        },
        "required": ["file_path", "line", "column"]
    })
}

fn call_hierarchy_schema(_description: &str) -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "file_path": {
                "type": "string",
                "description": "Absolute path to the source file"
            },
            "line": {
                "type": "integer",
                "description": "Line number (1-based)"
            },
            "column": {
                "type": "integer",
                "description": "Column number (1-based)"
            },
            "depth": {
                "type": "integer",
                "enum": [1],
                "default": 1,
                "description": "Call chain depth (currently only 1 is supported)"
            },
            "json": {
                "type": "boolean",
                "default": false,
                "description": "Return JSON output instead of human-readable text"
            }
        },
        "required": ["file_path", "line", "column"]
    })
}

// ---------------------------------------------------------------------------
// Handler entry points (5.2–5.4)
// ---------------------------------------------------------------------------

pub async fn find_references(
    registry: &Arc<ProjectRegistry>,
    params: Value,
) -> Result<Value, ToolError> {
    execute_query(registry, params, QueryKind::References).await
}

pub async fn goto_definition(
    registry: &Arc<ProjectRegistry>,
    params: Value,
) -> Result<Value, ToolError> {
    execute_query(registry, params, QueryKind::Definition).await
}

pub async fn find_implementations(
    registry: &Arc<ProjectRegistry>,
    params: Value,
) -> Result<Value, ToolError> {
    execute_query(registry, params, QueryKind::Implementations).await
}

pub async fn incoming_calls(
    registry: &Arc<ProjectRegistry>,
    params: Value,
) -> Result<Value, ToolError> {
    execute_call_hierarchy(registry, params, CallDirection::Incoming).await
}

pub async fn outgoing_calls(
    registry: &Arc<ProjectRegistry>,
    params: Value,
) -> Result<Value, ToolError> {
    execute_call_hierarchy(registry, params, CallDirection::Outgoing).await
}

pub async fn hover(registry: &Arc<ProjectRegistry>, params: Value) -> Result<Value, ToolError> {
    let p = ToolParams::parse(&params)?;
    let path = Path::new(&p.file_path);

    if !path
        .try_exists()
        .map_err(|e| ToolError::invalid_params(format!("cannot check path {}: {e}", p.file_path)))?
    {
        return Err(ToolError::invalid_params(format!(
            "file not found: {}",
            p.file_path
        )));
    }
    if !path.is_file() {
        return Err(ToolError::invalid_params(format!(
            "not a file: {}",
            p.file_path
        )));
    }

    let manager = registry
        .get_manager_for_file(path)
        .map_err(|e| ToolError::invalid_params(e.to_string()))?;
    let (language, mut servers) = manager
        .get_client_for_file(path)
        .await
        .map_err(|e| match e {
            ManagerError::UnsupportedLanguage(_) => ToolError::invalid_params(e.to_string()),
            ManagerError::StartupFailed(_) | ManagerError::ClientError(_) => {
                ToolError::internal(e.to_string())
            }
        })?;
    manager.monitor().touch(language).await;

    let entry = servers.get_mut(&language).expect("server entry must exist");
    let language_id = manager
        .config_for(language)
        .expect("config must exist")
        .language_id;

    let uri = entry
        .file_manager
        .ensure_open(&mut entry.client, path, language_id)
        .await
        .map_err(|e| ToolError::internal(e.to_string()))?;

    let position = Position {
        line: p.line.saturating_sub(1),
        character: p.column.saturating_sub(1),
    };

    let result = entry
        .client
        .hover(&uri, position)
        .await
        .map_err(|e| ToolError::internal(e.to_string()))?;

    let text = match result {
        None => "No hover information available".into(),
        Some(hover) if p.json => {
            let mut obj = serde_json::Map::new();
            obj.insert("contents".into(), hover.contents);
            if let Some(range) = hover.range {
                obj.insert("range".into(), serde_json::to_value(range).unwrap());
            }
            serde_json::to_string_pretty(&obj).unwrap()
        }
        Some(hover) => format_hover_text(&hover.contents),
    };

    Ok(mcp_tool_result(text))
}

pub async fn diagnostics(
    registry: &Arc<ProjectRegistry>,
    params: Value,
) -> Result<Value, ToolError> {
    let file_path = params
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::invalid_params("missing file_path"))?
        .to_string();
    let json = params
        .get("json")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let path = Path::new(&file_path);
    if !path
        .try_exists()
        .map_err(|e| ToolError::invalid_params(format!("cannot check path {}: {e}", file_path)))?
    {
        return Err(ToolError::invalid_params(format!(
            "file not found: {}",
            file_path
        )));
    }
    if !path.is_file() {
        return Err(ToolError::invalid_params(format!(
            "not a file: {}",
            file_path
        )));
    }

    let manager = registry
        .get_manager_for_file(path)
        .map_err(|e| ToolError::invalid_params(e.to_string()))?;
    let (language, mut servers) = manager
        .get_client_for_file(path)
        .await
        .map_err(|e| match e {
            ManagerError::UnsupportedLanguage(_) => ToolError::invalid_params(e.to_string()),
            ManagerError::StartupFailed(_) | ManagerError::ClientError(_) => {
                ToolError::internal(e.to_string())
            }
        })?;
    manager.monitor().touch(language).await;

    let entry = servers.get_mut(&language).expect("server entry must exist");
    let language_id = manager
        .config_for(language)
        .expect("config must exist")
        .language_id;

    let uri = entry
        .file_manager
        .ensure_open(&mut entry.client, path, language_id)
        .await
        .map_err(|e| ToolError::internal(e.to_string()))?;

    let diagnostics: Vec<Diagnostic> = entry
        .client
        .diagnostic(&uri)
        .await
        .map_err(|e| ToolError::internal(e.to_string()))?;

    let text = if diagnostics.is_empty() {
        "No diagnostics found".into()
    } else if json {
        let items: Vec<Value> = diagnostics
            .iter()
            .map(|d| {
                let mut obj = serde_json::Map::new();
                obj.insert("file_path".into(), Value::String(file_path.clone()));
                obj.insert(
                    "line".into(),
                    Value::Number((d.range.start.line + 1).into()),
                );
                obj.insert(
                    "column".into(),
                    Value::Number((d.range.start.character + 1).into()),
                );
                if let Some(sev) = d.severity {
                    obj.insert("severity".into(), severity_name(sev).into());
                }
                if let Some(code) = &d.code {
                    obj.insert("code".into(), code.clone());
                }
                if let Some(src) = &d.source {
                    obj.insert("source".into(), Value::String(src.clone()));
                }
                obj.insert("message".into(), Value::String(d.message.clone()));
                Value::Object(obj)
            })
            .collect();
        serde_json::to_string_pretty(&items).unwrap()
    } else {
        let mut lines: Vec<String> = diagnostics
            .iter()
            .map(|d| {
                let sev = d.severity.map(severity_name).unwrap_or("unknown");
                format!(
                    "{}:{} [{}] {}",
                    d.range.start.line + 1,
                    d.range.start.character + 1,
                    sev,
                    d.message
                )
            })
            .collect();
        lines.push(format!("({} diagnostics)", diagnostics.len()));
        lines.join("\n")
    };

    Ok(mcp_tool_result(text))
}

pub async fn rename(registry: &Arc<ProjectRegistry>, params: Value) -> Result<Value, ToolError> {
    let p = ToolParams::parse(&params)?;
    let new_name = params
        .get("new_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::invalid_params("missing new_name"))?
        .trim()
        .to_string();
    if new_name.is_empty() {
        return Err(ToolError::invalid_params("new_name must be non-empty"));
    }

    let path = Path::new(&p.file_path);
    if !path
        .try_exists()
        .map_err(|e| ToolError::invalid_params(format!("cannot check path {}: {e}", p.file_path)))?
    {
        return Err(ToolError::invalid_params(format!(
            "file not found: {}",
            p.file_path
        )));
    }
    if !path.is_file() {
        return Err(ToolError::invalid_params(format!(
            "not a file: {}",
            p.file_path
        )));
    }

    let manager = registry
        .get_manager_for_file(path)
        .map_err(|e| ToolError::invalid_params(e.to_string()))?;
    let (language, mut servers) = manager
        .get_client_for_file(path)
        .await
        .map_err(|e| match e {
            ManagerError::UnsupportedLanguage(_) => ToolError::invalid_params(e.to_string()),
            ManagerError::StartupFailed(_) | ManagerError::ClientError(_) => {
                ToolError::internal(e.to_string())
            }
        })?;
    manager.monitor().touch(language).await;

    let entry = servers.get_mut(&language).expect("server entry must exist");
    let language_id = manager
        .config_for(language)
        .expect("config must exist")
        .language_id;

    let uri = entry
        .file_manager
        .ensure_open(&mut entry.client, path, language_id)
        .await
        .map_err(|e| ToolError::internal(e.to_string()))?;

    let position = Position {
        line: p.line.saturating_sub(1),
        character: p.column.saturating_sub(1),
    };

    let result = entry
        .client
        .rename(&uri, position, &new_name)
        .await
        .map_err(|e| ToolError::internal(e.to_string()))?;

    let text = match result {
        None => "No rename changes".into(),
        Some(edit) if p.json => format_rename_json(&edit),
        Some(edit) => format_rename_text(&edit),
    };

    Ok(mcp_tool_result(text))
}

// ---------------------------------------------------------------------------
// Shared query dispatch
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum QueryKind {
    References,
    Definition,
    Implementations,
}

async fn execute_query(
    registry: &Arc<ProjectRegistry>,
    params: Value,
    kind: QueryKind,
) -> Result<Value, ToolError> {
    let p = ToolParams::parse(&params)?;
    let path = Path::new(&p.file_path);

    // Validate path (5.7)
    if !path
        .try_exists()
        .map_err(|e| ToolError::invalid_params(format!("cannot check path {}: {e}", p.file_path)))?
    {
        return Err(ToolError::invalid_params(format!(
            "file not found: {}",
            p.file_path
        )));
    }
    if !path.is_file() {
        return Err(ToolError::invalid_params(format!(
            "not a file: {}",
            p.file_path
        )));
    }

    // Route to the correct project manager.
    let manager = registry
        .get_manager_for_file(path)
        .map_err(|e| ToolError::invalid_params(e.to_string()))?;

    // Resolve language and get (or lazily start) the server.
    let (language, mut servers) = manager
        .get_client_for_file(path)
        .await
        .map_err(|e| match e {
            ManagerError::UnsupportedLanguage(_) => ToolError::invalid_params(e.to_string()),
            ManagerError::StartupFailed(_) | ManagerError::ClientError(_) => {
                ToolError::internal(e.to_string())
            }
        })?;

    // Record activity for idle monitor.
    manager.monitor().touch(language).await;

    let entry = servers
        .get_mut(&language)
        .expect("server entry must exist after get_client_for_file");

    let language_id = manager
        .config_for(language)
        .expect("config must exist for language")
        .language_id;

    // Ensure file is synced with the language server.
    let uri = entry
        .file_manager
        .ensure_open(&mut entry.client, path, language_id)
        .await
        .map_err(|e| ToolError::internal(e.to_string()))?;

    // LSP positions are 0-based; tool parameters are 1-based.
    let position = Position {
        line: p.line.saturating_sub(1),
        character: p.column.saturating_sub(1),
    };

    let locations = match kind {
        QueryKind::References => entry.client.references(&uri, position).await,
        QueryKind::Definition => entry.client.goto_definition(&uri, position).await,
        QueryKind::Implementations => entry.client.implementations(&uri, position).await,
    }
    .map_err(|e| ToolError::internal(e.to_string()))?;

    // Format output (5.5 / 5.6).
    let text = if p.json {
        format_json(&locations)
    } else {
        let no_results = match kind {
            QueryKind::References => "No references found",
            QueryKind::Definition => "No definition found",
            QueryKind::Implementations => "No implementations found",
        };
        format_text(&locations, no_results)
    };

    Ok(mcp_tool_result(text))
}

// ---------------------------------------------------------------------------
// Call hierarchy dispatch
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum CallDirection {
    Incoming,
    Outgoing,
}

async fn execute_call_hierarchy(
    registry: &Arc<ProjectRegistry>,
    params: Value,
    direction: CallDirection,
) -> Result<Value, ToolError> {
    let p = CallHierarchyParams::parse(&params)?;
    let path = Path::new(&p.file_path);

    if !path
        .try_exists()
        .map_err(|e| ToolError::invalid_params(format!("cannot check path {}: {e}", p.file_path)))?
    {
        return Err(ToolError::invalid_params(format!(
            "file not found: {}",
            p.file_path
        )));
    }
    if !path.is_file() {
        return Err(ToolError::invalid_params(format!(
            "not a file: {}",
            p.file_path
        )));
    }

    let manager = registry
        .get_manager_for_file(path)
        .map_err(|e| ToolError::invalid_params(e.to_string()))?;

    let (language, mut servers) = manager
        .get_client_for_file(path)
        .await
        .map_err(|e| match e {
            ManagerError::UnsupportedLanguage(_) => ToolError::invalid_params(e.to_string()),
            ManagerError::StartupFailed(_) | ManagerError::ClientError(_) => {
                ToolError::internal(e.to_string())
            }
        })?;

    manager.monitor().touch(language).await;

    let entry = servers
        .get_mut(&language)
        .expect("server entry must exist after get_client_for_file");

    let language_id = manager
        .config_for(language)
        .expect("config must exist for language")
        .language_id;

    let uri = entry
        .file_manager
        .ensure_open(&mut entry.client, path, language_id)
        .await
        .map_err(|e| ToolError::internal(e.to_string()))?;

    let position = Position {
        line: p.line.saturating_sub(1),
        character: p.column.saturating_sub(1),
    };

    let items = entry
        .client
        .prepare_call_hierarchy(&uri, position)
        .await
        .map_err(|e| ToolError::internal(e.to_string()))?;

    if items.is_empty() {
        let text = if p.json {
            serde_json::to_string(&serde_json::json!({ "results": [] })).unwrap()
        } else {
            "No callable symbol at this position".to_string()
        };
        return Ok(mcp_tool_result(text));
    }

    let item = &items[0];

    let text = match direction {
        CallDirection::Incoming => {
            let calls = entry
                .client
                .incoming_calls(item)
                .await
                .map_err(|e| ToolError::internal(e.to_string()))?;
            format_call_hierarchy_text(
                &calls
                    .iter()
                    .map(|c| (&c.from, c.from_ranges.first()))
                    .collect::<Vec<_>>(),
                item,
                "←",
                "caller",
                "No callers found",
                p.json,
            )
        }
        CallDirection::Outgoing => {
            let calls = entry
                .client
                .outgoing_calls(item)
                .await
                .map_err(|e| ToolError::internal(e.to_string()))?;
            format_call_hierarchy_text(
                &calls
                    .iter()
                    .map(|c| (&c.to, c.from_ranges.first()))
                    .collect::<Vec<_>>(),
                item,
                "→",
                "callee",
                "No callees found",
                p.json,
            )
        }
    };

    Ok(mcp_tool_result(text))
}

fn format_call_hierarchy_text(
    items: &[(&CallHierarchyItem, Option<&crate::lsp::types::Range>)],
    _target: &CallHierarchyItem,
    arrow: &str,
    label: &str,
    no_results_msg: &str,
    json: bool,
) -> String {
    if items.is_empty() {
        return if json {
            serde_json::to_string(&serde_json::json!({ "results": [] })).unwrap()
        } else {
            no_results_msg.to_string()
        };
    }

    if json {
        let mut line_cache: HashMap<PathBuf, Vec<String>> = HashMap::new();
        let entries: Vec<Value> = items
            .iter()
            .map(|(item, _range)| {
                let file_path = uri_to_path(&item.uri).to_string_lossy().into_owned();
                let line_text = read_line_text(
                    &mut line_cache,
                    Path::new(&file_path),
                    item.selection_range.start.line as usize,
                );
                serde_json::json!({
                    "name": item.name,
                    "file_path": file_path,
                    "line": item.selection_range.start.line + 1,
                    "column": item.selection_range.start.character + 1,
                    "line_text": line_text.trim_end()
                })
            })
            .collect();
        serde_json::to_string(&serde_json::json!({ "results": entries })).unwrap()
    } else {
        let mut result = String::new();
        for (item, _range) in items {
            let file_path = uri_to_path(&item.uri);
            result.push_str(&format!(
                "  {} {}:{}:{}  {}()\n",
                arrow,
                file_path.display(),
                item.selection_range.start.line + 1,
                item.selection_range.start.character + 1,
                item.name,
            ));
        }
        result.push_str(&format!("({} {}s)", items.len(), label));
        result
    }
}

struct CallHierarchyParams {
    file_path: String,
    line: u32,
    column: u32,
    json: bool,
}

impl CallHierarchyParams {
    fn parse(value: &Value) -> Result<Self, ToolError> {
        let file_path = value
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::invalid_params("missing or invalid file_path"))?
            .to_string();

        let line = parse_u32_field(value, "line")?;
        if line == 0 {
            return Err(ToolError::invalid_params("line must be >= 1"));
        }

        let column = parse_u32_field(value, "column")?;
        if column == 0 {
            return Err(ToolError::invalid_params("column must be >= 1"));
        }

        let json = value.get("json").and_then(|v| v.as_bool()).unwrap_or(false);

        let depth = match value.get("depth") {
            None => 1,
            Some(v) => v
                .as_u64()
                .ok_or_else(|| ToolError::invalid_params("depth must be the integer 1"))?,
        };
        if depth != 1 {
            return Err(ToolError::invalid_params("Only depth 1 is supported"));
        }

        Ok(CallHierarchyParams {
            file_path,
            line,
            column,
            json,
        })
    }
}
fn format_text(locations: &[Location], no_results_msg: &str) -> String {
    if locations.is_empty() {
        return no_results_msg.to_string();
    }

    let mut result = format!("{} results:\n", locations.len());
    let mut line_cache: HashMap<PathBuf, Vec<String>> = HashMap::new();
    let last_idx = locations.len() - 1;

    for (i, loc) in locations.iter().enumerate() {
        let path = uri_to_path(&loc.uri);

        let line_text = read_line_text(&mut line_cache, &path, loc.range.start.line as usize);
        result.push_str(&format!(
            "{}:{}:{}  {}",
            path.display(),
            loc.range.start.line + 1,
            loc.range.start.character + 1,
            line_text.trim_end()
        ));
        if i != last_idx {
            result.push('\n');
        }
    }

    result
}

/// JSON: array of `{ file_path, line, column, line_text }`.
fn format_json(locations: &[Location]) -> String {
    let mut line_cache: HashMap<PathBuf, Vec<String>> = HashMap::new();
    let entries: Vec<Value> = locations
        .iter()
        .map(|loc| {
            let file_path = uri_to_path(&loc.uri).to_string_lossy().into_owned();

            let line_text = read_line_text(
                &mut line_cache,
                Path::new(&file_path),
                loc.range.start.line as usize,
            );

            serde_json::json!({
                "file_path": file_path,
                "line": loc.range.start.line + 1,
                "column": loc.range.start.character + 1,
                "line_text": line_text.trim_end()
            })
        })
        .collect();

    serde_json::to_string(&serde_json::json!({ "results": entries })).unwrap()
}

struct ToolParams {
    file_path: String,
    line: u32,
    column: u32,
    json: bool,
}

impl ToolParams {
    fn parse(value: &Value) -> Result<Self, ToolError> {
        let file_path = value
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::invalid_params("missing or invalid file_path"))?
            .to_string();

        let line = parse_u32_field(value, "line")?;
        if line == 0 {
            return Err(ToolError::invalid_params("line must be >= 1"));
        }

        let column = parse_u32_field(value, "column")?;
        if column == 0 {
            return Err(ToolError::invalid_params("column must be >= 1"));
        }

        let json = value.get("json").and_then(|v| v.as_bool()).unwrap_or(false);

        Ok(ToolParams {
            file_path,
            line,
            column,
            json,
        })
    }
}

/// Read a specific line (0-based index) from a file, caching contents.
fn read_line_text(
    cache: &mut HashMap<PathBuf, Vec<String>>,
    path: &Path,
    line_idx: usize,
) -> String {
    if !cache.contains_key(path) {
        let lines = std::fs::read_to_string(path)
            .unwrap_or_default()
            .lines()
            .map(String::from)
            .collect();
        cache.insert(path.to_path_buf(), lines);
    }
    cache
        .get(path)
        .and_then(|lines| lines.get(line_idx))
        .cloned()
        .unwrap_or_default()
}

/// Parse a JSON integer field, rejecting values that don't fit in u32.
fn parse_u32_field(value: &Value, field: &str) -> Result<u32, ToolError> {
    let raw = value
        .get(field)
        .and_then(|v| v.as_u64())
        .ok_or_else(|| ToolError::invalid_params(format!("missing or invalid {field}")))?;
    u32::try_from(raw)
        .map_err(|_| ToolError::invalid_params(format!("{field} value {raw} is out of range")))
}

/// Wrap text output in MCP `tools/call` result envelope.
fn mcp_tool_result(text: String) -> Value {
    serde_json::json!({
        "content": [{ "type": "text", "text": text }]
    })
}

// ---------------------------------------------------------------------------
// Shared helpers for hover, diagnostics, rename
// ---------------------------------------------------------------------------

/// Normalize `Hover.contents` (which can be string, MarkupContent, MarkedString,
/// or array thereof) into a single human-readable string.
fn format_hover_text(contents: &Value) -> String {
    match contents {
        Value::String(s) => s.clone(),
        Value::Object(map) => map
            .get("value")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        Value::Array(items) => items
            .iter()
            .map(format_hover_text)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n"),
        _ => contents.to_string(),
    }
}

fn severity_name(severity: i32) -> &'static str {
    match severity {
        1 => "error",
        2 => "warning",
        3 => "info",
        4 => "hint",
        _ => "unknown",
    }
}

fn format_rename_text(edit: &WorkspaceEdit) -> String {
    let mut lines = Vec::new();
    let mut total_changes = 0usize;
    let mut file_count = 0usize;

    for (uri, edits) in collect_rename_edits(edit) {
        file_count += 1;
        let path = uri_to_path(&uri);
        for te in &edits {
            lines.push(format!(
                "{}:{}:{}  →  {}",
                path.display(),
                te.range.start.line + 1,
                te.range.start.character + 1,
                te.new_text,
            ));
            total_changes += 1;
        }
    }

    lines.push(format!(
        "({} changes in {} files)",
        total_changes, file_count
    ));
    lines.join("\n")
}

fn format_rename_json(edit: &WorkspaceEdit) -> String {
    let mut changes_map = serde_json::Map::new();

    for (uri, edits) in collect_rename_edits(edit) {
        let path = uri_to_path(&uri).display().to_string();
        let entries: Vec<Value> = edits
            .iter()
            .map(|te| {
                serde_json::json!({
                    "line": te.range.start.line + 1,
                    "column": te.range.start.character + 1,
                    "new_text": te.new_text,
                })
            })
            .collect();
        changes_map.insert(path, Value::Array(entries));
    }

    serde_json::to_string_pretty(&serde_json::json!({ "changes": changes_map })).unwrap()
}

/// Normalize both `changes` and `document_changes` from a `WorkspaceEdit`
/// into a flat list of `(uri, Vec<TextEdit>)` pairs.
fn collect_rename_edits(edit: &WorkspaceEdit) -> Vec<(String, Vec<TextEdit>)> {
    let mut result: Vec<(String, Vec<TextEdit>)> = Vec::new();

    if let Some(changes) = &edit.changes {
        for (uri, edits) in changes {
            result.push((uri.clone(), edits.clone()));
        }
    }

    if let Some(Value::Array(docs)) = &edit.document_changes {
        for doc in docs {
            if let Some(uri) = doc
                .get("textDocument")
                .and_then(|td| td.get("uri"))
                .and_then(|v| v.as_str())
            {
                let edits: Option<Vec<TextEdit>> = doc
                    .get("edits")
                    .and_then(|e| serde_json::from_value(e.clone()).ok());
                if let Some(edits) = edits {
                    result.push((uri.to_string(), edits));
                }
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lsp::types::{Range, TextEdit};
    use serde_json::json;

    // --- format_hover_text ---

    #[test]
    fn hover_text_plain_string() {
        assert_eq!(format_hover_text(&json!("hello")), "hello");
    }

    #[test]
    fn hover_text_markup_content() {
        let contents = json!({ "kind": "markdown", "value": "pub fn foo()" });
        assert_eq!(format_hover_text(&contents), "pub fn foo()");
    }

    #[test]
    fn hover_text_array_joins_with_blank_line() {
        let contents = json!(["first", { "value": "second" }]);
        assert_eq!(format_hover_text(&contents), "first\n\nsecond");
    }

    #[test]
    fn hover_text_empty_strings_filtered() {
        let contents = json!(["hello", "", { "value": "" }, "world"]);
        assert_eq!(format_hover_text(&contents), "hello\n\nworld");
    }

    // --- severity_name ---

    #[test]
    fn severity_names() {
        assert_eq!(severity_name(1), "error");
        assert_eq!(severity_name(2), "warning");
        assert_eq!(severity_name(3), "info");
        assert_eq!(severity_name(4), "hint");
        assert_eq!(severity_name(99), "unknown");
    }

    // --- format_rename_text ---

    #[test]
    fn rename_text_single_file() {
        let edit = WorkspaceEdit {
            changes: Some({
                let mut map = HashMap::new();
                map.insert(
                    "file:///src/main.rs".into(),
                    vec![TextEdit {
                        range: Range {
                            start: crate::lsp::types::Position {
                                line: 9,
                                character: 4,
                            },
                            end: crate::lsp::types::Position {
                                line: 9,
                                character: 7,
                            },
                        },
                        new_text: "foo".into(),
                    }],
                );
                map
            }),
            document_changes: None,
        };
        let text = format_rename_text(&edit);
        assert!(text.contains("foo"));
        assert!(text.contains("(1 changes in 1 files)"));
    }

    #[test]
    fn rename_text_empty_edit() {
        let edit = WorkspaceEdit {
            changes: None,
            document_changes: None,
        };
        let text = format_rename_text(&edit);
        assert_eq!(text, "(0 changes in 0 files)");
    }

    // --- format_rename_json ---

    #[test]
    fn rename_json_output() {
        let edit = WorkspaceEdit {
            changes: Some({
                let mut map = HashMap::new();
                map.insert(
                    "file:///src/main.rs".into(),
                    vec![TextEdit {
                        range: Range {
                            start: crate::lsp::types::Position {
                                line: 4,
                                character: 7,
                            },
                            end: crate::lsp::types::Position {
                                line: 4,
                                character: 10,
                            },
                        },
                        new_text: "bar".into(),
                    }],
                );
                map
            }),
            document_changes: None,
        };
        let json_str = format_rename_json(&edit);
        let parsed: Value = serde_json::from_str(&json_str).unwrap();
        let changes = parsed.get("changes").unwrap().as_object().unwrap();
        assert!(changes.len() == 1);
        let entries = changes.values().next().unwrap().as_array().unwrap();
        assert_eq!(entries[0]["line"], 5);
        assert_eq!(entries[0]["column"], 8);
        assert_eq!(entries[0]["new_text"], "bar");
    }

    // --- ToolParams parsing ---

    #[test]
    fn tool_params_valid() {
        let params = json!({
            "file_path": "/src/main.rs",
            "line": 10,
            "column": 5,
            "json": true
        });
        let p = ToolParams::parse(&params).unwrap();
        assert_eq!(p.file_path, "/src/main.rs");
        assert_eq!(p.line, 10);
        assert_eq!(p.column, 5);
        assert!(p.json);
    }

    #[test]
    fn tool_params_defaults_json_false() {
        let params = json!({
            "file_path": "/src/main.rs",
            "line": 1,
            "column": 1
        });
        let p = ToolParams::parse(&params).unwrap();
        assert!(!p.json);
    }

    #[test]
    fn tool_params_missing_field() {
        let params = json!({ "file_path": "/src/main.rs" });
        assert!(ToolParams::parse(&params).is_err());
    }

    // --- document_changes handling ---

    #[test]
    fn rename_text_document_changes() {
        let edit = WorkspaceEdit {
            changes: None,
            document_changes: Some(json!([
                {
                    "textDocument": { "uri": "file:///src/lib.rs", "version": 3 },
                    "edits": [
                        { "range": { "start": {"line": 0, "character": 4}, "end": {"line": 0, "character": 7} }, "newText": "baz" }
                    ]
                }
            ])),
        };
        let text = format_rename_text(&edit);
        assert!(text.contains("baz"));
        assert!(text.contains("(1 changes in 1 files)"));
    }

    #[test]
    fn rename_json_document_changes() {
        let edit = WorkspaceEdit {
            changes: None,
            document_changes: Some(json!([
                {
                    "textDocument": { "uri": "file:///src/lib.rs", "version": 1 },
                    "edits": [
                        { "range": { "start": {"line": 2, "character": 0}, "end": {"line": 2, "character": 3} }, "newText": "qux" }
                    ]
                }
            ])),
        };
        let json_str = format_rename_json(&edit);
        let parsed: Value = serde_json::from_str(&json_str).unwrap();
        let changes = parsed.get("changes").unwrap().as_object().unwrap();
        let entries = changes.values().next().unwrap().as_array().unwrap();
        assert_eq!(entries[0]["new_text"], "qux");
    }

    #[test]
    fn rename_both_changes_and_document_changes() {
        let edit = WorkspaceEdit {
            changes: Some({
                let mut map = HashMap::new();
                map.insert(
                    "file:///src/a.rs".into(),
                    vec![TextEdit {
                        range: Range {
                            start: crate::lsp::types::Position {
                                line: 0,
                                character: 0,
                            },
                            end: crate::lsp::types::Position {
                                line: 0,
                                character: 3,
                            },
                        },
                        new_text: "x".into(),
                    }],
                );
                map
            }),
            document_changes: Some(json!([
                {
                    "textDocument": { "uri": "file:///src/b.rs", "version": 1 },
                    "edits": [
                        { "range": { "start": {"line": 1, "character": 0}, "end": {"line": 1, "character": 3} }, "newText": "y" }
                    ]
                }
            ])),
        };
        let text = format_rename_text(&edit);
        assert!(text.contains("x"));
        assert!(text.contains("y"));
        assert!(text.contains("(2 changes in 2 files)"));
    }
}
