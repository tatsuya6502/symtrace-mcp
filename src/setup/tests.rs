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
    // A user agent named Explore shadows the built-in one and would inherit
    // every tool without this line.
    assert!(EXPLORE_FRONTMATTER.contains("\ndisallowedTools: Write, Edit, NotebookEdit\n"));
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
fn empty_explore_md_gets_frontmatter_and_is_deleted_on_uninstall() {
    for seed in ["", "  \n\n"] {
        let e = env();
        fs::create_dir_all(e.targets.explore_md.parent().unwrap()).unwrap();
        fs::write(&e.targets.explore_md, seed).unwrap();
        let r = execute(&args(), &e.targets);
        assert_eq!(statuses(&r)[1], ("explore", "created"), "{seed:?}");
        assert_eq!(
            read(&e.targets.explore_md),
            format!("{EXPLORE_FRONTMATTER}\n{}", block())
        );
        let r = execute(
            &SetupArgs {
                uninstall: true,
                ..args()
            },
            &e.targets,
        );
        assert_eq!(statuses(&r)[1], ("explore", "removed"));
        assert!(!e.targets.explore_md.exists(), "deleted: {seed:?}");
    }
}

#[test]
fn frontmatterless_explore_md_gets_frontmatter_prepended() {
    let e = env();
    fs::create_dir_all(e.targets.explore_md.parent().unwrap()).unwrap();
    fs::write(&e.targets.explore_md, "\nMy notes.\n").unwrap();
    let r = execute(&args(), &e.targets);
    assert_eq!(statuses(&r)[1], ("explore", "updated"));
    let expected = format!("{EXPLORE_FRONTMATTER}\nMy notes.\n\n{}", block());
    assert_eq!(read(&e.targets.explore_md), expected);
    assert!(read(&e.targets.explore_md).starts_with("---\nname: Explore\n"));
    let r = execute(&args(), &e.targets);
    assert_eq!(statuses(&r)[1], ("explore", "unchanged"));
    // Uninstall keeps the user's notes (now under our frontmatter).
    let r = execute(
        &SetupArgs {
            uninstall: true,
            ..args()
        },
        &e.targets,
    );
    assert_eq!(statuses(&r)[1], ("explore", "removed"));
    assert_eq!(
        read(&e.targets.explore_md),
        format!("{EXPLORE_FRONTMATTER}\nMy notes.\n")
    );
}

#[test]
fn crlf_claude_md_stays_crlf_and_unchanged_on_rerun() {
    let e = env();
    fs::create_dir_all(e.targets.claude_md.parent().unwrap()).unwrap();
    fs::write(&e.targets.claude_md, "# notes\r\nline\r\n").unwrap();
    let r = execute(&args(), &e.targets);
    assert_eq!(statuses(&r)[0], ("prompt", "created"));
    let text = read(&e.targets.claude_md);
    assert_eq!(
        text,
        format!("# notes\r\nline\r\n\r\n{}", block().replace('\n', "\r\n"))
    );
    assert!(!text.replace("\r\n", "").contains('\n'), "no bare LF");
    let r = execute(&args(), &e.targets);
    assert_eq!(statuses(&r)[0], ("prompt", "unchanged"));
    let r = execute(
        &SetupArgs {
            status: true,
            ..args()
        },
        &e.targets,
    );
    assert_eq!(statuses(&r)[0].1, format!("installed v{VERSION}"));
    let r = execute(
        &SetupArgs {
            uninstall: true,
            ..args()
        },
        &e.targets,
    );
    assert_eq!(statuses(&r)[0], ("prompt", "removed"));
    assert_eq!(read(&e.targets.claude_md), "# notes\r\nline\r\n");
}

