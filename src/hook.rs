//! `symtrace-mcp hook`: Claude Code PreToolUse hook.
//!
//! Reads one event from stdin and, when the call is a text/tree-sitter
//! heuristic for a semantic question (`sb callers`, identifier grep over
//! source code, `Grep` tool with a bare identifier), prints a JSON response
//! that lets the call proceed but attaches a reminder to use the
//! symtrace-mcp tools. See `openspec/changes/setup-command/specs/tool-reminder-hook/`.

mod decide;

use std::io::Read;

use serde_json::{Value, json};

use decide::{DEFAULT_SRC_DIRS, Detection, decide};

/// Environment variable that replaces the default source-directory list
/// (`|`-separated).
const SRC_DIRS_ENV: &str = "SYMTRACE_HOOK_SRC_DIRS";

/// Run the hook. Always returns 0: a hook failure must never block a tool call.
pub fn run() -> i32 {
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        return 0;
    }
    let override_dirs: Option<Vec<String>> = std::env::var(SRC_DIRS_ENV).ok().map(|v| {
        v.split('|')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect()
    });
    let src_dirs: Vec<&str> = match &override_dirs {
        Some(dirs) => dirs.iter().map(String::as_str).collect(),
        None => DEFAULT_SRC_DIRS.to_vec(),
    };
    if let Some(line) = respond(&input, &src_dirs) {
        println!("{line}");
    }
    0
}

/// Parse one event leniently and build the one-line hook response, or `None`
/// when nothing should be printed.
fn respond(input: &str, src_dirs: &[&str]) -> Option<String> {
    let event: Value = serde_json::from_str(input).ok()?;
    let tool_name = event.get("tool_name")?.as_str()?;
    let tool_input = event.get("tool_input").unwrap_or(&Value::Null);
    let detection = decide(tool_name, tool_input, src_dirs)?;
    let output = json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "allow",
            "additionalContext": reminder_text(&detection),
        }
    });
    Some(output.to_string())
}

fn reminder_text(detection: &Detection) -> String {
    format!(
        "symtrace-mcp hook: this call ({}) is a text/tree-sitter heuristic. \
         For call graphs, trait implementations, definitions, references or types, \
         use the symtrace-mcp tools (mcp__symtrace-mcp__incoming_calls / outgoing_calls / \
         find_implementations / goto_definition / find_references / hover; load with \
         ToolSearch(\"select:mcp__symtrace-mcp__<tool>\") if deferred). They resolve \
         through types, generics and re-exports, so prefer them for the final answer.",
        detection.label()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_json_shape() {
        let input = r#"{"tool_name":"Bash","tool_input":{"command":"grep -rn stop_server src/"}}"#;
        let line = respond(input, DEFAULT_SRC_DIRS).expect("reminder expected");
        assert!(!line.contains('\n'));
        let parsed: Value = serde_json::from_str(&line).expect("valid JSON");
        let out = &parsed["hookSpecificOutput"];
        assert_eq!(out["hookEventName"], "PreToolUse");
        assert_eq!(out["permissionDecision"], "allow");
        let ctx = out["additionalContext"].as_str().expect("string context");
        assert!(ctx.contains("grep for identifier 'stop_server'"));
        for tool in [
            "mcp__symtrace-mcp__incoming_calls",
            "outgoing_calls",
            "find_implementations",
            "goto_definition",
            "find_references",
            "hover",
            "ToolSearch(\"select:mcp__symtrace-mcp__<tool>\")",
        ] {
            assert!(ctx.contains(tool), "missing {tool}");
        }
        assert_eq!(parsed.as_object().unwrap().len(), 1);
        assert_eq!(out.as_object().unwrap().len(), 3);
    }

    #[test]
    fn sb_callers_response() {
        let input = r#"{"tool_name":"Bash","tool_input":{"command":"sb callers stop_server"}}"#;
        let line = respond(input, DEFAULT_SRC_DIRS).unwrap();
        let parsed: Value = serde_json::from_str(&line).unwrap();
        assert!(
            parsed["hookSpecificOutput"]["additionalContext"]
                .as_str()
                .unwrap()
                .contains("(sb callers)")
        );
    }

    #[test]
    fn silent_cases() {
        for input in [
            "",
            "   \n",
            "not json",
            "[1,2]",
            r#"{"tool_name":"Edit","tool_input":{"file_path":"src/main.rs"}}"#,
            r#"{"tool_name":"Bash"}"#,
            r#"{"tool_name":"Bash","tool_input":{"command":"sb map src/main.rs"}}"#,
            r#"{"tool_input":{"command":"sb callers x"}}"#,
        ] {
            assert_eq!(respond(input, DEFAULT_SRC_DIRS), None, "input: {input:?}");
        }
    }
}
