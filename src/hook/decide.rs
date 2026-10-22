//! Pure decision logic for the `hook` subcommand.
//!
//! `decide` inspects one Claude Code `PreToolUse` event and reports whether
//! the call is a text/tree-sitter heuristic for a semantic question that the
//! symtrace-mcp tools answer better. It has no I/O and reads no environment
//! variables, so every spec scenario is a table-driven test.

use serde_json::Value;

/// Default source directories: a relative grep target with one of these
/// path components counts as a source-code search.
pub const DEFAULT_SRC_DIRS: &[&str] = &[
    "src", "crates", "lib", "tests", "test", "app", "apps", "packages", "pkg", "internal", "cmd",
];

/// File extensions that mark a grep target as source code.
const SRC_EXTS: &[&str] = &[
    "rs", "ts", "tsx", "js", "jsx", "mjs", "cjs", "py", "go", "java", "kt", "kts", "rb", "c", "cc",
    "cpp", "h", "hpp", "cs", "swift", "scala",
];

/// `Grep` tool `type` values that denote a source language.
const GREP_TOOL_SRC_TYPES: &[&str] = &[
    "rust",
    "ts",
    "typescript",
    "js",
    "javascript",
    "py",
    "python",
    "go",
    "java",
    "kotlin",
    "ruby",
    "c",
    "cpp",
    "csharp",
    "swift",
    "scala",
];

/// Words that are syntactically identifiers but never a code symbol worth a
/// semantic lookup (exact, case-sensitive).
const IDENTIFIER_STOP_LIST: &[&str] = &["TODO", "FIXME", "XXX", "HACK", "NOTE", "BUG"];

const SB_PROGRAMS: &[&str] = &["sb", "ast-bro"];
const SB_SUBCOMMANDS: &[&str] = &["callers", "callees", "implements", "impact", "trace"];

/// Wrapper programs that are stripped to reveal the real command
/// (`sudo grep ...`, `xargs grep ...`). Their own leading `-` options are
/// skipped; `env` additionally skips `NAME=value` arguments.
const WRAPPER_PROGRAMS: &[&str] = &["sudo", "env", "command", "nice", "time", "xargs"];

/// Short options of `grep` / `ugrep` (and `rg`) that take a separate value.
const GREP_VALUE_SHORT: &[char] = &['A', 'B', 'C', 'm', 'e', 'f', 'd', 'D'];
/// Long options of `grep` / `ugrep` (and `rg`) that take a separate value.
const GREP_VALUE_LONG: &[&str] = &[
    "--after-context",
    "--before-context",
    "--context",
    "--max-count",
    "--regexp",
    "--file",
    "--include",
    "--exclude",
    "--exclude-dir",
];
/// Additional value-taking short options of `rg`.
const RG_VALUE_SHORT: &[char] = &['g', 't', 'T', 'E', 'M', 'j', 'r'];
/// Additional value-taking long options of `rg`.
const RG_VALUE_LONG: &[&str] = &[
    "--glob",
    "--iglob",
    "--type",
    "--type-not",
    "--encoding",
    "--max-columns",
    "--threads",
    "--replace",
    "--max-depth",
    "--sort",
    "--sortr",
    // `rg --color WHEN` takes a separate value; GNU grep / ugrep only accept
    // the attached form `--color[=WHEN]`, so a bare `--color` is a flag there.
    "--color",
    "--colour",
];

/// What the hook detected in a tool call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Detection {
    /// `sb <subcommand>` / `ast-bro <subcommand>` call-graph query.
    SbQuery(String),
    /// `grep` / `rg` / `ugrep` / `git grep` for a bare identifier over source code.
    IdentifierGrep(String),
    /// `Grep` tool with a bare identifier pattern.
    GrepTool(String),
}

impl Detection {
    /// Short human-readable label used in the reminder text.
    pub fn label(&self) -> String {
        match self {
            Detection::SbQuery(sub) => format!("sb {sub}"),
            Detection::IdentifierGrep(id) => format!("grep for identifier '{id}'"),
            Detection::GrepTool(id) => format!("Grep for identifier '{id}'"),
        }
    }
}

/// Decide whether a `PreToolUse` event deserves the reminder.
///
/// `cwd` is the event's working directory, used to interpret absolute paths
/// inside it as relative ones. `src_dirs` is the list of directory names
/// that make a grep count as a source-code search (see [`DEFAULT_SRC_DIRS`]).
pub fn decide(
    tool_name: &str,
    tool_input: &Value,
    cwd: Option<&str>,
    src_dirs: &[&str],
) -> Option<Detection> {
    match tool_name {
        "Bash" => {
            let command = tool_input.get("command")?.as_str()?;
            decide_bash(command, cwd, src_dirs)
        }
        "Grep" => decide_grep_tool(tool_input, cwd, src_dirs),
        _ => None,
    }
}

