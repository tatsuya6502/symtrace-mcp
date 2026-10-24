//! Surgical edits to Claude Code's JSON files (`settings.json`,
//! `~/.claude.json`, `.mcp.json`).
//!
//! Files are parsed into `serde_json::Value` (with `preserve_order`, so key
//! order survives a round trip), mutated only at our own paths, and written
//! back atomically. The mutation helpers are pure functions over `Value`.

use std::path::Path;

use serde_json::{Map, Value, json};

use super::SetupError;
use super::atomic_write::write_atomic;

/// The command Claude Code runs for our hook. An entry is recognised as
/// ours (on re-run and uninstall) when its command ends with this text and
/// the binary name is preceded by nothing, a `/` or whitespace, so a user
/// who prefixed an env var or wrote the absolute path keeps one entry.
pub const HOOK_COMMAND: &str = "symtrace-mcp hook";
/// The MCP server name under `mcpServers`.
pub const MCP_NAME: &str = "symtrace-mcp";
/// Matchers that get a `PreToolUse` entry.
pub const HOOK_MATCHERS: [&str; 2] = ["Bash", "Grep"];

/// Load a JSON object. A missing, empty or whitespace-only file yields `{}`;
/// the returned flag is `true` in exactly that case.
pub fn load(path: &Path) -> Result<(Value, bool), SetupError> {
    let Some(text) = super::read_optional(path)?.filter(|t| !t.trim().is_empty()) else {
        return Ok((json!({}), true));
    };
    let value: Value = serde_json::from_str(&text)
        .map_err(|e| SetupError::JsonParse(path.to_path_buf(), e.to_string()))?;
    if !value.is_object() {
        return Err(SetupError::JsonParse(
            path.to_path_buf(),
            "top-level value is not an object".into(),
        ));
    }
    Ok((value, false))
}

/// Write `value` pretty-printed (two-space indent, trailing newline) to
/// `path` atomically (see `atomic_write`). An existing target keeps its
/// mode; a new file is created `0o600`, since `~/.claude.json` also holds
/// OAuth state.
pub fn save(path: &Path, value: &Value) -> Result<(), SetupError> {
    let mut text = serde_json::to_string_pretty(value)
        .map_err(|e| SetupError::JsonParse(path.to_path_buf(), e.to_string()))?;
    text.push('\n');
    write_atomic(path, text.as_bytes(), 0o600)
}

/// Result of `upsert_hook`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookChange {
    Changed,
    Unchanged,
}

fn our_hook_object() -> Value {
    json!({ "type": "command", "command": HOOK_COMMAND })
}

fn is_our_hook(v: &Value) -> bool {
    v.get("command")
        .and_then(Value::as_str)
        .is_some_and(is_our_hook_command)
}

/// `symtrace-mcp hook`, `/abs/path/symtrace-mcp hook`,
/// `VAR=x symtrace-mcp hook`; not `my-symtrace-mcp hook` or
/// `symtrace-mcp hook --flag`.
fn is_our_hook_command(command: &str) -> bool {
    let Some(prefix) = command.trim().strip_suffix(HOOK_COMMAND) else {
        return false;
    };
    prefix.is_empty() || prefix.ends_with('/') || prefix.ends_with(char::is_whitespace)
}

/// The binary an `is_our_hook_command` command runs: the last word before
/// ` hook` (`VAR=x /p/symtrace-mcp hook` gives `/p/symtrace-mcp`).
fn hook_binary(command: &str) -> Option<&str> {
    command
        .trim()
        .strip_suffix(" hook")?
        .split_whitespace()
        .next_back()
}

/// The binary of every hook object of ours in `hooks.PreToolUse`.
pub fn hook_binaries(root: &Value) -> Result<Vec<&str>, SetupError> {
    let Some(entries) = pre_tool_use(root)? else {
        return Ok(Vec::new());
    };
    Ok(entries
        .iter()
        .filter_map(|e| e.get("hooks").and_then(Value::as_array))
        .flatten()
        .filter(|h| is_our_hook(h))
        .filter_map(|h| h.get("command").and_then(Value::as_str))
        .filter_map(hook_binary)
        .collect())
}

