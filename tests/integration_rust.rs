#![cfg(feature = "integration-rust")]

mod common;

use std::path::PathBuf;
use std::time::Duration;

use serde_json::json;

use common::McpClient;

/// Absolute path to the Rust fixture's lib.rs.
fn fixture_lib() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("fixtures/rust-project/src/lib.rs");
    p
}

async fn spawn_client() -> McpClient {
    common::require_command("rust-analyzer");

    let mut cwd = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    cwd.push("fixtures/rust-project");

    let mut client = McpClient::spawn(&cwd).await.unwrap();

    let lib = fixture_lib();
    let lib_str = lib.to_str().unwrap();

    // Wait for rust-analyzer to finish indexing (poll goto_definition on User::new)
    // Line 52, col 19: `User::new("Alice", 30)` — cursor on `new`
    client
        .wait_for_ready(lib_str, 52, 19, Duration::from_secs(30))
        .await
        .unwrap();

    client
}

fn extract_text(result: &serde_json::Value) -> &str {
    result["content"][0]["text"].as_str().unwrap()
}

fn extract_json(result: &serde_json::Value) -> serde_json::Value {
    let text = extract_text(result);
    serde_json::from_str(text)
        .unwrap_or_else(|e| panic!("invalid JSON in tool output: {e}\n  text: {text}"))
}

/// Extract the "results" array from a tool response.
fn extract_results(result: &serde_json::Value) -> Vec<serde_json::Value> {
    let data = extract_json(result);
    if let Some(arr) = data.get("results").and_then(|v| v.as_array()) {
        arr.clone()
    } else if let Some(arr) = data.as_array() {
        arr.clone()
    } else {
        panic!("expected array or {{results: [...]}} in JSON output, got: {data}");
    }
}

#[tokio::test]
async fn test_tools_list() {
    let mut client = spawn_client().await;

    let result = client.tools_list().await.unwrap();
    let tools = result["tools"].as_array().unwrap();

    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    for expected in [
        "find_references",
        "goto_definition",
        "find_implementations",
        "incoming_calls",
        "outgoing_calls",
        "hover",
        "diagnostics",
        "rename",
    ] {
        assert!(names.contains(&expected), "missing tool: {expected}");
    }

    // Each tool should have a name, description, and inputSchema
    for tool in tools {
        assert!(tool["name"].is_string());
        assert!(tool["description"].is_string());
        assert!(tool["inputSchema"].is_object());
    }

    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_find_references() {
    let mut client = spawn_client().await;
    let lib = fixture_lib().to_str().unwrap().to_string();

    // Find references to `User` struct (line 7, col 12 — "User" in struct def)
    let result = client
        .tools_call(
            "find_references",
            json!({
                "file_path": lib,
                "line": 7,
                "column": 12,
                "json": true,
            }),
        )
        .await
        .unwrap();

    let arr = extract_results(&result);
    assert!(
        arr.len() >= 3,
        "expected at least 3 references to User, got {}",
        arr.len()
    );

    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_goto_definition() {
    let mut client = spawn_client().await;
    let lib = fixture_lib().to_str().unwrap().to_string();

    // Goto definition of `new` in `User::new("Alice", 30)` (line 52, col 19)
    let result = client
        .tools_call(
            "goto_definition",
            json!({
                "file_path": lib,
                "line": 52,
                "column": 19,
                "json": true,
            }),
        )
        .await
        .unwrap();

    let arr = extract_results(&result);
    assert!(!arr.is_empty(), "expected at least one definition location");

    // Should point to the `new` function definition around line 13
    let target_line = arr[0]["line"].as_u64().unwrap();
    assert!(
        target_line <= 15,
        "definition should be near line 13-14, got line {target_line}"
    );

    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_find_implementations() {
    let mut client = spawn_client().await;
    let lib = fixture_lib().to_str().unwrap().to_string();

    // Find implementations of `Named` trait (line 2, col 13 — "Named")
    let result = client
        .tools_call(
            "find_implementations",
            json!({
                "file_path": lib,
                "line": 2,
                "column": 13,
                "json": true,
            }),
        )
        .await
        .unwrap();

    let arr = extract_results(&result);
    assert!(
        arr.len() >= 2,
        "expected at least 2 implementations of Named, got {}",
        arr.len()
    );

    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_incoming_calls() {
    let mut client = spawn_client().await;
    let lib = fixture_lib().to_str().unwrap().to_string();

    // Incoming calls to `greet_named` function (line 46, col 12)
    let result = client
        .tools_call(
            "incoming_calls",
            json!({
                "file_path": lib,
                "line": 46,
                "column": 12,
                "json": true,
            }),
        )
        .await
        .unwrap();

    let arr = extract_results(&result);
    assert!(
        !arr.is_empty(),
        "expected at least one incoming call to greet_named"
    );

    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_outgoing_calls() {
    let mut client = spawn_client().await;
    let lib = fixture_lib().to_str().unwrap().to_string();

    // Outgoing calls from `main` function (line 51, col 12)
    let result = client
        .tools_call(
            "outgoing_calls",
            json!({
                "file_path": lib,
                "line": 51,
                "column": 12,
                "json": true,
            }),
        )
        .await
        .unwrap();

    let arr = extract_results(&result);
    assert!(
        !arr.is_empty(),
        "expected at least one outgoing call from main"
    );

    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_hover() {
    let mut client = spawn_client().await;
    let lib = fixture_lib().to_str().unwrap().to_string();

    // Hover on `User` in struct definition (line 7, col 12)
    let result = client
        .tools_call(
            "hover",
            json!({
                "file_path": lib,
                "line": 7,
                "column": 12,
            }),
        )
        .await
        .unwrap();

    let text = extract_text(&result);
    assert!(!text.is_empty(), "hover should return non-empty content");
    // Should mention User struct or its fields
    assert!(
        text.contains("User") || text.contains("struct"),
        "hover should mention User or struct, got: {text}"
    );

    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_diagnostics() {
    let mut client = spawn_client().await;
    let lib = fixture_lib().to_str().unwrap().to_string();

    let result = client
        .tools_call(
            "diagnostics",
            json!({
                "file_path": lib,
            }),
        )
        .await
        .unwrap();

    // Should return successfully (may have warnings like dead_code, that's fine)
    assert!(
        result["content"].is_array(),
        "diagnostics should return content array"
    );

    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_rename() {
    let mut client = spawn_client().await;
    let lib = fixture_lib().to_str().unwrap().to_string();

    // Rename `greet_named` function (line 46, col 12) to `say_hi`
    let result = client
        .tools_call(
            "rename",
            json!({
                "file_path": lib,
                "line": 46,
                "column": 12,
                "new_name": "say_hi",
                "json": true,
            }),
        )
        .await
        .unwrap();

    let text = extract_text(&result);
    // Should show the rename changes (workspace edit)
    assert!(
        !text.trim().is_empty(),
        "rename should return non-empty content"
    );
    assert!(
        !text.to_lowercase().contains("error"),
        "rename returned an error: {text}"
    );
    assert!(
        text.contains("say_hi") || text.contains("\"changes\"") || text.contains("WorkspaceEdit"),
        "rename should include edit payload, got: {text}"
    );

    client.shutdown().await.unwrap();
}
