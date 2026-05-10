//! MCP tool handlers — schemas, query dispatch, output formatting, and error handling.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;

use super::tools::ToolError;
use crate::lsp::types::{Location, Position};
use crate::server::idle_monitor::IdleMonitor;
use crate::server::manager::{LanguageServerManager, ManagerError};
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
    manager: &Arc<LanguageServerManager>,
    monitor: &Arc<IdleMonitor>,
    params: Value,
) -> Result<Value, ToolError> {
    execute_query(manager, monitor, params, QueryKind::References).await
}

pub async fn goto_definition(
    manager: &Arc<LanguageServerManager>,
    monitor: &Arc<IdleMonitor>,
    params: Value,
) -> Result<Value, ToolError> {
    execute_query(manager, monitor, params, QueryKind::Definition).await
}

pub async fn find_implementations(
    manager: &Arc<LanguageServerManager>,
    monitor: &Arc<IdleMonitor>,
    params: Value,
) -> Result<Value, ToolError> {
    execute_query(manager, monitor, params, QueryKind::Implementations).await
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
    manager: &Arc<LanguageServerManager>,
    monitor: &Arc<IdleMonitor>,
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
    monitor.touch(language).await;

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
// Output formatting (5.5 / 5.6)
// ---------------------------------------------------------------------------

/// Human-readable: `file:line:col  line_text` with summary line.
fn format_text(locations: &[Location], no_results_msg: &str) -> String {
    if locations.is_empty() {
        return no_results_msg.to_string();
    }

    let mut lines = Vec::with_capacity(locations.len() + 1);
    let mut file_set = HashMap::<PathBuf, ()>::new();
    let mut file_cache: HashMap<PathBuf, Vec<String>> = HashMap::new();

    for loc in locations {
        let path = uri_to_path(&loc.uri);
        let line_num = loc.range.start.line as usize + 1;
        let col = loc.range.start.character as usize + 1;

        let line_text = read_line_text(&mut file_cache, &path, loc.range.start.line as usize);
        lines.push(format!(
            "{}:{}:{}  {}",
            path.display(),
            line_num,
            col,
            line_text
        ));
        file_set.entry(path).or_insert(());
    }

    let n = lines.len();
    let m = file_set.len();
    lines.push(format!("({n} results in {m} files)"));

    lines.join("\n")
}

/// JSON: array of `{ file_path, line, column, line_text }`.
fn format_json(locations: &[Location]) -> String {
    let mut file_cache: HashMap<PathBuf, Vec<String>> = HashMap::new();

    let results: Vec<Value> = locations
        .iter()
        .map(|loc| {
            let path = uri_to_path(&loc.uri);
            let line_text = read_line_text(&mut file_cache, &path, loc.range.start.line as usize);

            serde_json::json!({
                "file_path": path.display().to_string(),
                "line": loc.range.start.line + 1,
                "column": loc.range.start.character + 1,
                "line_text": line_text,
            })
        })
        .collect();

    serde_json::to_string_pretty(&results).unwrap_or_else(|e| format!("[] // error: {e}"))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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
            .ok_or_else(|| ToolError::invalid_params("missing or invalid 'file_path' parameter"))?
            .to_string();

        let line = value
            .get("line")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| ToolError::invalid_params("missing or invalid 'line' parameter"))?
            as u32;

        let column = value
            .get("column")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| ToolError::invalid_params("missing or invalid 'column' parameter"))?
            as u32;

        if line == 0 {
            return Err(ToolError::invalid_params("'line' must be >= 1 (1-based)"));
        }
        if column == 0 {
            return Err(ToolError::invalid_params("'column' must be >= 1 (1-based)"));
        }

        let json = value.get("json").and_then(|v| v.as_bool()).unwrap_or(false);

        Ok(Self {
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
    let lines = cache.entry(path.to_path_buf()).or_insert_with(|| {
        std::fs::read_to_string(path)
            .map(|s| s.lines().map(|l| l.to_string()).collect())
            .unwrap_or_default()
    });
    lines
        .get(line_idx)
        .map(|l| l.trim().to_string())
        .unwrap_or_default()
}

/// Wrap text output in MCP `tools/call` result envelope.
fn mcp_tool_result(text: String) -> Value {
    serde_json::json!({
        "content": [{ "type": "text", "text": text }]
    })
}