fn entry_matcher(entry: &Value) -> Option<&str> {
    entry.get("matcher").and_then(Value::as_str)
}

/// Borrow `hooks.PreToolUse` as an array, creating it when `create` is set.
/// Errors when an intermediate value has the wrong type.
fn pre_tool_use_mut(root: &mut Value, create: bool) -> Result<Option<&mut Vec<Value>>, SetupError> {
    let obj = root
        .as_object_mut()
        .ok_or_else(|| SetupError::JsonShape("top-level value is not an object".into()))?;
    if !obj.contains_key("hooks") {
        if !create {
            return Ok(None);
        }
        obj.insert("hooks".into(), Value::Object(Map::new()));
    }
    let hooks = obj
        .get_mut("hooks")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| SetupError::JsonShape("`hooks` is not an object".into()))?;
    if !hooks.contains_key("PreToolUse") {
        if !create {
            return Ok(None);
        }
        hooks.insert("PreToolUse".into(), Value::Array(Vec::new()));
    }
    hooks
        .get_mut("PreToolUse")
        .and_then(Value::as_array_mut)
        .map(Some)
        .ok_or_else(|| SetupError::JsonShape("`hooks.PreToolUse` is not an array".into()))
}

fn pre_tool_use(root: &Value) -> Result<Option<&Vec<Value>>, SetupError> {
    let Some(hooks) = root.get("hooks") else {
        return Ok(None);
    };
    let hooks = hooks
        .as_object()
        .ok_or_else(|| SetupError::JsonShape("`hooks` is not an object".into()))?;
    let Some(pre) = hooks.get("PreToolUse") else {
        return Ok(None);
    };
    pre.as_array()
        .map(Some)
        .ok_or_else(|| SetupError::JsonShape("`hooks.PreToolUse` is not an array".into()))
}

/// Does `hooks.PreToolUse` already contain our hook for `matcher`?
pub fn has_hook(root: &Value, matcher: &str) -> Result<bool, SetupError> {
    let Some(entries) = pre_tool_use(root)? else {
        return Ok(false);
    };
    Ok(entries.iter().any(|e| {
        entry_matcher(e) == Some(matcher)
            && e.get("hooks")
                .and_then(Value::as_array)
                .is_some_and(|hs| hs.iter().any(is_our_hook))
    }))
}

/// Ensure `hooks.PreToolUse` has an entry
/// `{"matcher": M, "hooks": [{"type": "command", "command": "symtrace-mcp hook"}]}`.
/// A matcher entry owned by another tool is never merged into; a new entry
/// is appended instead. An entry already recognised as ours is left as
/// written, even when the user customised its command text.
pub fn upsert_hook(root: &mut Value, matcher: &str) -> Result<HookChange, SetupError> {
    if has_hook(root, matcher)? {
        return Ok(HookChange::Unchanged);
    }
    let entries = pre_tool_use_mut(root, true)?.expect("created above");
    entries.push(json!({ "matcher": matcher, "hooks": [our_hook_object()] }));
    Ok(HookChange::Changed)
}

/// Remove every hook object whose command is ours. A `PreToolUse` entry is
/// dropped only when its `hooks` array becomes empty; `PreToolUse` and
/// `hooks` are dropped only when they become empty (with `shift_remove`,
/// so the remaining keys keep their order). Returns whether anything was
/// removed.
pub fn remove_hooks(root: &mut Value) -> Result<bool, SetupError> {
    let Some(entries) = pre_tool_use_mut(root, false)? else {
        return Ok(false);
    };
    let mut removed = false;
    entries.retain_mut(|entry| {
        let Some(hs) = entry.get_mut("hooks").and_then(Value::as_array_mut) else {
            return true;
        };
        let before = hs.len();
        hs.retain(|h| !is_our_hook(h));
        if hs.len() != before {
            removed = true;
        }
        !(before > 0 && hs.is_empty())
    });
    if !removed {
        return Ok(false);
    }
    // Prune empty containers.
    let obj = root.as_object_mut().expect("checked above");
    if let Some(hooks) = obj.get_mut("hooks").and_then(Value::as_object_mut) {
        if hooks
            .get("PreToolUse")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty)
        {
            hooks.shift_remove("PreToolUse");
        }
        if hooks.is_empty() {
            obj.shift_remove("hooks");
        }
    }
    Ok(true)
}

