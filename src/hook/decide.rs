//! Pure decision logic for the `hook` subcommand.
//!
//! `decide` inspects one Claude Code `PreToolUse` event and reports whether
//! the call is a text/tree-sitter heuristic for a semantic question that the
//! symtrace-mcp tools answer better. It has no I/O and reads no environment
//! variables, so every spec scenario is a table-driven test.

use serde_json::Value;

/// Default source directories: a grep targeting a path with one of these
/// components counts as a source-code search.
pub const DEFAULT_SRC_DIRS: &[&str] = &[
    "src", "crates", "lib", "tests", "test", "app", "apps", "packages", "pkg", "internal", "cmd",
];

/// File extensions that mark a grep target as source code.
const SRC_EXTS: &[&str] = &[
    "rs", "ts", "tsx", "js", "jsx", "mjs", "cjs", "py", "go", "java", "kt", "kts", "rb", "c", "cc",
    "cpp", "h", "hpp", "cs", "swift", "scala",
];

const SB_PROGRAMS: &[&str] = &["sb", "ast-bro"];
const SB_SUBCOMMANDS: &[&str] = &["callers", "callees", "implements", "impact", "trace"];
const GREP_PROGRAMS: &[&str] = &["grep", "rg", "ugrep"];

/// What the hook detected in a tool call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Detection {
    /// `sb <subcommand>` / `ast-bro <subcommand>` call-graph query.
    SbQuery(String),
    /// `grep` / `rg` / `ugrep` for a bare identifier over source code.
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
/// `src_dirs` is the list of directory names that make a grep count as a
/// source-code search (see [`DEFAULT_SRC_DIRS`]).
pub fn decide(tool_name: &str, tool_input: &Value, src_dirs: &[&str]) -> Option<Detection> {
    match tool_name {
        "Bash" => {
            let command = tool_input.get("command")?.as_str()?;
            decide_bash(command, src_dirs)
        }
        "Grep" => {
            let pattern = tool_input.get("pattern")?.as_str()?;
            is_identifier(pattern).then(|| Detection::GrepTool(pattern.to_string()))
        }
        _ => None,
    }
}

fn decide_bash(command: &str, src_dirs: &[&str]) -> Option<Detection> {
    segments(command)
        .into_iter()
        .find_map(|segment| decide_segment(&segment, src_dirs))
}

