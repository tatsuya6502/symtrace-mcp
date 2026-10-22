//! Integration-style tests: a `Targets` in a tempdir, driven through `execute`.

use std::fs;
use std::path::Path;

use serde_json::{Value, json};
use tempfile::TempDir;

use super::json_file::HOOK_COMMAND;
use super::targets::Targets;
use super::{EXPLORE_FRONTMATTER, PROMPT, Report, SetupArgs, VERSION, execute};

struct Env {
    _dir: TempDir,
    home: std::path::PathBuf,
    targets: Targets,
}

fn env() -> Env {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    fs::create_dir_all(&home).unwrap();
    let targets = Targets::global(&home);
    Env {
        _dir: dir,
        home,
        targets,
    }
}

fn args() -> SetupArgs {
    SetupArgs::default()
}

fn statuses(report: &Report) -> Vec<(&'static str, &str)> {
    report
        .lines
        .iter()
        .map(|l| (l.item, l.status.as_str()))
        .collect()
}

fn all<'a>(report: &Report, status: &'a str) -> Vec<(&'static str, &'a str)> {
    report
        .lines
        .iter()
        .map(|l| (l.item, status))
        .collect::<Vec<_>>()
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap()
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&read(path)).unwrap()
}

fn block() -> String {
    super::block()
}

