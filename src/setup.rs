//! `symtrace-mcp setup`: install, inspect or remove the Claude Code integration.
//!
//! See `openspec/changes/setup-command/` for the specification. The four
//! items are: the prompt block in CLAUDE.md, the same block in the Explore
//! subagent file, PreToolUse hook entries in settings.json, and the MCP
//! server entry.

mod json_file;
mod marker_block;
mod preflight;
mod targets;
#[cfg(test)]
mod tests;

use std::fs;
use std::path::{Path, PathBuf};

use clap::Args;

use json_file::{HOOK_MATCHERS, HookChange, McpChange, McpState};
use marker_block::{Found, State};
use targets::Targets;

/// Body of the prompt block (no markers).
pub const PROMPT: &str = include_str!("setup/prompt.md");
/// YAML frontmatter written when Explore.md is created from scratch.
pub const EXPLORE_FRONTMATTER: &str = include_str!("setup/explore_frontmatter.md");
/// Version written into the begin marker.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Args, Debug, Clone, Default)]
pub struct SetupArgs {
    /// Target the project files in the current directory
    /// (./CLAUDE.md, ./.claude/agents/Explore.md, ./.claude/settings.json,
    /// ./.mcp.json) instead of the per-user files under $HOME
    #[arg(long)]
    pub local: bool,

    /// Report what would change without writing any file
    #[arg(long)]
    pub dry_run: bool,

    /// Replace a prompt block or MCP entry that was edited by the user
    #[arg(long)]
    pub force: bool,

    /// Do not touch CLAUDE.md
    #[arg(long)]
    pub skip_prompt: bool,

    /// Do not touch the Explore subagent file
    #[arg(long)]
    pub skip_explore: bool,

    /// Do not touch the hook entries in settings.json
    #[arg(long)]
    pub skip_hook: bool,

    /// Do not touch the MCP server entry
    #[arg(long)]
    pub skip_mcp: bool,

    /// Remove everything `setup` installed (only symtrace-mcp content)
    #[arg(long, conflicts_with = "status")]
    pub uninstall: bool,

    /// Report the installation state of each item without writing
    #[arg(long, conflicts_with = "uninstall")]
    pub status: bool,
}

/// Errors from a single item. Reported as `error: …` on that item's line.
#[derive(Debug)]
pub enum SetupError {
    /// `HOME` is unset in global scope.
    HomeUnset,
    Io(PathBuf, std::io::Error),
    /// A JSON file could not be parsed (or is not an object).
    JsonParse(PathBuf, String),
    /// A JSON value at one of our paths has the wrong type.
    JsonShape(String),
    /// Markers in a markdown file do not form one block.
    DamagedMarkers,
}

impl std::fmt::Display for SetupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SetupError::HomeUnset => write!(f, "HOME is not set; use --local or set HOME"),
            SetupError::Io(path, e) => write!(f, "{}: {e}", path.display()),
            SetupError::JsonParse(path, msg) => write!(f, "{}: {msg}", path.display()),
            SetupError::JsonShape(msg) => write!(f, "{msg}"),
            SetupError::DamagedMarkers => {
                write!(f, "damaged symtrace-mcp markers; remove them by hand")
            }
        }
    }
}

impl std::error::Error for SetupError {}

/// Run the setup command. Returns the process exit code: 0 when no item
/// failed, non-zero when an item errored or needs `--force`.
pub fn run(args: SetupArgs, cwd: &Path) -> i32 {
    let targets = if args.local {
        Targets::local(cwd)
    } else {
        match std::env::var_os("HOME").filter(|h| !h.is_empty()) {
            Some(home) => Targets::global(Path::new(&home)),
            None => {
                eprintln!("symtrace-mcp setup: {}", SetupError::HomeUnset);
                return 1;
            }
        }
    };
    // Installing registers the bare command name; make sure Claude Code
    // will resolve it to this binary.
    if !args.status
        && !args.uninstall
        && let Some(w) = preflight::warning()
    {
        eprintln!("warning: {w}");
    }
    run_with_targets(&args, &targets)
}