/// `Grep` tool: a bare identifier pattern, and every present scope field
/// (`path`, `glob`, `type`) must point at source code. An absent `path` is
/// the working directory and counts as source.
fn decide_grep_tool(tool_input: &Value, cwd: Option<&str>, src_dirs: &[&str]) -> Option<Detection> {
    let pattern = tool_input.get("pattern")?.as_str()?;
    if !is_searchable_identifier(pattern) {
        return None;
    }
    if let Some(path) = tool_input.get("path")
        && !grep_tool_path_is_source(path.as_str()?, cwd, src_dirs)
    {
        return None;
    }
    if let Some(glob) = tool_input.get("glob")
        && !glob_has_source_ext(glob.as_str()?)
    {
        return None;
    }
    if let Some(ty) = tool_input.get("type")
        && !GREP_TOOL_SRC_TYPES.contains(&ty.as_str()?)
    {
        return None;
    }
    Some(Detection::GrepTool(pattern.to_string()))
}

/// The `Grep` tool's `path` follows the same source-target rule as a Bash
/// grep target. Without a `cwd` to relativize against, an absolute path
/// additionally counts when its last component is a source directory
/// (the tool usually receives absolute project paths).
fn grep_tool_path_is_source(path: &str, cwd: Option<&str>, src_dirs: &[&str]) -> bool {
    if is_source_target(path, cwd, src_dirs) {
        return true;
    }
    if cwd.is_none() && path.starts_with('/') {
        let last = path.trim_end_matches('/').rsplit('/').next().unwrap_or("");
        return src_dirs.contains(&last);
    }
    false
}

/// `*.rs`, `**/*.ts`: the glob's final extension is a source extension.
fn glob_has_source_ext(glob: &str) -> bool {
    glob.rsplit_once('.')
        .is_some_and(|(_, ext)| SRC_EXTS.contains(&ext))
}

fn decide_bash(command: &str, cwd: Option<&str>, src_dirs: &[&str]) -> Option<Detection> {
    let segs = segments(command);
    segs.iter().enumerate().find_map(|(i, segment)| {
        // The producer of a piped segment supplies `xargs` with its file list.
        let producer = (segment.piped && i > 0).then(|| segs[i - 1].words.as_slice());
        decide_segment(segment, producer, cwd, src_dirs)
    })
}

fn decide_segment(
    segment: &Segment,
    producer: Option<&[Word]>,
    cwd: Option<&str>,
    src_dirs: &[&str],
) -> Option<Detection> {
    let (words, via_xargs) = unwrap_wrappers(&segment.words);
    let program = words.first()?;
    let name = program_name(&program.text);

    if SB_PROGRAMS.contains(&name) {
        let sub = words.get(1)?;
        return SB_SUBCOMMANDS
            .contains(&sub.text.as_str())
            .then(|| Detection::SbQuery(sub.text.clone()));
    }

    let (kind, args) = match name {
        "grep" | "ugrep" => (GrepKind::Grep, &words[1..]),
        "rg" => (GrepKind::Rg, &words[1..]),
        "git" if words.get(1).is_some_and(|w| !w.quoted && w.text == "grep") => {
            (GrepKind::Grep, &words[2..])
        }
        _ => return None,
    };
    let parsed = parse_grep_args(kind, args);
    let pattern = parsed.pattern?;
    if !is_searchable_identifier(&pattern) {
        return None;
    }

    let targets_source = if !parsed.targets.is_empty() {
        parsed
            .targets
            .iter()
            .any(|t| is_source_target(t, cwd, src_dirs))
    } else if via_xargs {
        // `find src -name '*.rs' | xargs grep Foo`: the files come from the
        // producer segment, so look for a source target there.
        producer.is_some_and(|p| p.iter().any(|w| is_source_target(&w.text, cwd, src_dirs)))
    } else if segment.piped {
        // `cargo test | grep FAILED`: a filter over another command's output.
        false
    } else {
        // No target: a search of the working directory. `rg` recurses by
        // default; `grep` / `ugrep` / `git grep` only with a recursive flag.
        kind == GrepKind::Rg || parsed.recursive
    };
    targets_source.then_some(Detection::IdentifierGrep(pattern))
}

/// Skip leading `NAME=value` assignments and wrapper programs
/// (`sudo grep ...`, `env FOO=1 grep ...`, `xargs grep ...`).
/// Returns the remaining words and whether `xargs` was among the wrappers.
fn unwrap_wrappers(words: &[Word]) -> (&[Word], bool) {
    let mut words = skip_env_assignments(words);
    let mut via_xargs = false;
    while let Some(first) = words.first() {
        if first.quoted {
            break;
        }
        let name = program_name(&first.text);
        if !WRAPPER_PROGRAMS.contains(&name) {
            break;
        }
        via_xargs |= name == "xargs";
        words = &words[1..];
        // Skip the wrapper's own flags (`xargs -0`, `sudo -E`). Flags that
        // take a value (`nice -n 10`) are not modelled.
        while words
            .first()
            .is_some_and(|w| !w.quoted && w.text.starts_with('-'))
        {
            words = &words[1..];
        }
        if name == "env" {
            words = skip_env_assignments(words);
        }
    }
    (words, via_xargs)
}