fn hook_commands(settings: &Value) -> Vec<(String, String)> {
    settings["hooks"]["PreToolUse"]
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .flat_map(|e| {
                    let m = e["matcher"].as_str().unwrap_or("").to_string();
                    e["hooks"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(move |h| (m.clone(), h["command"].as_str().unwrap().to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn embedded_prompt_has_all_six_tools() {
    for tool in [
        "incoming_calls",
        "outgoing_calls",
        "find_implementations",
        "goto_definition",
        "find_references",
        "hover",
    ] {
        assert!(
            PROMPT.contains(&format!("mcp__symtrace-mcp__{tool}")),
            "{tool}"
        );
    }
    assert!(!PROMPT.contains("symtrace-mcp:begin"));
    assert!(EXPLORE_FRONTMATTER.starts_with("---\nname: Explore\ndescription: "));
    assert!(EXPLORE_FRONTMATTER.trim_end().ends_with("\n---"));
}

#[test]
fn fresh_install_creates_four_items() {
    let e = env();
    let r = execute(&args(), &e.targets);
    assert_eq!(statuses(&r), all(&r, "created"));
    assert_eq!(r.exit_code, 0);
    assert_eq!(read(&e.targets.claude_md), block());
    assert_eq!(
        read(&e.targets.explore_md),
        format!("{EXPLORE_FRONTMATTER}\n{}", block())
    );
    let settings = read_json(&e.targets.settings_json);
    assert_eq!(
        hook_commands(&settings),
        [
            ("Bash".to_string(), HOOK_COMMAND.to_string()),
            ("Grep".to_string(), HOOK_COMMAND.to_string())
        ]
    );
    let mcp = read_json(&e.targets.mcp_json);
    assert_eq!(
        mcp,
        json!({ "mcpServers": { "symtrace-mcp": { "command": "symtrace-mcp", "args": [] } } })
    );
    // Output line format.
    let line = r.lines[0].to_string();
    assert_eq!(
        line,
        format!("prompt   created    {}", e.targets.claude_md.display())
    );
}

#[test]
fn rerun_is_unchanged_and_byte_identical() {
    let e = env();
    execute(&args(), &e.targets);
    let before: Vec<String> = e.paths().iter().map(|p| read(p)).collect();
    let r = execute(&args(), &e.targets);
    assert_eq!(statuses(&r), all(&r, "unchanged"));
    assert_eq!(r.exit_code, 0);
    let after: Vec<String> = e.paths().iter().map(|p| read(p)).collect();
    assert_eq!(before, after);
}

impl Env {
    fn paths(&self) -> [&Path; 4] {
        [
            &self.targets.claude_md,
            &self.targets.explore_md,
            &self.targets.settings_json,
            &self.targets.mcp_json,
        ]
    }
}

#[test]
fn outdated_block_is_updated_in_place() {
    let e = env();
    fs::create_dir_all(e.targets.claude_md.parent().unwrap()).unwrap();
    let old = super::marker_block::render("0.0.1", "old text\n");
    fs::write(&e.targets.claude_md, format!("# mine\n\n{old}\n# after\n")).unwrap();
    let r = execute(&args(), &e.targets);
    assert_eq!(statuses(&r)[0], ("prompt", "updated"));
    assert_eq!(r.exit_code, 0);
    assert_eq!(
        read(&e.targets.claude_md),
        format!("# mine\n\n{}\n# after\n", block())
    );
}

#[test]
fn modified_block_needs_force() {
    let e = env();
    execute(&args(), &e.targets);
    let edited = read(&e.targets.claude_md).replace("read-only and cheap", "EDITED");
    assert_ne!(edited, block());
    fs::write(&e.targets.claude_md, &edited).unwrap();

    let r = execute(&args(), &e.targets);
    assert_eq!(statuses(&r)[0], ("prompt", "modified (use --force)"));
    assert_eq!(statuses(&r)[1], ("explore", "unchanged"));
    assert_eq!(r.exit_code, 1);
    assert_eq!(read(&e.targets.claude_md), edited, "not rewritten");

    let r = execute(
        &SetupArgs {
            force: true,
            ..args()
        },
        &e.targets,
    );
    assert_eq!(statuses(&r)[0], ("prompt", "updated"));
    assert_eq!(r.exit_code, 0);
    assert_eq!(read(&e.targets.claude_md), block());
}

#[test]
fn unversioned_marker_is_modified() {
    let e = env();
    fs::create_dir_all(e.targets.claude_md.parent().unwrap()).unwrap();
    let text = format!("<!-- symtrace-mcp:begin -->\n{PROMPT}<!-- symtrace-mcp:end -->\n");
    fs::write(&e.targets.claude_md, &text).unwrap();
    let r = execute(&args(), &e.targets);
    assert_eq!(statuses(&r)[0], ("prompt", "modified (use --force)"));
    assert_eq!(read(&e.targets.claude_md), text);
    let r = execute(
        &SetupArgs {
            force: true,
            ..args()
        },
        &e.targets,
    );
    assert_eq!(statuses(&r)[0], ("prompt", "updated"));
    assert_eq!(read(&e.targets.claude_md), block());
}

#[test]
fn dry_run_on_fresh_writes_nothing() {
    let e = env();
    let r = execute(
        &SetupArgs {
            dry_run: true,
            ..args()
        },
        &e.targets,
    );
    assert_eq!(statuses(&r), all(&r, "created"));
    assert_eq!(r.exit_code, 0);
    for l in &r.lines {
        assert!(l.to_string().starts_with("dry-run "), "{l}");
    }
    for p in e.paths() {
        assert!(!p.exists(), "{}", p.display());
    }
    assert!(!e.home.join(".claude").exists(), "no parent dirs created");
}

#[test]
fn dry_run_uninstall_writes_nothing() {
    let e = env();
    execute(&args(), &e.targets);
    let before: Vec<String> = e.paths().iter().map(|p| read(p)).collect();
    let r = execute(
        &SetupArgs {
            dry_run: true,
            uninstall: true,
            ..args()
        },
        &e.targets,
    );
    assert_eq!(statuses(&r), all(&r, "removed"));
    let after: Vec<String> = e.paths().iter().map(|p| read(p)).collect();
    assert_eq!(before, after);
}

#[test]
fn existing_claude_md_content_is_preserved() {
    let e = env();
    fs::create_dir_all(e.targets.claude_md.parent().unwrap()).unwrap();
    let original = "# My notes\n\n<!-- ast-bro:begin v=4.2.0 -->\nuse sb\n<!-- ast-bro:end -->\n";
    fs::write(&e.targets.claude_md, original).unwrap();
    let r = execute(&args(), &e.targets);
    assert_eq!(statuses(&r)[0], ("prompt", "created"));
    assert_eq!(
        read(&e.targets.claude_md),
        format!("{original}\n{}", block())
    );
}

#[test]
fn explore_shadowed_by_other_tool_gets_appended_block() {
    let e = env();
    fs::create_dir_all(e.targets.explore_md.parent().unwrap()).unwrap();
    let original = format!(
        "{EXPLORE_FRONTMATTER}\n<!-- ast-bro:begin v=4.2.0 -->\nuse sb\n<!-- ast-bro:end -->\n"
    );
    fs::write(&e.targets.explore_md, &original).unwrap();
    let r = execute(&args(), &e.targets);
    assert_eq!(statuses(&r)[1], ("explore", "created"));
    assert_eq!(
        read(&e.targets.explore_md),
        format!("{original}\n{}", block())
    );
}

#[test]
fn settings_with_unrelated_hook_and_keys() {
    let e = env();
    fs::create_dir_all(e.targets.settings_json.parent().unwrap()).unwrap();
    let original = json!({
        "statusLine": { "type": "command", "command": "my-status" },
        "hooks": { "PreToolUse": [
            { "matcher": "Read", "hooks": [ { "type": "command", "command": "other-tool" } ] }
        ] },
        "permissions": { "allow": ["Bash(ls)"] }
    });
    fs::write(
        &e.targets.settings_json,
        serde_json::to_string(&original).unwrap(),
    )
    .unwrap();
    let r = execute(&args(), &e.targets);
    assert_eq!(statuses(&r)[2], ("hook", "created"));
    let settings = read_json(&e.targets.settings_json);
    assert_eq!(settings["statusLine"], original["statusLine"]);
    assert_eq!(settings["permissions"], original["permissions"]);
    assert_eq!(
        hook_commands(&settings),
        [
            ("Read".to_string(), "other-tool".to_string()),
            ("Bash".to_string(), HOOK_COMMAND.to_string()),
            ("Grep".to_string(), HOOK_COMMAND.to_string())
        ]
    );
    // Two-space indent, key order preserved.
    let text = read(&e.targets.settings_json);
    assert!(
        text.starts_with("{\n  \"statusLine\": {\n    \"type\""),
        "{text}"
    );
    assert!(text.ends_with("}\n"));
}

#[test]
fn existing_hook_entry_not_duplicated() {
    let e = env();
    fs::create_dir_all(e.targets.settings_json.parent().unwrap()).unwrap();
    let original = json!({ "hooks": { "PreToolUse": [
        { "matcher": "Bash", "hooks": [ { "type": "command", "command": HOOK_COMMAND } ] }
    ] } });
    fs::write(
        &e.targets.settings_json,
        serde_json::to_string(&original).unwrap(),
    )
    .unwrap();
    let r = execute(&args(), &e.targets);
    // Grep was missing, so the item is updated, with one Bash entry only.
    assert_eq!(statuses(&r)[2], ("hook", "updated"));
    let cmds = hook_commands(&read_json(&e.targets.settings_json));
    assert_eq!(cmds.iter().filter(|(m, _)| m == "Bash").count(), 1);
    assert_eq!(cmds.len(), 2);
    let r = execute(&args(), &e.targets);
    assert_eq!(statuses(&r)[2], ("hook", "unchanged"));
}

#[test]
fn invalid_settings_json_errors_only_hook_item() {
    let e = env();
    fs::create_dir_all(e.targets.settings_json.parent().unwrap()).unwrap();
    fs::write(&e.targets.settings_json, "{ broken").unwrap();
    let r = execute(&args(), &e.targets);
    let s = statuses(&r);
    assert_eq!(s[0], ("prompt", "created"));
    assert_eq!(s[1], ("explore", "created"));
    assert!(s[2].1.starts_with("error: "), "{}", s[2].1);
    assert!(s[2].1.contains("settings.json"));
    assert_eq!(s[3], ("mcp", "created"));
    assert_eq!(r.exit_code, 1);
    assert_eq!(read(&e.targets.settings_json), "{ broken");
    assert!(e.targets.mcp_json.exists());
    // Same under --dry-run: still reported.
    let r = execute(
        &SetupArgs {
            dry_run: true,
            ..args()
        },
        &e.targets,
    );
    assert!(statuses(&r)[2].1.starts_with("error: "));
}

#[test]
fn claude_json_other_keys_kept() {
    let e = env();
    let original = json!({
        "numStartups": 12,
        "oauthAccount": { "emailAddress": "x@example.com" },
        "projects": { "/p": { "allowedTools": [] } },
        "mcpServers": { "other": { "command": "other", "args": ["-x"] } }
    });
    fs::write(
        &e.targets.mcp_json,
        serde_json::to_string(&original).unwrap(),
    )
    .unwrap();
    let r = execute(&args(), &e.targets);
    assert_eq!(statuses(&r)[3], ("mcp", "created"));
    let v = read_json(&e.targets.mcp_json);
    let mut expected = original.clone();
    expected["mcpServers"]["symtrace-mcp"] = json!({ "command": "symtrace-mcp", "args": [] });
    assert_eq!(v, expected);
    let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
    assert_eq!(
        keys,
        ["numStartups", "oauthAccount", "projects", "mcpServers"]
    );
}

#[test]
fn customized_mcp_entry_is_modified_without_force() {
    let e = env();
    let original = json!({ "mcpServers": { "symtrace-mcp": {
        "command": "symtrace-mcp", "args": [], "env": { "RUST_LOG": "debug" }
    } } });
    let text = serde_json::to_string(&original).unwrap();
    fs::write(&e.targets.mcp_json, &text).unwrap();
    let r = execute(&args(), &e.targets);
    assert_eq!(statuses(&r)[3], ("mcp", "modified (use --force)"));
    assert_eq!(r.exit_code, 1);
    assert_eq!(read(&e.targets.mcp_json), text, "untouched");
    let r = execute(
        &SetupArgs {
            force: true,
            ..args()
        },
        &e.targets,
    );
    assert_eq!(statuses(&r)[3], ("mcp", "updated"));
    assert_eq!(r.exit_code, 0);
    assert_eq!(
        read_json(&e.targets.mcp_json),
        json!({ "mcpServers": { "symtrace-mcp": { "command": "symtrace-mcp", "args": [] } } })
    );
}

#[test]
fn uninstall_after_full_install() {
    let e = env();
    execute(&args(), &e.targets);
    let r = execute(
        &SetupArgs {
            uninstall: true,
            ..args()
        },
        &e.targets,
    );
    assert_eq!(statuses(&r), all(&r, "removed"));
    assert_eq!(r.exit_code, 0);
    assert_eq!(read(&e.targets.claude_md), "");
    assert!(!e.targets.explore_md.exists(), "Explore.md deleted");
    let settings = read_json(&e.targets.settings_json);
    assert!(hook_commands(&settings).is_empty());
    assert_eq!(settings, json!({}));
    assert_eq!(read_json(&e.targets.mcp_json), json!({}));
    // Second uninstall: everything absent, exit 0.
    let r = execute(
        &SetupArgs {
            uninstall: true,
            ..args()
        },
        &e.targets,
    );
    assert_eq!(statuses(&r), all(&r, "absent"));
    assert_eq!(r.exit_code, 0);
}

#[test]
fn uninstall_preserves_surrounding_content() {
    let e = env();
    fs::create_dir_all(e.targets.claude_md.parent().unwrap()).unwrap();
    fs::create_dir_all(e.targets.settings_json.parent().unwrap()).unwrap();
    fs::write(&e.targets.claude_md, "# notes\n").unwrap();
    let settings = json!({ "statusLine": 1, "hooks": { "PreToolUse": [
        { "matcher": "Read", "hooks": [ { "type": "command", "command": "other" } ] }
    ] } });
    fs::write(
        &e.targets.settings_json,
        serde_json::to_string(&settings).unwrap(),
    )
    .unwrap();
    let mcp = json!({ "a": 1, "mcpServers": { "other": {} } });
    fs::write(&e.targets.mcp_json, serde_json::to_string(&mcp).unwrap()).unwrap();
    execute(&args(), &e.targets);
    let r = execute(
        &SetupArgs {
            uninstall: true,
            ..args()
        },
        &e.targets,
    );
    assert_eq!(statuses(&r), all(&r, "removed"));
    assert_eq!(read(&e.targets.claude_md), "# notes\n");
    assert_eq!(read_json(&e.targets.settings_json), settings);
    assert_eq!(read_json(&e.targets.mcp_json), mcp);
}

#[test]
fn uninstall_with_shared_explore_md_keeps_file() {
    let e = env();
    fs::create_dir_all(e.targets.explore_md.parent().unwrap()).unwrap();
    let original = format!(
        "{EXPLORE_FRONTMATTER}\n<!-- ast-bro:begin v=4.2.0 -->\nuse sb\n<!-- ast-bro:end -->\n"
    );
    fs::write(&e.targets.explore_md, &original).unwrap();
    execute(&args(), &e.targets);
    let r = execute(
        &SetupArgs {
            uninstall: true,
            ..args()
        },
        &e.targets,
    );
    assert_eq!(statuses(&r)[1], ("explore", "removed"));
    assert_eq!(read(&e.targets.explore_md), original);
}

#[test]
fn uninstall_absent_does_not_rewrite() {
    let e = env();
    fs::create_dir_all(e.targets.claude_md.parent().unwrap()).unwrap();
    fs::write(&e.targets.claude_md, "no newline at end").unwrap();
    fs::write(&e.targets.settings_json, "{\"a\":1}").unwrap();
    let r = execute(
        &SetupArgs {
            uninstall: true,
            ..args()
        },
        &e.targets,
    );
    assert_eq!(statuses(&r), all(&r, "absent"));
    assert_eq!(read(&e.targets.claude_md), "no newline at end");
    assert_eq!(read(&e.targets.settings_json), "{\"a\":1}");
}

#[test]
fn status_on_partial_install() {
    let e = env();
    execute(
        &SetupArgs {
            skip_hook: true,
            skip_mcp: true,
            skip_explore: true,
            ..args()
        },
        &e.targets,
    );
    let r = execute(
        &SetupArgs {
            status: true,
            ..args()
        },
        &e.targets,
    );
    assert_eq!(
        statuses(&r),
        [
            ("prompt", format!("installed v{VERSION}").as_str()),
            ("explore", "absent"),
            ("hook", "absent"),
            ("mcp", "absent")
        ]
    );
    assert_eq!(r.exit_code, 0);
    assert!(!e.targets.settings_json.exists());

    // Outdated / modified / full install.
    let old = super::marker_block::render("0.0.1", "old\n");
    fs::write(&e.targets.claude_md, &old).unwrap();
    execute(
        &SetupArgs {
            skip_prompt: true,
            ..args()
        },
        &e.targets,
    );
    let mut mcp = read_json(&e.targets.mcp_json);
    mcp["mcpServers"]["symtrace-mcp"]["args"] = json!(["--x"]);
    fs::write(&e.targets.mcp_json, serde_json::to_string(&mcp).unwrap()).unwrap();
    let r = execute(
        &SetupArgs {
            status: true,
            skip_hook: true,
            ..args()
        },
        &e.targets,
    );
    assert_eq!(
        statuses(&r),
        [
            ("prompt", "outdated v0.0.1"),
            ("explore", format!("installed v{VERSION}").as_str()),
            ("hook", "skipped"),
            ("mcp", "modified")
        ]
    );
    assert_eq!(r.exit_code, 0, "status never fails");
    assert_eq!(read(&e.targets.claude_md), old, "status never writes");
}

#[test]
fn skip_mcp_leaves_claude_json_alone() {
    let e = env();
    fs::write(&e.targets.mcp_json, "not even json").unwrap();
    let r = execute(
        &SetupArgs {
            skip_mcp: true,
            ..args()
        },
        &e.targets,
    );
    assert_eq!(statuses(&r)[3], ("mcp", "skipped"));
    assert_eq!(r.exit_code, 0);
    assert_eq!(read(&e.targets.mcp_json), "not even json");
}

#[test]
fn local_scope_writes_under_cwd_only() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let cwd = dir.path().join("proj");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&cwd).unwrap();
    let targets = Targets::local(&cwd);
    let r = execute(&args(), &targets);
    assert_eq!(statuses(&r), all(&r, "created"));
    assert!(cwd.join("CLAUDE.md").exists());
    assert!(cwd.join(".claude/agents/Explore.md").exists());
    assert!(cwd.join(".claude/settings.json").exists());
    assert!(cwd.join(".mcp.json").exists());
    assert_eq!(
        fs::read_dir(&home).unwrap().count(),
        0,
        "nothing under home"
    );
}

#[test]
fn run_resolves_local_from_cwd() {
    let dir = tempfile::tempdir().unwrap();
    let code = super::run(
        SetupArgs {
            local: true,
            dry_run: true,
            ..args()
        },
        dir.path(),
    );
    assert_eq!(code, 0);
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
}