/// Run against explicit paths, print one line per item, return the exit code.
pub fn run_with_targets(args: &SetupArgs, targets: &Targets) -> i32 {
    let report = execute(args, targets);
    for line in &report.lines {
        println!("{line}");
    }
    report.exit_code
}

/// One result line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemLine {
    pub item: &'static str,
    pub status: String,
    pub path: PathBuf,
    pub dry_run: bool,
}

impl std::fmt::Display for ItemLine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.dry_run {
            f.write_str("dry-run ")?;
        }
        write!(
            f,
            "{:<7}  {:<9}  {}",
            self.item,
            self.status,
            self.path.display()
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub lines: Vec<ItemLine>,
    pub exit_code: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Install,
    Uninstall,
    Status,
}

const MODIFIED: &str = "modified (use --force)";

/// Compute (and, unless `--dry-run`/`--status`, apply) every item.
pub fn execute(args: &SetupArgs, targets: &Targets) -> Report {
    let mode = if args.status {
        Mode::Status
    } else if args.uninstall {
        Mode::Uninstall
    } else {
        Mode::Install
    };
    let write = !args.dry_run && mode != Mode::Status;
    let ctx = Ctx {
        write,
        force: args.force,
    };

    let items: [(&'static str, bool, &Path, ItemFn); 4] = [
        ("prompt", args.skip_prompt, &targets.claude_md, prompt_item),
        (
            "explore",
            args.skip_explore,
            &targets.explore_md,
            explore_item,
        ),
        ("hook", args.skip_hook, &targets.settings_json, hook_item),
        ("mcp", args.skip_mcp, &targets.mcp_json, mcp_item),
    ];

    let mut lines = Vec::with_capacity(4);
    let mut failed = false;
    for (item, skip, path, f) in items {
        let status = if skip {
            "skipped".to_string()
        } else {
            match f(&ctx, path, mode) {
                Ok(s) => s,
                Err(e) => {
                    failed = true;
                    format!("error: {e}")
                }
            }
        };
        if status == MODIFIED {
            failed = true;
        }
        lines.push(ItemLine {
            item,
            status,
            path: path.to_path_buf(),
            dry_run: args.dry_run,
        });
    }
    let exit_code = if failed && mode != Mode::Status { 1 } else { 0 };
    Report { lines, exit_code }
}

struct Ctx {
    write: bool,
    force: bool,
}

/// One item's handler: compute its status for `mode`, writing when `ctx.write`.
type ItemFn = fn(&Ctx, &Path, Mode) -> Result<String, SetupError>;

fn block() -> String {
    marker_block::render(VERSION, PROMPT)
}

fn read_optional(path: &Path) -> Result<Option<String>, SetupError> {
    match fs::read_to_string(path) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(SetupError::Io(path.to_path_buf(), e)),
    }
}

fn write_text(ctx: &Ctx, path: &Path, old: Option<&str>, new: &str) -> Result<(), SetupError> {
    if !ctx.write || old == Some(new) {
        return Ok(());
    }
    let io = |e: std::io::Error| SetupError::Io(path.to_path_buf(), e);
    if let Some(dir) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(dir).map_err(io)?;
    }
    fs::write(path, new).map_err(io)
}

fn prompt_item(ctx: &Ctx, path: &Path, mode: Mode) -> Result<String, SetupError> {
    markdown_item(ctx, path, mode, None)
}

fn explore_item(ctx: &Ctx, path: &Path, mode: Mode) -> Result<String, SetupError> {
    markdown_item(ctx, path, mode, Some(EXPLORE_FRONTMATTER))
}

