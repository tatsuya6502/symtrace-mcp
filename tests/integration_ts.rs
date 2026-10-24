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

/// Absolute path to the TypeScript fixture's caller.ts.
fn fixture_caller() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("fixtures/ts-project/caller.ts");
    p
}

async fn spawn_client() -> McpClient {
    let mut client = spawn_cold_client().await;

    let index = fixture_index();
    let index_str = index.to_str().unwrap();

    // Wait for typescript-language-server to finish indexing
    // Line 25, col 20: `new User("Alice", 30)` — cursor on User constructor
    client
        .wait_for_ready(index_str, 25, 20, Duration::from_secs(60))
        .await
        .unwrap();

    client
}

/// Spawn a client WITHOUT `wait_for_ready`, for first-call tests.
///
/// `spawn_client()`'s readiness poll issues its own `goto_definition`, which
/// opens a file and starts typescript-language-server's project load; later
/// queries in that session may then fall outside the project-loading window
/// in which the syntax-only server would answer (Issue #30). A cold client
/// lets the test's own query be the first `tools/call` of the session.
async fn spawn_cold_client() -> McpClient {
    common::require_command("typescript-language-server");

    let mut cwd = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    cwd.push("fixtures/ts-project");

    McpClient::spawn(&cwd).await.unwrap()
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

#[tokio::test]
async fn test_incoming_calls_cross_file() {
    let mut client = spawn_client().await;
    let index = fixture_index().to_str().unwrap().to_string();

    // Incoming calls to `greetEntity` (line 21, col 10), which caller.ts imports and calls
    let result = client
        .tools_call(
            "incoming_calls",
            json!({
                "file_path": index,
                "line": 21,
                "column": 10,
                "json": true,
            }),
        )
        .await
        .unwrap();

    let arr = extract_results(&result);
    assert!(
        !arr.is_empty(),
        "expected at least one incoming call to greetEntity"
    );
    assert!(
        arr.iter().any(|c| c["file_path"]
            .as_str()
            .is_some_and(|f| f.ends_with("caller.ts"))),
        "expected a caller of greetEntity in caller.ts, got: {arr:?}"
    );

    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_outgoing_calls() {
    let mut client = spawn_client().await;
    let caller = fixture_caller().to_str().unwrap().to_string();

    // Outgoing calls from `useGreeting` in caller.ts (line 3, col 17)
    let result = client
        .tools_call(
            "outgoing_calls",
            json!({
                "file_path": caller,
                "line": 3,
                "column": 17,
                "json": true,
            }),
        )
        .await
        .unwrap();

    let arr = extract_results(&result);
    assert!(
        !arr.is_empty(),
        "expected at least one outgoing call from useGreeting"
    );

    client.shutdown().await.unwrap();
}

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

#[tokio::test]
async fn test_goto_definition_first_call_cross_file() {
    let mut client = spawn_cold_client().await;
    let caller = fixture_caller().to_str().unwrap().to_string();

    // First tools/call of the session: goto_definition on the `greetEntity`
    // call in caller.ts (line 4, col 10), which is defined in index.ts:21.
    let result = client
        .tools_call(
            "goto_definition",
            json!({
                "file_path": caller,
                "line": 4,
                "column": 10,
                "json": true,
            }),
        )
        .await
        .unwrap();

    let arr = extract_results(&result);
    assert!(!arr.is_empty(), "expected at least one definition location");
    let file_path = arr[0]["file_path"].as_str().unwrap();
    assert!(
        file_path.ends_with("index.ts"),
        "first goto_definition should resolve into index.ts, got: {arr:?}"
    );
    assert_eq!(
        arr[0]["line"].as_u64(),
        Some(21),
        "first goto_definition should point to greetEntity at index.ts:21, got: {arr:?}"
    );

    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_find_references_first_call_cross_file() {
    let mut client = spawn_cold_client().await;
    let caller = fixture_caller().to_str().unwrap().to_string();

    // First tools/call of the session: find_references on the `greetEntity`
    // call in caller.ts (line 4, col 10).
    let result = client
        .tools_call(
            "find_references",
            json!({
                "file_path": caller,
                "line": 4,
                "column": 10,
                "json": true,
            }),
        )
        .await
        .unwrap();

    let arr = extract_results(&result);
    assert!(
        arr.len() >= 5,
        "expected at least 5 references to greetEntity across both files, got {}: {arr:?}",
        arr.len()
    );
    assert!(
        arr.iter().any(|r| r["file_path"]
            .as_str()
            .is_some_and(|f| f.ends_with("index.ts"))),
        "expected a reference to greetEntity in index.ts, got: {arr:?}"
    );
    assert!(
        arr.iter().any(|r| r["file_path"]
            .as_str()
            .is_some_and(|f| f.ends_with("caller.ts"))),
        "expected a reference to greetEntity in caller.ts, got: {arr:?}"
    );

    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_hover_first_call_cross_file() {
    let mut client = spawn_cold_client().await;
    let caller = fixture_caller().to_str().unwrap().to_string();

    // First tools/call of the session: hover on the `greetEntity` call in
    // caller.ts (line 4, col 10). A syntax-only answer shows just the import
    // (`import greetEntity`) instead of the function signature from index.ts.
    let result = client
        .tools_call(
            "hover",
            json!({
                "file_path": caller,
                "line": 4,
                "column": 10,
            }),
        )
        .await
        .unwrap();

    let text = extract_text(&result);
    assert!(
        text.contains("greetEntity"),
        "first hover should mention greetEntity, got: {text}"
    );
    assert!(
        text.contains("Greetable") || text.contains("string"),
        "first hover should show greetEntity's type signature, got: {text}"
    );
    assert_ne!(
        text.trim(),
        "import greetEntity",
        "first hover should not be the bare import form"
    );

    client.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_rename_first_call_cross_file() {
    let mut client = spawn_cold_client().await;
    let index = fixture_index().to_str().unwrap().to_string();

    // First tools/call of the session: rename (preview only, no file
    // mutation) on `greetEntity` in the `export { greetEntity, User }`
    // specifier in index.ts (line 30, col 10). Not the call in caller.ts or
    // the declaration at index.ts:21: from those, tsserver keeps the other
    // side's name with an alias (`greetEntity as greetEntity2`) and edits a
    // single file, even on a warm server. From the export specifier, the
    // edit set reaches caller.ts only if the semantic project has loaded it.
    let result = client
        .tools_call(
            "rename",
            json!({
                "file_path": index,
                "line": 30,
                "column": 10,
                "new_name": "greetEntity2",
                "json": true,
            }),
        )
        .await
        .unwrap();

    let data = extract_json(&result);
    let changes = data["changes"]
        .as_object()
        .unwrap_or_else(|| panic!("expected {{changes: {{...}}}} in rename output, got: {data}"));
    for file in ["caller.ts", "index.ts"] {
        assert!(
            changes.keys().any(|path| path.ends_with(file)),
            "first rename should edit {file}, got: {data}"
        );
    }

    client.shutdown().await.unwrap();
}