#[test]
fn markdown_writes_leave_no_temp_file() {
    let e = env();
    execute(&args(), &e.targets);
    // Modify CLAUDE.md in place (outdated → updated) and uninstall; every
    // write goes through the temp file + rename path.
    let old = super::marker_block::render("0.0.1", "old\n");
    fs::write(&e.targets.claude_md, format!("# mine\n\n{old}")).unwrap();
    execute(&args(), &e.targets);
    execute(
        &SetupArgs {
            uninstall: true,
            ..args()
        },
        &e.targets,
    );
    let mut names: Vec<_> = fs::read_dir(e.home.join(".claude"))
        .unwrap()
        .map(|d| d.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(names, ["CLAUDE.md", "agents", "settings.json"]);
    assert_eq!(
        fs::read_dir(e.home.join(".claude/agents")).unwrap().count(),
        0
    );
    assert_eq!(read(&e.targets.claude_md), "# mine\n");
}

#[cfg(unix)]
#[test]
fn file_modes_on_fresh_install() {
    use std::os::unix::fs::PermissionsExt;
    let e = env();
    execute(&args(), &e.targets);
    let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(
        mode(&e.targets.mcp_json),
        0o600,
        ".claude.json holds OAuth state"
    );
    assert_eq!(mode(&e.targets.settings_json), 0o600);
    for p in [&e.targets.claude_md, &e.targets.explore_md] {
        let m = mode(p);
        assert_eq!(m & 0o600, 0o600, "{}: {m:o}", p.display());
        assert_eq!(m & 0o133, 0, "{}: {m:o}", p.display());
    }
}

#[cfg(unix)]
#[test]
fn existing_file_modes_are_kept() {
    use std::os::unix::fs::PermissionsExt;
    let e = env();
    fs::create_dir_all(e.targets.settings_json.parent().unwrap()).unwrap();
    fs::write(
        &e.targets.settings_json,
        "{\"first\":1,\"hooks\":{},\"mid\":2,\"last\":3}",
    )
    .unwrap();
    fs::set_permissions(&e.targets.settings_json, fs::Permissions::from_mode(0o644)).unwrap();
    fs::write(&e.targets.claude_md, "# notes\n").unwrap();
    fs::set_permissions(&e.targets.claude_md, fs::Permissions::from_mode(0o600)).unwrap();
    execute(&args(), &e.targets);
    let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&e.targets.settings_json), 0o644);
    assert_eq!(mode(&e.targets.claude_md), 0o600);
    // Key order survives install and uninstall (`hooks` pruned in place).
    let text = read(&e.targets.settings_json);
    let pos = |k: &str| text.find(&format!("\"{k}\"")).unwrap();
    assert!(
        pos("first") < pos("hooks") && pos("hooks") < pos("mid") && pos("mid") < pos("last"),
        "{text}"
    );
    execute(
        &SetupArgs {
            uninstall: true,
            ..args()
        },
        &e.targets,
    );
    assert_eq!(
        read(&e.targets.settings_json),
        "{\n  \"first\": 1,\n  \"mid\": 2,\n  \"last\": 3\n}\n"
    );
    assert_eq!(mode(&e.targets.settings_json), 0o644);
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
fn explore_md_without_tool_restriction_warns() {
    // ast-bro's installer writes only name + description.
    let e = env();
    fs::create_dir_all(e.targets.explore_md.parent().unwrap()).unwrap();
    let foreign = "---\nname: Explore\ndescription: Fast read-only search agent.\n---\n\n\
                   <!-- ast-bro:begin v=4.2.0 -->\nuse sb\n<!-- ast-bro:end -->\n";
    fs::write(&e.targets.explore_md, foreign).unwrap();

    let r = execute(&args(), &e.targets);
    assert_eq!(statuses(&r)[1], ("explore", "created"));
    assert_eq!(r.exit_code, 0);
    assert_eq!(r.warnings.len(), 1, "{:?}", r.warnings);
    assert!(r.warnings[0].contains("disallowedTools: Write, Edit, NotebookEdit"));
    assert!(r.warnings[0].contains(&e.targets.explore_md.display().to_string()));
    // The foreign frontmatter is left exactly as it was.
    assert!(read(&e.targets.explore_md).starts_with(foreign));

    // --status warns too; --uninstall and --skip-explore do not.
    let status = execute(
        &SetupArgs {
            status: true,
            ..args()
        },
        &e.targets,
    );
    assert_eq!(status.warnings.len(), 1);
    let skipped = execute(
        &SetupArgs {
            skip_explore: true,
            ..args()
        },
        &e.targets,
    );
    assert!(skipped.warnings.is_empty());
    let uninstall = execute(
        &SetupArgs {
            uninstall: true,
            ..args()
        },
        &e.targets,
    );
    assert!(uninstall.warnings.is_empty());
}

#[test]
fn restricted_explore_md_does_not_warn() {
    let e = env();
    // Fresh install writes our frontmatter, which carries disallowedTools.
    let r = execute(&args(), &e.targets);
    assert!(r.warnings.is_empty(), "{:?}", r.warnings);

    // A `tools:` allow-list counts as a restriction as well.
    fs::write(
        &e.targets.explore_md,
        "---\nname: Explore\n  tools: Read, Grep, Glob\n---\n",
    )
    .unwrap();
    let r = execute(&args(), &e.targets);
    assert!(r.warnings.is_empty(), "{:?}", r.warnings);

    // A missing file produces no warning (it is created with our frontmatter).
    fs::remove_file(&e.targets.explore_md).unwrap();
    let r = execute(&args(), &e.targets);
    assert!(r.warnings.is_empty(), "{:?}", r.warnings);
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