/// The entry `setup` installs under `mcpServers.symtrace-mcp`.
pub fn mcp_entry() -> Value {
    json!({ "command": "symtrace-mcp", "args": [] })
}

/// Whether an existing entry is equivalent to ours. `claude mcp add` writes
/// `{"type":"stdio","command":…,"args":[],"env":{}}`, so an explicit stdio
/// type and an empty env are accepted as the same registration, and the
/// command may be the bare name or an absolute path to it (as `claude mcp
/// add -- /home/u/.cargo/bin/symtrace-mcp` writes); anything else (other
/// binary, arguments, env vars, extra keys) counts as modified.
fn is_equivalent_mcp_entry(v: &Value) -> bool {
    let Some(obj) = v.as_object() else {
        return false;
    };
    obj.iter().all(|(k, v)| match k.as_str() {
        "command" => v.as_str().is_some_and(is_our_binary),
        "args" => v.as_array().is_some_and(Vec::is_empty),
        "type" => v == "stdio",
        "env" => v.as_object().is_some_and(Map::is_empty),
        _ => false,
    }) && obj.get("command").is_some()
}

/// `symtrace-mcp` itself or a path whose final component is `symtrace-mcp`.
fn is_our_binary(command: &str) -> bool {
    Path::new(command).file_name() == Some(std::ffi::OsStr::new(MCP_NAME))
}

/// State of `mcpServers.symtrace-mcp`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpState {
    Absent,
    Current,
    Modified,
}

/// Result of `upsert_mcp`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpChange {
    Created,
    Unchanged,
    /// Differs from ours and `force` was not set; left as is.
    Modified,
    Updated,
}

fn mcp_servers(root: &Value) -> Result<Option<&Map<String, Value>>, SetupError> {
    match root.get("mcpServers") {
        None => Ok(None),
        Some(v) => v
            .as_object()
            .map(Some)
            .ok_or_else(|| SetupError::JsonShape("`mcpServers` is not an object".into())),
    }
}

fn mcp_servers_mut(
    root: &mut Value,
    create: bool,
) -> Result<Option<&mut Map<String, Value>>, SetupError> {
    let obj = root
        .as_object_mut()
        .ok_or_else(|| SetupError::JsonShape("top-level value is not an object".into()))?;
    if !obj.contains_key("mcpServers") {
        if !create {
            return Ok(None);
        }
        obj.insert("mcpServers".into(), Value::Object(Map::new()));
    }
    obj.get_mut("mcpServers")
        .and_then(Value::as_object_mut)
        .map(Some)
        .ok_or_else(|| SetupError::JsonShape("`mcpServers` is not an object".into()))
}

pub fn mcp_state(root: &Value) -> Result<McpState, SetupError> {
    let Some(servers) = mcp_servers(root)? else {
        return Ok(McpState::Absent);
    };
    Ok(match servers.get(MCP_NAME) {
        None => McpState::Absent,
        Some(v) if is_equivalent_mcp_entry(v) => McpState::Current,
        Some(_) => McpState::Modified,
    })
}

/// The `command` of `mcpServers.symtrace-mcp`, if any.
pub fn mcp_command(root: &Value) -> Result<Option<&str>, SetupError> {
    Ok(mcp_servers(root)?
        .and_then(|s| s.get(MCP_NAME))
        .and_then(|v| v.get("command"))
        .and_then(Value::as_str))
}

/// Ensure `mcpServers.symtrace-mcp == {"command": "symtrace-mcp", "args": []}`.
pub fn upsert_mcp(root: &mut Value, force: bool) -> Result<McpChange, SetupError> {
    let state = mcp_state(root)?;
    let change = match state {
        McpState::Current => return Ok(McpChange::Unchanged),
        McpState::Modified if !force => return Ok(McpChange::Modified),
        McpState::Modified => McpChange::Updated,
        McpState::Absent => McpChange::Created,
    };
    let servers = mcp_servers_mut(root, true)?.expect("created above");
    servers.insert(MCP_NAME.into(), mcp_entry());
    Ok(change)
}

