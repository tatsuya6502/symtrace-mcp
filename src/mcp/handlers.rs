//! MCP tool handlers — schemas, query dispatch, output formatting, and error handling.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;

use super::tools::ToolError;
use crate::lsp::types::{Location, Position};
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

/// Human-readable: `file:line:col  line_text` with summary line.
fn format_text(locations: &[Location], no_results_msg: &str) -> String {
    if locations.is_empty() {
        return no_results_msg.to_string();
    }

    let mut result = format!("{} results:\n", locations.len());
    let mut line_cache: HashMap<PathBuf, Vec<String>> = HashMap::new();

    for loc in locations {
        let path = uri_to_path(&loc.uri);

        let line_text = read_line_text(&mut line_cache, &path, loc.range.start.line as usize);
        result.push_str(&format!(
            "{}:{}:{}  {}",
            path.display(),
            loc.range.start.line + 1,
            loc.range.start.character + 1,
            line_text.trim_end()
        ));
        if !std::ptr::eq(loc, locations.last().unwrap()) {
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

        let line = value
            .get("line")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| ToolError::invalid_params("missing or invalid line"))?
            as u32;

        let column = value
            .get("column")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| ToolError::invalid_params("missing or invalid column"))?
            as u32;

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

/// Wrap text output in MCP `tools/call` result envelope.
fn mcp_tool_result(text: String) -> Value {
    serde_json::json!({
        "content": [{ "type": "text", "text": text }]
    })
}