fn decide_segment(words: &[Word], src_dirs: &[&str]) -> Option<Detection> {
    // Skip leading environment assignments such as `FOO=1 grep ...`.
    let start = words
        .iter()
        .position(|w| !w.is_env_assignment())
        .unwrap_or(words.len());
    let words = &words[start..];
    let program = words.first()?;
    let name = program_name(&program.text);

    if SB_PROGRAMS.contains(&name) {
        let sub = words.get(1)?;
        return SB_SUBCOMMANDS
            .contains(&sub.text.as_str())
            .then(|| Detection::SbQuery(sub.text.clone()));
    }

    if GREP_PROGRAMS.contains(&name) {
        // The pattern is the first token after the program that is not an
        // option. `--key=value` options whose value is the pattern are not
        // modelled (accepted false negative).
        let pattern_idx = words[1..]
            .iter()
            .position(|w| w.quoted || !w.text.starts_with('-'))
            .map(|i| i + 1)?;
        let pattern = &words[pattern_idx];
        if !is_identifier(&pattern.text) {
            return None;
        }
        let targets_source = words[pattern_idx + 1..]
            .iter()
            .any(|w| is_source_target(&w.text, src_dirs));
        return targets_source.then(|| Detection::IdentifierGrep(pattern.text.clone()));
    }

    None
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

/// Source-target rule: a path component equal to one of `src_dirs`, a file
/// with a source extension (also `--include=*.rs`), or the bare path `.`.
/// Redirections (`2>/dev/null`, `<in`) never count.
fn is_source_target(token: &str, src_dirs: &[&str]) -> bool {
    if token.contains('>') || token.contains('<') {
        return false;
    }
    if token == "." {
        return true;
    }
    if token
        .split('/')
        .any(|component| src_dirs.contains(&component))
    {
        return true;
    }
    match token.rsplit_once('.') {
        Some((stem, ext)) => !stem.is_empty() && SRC_EXTS.contains(&ext),
        None => false,
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
    fn is_env_assignment(&self) -> bool {
        if self.quoted {
            return false;
        }
        match self.text.split_once('=') {
            Some((name, _)) => is_identifier(name),
            None => false,
        }
    }
}

/// Split a command into segments on `||`, `&&`, `;` and `|` (outside quotes),
/// tokenizing each segment into words. Single and double quotes group a word;
/// no escape handling beyond that. A lone `&` is treated as a word character
/// so that `2>&1` stays one token.
fn segments(command: &str) -> Vec<Vec<Word>> {
    let mut result = Vec::new();
    let mut segment = Vec::new();
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
                    result.push(std::mem::take(&mut segment));
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
    result.push(segment);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn bash(command: &str) -> Value {
        json!({ "command": command })
    }

    #[test]
    fn bash_scenarios_with_default_dirs() {
        let cases: &[(&str, Option<Detection>)] = &[
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
            (
                "grep -rn stop_server src/",
                Some(Detection::IdentifierGrep("stop_server".into())),
            ),
            (
                "sed -n 1,3p x; grep -rn \"stop_server\" src/ tests/ 2>/dev/null",
                Some(Detection::IdentifierGrep("stop_server".into())),
            ),
            (
                "grep -rn 'stop_server' ./crates/foo",
                Some(Detection::IdentifierGrep("stop_server".into())),
            ),
            (
                "grep -n LspClientApi client.rs",
                Some(Detection::IdentifierGrep("LspClientApi".into())),
            ),
            (
                "rg -n Config crates/foo",
                Some(Detection::IdentifierGrep("Config".into())),
            ),
            (
                "rg Config .",
                Some(Detection::IdentifierGrep("Config".into())),
            ),
            (
                "grep -rn Config --include=*.rs .",
                Some(Detection::IdentifierGrep("Config".into())),
            ),
            (
                "ugrep -r Foo lib",
                Some(Detection::IdentifierGrep("Foo".into())),
            ),
            (
                "/usr/bin/grep -rn Foo src",
                Some(Detection::IdentifierGrep("Foo".into())),
            ),
            // regex patterns
            ("grep -rn \"fn [a-z_]*(\" src/", None),
            ("rg 'fn\\s+\\w+' src/", None),
            // non-source targets
            ("grep -c worktrees /proc/self/mountinfo", None),
            ("grep -n Install README.md", None),
            ("grep -rn Foo 2>/dev/null", None),
            // pipe filters
            ("cargo test 2>&1 | grep FAILED", None),
            ("cargo test 2>&1 | grep -c FAILED", None),
            // grep with no pattern
            ("grep -rn", None),
            // segment splitting: pipe without a target in the grep segment
            ("ls src/ | grep Foo", None),
            (
                "ls || grep Foo src/",
                Some(Detection::IdentifierGrep("Foo".into())),
            ),
        ];
        for (command, expected) in cases {
            assert_eq!(
                decide("Bash", &bash(command), DEFAULT_SRC_DIRS),
                *expected,
                "command: {command}"
            );
        }
    }

    #[test]
    fn overridden_src_dirs() {
        let input = bash("rg -n Config crates/foo");
        assert!(decide("Bash", &input, &["src"]).is_none());
        assert!(decide("Bash", &input, DEFAULT_SRC_DIRS).is_some());
        // Extensions and bare `.` still apply with an overridden list.
        assert!(decide("Bash", &bash("rg Config foo.rs"), &["src"]).is_some());
        assert!(decide("Bash", &bash("rg Config ."), &["src"]).is_some());
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
        ];
        for (input, expected) in cases {
            assert_eq!(
                decide("Grep", input, DEFAULT_SRC_DIRS),
                *expected,
                "input: {input}"
            );
        }
    }

    #[test]
    fn unrelated_or_malformed_events() {
        assert_eq!(
            decide(
                "Edit",
                &json!({ "file_path": "src/main.rs" }),
                DEFAULT_SRC_DIRS
            ),
            None
        );
        assert_eq!(decide("Bash", &json!({}), DEFAULT_SRC_DIRS), None);
        assert_eq!(
            decide("Bash", &json!({ "command": 1 }), DEFAULT_SRC_DIRS),
            None
        );
        assert_eq!(decide("Bash", &Value::Null, DEFAULT_SRC_DIRS), None);
        assert_eq!(decide("Bash", &bash(""), DEFAULT_SRC_DIRS), None);
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
    fn tokenizer_keeps_quoted_strings_and_splits_operators() {
        let segs = segments("a 'b c' \"d|e\"&&f;g||h | i");
        let texts: Vec<Vec<&str>> = segs
            .iter()
            .map(|s| s.iter().map(|w| w.text.as_str()).collect())
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
        assert!(segs[0][1].quoted);
        assert!(!segs[0][0].quoted);
    }
}
