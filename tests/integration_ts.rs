#![cfg(feature = "integration-typescript")]

mod common;

use std::path::PathBuf;
use std::time::Duration;

use serde_json::json;

use common::McpClient;

/// Absolute path to the TypeScript fixture's index.ts.
fn fixture_index() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("fixtures/ts-project/index.ts");
    p
}

async fn spawn_client() -> McpClient {
    let mut cwd = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    cwd.push("fixtures/ts-project");

    let mut client = McpClient::spawn(&cwd).await.unwrap();

    let index = fixture_index();
    let index_str = index.to_str().unwrap();

    // Wait for typescript-language-server to finish indexing
    // Line 25, col 20: `new User("Alice", 30)` — cursor on User constructor
    client
        .wait_for_ready(index_str, 25, 20, Duration::from_secs(30))
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
    let index = fixture_index().to_str().unwrap().to_string();

    // Find references to `User` class (line 5, col 7 — "User" in class def)
    let result = client
        .tools_call(
            "find_references",
            json!({
                "file_path": index,
                "line": 5,
                "column": 7,
                "json": true,
            }),
        )
        .await
        .unwrap();

    let arr = extract_results(&result);
    assert!(
        arr.len() >= 2,
        "expected at least 2 references to User, got {}",
        arr.len()
    );

    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_goto_definition() {
    let mut client = spawn_client().await;
    let index = fixture_index().to_str().unwrap().to_string();

    // Goto definition of `User` in `new User("Alice", 30)` (line 25, col 20)
    let result = client
        .tools_call(
            "goto_definition",
            json!({
                "file_path": index,
                "line": 25,
                "column": 20,
                "json": true,
            }),
        )
        .await
        .unwrap();

    let arr = extract_results(&result);
    assert!(!arr.is_empty(), "expected at least one definition location");

    // Should point to the User class definition around line 5
    let target_line = arr[0]["line"].as_u64().unwrap();
    assert!(
        target_line <= 6,
        "definition should be near line 5-6, got line {target_line}"
    );

    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_find_implementations() {
    let mut client = spawn_client().await;
    let index = fixture_index().to_str().unwrap().to_string();

    // Find implementations of `Greetable` interface (line 1, col 11 — "Greetable")
    let result = client
        .tools_call(
            "find_implementations",
            json!({
                "file_path": index,
                "line": 1,
                "column": 11,
                "json": true,
            }),
        )
        .await
        .unwrap();

    let arr = extract_results(&result);
    assert!(
        arr.len() >= 2,
        "expected at least 2 implementations of Greetable, got {}",
        arr.len()
    );

    client.shutdown().await.unwrap();
}

// typescript-language-server does not support the LSP Call Hierarchy protocol.
// incoming_calls and outgoing_calls are only tested with rust-analyzer.

#[tokio::test]
async fn test_hover() {
    let mut client = spawn_client().await;
    let index = fixture_index().to_str().unwrap().to_string();

    // Hover on `User` in class definition (line 5, col 7)
    let result = client
        .tools_call(
            "hover",
            json!({
                "file_path": index,
                "line": 5,
                "column": 7,
            }),
        )
        .await
        .unwrap();

    let text = extract_text(&result);
    assert!(!text.is_empty(), "hover should return non-empty content");
    assert!(
        text.contains("User") || text.contains("class"),
        "hover should mention User or class, got: {text}"
    );

    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_diagnostics() {
    let mut client = spawn_client().await;
    let index = fixture_index().to_str().unwrap().to_string();

    let result = client
        .tools_call(
            "diagnostics",
            json!({
                "file_path": index,
            }),
        )
        .await
        .unwrap();

    assert!(
        result["content"].is_array(),
        "diagnostics should return content array"
    );

    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_rename() {
    let mut client = spawn_client().await;
    let index = fixture_index().to_str().unwrap().to_string();

    // Rename `greetEntity` function (line 21, col 10) to `sayHello`
    let result = client
        .tools_call(
            "rename",
            json!({
                "file_path": index,
                "line": 21,
                "column": 10,
                "new_name": "sayHello",
                "json": true,
            }),
        )
        .await
        .unwrap();

    let text = extract_text(&result);
    assert!(
        !text.contains("Error") && (text.contains("sayHello") || text.contains("rename")),
        "rename should produce changes or confirmation, got: {text}"
    );

    client.shutdown().await.unwrap();
}