/// Remove `mcpServers.symtrace-mcp`; drop `mcpServers` when it becomes
/// empty. Returns whether the entry existed.
pub fn remove_mcp(root: &mut Value) -> Result<bool, SetupError> {
    let Some(servers) = mcp_servers_mut(root, false)? else {
        return Ok(false);
    };
    if servers.shift_remove(MCP_NAME).is_none() {
        return Ok(false);
    }
    if servers.is_empty() {
        root.as_object_mut()
            .expect("checked above")
            .shift_remove("mcpServers");
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn settings_with_read_hook() -> Value {
        json!({
            "statusLine": { "type": "command", "command": "my-status" },
            "hooks": {
                "PreToolUse": [
                    { "matcher": "Read", "hooks": [ { "type": "command", "command": "other-tool" } ] }
                ]
            }
        })
    }

    #[test]
    fn upsert_hook_preserves_unrelated_and_adds_both() {
        let mut v = settings_with_read_hook();
        assert_eq!(upsert_hook(&mut v, "Bash").unwrap(), HookChange::Changed);
        assert_eq!(upsert_hook(&mut v, "Grep").unwrap(), HookChange::Changed);
        assert_eq!(v["statusLine"]["command"], "my-status");
        let pre = v["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre.len(), 3);
        assert_eq!(pre[0]["matcher"], "Read");
        assert_eq!(pre[0]["hooks"][0]["command"], "other-tool");
        assert_eq!(
            pre[1],
            json!({ "matcher": "Bash", "hooks": [ { "type": "command", "command": HOOK_COMMAND } ] })
        );
        assert_eq!(pre[2]["matcher"], "Grep");
        // Key order preserved: statusLine first.
        let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["statusLine", "hooks"]);
    }

    #[test]
    fn upsert_hook_no_duplicate() {
        let mut v = json!({});
        assert_eq!(upsert_hook(&mut v, "Bash").unwrap(), HookChange::Changed);
        assert_eq!(upsert_hook(&mut v, "Bash").unwrap(), HookChange::Unchanged);
        assert_eq!(v["hooks"]["PreToolUse"].as_array().unwrap().len(), 1);
        assert!(has_hook(&v, "Bash").unwrap());
        assert!(!has_hook(&v, "Grep").unwrap());
    }

    #[test]
    fn upsert_hook_does_not_merge_into_other_tools_bash_entry() {
        let mut v = json!({ "hooks": { "PreToolUse": [
            { "matcher": "Bash", "hooks": [ { "type": "command", "command": "other" } ] }
        ] } });
        assert_eq!(upsert_hook(&mut v, "Bash").unwrap(), HookChange::Changed);
        let pre = v["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre.len(), 2);
        assert_eq!(pre[0]["hooks"].as_array().unwrap().len(), 1);
        assert_eq!(pre[1]["hooks"][0]["command"], HOOK_COMMAND);
    }

    #[test]
    fn upsert_hook_rejects_bad_shapes() {
        for bad in [
            json!({ "hooks": [] }),
            json!({ "hooks": "x" }),
            json!({ "hooks": { "PreToolUse": {} } }),
        ] {
            let mut v = bad.clone();
            assert!(upsert_hook(&mut v, "Bash").is_err(), "{bad}");
            assert_eq!(v, bad, "must not mutate on error");
            assert!(has_hook(&bad, "Bash").is_err());
        }
        // Entry without a matcher (match-all) is tolerated.
        let mut v = json!({ "hooks": { "PreToolUse": [ { "hooks": [ { "type": "command", "command": "x" } ] } ] } });
        assert_eq!(upsert_hook(&mut v, "Bash").unwrap(), HookChange::Changed);
    }

    #[test]
    fn remove_hooks_drops_only_empty_entries_and_containers() {
        let mut v = settings_with_read_hook();
        upsert_hook(&mut v, "Bash").unwrap();
        upsert_hook(&mut v, "Grep").unwrap();
        assert!(remove_hooks(&mut v).unwrap());
        assert_eq!(v, settings_with_read_hook());
        assert!(!remove_hooks(&mut v).unwrap());

        // Fully ours: containers pruned, other keys kept.
        let mut v = json!({ "a": 1 });
        upsert_hook(&mut v, "Bash").unwrap();
        assert!(remove_hooks(&mut v).unwrap());
        assert_eq!(v, json!({ "a": 1 }));

        // User merged our hook into a shared entry: keep the entry.
        let mut v = json!({ "hooks": { "PreToolUse": [
            { "matcher": "Bash", "hooks": [
                { "type": "command", "command": "other" },
                { "type": "command", "command": HOOK_COMMAND }
            ] }
        ], "PostToolUse": [] } });
        assert!(remove_hooks(&mut v).unwrap());
        assert_eq!(
            v,
            json!({ "hooks": { "PreToolUse": [
            { "matcher": "Bash", "hooks": [ { "type": "command", "command": "other" } ] }
        ], "PostToolUse": [] } })
        );
    }

    #[test]
    fn mcp_upsert_states() {
        let mut v = json!({ "oauth": { "token": "x" }, "projects": {} });
        assert_eq!(mcp_state(&v).unwrap(), McpState::Absent);
        assert_eq!(upsert_mcp(&mut v, false).unwrap(), McpChange::Created);
        assert_eq!(v["oauth"]["token"], "x");
        assert_eq!(v["mcpServers"][MCP_NAME], mcp_entry());
        assert_eq!(upsert_mcp(&mut v, false).unwrap(), McpChange::Unchanged);
        assert_eq!(mcp_state(&v).unwrap(), McpState::Current);

        v["mcpServers"][MCP_NAME]["env"] = json!({ "RUST_LOG": "debug" });
        assert_eq!(mcp_state(&v).unwrap(), McpState::Modified);
        assert_eq!(upsert_mcp(&mut v, false).unwrap(), McpChange::Modified);
        assert!(v["mcpServers"][MCP_NAME].get("env").is_some(), "left as is");
        assert_eq!(upsert_mcp(&mut v, true).unwrap(), McpChange::Updated);
        assert_eq!(v["mcpServers"][MCP_NAME], mcp_entry());

        let bad = json!({ "mcpServers": 3 });
        assert!(mcp_state(&bad).is_err());
        assert!(upsert_mcp(&mut bad.clone(), false).is_err());
    }

    #[test]
    fn mcp_entry_written_by_claude_mcp_add_is_current() {
        // `claude mcp add --scope user symtrace-mcp -- symtrace-mcp` writes this shape.
        let mut v = json!({ "mcpServers": { MCP_NAME: {
            "type": "stdio", "command": "symtrace-mcp", "args": [], "env": {}
        } } });
        assert_eq!(mcp_state(&v).unwrap(), McpState::Current);
        assert_eq!(upsert_mcp(&mut v, false).unwrap(), McpChange::Unchanged);
        assert_eq!(
            v["mcpServers"][MCP_NAME]["type"], "stdio",
            "left as written"
        );

        for modified in [
            json!({ "type": "sse", "command": "symtrace-mcp", "args": [] }),
            json!({ "command": "symtrace-mcp", "args": ["--foo"] }),
            json!({ "command": "/opt/other-binary", "args": [] }),
            json!({ "command": "my-symtrace-mcp", "args": [] }),
            json!({ "command": "symtrace-mcp", "args": [], "env": { "A": "1" } }),
            json!({ "command": "symtrace-mcp", "args": [], "cwd": "/x" }),
            json!({ "args": [] }),
        ] {
            let v = json!({ "mcpServers": { MCP_NAME: modified } });
            assert_eq!(mcp_state(&v).unwrap(), McpState::Modified, "{v}");
        }
    }

    #[test]
    fn mcp_entry_with_absolute_path_is_current() {
        // `claude mcp add -- /home/u/.cargo/bin/symtrace-mcp` writes the path.
        for cmd in [
            "/home/u/.cargo/bin/symtrace-mcp",
            "symtrace-mcp",
            "./target/debug/symtrace-mcp",
        ] {
            let entry = json!({ "type": "stdio", "command": cmd, "args": [], "env": {} });
            let mut v = json!({ "mcpServers": { MCP_NAME: entry.clone() } });
            assert_eq!(mcp_state(&v).unwrap(), McpState::Current, "{cmd}");
            assert_eq!(upsert_mcp(&mut v, false).unwrap(), McpChange::Unchanged);
            // `--force` must not downgrade the path to the bare name.
            assert_eq!(upsert_mcp(&mut v, true).unwrap(), McpChange::Unchanged);
            assert_eq!(v["mcpServers"][MCP_NAME], entry, "{cmd}");
        }
        let v = json!({ "mcpServers": { MCP_NAME: { "command": "/opt/other-binary" } } });
        assert_eq!(mcp_state(&v).unwrap(), McpState::Modified);
    }

    #[test]
    fn remove_mcp_keeps_key_order() {
        // `Map::remove` is `swap_remove` under preserve_order; compare the
        // serialised text, since `Value` equality ignores order.
        let mut v = json!({ "mcpServers": {
            "other": { "command": "o" }, MCP_NAME: mcp_entry(), "y": {}, "zzz": {}
        } });
        assert!(remove_mcp(&mut v).unwrap());
        assert_eq!(
            serde_json::to_string(&v).unwrap(),
            r#"{"mcpServers":{"other":{"command":"o"},"y":{},"zzz":{}}}"#
        );
        // Dropping an emptied `mcpServers` keeps the surrounding order too.
        let mut v =
            json!({ "first": 1, "mcpServers": { MCP_NAME: mcp_entry() }, "mid": 2, "last": 3 });
        assert!(remove_mcp(&mut v).unwrap());
        assert_eq!(
            serde_json::to_string(&v).unwrap(),
            r#"{"first":1,"mid":2,"last":3}"#
        );
    }

    #[test]
    fn remove_hooks_keeps_key_order() {
        let mut v = json!({ "first": 1, "hooks": {}, "mid": 2, "last": 3 });
        upsert_hook(&mut v, "Bash").unwrap();
        upsert_hook(&mut v, "Grep").unwrap();
        assert!(remove_hooks(&mut v).unwrap());
        assert_eq!(
            serde_json::to_string(&v).unwrap(),
            r#"{"first":1,"mid":2,"last":3}"#
        );
        // `PreToolUse` pruned inside a `hooks` that keeps other events.
        let mut v = json!({ "hooks": { "Stop": [], "PreToolUse": [], "PostToolUse": [] } });
        upsert_hook(&mut v, "Bash").unwrap();
        assert!(remove_hooks(&mut v).unwrap());
        assert_eq!(
            serde_json::to_string(&v).unwrap(),
            r#"{"hooks":{"Stop":[],"PostToolUse":[]}}"#
        );
    }

    #[test]
    fn customised_hook_command_is_still_ours() {
        for cmd in [
            "SYMTRACE_HOOK_SRC_DIRS=src symtrace-mcp hook",
            "/home/u/.cargo/bin/symtrace-mcp hook",
            "  symtrace-mcp hook\n",
        ] {
            let mut v = json!({ "hooks": { "PreToolUse": [
                { "matcher": "Bash", "hooks": [ { "type": "command", "command": cmd } ] }
            ] } });
            let before = v.clone();
            assert!(has_hook(&v, "Bash").unwrap(), "{cmd:?}");
            assert_eq!(
                upsert_hook(&mut v, "Bash").unwrap(),
                HookChange::Unchanged,
                "{cmd:?}"
            );
            assert_eq!(v, before, "customisation kept as written: {cmd:?}");
            assert!(remove_hooks(&mut v).unwrap(), "{cmd:?}");
            assert_eq!(v, json!({}), "{cmd:?}");
        }
        for cmd in [
            "my-symtrace-mcp hook",
            "symtrace-mcp hook --flag",
            "symtrace-mcp",
            "hook",
        ] {
            assert!(!is_our_hook_command(cmd), "{cmd:?}");
            let v = json!({ "hooks": { "PreToolUse": [
                { "matcher": "Bash", "hooks": [ { "type": "command", "command": cmd } ] }
            ] } });
            assert!(!has_hook(&v, "Bash").unwrap(), "{cmd:?}");
        }
    }

    #[test]
    fn mcp_remove() {
        let mut v = json!({ "mcpServers": { "other": { "command": "o" } } });
        upsert_mcp(&mut v, false).unwrap();
        assert!(remove_mcp(&mut v).unwrap());
        assert_eq!(v, json!({ "mcpServers": { "other": { "command": "o" } } }));
        assert!(!remove_mcp(&mut v).unwrap());
        let mut v = json!({ "k": 1 });
        upsert_mcp(&mut v, false).unwrap();
        remove_mcp(&mut v).unwrap();
        assert_eq!(v, json!({ "k": 1 }));
    }

    #[test]
    fn load_and_save_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("settings.json");
        assert_eq!(load(&path).unwrap(), (json!({}), true));
        let v = json!({ "z": 1, "a": { "y": [1, 2], "b": null } });
        save(&path, &v).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert_eq!(
            text,
            "{\n  \"z\": 1,\n  \"a\": {\n    \"y\": [\n      1,\n      2\n    ],\n    \"b\": null\n  }\n}\n"
        );
        assert_eq!(load(&path).unwrap(), (v, false));
        // No leftover temp file.
        let names: Vec<_> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["settings.json"]);
    }

    #[test]
    fn load_edge_cases() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.json");
        fs::write(&path, "  \n").unwrap();
        assert_eq!(load(&path).unwrap(), (json!({}), true));
        fs::write(&path, "{ not json").unwrap();
        let err = load(&path).unwrap_err().to_string();
        assert!(err.contains("s.json"), "{err}");
        fs::write(&path, "[1]").unwrap();
        assert!(load(&path).is_err());
    }

    #[test]
    fn missing_or_blank_detection() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.json");
        let blank = |p: &Path| load(p).unwrap().1;
        assert!(blank(&path));
        fs::write(&path, "").unwrap();
        assert!(blank(&path));
        fs::write(&path, " \r\n\t\n").unwrap();
        assert!(blank(&path));
        fs::write(&path, "{}").unwrap();
        assert!(!blank(&path));
    }

    #[test]
    fn hook_binaries_and_mcp_command() {
        let v = json!({
            "hooks": { "PreToolUse": [
                { "matcher": "Bash", "hooks": [
                    { "type": "command", "command": "other" },
                    { "type": "command", "command": "VAR=x /opt/bin/symtrace-mcp hook" }
                ] },
                { "matcher": "Grep", "hooks": [ { "type": "command", "command": HOOK_COMMAND } ] }
            ] },
            "mcpServers": { MCP_NAME: { "command": "/opt/bin/symtrace-mcp" } }
        });
        assert_eq!(
            hook_binaries(&v).unwrap(),
            ["/opt/bin/symtrace-mcp", "symtrace-mcp"]
        );
        assert_eq!(mcp_command(&v).unwrap(), Some("/opt/bin/symtrace-mcp"));
        assert!(hook_binaries(&json!({})).unwrap().is_empty());
        assert_eq!(mcp_command(&json!({})).unwrap(), None);
    }

    #[cfg(unix)]
    #[test]
    fn save_follows_symlinked_target() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("dotfiles").join("claude.json");
        fs::create_dir_all(real.parent().unwrap()).unwrap();
        fs::write(&real, "{}").unwrap();
        let link = dir.path().join("claude.json");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        save(&link, &json!({ "a": 1 })).unwrap();
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(load(&real).unwrap().0, json!({ "a": 1 }));
    }

    #[cfg(unix)]
    #[test]
    fn save_preserves_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("claude.json");
        fs::write(&path, "{}").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        save(&path, &json!({ "a": 1 })).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);

        // An existing world-readable settings.json stays that way.
        let path = dir.path().join("settings.json");
        fs::write(&path, "{}").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        save(&path, &json!({ "a": 1 })).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o644);
    }

    #[cfg(unix)]
    #[test]
    fn save_creates_new_file_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".claude.json");
        save(&path, &json!({ "mcpServers": {} })).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[cfg(unix)]
    #[test]
    fn save_into_unwritable_dir_leaves_target_untouched() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        // Root ignores directory permissions; skip there.
        if fs::metadata("/proc/self").map(|m| m.uid()).unwrap_or(1) == 0 {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.json");
        fs::write(&path, "{\"orig\":true}").unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o500)).unwrap();
        let result = save(&path, &json!({ "a": 1 }));
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
        assert!(result.is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "{\"orig\":true}");
        let names: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["s.json"]);
    }
}