fn skip_env_assignments(words: &[Word]) -> &[Word] {
    let start = words
        .iter()
        .position(|w| !w.is_env_assignment())
        .unwrap_or(words.len());
    &words[start..]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GrepKind {
    /// `grep`, `ugrep`, `git grep`.
    Grep,
    /// ripgrep.
    Rg,
}

impl GrepKind {
    fn takes_value_short(self, c: char) -> bool {
        GREP_VALUE_SHORT.contains(&c) || (self == GrepKind::Rg && RG_VALUE_SHORT.contains(&c))
    }

    fn takes_value_long(self, opt: &str) -> bool {
        GREP_VALUE_LONG.contains(&opt) || (self == GrepKind::Rg && RG_VALUE_LONG.contains(&opt))
    }
}

/// A grep invocation's arguments, with options resolved.
#[derive(Debug, Default, PartialEq, Eq)]
struct GrepArgs {
    /// The pattern: the value of the first `-e` / `--regexp`, else the first
    /// positional argument.
    pattern: Option<String>,
    /// Positional arguments after the pattern (paths, or redirections).
    targets: Vec<String>,
    /// `-r`, `-R`, `--recursive`, or a short cluster containing `r` / `R`
    /// (`grep` family only).
    recursive: bool,
}

/// Walk grep arguments, skipping options and the values of value-taking
/// options. `--key=value` is one token and is skipped. `--` ends options.
/// In a short cluster (`-rnA3`, `-eFoo`) a value-taking letter consumes the
/// rest of the cluster, or the next token when it is last.
fn parse_grep_args(kind: GrepKind, args: &[Word]) -> GrepArgs {
    let mut positionals: Vec<String> = Vec::new();
    let mut explicit_pattern: Option<String> = None;
    let mut recursive = false;
    let mut options_done = false;
    let mut i = 0;
    while i < args.len() {
        let word = &args[i];
        i += 1;
        if options_done || word.quoted || !word.text.starts_with('-') || word.text == "-" {
            // Redirections (`2>/dev/null`, `<in`) are not search targets.
            if !word.quoted && (word.text.contains('>') || word.text.contains('<')) {
                continue;
            }
            positionals.push(word.text.clone());
            continue;
        }
        if word.text == "--" {
            options_done = true;
            continue;
        }
        if word.text.starts_with("--") {
            if word.text.contains('=') {
                continue;
            }
            if word.text == "--recursive" {
                recursive = true;
            } else if kind.takes_value_long(&word.text) {
                let value = args.get(i).map(|w| w.text.clone());
                i += 1;
                if word.text == "--regexp" && explicit_pattern.is_none() {
                    explicit_pattern = value;
                }
            }
            continue;
        }
        let flags: Vec<char> = word.text[1..].chars().collect();
        let mut j = 0;
        while j < flags.len() {
            let c = flags[j];
            j += 1;
            if kind.takes_value_short(c) {
                let value = if j < flags.len() {
                    Some(flags[j..].iter().collect::<String>())
                } else {
                    let v = args.get(i).map(|w| w.text.clone());
                    i += 1;
                    v
                };
                if c == 'e' && explicit_pattern.is_none() {
                    explicit_pattern = value;
                }
                break;
            }
            if kind == GrepKind::Grep && (c == 'r' || c == 'R') {
                recursive = true;
            }
        }
    }
    let (pattern, targets) = match explicit_pattern {
        Some(p) => (Some(p), positionals),
        None => {
            let mut it = positionals.into_iter();
            (it.next(), it.collect())
        }
    };
    GrepArgs {
        pattern,
        targets,
        recursive,
    }
}

/// `/usr/bin/grep` -> `grep`.
fn program_name(text: &str) -> &str {
    text.rsplit('/').next().unwrap_or(text)
}

/// Whole bare identifier: `[A-Za-z_][A-Za-z0-9_]*`.
fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// An identifier that is plausibly a code symbol: [`is_identifier`] minus the
/// [`IDENTIFIER_STOP_LIST`] (`TODO`, `FIXME`, ...).
fn is_searchable_identifier(s: &str) -> bool {
    is_identifier(s) && !IDENTIFIER_STOP_LIST.contains(&s)
}

/// Source-target rule. A leading `./` is stripped; `.` and `./` are the
/// working directory and count. An absolute path inside `cwd` is made
/// relative to it first (a path equal to `cwd` counts as `.`). A relative
/// path counts when any component is one of `src_dirs`. Any path counts
/// when its last component has a source extension. Absolute paths (`/`,
/// `~`, `$`) outside `cwd` are only matched by the extension rule, so
/// `/var/lib/apt/x` and `$HOME/.cache/test/x` do not count. Redirections
/// (`2>/dev/null`, `<in`) never count.
fn is_source_target(token: &str, cwd: Option<&str>, src_dirs: &[&str]) -> bool {
    if token.contains('>') || token.contains('<') {
        return false;
    }
    if token == "." || token == "./" {
        return true;
    }
    let path = token.strip_prefix("./").unwrap_or(token);
    let relative = if path.starts_with(['/', '~', '$']) {
        relative_to_cwd(path, cwd)
    } else {
        Some(path)
    };
    if let Some(relative) = relative
        && (relative.is_empty() || relative.split('/').any(|c| src_dirs.contains(&c)))
    {
        return true;
    }
    let last = path.trim_end_matches('/').rsplit('/').next().unwrap_or("");
    match last.rsplit_once('.') {
        Some((stem, ext)) => !stem.is_empty() && SRC_EXTS.contains(&ext),
        None => false,
    }
}

/// `/home/x/proj/src/a` with cwd `/home/x/proj` -> `src/a`; `/home/x/proj`
/// -> `` (empty). `None` when `cwd` is unknown or the path is outside it.
fn relative_to_cwd<'a>(path: &'a str, cwd: Option<&str>) -> Option<&'a str> {
    let cwd = cwd?.trim_end_matches('/');
    if cwd.is_empty() {
        return None;
    }
    let rest = path.strip_prefix(cwd)?;
    if rest.is_empty() {
        Some("")
    } else {
        rest.strip_prefix('/')
    }
}