/// Shared logic for CLAUDE.md and Explore.md. `frontmatter` is written
/// when the file is created, and its presence alone allows deletion on
/// uninstall.
fn markdown_item(
    ctx: &Ctx,
    path: &Path,
    mode: Mode,
    frontmatter: Option<&str>,
) -> Result<String, SetupError> {
    let existing = read_optional(path)?;
    let text = existing.as_deref().unwrap_or("");
    let state = marker_block::classify(text, VERSION, PROMPT);

    match mode {
        Mode::Status => Ok(match state {
            State::Absent => "absent".into(),
            State::Current => format!("installed v{VERSION}"),
            State::Outdated(v) => format!("outdated v{v}"),
            State::Modified => "modified".into(),
        }),
        Mode::Install => {
            let block = block();
            let (new, status) = match state {
                State::Current => return Ok("unchanged".into()),
                State::Absent => {
                    let base = match (existing.as_deref(), frontmatter) {
                        (None, Some(fm)) => fm,
                        _ => text,
                    };
                    (marker_block::insert_at_end(base, &block), "created")
                }
                State::Outdated(_) => (
                    marker_block::replace_in_place(text, &block).expect("classified as a block"),
                    "updated",
                ),
                State::Modified if !ctx.force => return Ok(MODIFIED.into()),
                State::Modified => {
                    let new = match marker_block::find(text) {
                        Found::Block(b) => {
                            marker_block::replace_span(text, (b.start, b.end), &block)
                        }
                        Found::Damaged { span } => marker_block::replace_span(text, span, &block),
                        Found::None => unreachable!("Modified implies markers"),
                    };
                    (new, "updated")
                }
            };
            write_text(ctx, path, existing.as_deref(), &new)?;
            Ok(status.into())
        }
        Mode::Uninstall => {
            let Some(text) = existing.as_deref() else {
                return Ok("absent".into());
            };
            let new = match marker_block::find(text) {
                Found::None => return Ok("absent".into()),
                Found::Damaged { .. } => return Err(SetupError::DamagedMarkers),
                Found::Block(_) => marker_block::remove(text).expect("block found"),
            };
            let only_frontmatter = frontmatter.is_some_and(|fm| new.trim_end() == fm.trim_end());
            if only_frontmatter {
                if ctx.write {
                    fs::remove_file(path).map_err(|e| SetupError::Io(path.to_path_buf(), e))?;
                }
            } else {
                write_text(ctx, path, Some(text), &new)?;
            }
            Ok("removed".into())
        }
    }
}

fn hook_item(ctx: &Ctx, path: &Path, mode: Mode) -> Result<String, SetupError> {
    let mut value = json_file::load(path)?;
    let present = HOOK_MATCHERS
        .iter()
        .map(|m| json_file::has_hook(&value, m))
        .collect::<Result<Vec<bool>, _>>()?;
    let all = present.iter().all(|&p| p);
    let any = present.iter().any(|&p| p);
    match mode {
        Mode::Status => Ok(if all {
            "installed".into()
        } else if any {
            "modified".into()
        } else {
            "absent".into()
        }),
        Mode::Install => {
            let mut changed = false;
            for m in HOOK_MATCHERS {
                if json_file::upsert_hook(&mut value, m)? == HookChange::Changed {
                    changed = true;
                }
            }
            if !changed {
                return Ok("unchanged".into());
            }
            if ctx.write {
                json_file::save(path, &value)?;
            }
            Ok(if any { "updated" } else { "created" }.into())
        }
        Mode::Uninstall => {
            if !json_file::remove_hooks(&mut value)? {
                return Ok("absent".into());
            }
            if ctx.write {
                json_file::save(path, &value)?;
            }
            Ok("removed".into())
        }
    }
}

fn mcp_item(ctx: &Ctx, path: &Path, mode: Mode) -> Result<String, SetupError> {
    let mut value = json_file::load(path)?;
    match mode {
        Mode::Status => Ok(match json_file::mcp_state(&value)? {
            McpState::Absent => "absent".into(),
            McpState::Current => "installed".into(),
            McpState::Modified => "modified".into(),
        }),
        Mode::Install => {
            let status = match json_file::upsert_mcp(&mut value, ctx.force)? {
                McpChange::Unchanged => return Ok("unchanged".into()),
                McpChange::Modified => return Ok(MODIFIED.into()),
                McpChange::Created => "created",
                McpChange::Updated => "updated",
            };
            if ctx.write {
                json_file::save(path, &value)?;
            }
            Ok(status.into())
        }
        Mode::Uninstall => {
            if !json_file::remove_mcp(&mut value)? {
                return Ok("absent".into());
            }
            if ctx.write {
                json_file::save(path, &value)?;
            }
            Ok("removed".into())
        }
    }
}
