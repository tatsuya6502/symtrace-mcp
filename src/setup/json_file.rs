//! Surgical edits to Claude Code's JSON files (`settings.json`,
//! `~/.claude.json`, `.mcp.json`).
//!
//! Files are parsed into `serde_json::Value` (with `preserve_order`, so key
//! order survives a round trip), mutated only at our own paths, and written
//! back atomically. The mutation helpers are pure functions over `Value`.

use std::fs;
use std::io::Write;
use std::path::Path;

use serde_json::{Map, Value, json};

use super::SetupError;

/// The command Claude Code runs for our hook. Also how we recognise our
/// own entries on re-run and uninstall.
pub const HOOK_COMMAND: &str = "symtrace-mcp hook";
/// The MCP server name under `mcpServers`.
pub const MCP_NAME: &str = "symtrace-mcp";
/// Matchers that get a `PreToolUse` entry.
pub const HOOK_MATCHERS: [&str; 2] = ["Bash", "Grep"];

/// Load a JSON object. A missing, empty or whitespace-only file yields `{}`.
pub fn load(path: &Path) -> Result<Value, SetupError> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(json!({})),
        Err(e) => return Err(SetupError::Io(path.to_path_buf(), e)),
    };
    if text.trim().is_empty() {
        return Ok(json!({}));
    }
    let value: Value = serde_json::from_str(&text)
        .map_err(|e| SetupError::JsonParse(path.to_path_buf(), e.to_string()))?;
    if !value.is_object() {
        return Err(SetupError::JsonParse(
            path.to_path_buf(),
            "top-level value is not an object".into(),
        ));
    }
    Ok(value)
}

/// Write `value` pretty-printed (two-space indent, trailing newline) to
/// `path` atomically: a temp file in the same directory is renamed over the
/// target. Parent directories are created. The target's permissions are
/// preserved when it already exists.
pub fn save(path: &Path, value: &Value) -> Result<(), SetupError> {
    // Follow a symlinked target (dotfiles repos) so the rename replaces the
    // real file rather than the link.
    let resolved = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let path = resolved.as_path();
    let io = |e: std::io::Error| SetupError::Io(path.to_path_buf(), e);
    let dir = path.parent().filter(|p| !p.as_os_str().is_empty());
    if let Some(dir) = dir {
        fs::create_dir_all(dir).map_err(io)?;
    }
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".into());
    let tmp = path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()));
    let mut text = serde_json::to_string_pretty(value)
        .map_err(|e| SetupError::JsonParse(path.to_path_buf(), e.to_string()))?;
    text.push('\n');

    let result = (|| {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        drop(f);
        if let Ok(meta) = fs::metadata(path) {
            fs::set_permissions(&tmp, meta.permissions())?;
        }
        fs::rename(&tmp, path)
    })();
    if let Err(e) = result {
        let _ = fs::remove_file(&tmp);
        return Err(io(e));
    }
    Ok(())
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
    v.get("command").and_then(Value::as_str) == Some(HOOK_COMMAND)
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
/// is appended instead.
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
/// `hooks` are dropped only when they become empty. Returns whether
/// anything was removed.
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
            hooks.remove("PreToolUse");
        }
        if hooks.is_empty() {
            obj.remove("hooks");
        }
    }
    Ok(true)
}

/// The entry `setup` installs under `mcpServers.symtrace-mcp`.
pub fn mcp_entry() -> Value {
    json!({ "command": "symtrace-mcp", "args": [] })
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
        Some(v) if *v == mcp_entry() => McpState::Current,
        Some(_) => McpState::Modified,
    })
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
    if servers.remove(MCP_NAME).is_none() {
        return Ok(false);
    }
    if servers.is_empty() {
        root.as_object_mut()
            .expect("checked above")
            .remove("mcpServers");
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(load(&path).unwrap(), json!({}));
        let v = json!({ "z": 1, "a": { "y": [1, 2], "b": null } });
        save(&path, &v).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert_eq!(
            text,
            "{\n  \"z\": 1,\n  \"a\": {\n    \"y\": [\n      1,\n      2\n    ],\n    \"b\": null\n  }\n}\n"
        );
        assert_eq!(load(&path).unwrap(), v);
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
        assert_eq!(load(&path).unwrap(), json!({}));
        fs::write(&path, "{ not json").unwrap();
        let err = load(&path).unwrap_err().to_string();
        assert!(err.contains("s.json"), "{err}");
        fs::write(&path, "[1]").unwrap();
        assert!(load(&path).is_err());
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
        assert_eq!(load(&real).unwrap(), json!({ "a": 1 }));
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