/// One shell word, with quotes removed.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Word {
    text: String,
    /// True when any part of the word was quoted; a quoted word is never an
    /// option, even if it starts with `-`.
    quoted: bool,
}

impl Word {
    /// `NAME=value` or `NAME="quoted value"`; only meaningful at the start
    /// of a segment.
    fn is_env_assignment(&self) -> bool {
        match self.text.split_once('=') {
            Some((name, _)) => is_identifier(name),
            None => false,
        }
    }
}

/// One pipeline/list segment of a command.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Segment {
    words: Vec<Word>,
    /// True when the segment follows a single `|`, i.e. it consumes the
    /// previous segment's output. Segments after `;`, `&&`, `||` or at the
    /// start of the command are not consumers.
    piped: bool,
}

/// Split a command into segments on `||`, `&&`, `;` and `|` (outside quotes),
/// tokenizing each segment into words. Single and double quotes group a word;
/// no escape handling beyond that. A lone `&` is treated as a word character
/// so that `2>&1` stays one token.
fn segments(command: &str) -> Vec<Segment> {
    let mut result = Vec::new();
    let mut segment = Vec::new();
    let mut piped = false;
    let mut word = String::new();
    let mut quoted = false;
    let mut in_quote: Option<char> = None;

    let flush_word = |word: &mut String, quoted: &mut bool, segment: &mut Vec<Word>| {
        if !word.is_empty() || *quoted {
            segment.push(Word {
                text: std::mem::take(word),
                quoted: *quoted,
            });
        }
        *quoted = false;
    };

    let chars: Vec<char> = command.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if let Some(q) = in_quote {
            if c == q {
                in_quote = None;
            } else {
                word.push(c);
            }
            i += 1;
            continue;
        }
        match c {
            '\'' | '"' => {
                in_quote = Some(c);
                quoted = true;
            }
            c if c.is_whitespace() => flush_word(&mut word, &mut quoted, &mut segment),
            '|' | '&' | ';' => {
                let two = c == '|' && chars.get(i + 1) == Some(&'|')
                    || c == '&' && chars.get(i + 1) == Some(&'&');
                let is_op = two || c == '|' || c == ';';
                if is_op {
                    flush_word(&mut word, &mut quoted, &mut segment);
                    result.push(Segment {
                        words: std::mem::take(&mut segment),
                        piped,
                    });
                    piped = c == '|' && !two;
                    if two {
                        i += 1;
                    }
                } else {
                    word.push(c);
                }
            }
            _ => word.push(c),
        }
        i += 1;
    }
    flush_word(&mut word, &mut quoted, &mut segment);
    result.push(Segment {
        words: segment,
        piped,
    });
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn bash(command: &str) -> Value {
        json!({ "command": command })
    }

    fn ident(id: &str) -> Option<Detection> {
        Some(Detection::IdentifierGrep(id.into()))
    }

    fn check_bash(cases: &[(&str, Option<Detection>)]) {
        for (command, expected) in cases {
            assert_eq!(
                decide("Bash", &bash(command), None, DEFAULT_SRC_DIRS),
                *expected,
                "command: {command}"
            );
        }
    }

    #[test]
    fn bash_scenarios_with_default_dirs() {
        check_bash(&[
            // sb call-graph queries
            (
                "sb callers stop_server",
                Some(Detection::SbQuery("callers".into())),
            ),
            (
                "cd src && sb trace a b | head",
                Some(Detection::SbQuery("trace".into())),
            ),
            (
                "ast-bro impact Foo",
                Some(Detection::SbQuery("impact".into())),
            ),
            (
                "FOO=1 sb callees run",
                Some(Detection::SbQuery("callees".into())),
            ),
            (
                "sb implements Trait src/",
                Some(Detection::SbQuery("implements".into())),
            ),
            // sb structural commands
            ("sb map src/main.rs", None),
            ("sb digest src/", None),
            ("sb", None),
            // identifier greps over source
            ("grep -rn stop_server src/", ident("stop_server")),
            (
                "sed -n 1,3p x; grep -rn \"stop_server\" src/ tests/ 2>/dev/null",
                ident("stop_server"),
            ),
            ("grep -rn 'stop_server' ./crates/foo", ident("stop_server")),
            ("grep -n LspClientApi client.rs", ident("LspClientApi")),
            ("rg -n Config crates/foo", ident("Config")),
            ("rg Config .", ident("Config")),
            ("grep -rn Config --include=*.rs .", ident("Config")),
            ("ugrep -r Foo lib", ident("Foo")),
            ("/usr/bin/grep -rn Foo src", ident("Foo")),
            // regex patterns
            ("grep -rn \"fn [a-z_]*(\" src/", None),
            ("rg 'fn\\s+\\w+' src/", None),
            // non-source targets
            ("grep -c worktrees /proc/self/mountinfo", None),
            ("grep -n Install README.md", None),
            // a redirection is not a target, so this is a recursive cwd search
            ("grep -rn Foo 2>/dev/null", ident("Foo")),
            ("grep -n Foo 2>/dev/null", None),
            // pipe filters
            ("cargo test 2>&1 | grep FAILED", None),
            ("cargo test 2>&1 | grep -c FAILED", None),
            // grep with no pattern
            ("grep -rn", None),
            // segment splitting: pipe without a target in the grep segment
            ("ls src/ | grep Foo", None),
            ("ls || grep Foo src/", ident("Foo")),
        ]);
    }

    #[test]
    fn value_taking_options_are_skipped() {
        check_bash(&[
            // separate value
            ("grep -A 3 Foo src/", ident("Foo")),
            ("grep -B 2 -C 1 Foo src/", ident("Foo")),
            ("grep -m 1 Foo src/", ident("Foo")),
            ("grep -d skip Foo src/", ident("Foo")),
            ("grep --max-count 1 Foo src/", ident("Foo")),
            ("grep --include *.rs -r Foo src/", ident("Foo")),
            ("grep --exclude-dir target -r Foo src/", ident("Foo")),
            ("rg --color always Foo src/", ident("Foo")),
            // bare `--color` is a plain flag for grep: `Foo` stays the pattern
            ("grep -rn --color Foo src/", ident("Foo")),
            ("grep --color -rn Foo src/", ident("Foo")),
            ("rg -t rust Foo src/", ident("Foo")),
            ("rg -T js Foo src/", ident("Foo")),
            ("rg -g '*.rs' Foo src/", ident("Foo")),
            ("rg --glob '!target' Foo src/", ident("Foo")),
            ("rg --type rust Foo src/", ident("Foo")),
            ("rg --type-not md Foo src/", ident("Foo")),
            ("rg -E utf8 Foo src/", ident("Foo")),
            ("rg -M 200 Foo src/", ident("Foo")),
            ("rg -j 4 Foo src/", ident("Foo")),
            ("rg -r repl Foo src/", ident("Foo")),
            ("rg --replace repl Foo src/", ident("Foo")),
            ("rg --max-depth 2 Foo src/", ident("Foo")),
            ("rg --sort path Foo src/", ident("Foo")),
            // value attached to the short cluster
            ("grep -rnA3 Foo src/", ident("Foo")),
            ("grep -nA3 -r Foo src/", ident("Foo")),
            // `--key=value` is one token
            ("grep --max-count=1 Foo src/", ident("Foo")),
            ("rg --type=rust Foo src/", ident("Foo")),
            // `-E` is a plain flag for grep, value-taking for rg
            ("grep -E Foo src/", ident("Foo")),
            // `-e` / `--regexp` value is the pattern
            ("grep -e Foo src/", ident("Foo")),
            ("grep -rn -e Foo src/", ident("Foo")),
            ("grep -eFoo src/", ident("Foo")),
            ("grep --regexp Foo src/", ident("Foo")),
            ("grep -e 'fn main' src/", None),
            ("grep -e Foo -e Bar src/", ident("Foo")),
            ("grep Foo -e Bar src/", ident("Bar")),
            // `--` ends options
            ("grep -r Foo -- src/", ident("Foo")),
            ("grep -r -- Foo src/", ident("Foo")),
            // the value is not mistaken for a pattern
            ("grep -A 3 'fn main' src/", None),
            ("rg -t rust 'fn main' src/", None),
        ]);
    }

    #[test]
    fn env_assignments_and_wrappers_are_unwrapped() {
        check_bash(&[
            ("NAME=\"quoted value\" grep -rn Foo src/", ident("Foo")),
            ("A=1 B='x y' rg Foo src/", ident("Foo")),
            ("sudo grep Foo src/", ident("Foo")),
            ("sudo -E grep Foo src/", ident("Foo")),
            ("env grep Foo src/", ident("Foo")),
            ("env FOO=1 grep Foo src/", ident("Foo")),
            ("env -i FOO=1 BAR=\"a b\" grep Foo src/", ident("Foo")),
            ("command grep Foo src/", ident("Foo")),
            ("nice grep Foo src/", ident("Foo")),
            ("time grep Foo src/", ident("Foo")),
            ("time rg Foo", ident("Foo")),
            ("sudo env FOO=1 nice grep -r Foo", ident("Foo")),
            ("git grep Foo src/", ident("Foo")),
            ("git grep -n Foo src/", ident("Foo")),
            ("git grep -n 'fn main' src/", None),
            // xargs: the file list comes from the producer segment
            ("find src -name '*.rs' | xargs grep Foo", ident("Foo")),
            ("find . -name '*.rs' | xargs -0 grep -n Foo", ident("Foo")),
            ("git ls-files src | xargs rg Foo", ident("Foo")),
            ("cat list.txt | xargs grep Foo", None),
            ("xargs grep Foo src/", ident("Foo")),
            ("xargs grep Foo", None),
            // wrappers are only unwrapped at the front
            ("ls sudo grep Foo src/", None),
        ]);
    }

    #[test]
    fn cwd_search_without_target() {
        check_bash(&[
            // rg recurses by default
            ("rg Foo", ident("Foo")),
            ("rg -n Foo", ident("Foo")),
            ("rg -n 'fn main'", None),
            // grep family needs a recursive flag
            ("grep -r Foo", ident("Foo")),
            ("grep -R Foo", ident("Foo")),
            ("grep -rn Foo", ident("Foo")),
            ("grep -nr Foo", ident("Foo")),
            ("grep -rnA3 Foo", ident("Foo")),
            ("grep --recursive Foo", ident("Foo")),
            ("grep -rn Config --include=*.rs", ident("Config")),
            ("ugrep -r Foo", ident("Foo")),
            ("git grep -r Foo", ident("Foo")),
            ("grep Foo", None),
            ("grep -n Foo", None),
            ("ugrep Foo", None),
            ("git grep Foo", None),
            // `-r` is `--replace` for rg, never a recursive flag for grep
            ("grep -e Foo -r", ident("Foo")),
            // pipe consumers stay silent
            ("cargo test | grep FAILED", None),
            ("cargo test | grep -r FAILED", None),
            ("cargo test | rg FAILED", None),
            ("cargo test 2>&1 | rg -n FAILED", None),
            // first segment of a pipeline, and after `;`, `&&`, `||`
            ("rg Foo | head", ident("Foo")),
            ("grep -r Foo | wc -l", ident("Foo")),
            ("ls; rg Foo", ident("Foo")),
            ("ls && rg Foo", ident("Foo")),
            ("ls || grep -r Foo", ident("Foo")),
            ("cargo build 2>&1 | head; grep -rn Foo", ident("Foo")),
        ]);
    }

    #[test]
    fn source_target_rule() {
        check_bash(&[
            // absolute paths: the source-dir rule does not apply
            ("grep Foo /var/lib/apt/x", None),
            ("grep -n Foo /usr/lib/x.so", None),
            ("grep Foo $HOME/.cache/test/x", None),
            ("grep Foo ~/lib/x", None),
            ("grep -r Foo /usr/src", None),
            // ... but the extension rule still does
            ("grep Foo /home/me/proj/src/main.rs", ident("Foo")),
            ("grep Foo ~/proj/lib.rs", ident("Foo")),
            ("grep Foo $HOME/proj/app.ts", ident("Foo")),
            // relative paths
            ("grep -rn Foo ./", ident("Foo")),
            ("grep -rn Foo ./src", ident("Foo")),
            ("grep -n Foo ./main.rs", ident("Foo")),
            ("grep -rn Foo src/", ident("Foo")),
            ("grep -rn Foo foo/lib/bar", ident("Foo")),
            ("grep -rn Foo docs/", None),
            ("grep -n Foo notes.txt", None),
            ("grep -n Foo main.rs/", ident("Foo")),
            ("grep -rn Foo target/debug", None),
            // the extension rule looks at the last component only
            ("grep -rn Foo /opt/x.rs/data", None),
            // stop-list: exact, case-sensitive
            ("grep -rn TODO src/", None),
            ("grep -rn FIXME src/", None),
            ("grep -rn XXX src/", None),
            ("grep -rn HACK src/", None),
            ("grep -rn NOTE src/", None),
            ("grep -rn BUG src/", None),
            ("rg TODO", None),
            ("rg -n 'TODO' .", None),
            ("grep -rn todo src/", ident("todo")),
            ("grep -rn TODOS src/", ident("TODOS")),
        ]);
    }

    #[test]
    fn overridden_src_dirs() {
        let input = bash("rg -n Config crates/foo");
        assert!(decide("Bash", &input, None, &["src"]).is_none());
        assert!(decide("Bash", &input, None, DEFAULT_SRC_DIRS).is_some());
        // Extensions and bare `.` still apply with an overridden list.
        assert!(decide("Bash", &bash("rg Config foo.rs"), None, &["src"]).is_some());
        assert!(decide("Bash", &bash("rg Config ."), None, &["src"]).is_some());
    }

    #[test]
    fn grep_tool_scenarios() {
        let cases: &[(Value, Option<Detection>)] = &[
            (
                json!({ "pattern": "LspClientApi" }),
                Some(Detection::GrepTool("LspClientApi".into())),
            ),
            (json!({ "pattern": "fn \\w+\\(" }), None),
            (json!({ "pattern": "" }), None),
            (json!({ "pattern": 42 }), None),
            (json!({}), None),
            // stop-list
            (json!({ "pattern": "TODO" }), None),
            (json!({ "pattern": "FIXME", "path": "src" }), None),
            // path: same source-target rule as Bash; absent = cwd
            (
                json!({ "pattern": "Foo", "path": "src" }),
                Some(Detection::GrepTool("Foo".into())),
            ),
            (
                json!({ "pattern": "Foo", "path": "." }),
                Some(Detection::GrepTool("Foo".into())),
            ),
            (
                json!({ "pattern": "Foo", "path": "./src" }),
                Some(Detection::GrepTool("Foo".into())),
            ),
            (
                json!({ "pattern": "Foo", "path": "crates/foo/lib.rs" }),
                Some(Detection::GrepTool("Foo".into())),
            ),
            (
                json!({ "pattern": "Foo", "path": "/home/me/proj/src/main.rs" }),
                Some(Detection::GrepTool("Foo".into())),
            ),
            (json!({ "pattern": "Foo", "path": "/var/lib/apt" }), None),
            (json!({ "pattern": "Foo", "path": "docs" }), None),
            (json!({ "pattern": "Foo", "path": "README.md" }), None),
            (json!({ "pattern": "Foo", "path": 42 }), None),
            // glob: must end with a source extension
            (
                json!({ "pattern": "Foo", "glob": "*.rs" }),
                Some(Detection::GrepTool("Foo".into())),
            ),
            (
                json!({ "pattern": "Foo", "glob": "**/*.ts" }),
                Some(Detection::GrepTool("Foo".into())),
            ),
            (json!({ "pattern": "Foo", "glob": "*.md" }), None),
            (json!({ "pattern": "Foo", "glob": "*.{rs,ts}" }), None),
            (json!({ "pattern": "Foo", "glob": "Cargo.*" }), None),
            // type: must be a source language
            (
                json!({ "pattern": "Foo", "type": "rust" }),
                Some(Detection::GrepTool("Foo".into())),
            ),
            (
                json!({ "pattern": "Foo", "type": "typescript" }),
                Some(Detection::GrepTool("Foo".into())),
            ),
            (
                json!({ "pattern": "Foo", "type": "py" }),
                Some(Detection::GrepTool("Foo".into())),
            ),
            (json!({ "pattern": "Foo", "type": "md" }), None),
            (json!({ "pattern": "Foo", "type": "toml" }), None),
            (json!({ "pattern": "Foo", "type": "Rust" }), None),
            // every present scope field must agree
            (
                json!({ "pattern": "Foo", "path": "src", "glob": "*.rs", "type": "rust" }),
                Some(Detection::GrepTool("Foo".into())),
            ),
            (
                json!({ "pattern": "Foo", "path": "src", "glob": "*.md" }),
                None,
            ),
            (
                json!({ "pattern": "Foo", "path": "docs", "type": "rust" }),
                None,
            ),
        ];
        for (input, expected) in cases {
            assert_eq!(
                decide("Grep", input, None, DEFAULT_SRC_DIRS),
                *expected,
                "input: {input}"
            );
        }
    }

    #[test]
    fn unrelated_or_malformed_events() {
        let d = |tool: &str, input: &Value| decide(tool, input, None, DEFAULT_SRC_DIRS);
        assert_eq!(d("Edit", &json!({ "file_path": "src/main.rs" })), None);
        assert_eq!(d("Bash", &json!({})), None);
        assert_eq!(d("Bash", &json!({ "command": 1 })), None);
        assert_eq!(d("Bash", &Value::Null), None);
        for command in ["", "FOO=1", "sudo", "git", "grep -e", "grep -A", "xargs"] {
            assert_eq!(d("Bash", &bash(command)), None, "command: {command}");
        }
    }

    #[test]
    fn absolute_paths_inside_cwd() {
        let cwd = Some("/home/x/proj");
        let grep = |path: &str, cwd: Option<&str>| {
            decide(
                "Grep",
                &json!({ "pattern": "Foo", "path": path }),
                cwd,
                DEFAULT_SRC_DIRS,
            )
            .is_some()
        };
        let sh = |command: &str, cwd: Option<&str>| {
            decide("Bash", &bash(command), cwd, DEFAULT_SRC_DIRS).is_some()
        };
        // Grep tool: an absolute path under cwd is relativized
        assert!(grep("/home/x/proj/src", cwd));
        assert!(grep("/home/x/proj/src/", cwd));
        assert!(grep("/home/x/proj", cwd));
        assert!(grep("/home/x/proj/", Some("/home/x/proj/")));
        assert!(grep("/home/x/proj/src", Some("/home/x/proj/")));
        assert!(grep("/home/x/proj/foo/tests/bar", cwd));
        assert!(!grep("/home/x/proj/src", Some("/home/y/other")));
        assert!(!grep("/home/x/proj/docs", cwd));
        assert!(!grep("/home/x/project/src", cwd));
        assert!(!grep("/var/lib/apt", cwd));
        assert!(grep("/home/y/other/main.rs", cwd));
        // Grep tool without cwd: last-component fallback
        assert!(grep("/home/x/proj/src", None));
        assert!(grep("/usr/lib", None));
        assert!(!grep("/var/lib/apt", None));
        assert!(!grep("/home/x/proj/src/sub", None));
        // Bash: absolute targets under cwd are relativized, no fallback
        assert!(sh("grep -rn Foo /home/x/proj/src", cwd));
        assert!(sh("rg Foo /home/x/proj", cwd));
        assert!(sh("grep -rn Foo /home/x/proj/crates/a", cwd));
        assert!(!sh("grep -rn Foo /home/x/proj/src", Some("/home/y/other")));
        assert!(!sh("grep -rn Foo /home/x/proj/src", None));
        assert!(!sh("grep -rn Foo /home/x/proj/docs", cwd));
        assert!(!sh("grep Foo /var/lib/apt/lists/x", cwd));
        assert!(!sh("grep Foo /var/lib/apt/lists/x", None));
        assert!(sh("grep Foo /var/lib/x.rs", cwd));
        // an empty cwd never relativizes
        assert!(!sh("grep -rn Foo /home/x/proj/src", Some("")));
    }

    #[test]
    fn detection_labels() {
        assert_eq!(Detection::SbQuery("callers".into()).label(), "sb callers");
        assert_eq!(
            Detection::IdentifierGrep("stop_server".into()).label(),
            "grep for identifier 'stop_server'"
        );
        assert_eq!(
            Detection::GrepTool("Foo".into()).label(),
            "Grep for identifier 'Foo'"
        );
    }

    #[test]
    fn grep_arg_parsing() {
        let words = |s: &str| segments(s).remove(0).words;
        let parsed = parse_grep_args(GrepKind::Grep, &words("-rnA3 -e Foo src/ tests/"));
        assert_eq!(
            parsed,
            GrepArgs {
                pattern: Some("Foo".into()),
                targets: vec!["src/".into(), "tests/".into()],
                recursive: true,
            }
        );
        let parsed = parse_grep_args(GrepKind::Rg, &words("-r x -t rust Foo"));
        assert_eq!(
            parsed,
            GrepArgs {
                pattern: Some("Foo".into()),
                targets: vec![],
                recursive: false,
            }
        );
        let parsed = parse_grep_args(GrepKind::Grep, &words("-rn 'fn main' . 2>/dev/null"));
        assert_eq!(
            parsed,
            GrepArgs {
                pattern: Some("fn main".into()),
                targets: vec![".".into()],
                recursive: true,
            }
        );
    }

    #[test]
    fn tokenizer_keeps_quoted_strings_and_splits_operators() {
        let segs = segments("a 'b c' \"d|e\"&&f;g||h | i");
        let texts: Vec<Vec<&str>> = segs
            .iter()
            .map(|s| s.words.iter().map(|w| w.text.as_str()).collect())
            .collect();
        assert_eq!(
            texts,
            vec![
                vec!["a", "b c", "d|e"],
                vec!["f"],
                vec!["g"],
                vec!["h"],
                vec!["i"]
            ]
        );
        assert!(segs[0].words[1].quoted);
        assert!(!segs[0].words[0].quoted);
        let piped: Vec<bool> = segs.iter().map(|s| s.piped).collect();
        assert_eq!(piped, vec![false, false, false, false, true]);
    }
}
