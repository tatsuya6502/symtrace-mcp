//! Pure string functions for the `<!-- symtrace-mcp:begin v=… -->` …
//! `<!-- symtrace-mcp:end -->` block in CLAUDE.md and Explore.md.
//!
//! Everything here works on `&str` and never touches the filesystem, so
//! each state transition is unit-tested below. Other tools' marker blocks
//! (ast-bro uses the same shape with its own name) are left untouched.

/// Prefix shared by every begin marker, whatever its version.
pub const BEGIN_PREFIX: &str = "<!-- symtrace-mcp:begin";
/// The end marker (no version).
pub const END_MARKER: &str = "<!-- symtrace-mcp:end -->";

/// The begin marker for a given version.
pub fn begin_marker(version: &str) -> String {
    format!("{BEGIN_PREFIX} v={version} -->")
}

/// Render a full block: begin line, body (normalised to end with one
/// newline), end line with trailing newline.
pub fn render(version: &str, body: &str) -> String {
    let body = body.trim_end_matches('\n');
    format!("{}\n{body}\n{END_MARKER}\n", begin_marker(version))
}

/// A located block inside a string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block<'a> {
    /// Byte offset of the begin marker.
    pub start: usize,
    /// Byte offset just past the end marker line (including its newline
    /// when present).
    pub end: usize,
    /// Version from the begin marker, `None` when it carries no `v=`.
    pub version: Option<&'a str>,
    /// Text between the begin line and the end marker.
    pub body: &'a str,
}

/// Result of looking for our block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found<'a> {
    /// No begin or end marker at all.
    None,
    /// Exactly one well-formed block.
    Block(Block<'a>),
    /// Markers exist but do not form one block (begin without end, end
    /// before begin, several begins, …). `span` is the widest range that
    /// covers every marker, used by a forced replacement.
    Damaged { span: (usize, usize) },
}

/// How an existing block compares to the running binary's template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    Absent,
    Current,
    /// Begin marker names another version; body not verified.
    Outdated(String),
    /// Same version with a different body, an unversioned marker, or
    /// damaged markers.
    Modified,
}

/// Locate the block.
pub fn find(text: &str) -> Found<'_> {
    let begins: Vec<usize> = text.match_indices(BEGIN_PREFIX).map(|(i, _)| i).collect();
    let ends: Vec<usize> = text.match_indices(END_MARKER).map(|(i, _)| i).collect();
    if begins.is_empty() && ends.is_empty() {
        return Found::None;
    }
    // Widest span: from the line start of the first marker to the line end
    // of the last one. A begin marker runs to the end of its line (its
    // version suffix has no fixed length); an end marker is the literal.
    let damaged = |text: &str| {
        let first = begins.iter().chain(ends.iter()).copied().min().unwrap_or(0);
        let first = text[..first].rfind('\n').map_or(0, |i| i + 1);
        let last_marker = begins
            .iter()
            .map(|&b| text[b..].find('\n').map_or(text.len(), |n| b + n))
            .chain(ends.iter().map(|&e| e + END_MARKER.len()))
            .max()
            .unwrap_or(text.len());
        Found::Damaged {
            span: (first, line_end_after(text, last_marker)),
        }
    };
    if begins.len() != 1 || ends.len() != 1 || ends[0] < begins[0] {
        return damaged(text);
    }
    let start = begins[0];
    let end_marker = ends[0];
    // Begin line must be a complete line: `<!-- symtrace-mcp:begin … -->\n`.
    let Some(begin_line_len) = text[start..].find('\n') else {
        return damaged(text);
    };
    let begin_line = &text[start..start + begin_line_len];
    if !begin_line.trim_end().ends_with("-->") || start + begin_line_len > end_marker {
        return damaged(text);
    }
    let version = parse_version(begin_line);
    let body = &text[start + begin_line_len + 1..end_marker];
    let end = line_end_after(text, end_marker + END_MARKER.len());
    Found::Block(Block {
        start,
        end,
        version,
        body,
    })
}

/// Offset just past the newline that follows `pos` (or `pos` itself when
/// the text ends there / no newline follows).
fn line_end_after(text: &str, pos: usize) -> usize {
    match text[pos..].find('\n') {
        Some(n) if text[pos..pos + n].trim().is_empty() => pos + n + 1,
        _ => pos,
    }
}

fn parse_version(begin_line: &str) -> Option<&str> {
    let rest = begin_line[BEGIN_PREFIX.len()..].trim();
    let rest = rest.strip_suffix("-->")?.trim();
    let v = rest.strip_prefix("v=")?;
    if v.is_empty() || v.contains(char::is_whitespace) {
        None
    } else {
        Some(v)
    }
}

/// Classify the block in `text` against the current version and template
/// body (`template` is the raw body without markers).
pub fn classify(text: &str, current_version: &str, template: &str) -> State {
    match find(text) {
        Found::None => State::Absent,
        Found::Damaged { .. } => State::Modified,
        Found::Block(b) => match b.version {
            None => State::Modified,
            Some(v) if v != current_version => State::Outdated(v.to_string()),
            Some(_) => {
                let body = b.body.replace("\r\n", "\n");
                if body.trim_end_matches('\n') == template.trim_end_matches('\n') {
                    State::Current
                } else {
                    State::Modified
                }
            }
        },
    }
}

/// Line ending `text` uses: CRLF as soon as it contains one, LF otherwise.
/// The block is rendered with LF and converted on insertion so a CRLF file
/// stays consistent (and keeps classifying as `Current`).
pub fn line_ending(text: &str) -> &'static str {
    if text.contains("\r\n") { "\r\n" } else { "\n" }
}

/// `block` (rendered with LF) converted to `eol`.
fn with_line_ending(block: &str, eol: &str) -> String {
    if eol == "\n" {
        block.to_string()
    } else {
        block.replace('\n', eol)
    }
}

/// Append `block` (already rendered) at the end of `text` with exactly one
/// blank line in between (none when `text` is empty) and a trailing newline,
/// using the line ending of `text`.
pub fn insert_at_end(text: &str, block: &str) -> String {
    let eol = line_ending(text);
    let head = text.trim_end_matches(['\r', '\n']);
    let mut out = String::with_capacity(head.len() + block.len() + 4);
    if !head.is_empty() {
        out.push_str(head);
        out.push_str(eol);
        out.push_str(eol);
    }
    out.push_str(&with_line_ending(block, eol));
    if !out.ends_with('\n') {
        out.push_str(eol);
    }
    out
}

/// Replace the byte range `span` with `block`, using the line ending of
/// `text`.
pub fn replace_span(text: &str, span: (usize, usize), block: &str) -> String {
    let eol = line_ending(text);
    let mut out = String::with_capacity(text.len() + block.len() + 2);
    out.push_str(&text[..span.0]);
    out.push_str(&with_line_ending(block, eol));
    if !out.ends_with('\n') {
        out.push_str(eol);
    }
    out.push_str(&text[span.1..]);
    out
}

/// Replace the existing well-formed block in place. Returns `None` when
/// there is no well-formed block (use `find` + `replace_span` for damaged
/// markers under `--force`).
pub fn replace_in_place(text: &str, block: &str) -> Option<String> {
    match find(text) {
        Found::Block(b) => Some(replace_span(text, (b.start, b.end), block)),
        _ => None,
    }
}

/// Remove the block and the blank line that precedes it. Every other byte
/// is preserved. Returns `None` when there is no well-formed block.
pub fn remove(text: &str) -> Option<String> {
    let Found::Block(b) = find(text) else {
        return None;
    };
    let mut head = &text[..b.start];
    if head.ends_with("\r\n\r\n") {
        head = &head[..head.len() - 2];
    } else if head.ends_with("\n\n") {
        head = &head[..head.len() - 1];
    }
    let mut out = String::with_capacity(text.len());
    out.push_str(head);
    out.push_str(&text[b.end..]);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const V: &str = "1.2.3";
    const BODY: &str = "## Title\n\nline one\nline two\n";
    const AST_BRO: &str = "<!-- ast-bro:begin v=4.2.0 -->\n## sb\n\nuse sb\n<!-- ast-bro:end -->\n";

    fn block() -> String {
        render(V, BODY)
    }

    #[test]
    fn render_shape() {
        assert_eq!(
            block(),
            "<!-- symtrace-mcp:begin v=1.2.3 -->\n## Title\n\nline one\nline two\n<!-- symtrace-mcp:end -->\n"
        );
        // Body without a trailing newline renders identically.
        assert_eq!(render(V, BODY.trim_end()), block());
    }

    #[test]
    fn insert_then_classify_is_current() {
        let text = insert_at_end("", &block());
        assert_eq!(text, block());
        assert_eq!(classify(&text, V, BODY), State::Current);
    }

    #[test]
    fn classify_states() {
        assert_eq!(classify("", V, BODY), State::Absent);
        assert_eq!(classify("# hi\n", V, BODY), State::Absent);
        assert_eq!(
            classify(&render("0.0.1", BODY), V, BODY),
            State::Outdated("0.0.1".into())
        );
        assert_eq!(classify(&render(V, "edited\n"), V, BODY), State::Modified);
        // Unversioned marker (the pre-setup hand-written form).
        let unversioned = format!("{BEGIN_PREFIX} -->\n{BODY}{END_MARKER}\n");
        assert_eq!(classify(&unversioned, V, BODY), State::Modified);
        // Damaged: begin without end, end without begin, two begins.
        assert_eq!(classify(&begin_marker(V), V, BODY), State::Modified);
        assert_eq!(classify(END_MARKER, V, BODY), State::Modified);
        let twice = format!("{}{}", block(), block());
        assert_eq!(classify(&twice, V, BODY), State::Modified);
    }

    #[test]
    fn insert_blank_line_rules() {
        let b = block();
        assert_eq!(insert_at_end("x", &b), format!("x\n\n{b}"));
        assert_eq!(insert_at_end("x\n", &b), format!("x\n\n{b}"));
        assert_eq!(insert_at_end("x\n\n\n", &b), format!("x\n\n{b}"));
        assert_eq!(insert_at_end(AST_BRO, &b), format!("{AST_BRO}\n{b}"));
    }

    #[test]
    fn remove_round_trips() {
        let b = block();
        for s in ["", "x", "x\n", AST_BRO, "a\n\nb\n"] {
            let inserted = insert_at_end(s, &b);
            let expected = if s.is_empty() {
                String::new()
            } else if s.ends_with('\n') {
                s.to_string()
            } else {
                format!("{s}\n")
            };
            assert_eq!(remove(&inserted).unwrap(), expected, "input {s:?}");
        }
        assert_eq!(remove("x\n"), None);
    }

    #[test]
    fn remove_preserves_following_bytes() {
        let b = block();
        let text = format!("head\n\n{b}\ntail\n");
        assert_eq!(remove(&text).unwrap(), "head\n\ntail\n");
        // Block without trailing newline at EOF.
        let text = format!("head\n\n{}", b.trim_end());
        assert_eq!(remove(&text).unwrap(), "head\n");
    }

    #[test]
    fn replace_in_place_keeps_surroundings() {
        let old = render("0.0.1", "old body\n");
        let text = format!("{AST_BRO}\n{old}\ntrailer\n");
        let new = replace_in_place(&text, &block()).unwrap();
        assert_eq!(new, format!("{AST_BRO}\n{}\ntrailer\n", block()));
        assert_eq!(classify(&new, V, BODY), State::Current);
        assert!(new.contains("<!-- ast-bro:begin v=4.2.0 -->"));
        assert_eq!(replace_in_place("nothing", &block()), None);
    }

    #[test]
    fn coexists_with_ast_bro_block() {
        let text = insert_at_end(AST_BRO, &block());
        let Found::Block(b) = find(&text) else {
            panic!("expected block")
        };
        assert_eq!(b.version, Some(V));
        assert_eq!(b.body, BODY);
        assert_eq!(&text[..b.start], format!("{AST_BRO}\n"));
        let removed = remove(&text).unwrap();
        assert_eq!(removed, AST_BRO);
    }

    #[test]
    fn damaged_span_covers_all_markers() {
        let twice = format!("pre\n{}{}post\n", block(), block());
        let Found::Damaged { span } = find(&twice) else {
            panic!("expected damaged")
        };
        assert_eq!(&twice[..span.0], "pre\n");
        assert_eq!(&twice[span.1..], "post\n");
        let fixed = replace_span(&twice, span, &block());
        assert_eq!(fixed, format!("pre\n{}post\n", block()));
        assert_eq!(classify(&fixed, V, BODY), State::Current);
    }

    #[test]
    fn damaged_span_extends_begin_marker_to_line_end() {
        let stray = format!("{} -->", begin_marker("0.5.1"));
        // Begin without end: the begin line goes; the old body stays.
        let text = format!("pre\n{stray}\nold body\npost\n");
        let Found::Damaged { span } = find(&text) else {
            panic!("expected damaged")
        };
        assert_eq!(&text[..span.0], "pre\n");
        assert_eq!(&text[span.1..], "old body\npost\n");
        let fixed = replace_span(&text, span, &block());
        assert_eq!(fixed, format!("pre\n{}old body\npost\n", block()));
        assert_eq!(classify(&fixed, V, BODY), State::Current);

        // End before begin: everything between (and both lines) goes.
        let text = format!("pre\n{END_MARKER}\nmid\n{stray}\npost\n");
        let Found::Damaged { span } = find(&text) else {
            panic!("expected damaged")
        };
        assert_eq!(&text[..span.0], "pre\n");
        assert_eq!(&text[span.1..], "post\n");
        let fixed = replace_span(&text, span, &block());
        assert_eq!(fixed, format!("pre\n{}post\n", block()));
        assert_eq!(classify(&fixed, V, BODY), State::Current);

        // A stray begin after a well-formed block.
        let text = format!("pre\n{}\n{stray}\npost\n", block());
        let Found::Damaged { span } = find(&text) else {
            panic!("expected damaged")
        };
        assert_eq!(&text[..span.0], "pre\n");
        assert_eq!(&text[span.1..], "post\n");
        let fixed = replace_span(&text, span, &block());
        assert_eq!(fixed, format!("pre\n{}post\n", block()));
        assert_eq!(classify(&fixed, V, BODY), State::Current);

        // Begin marker at EOF without a newline, and a marker mid-line.
        let text = format!("pre\n{stray}");
        let Found::Damaged { span } = find(&text) else {
            panic!("expected damaged")
        };
        assert_eq!(span, (4, text.len()));
        let text = format!("pre {stray}\npost\n");
        let Found::Damaged { span } = find(&text) else {
            panic!("expected damaged")
        };
        assert_eq!(span, (0, text.len() - "post\n".len()));
    }

    #[test]
    fn crlf_block_classifies_as_current() {
        let crlf = block().replace('\n', "\r\n");
        assert_eq!(classify(&crlf, V, BODY), State::Current);
        let text = format!("# notes\r\n\r\n{crlf}");
        assert_eq!(classify(&text, V, BODY), State::Current);
        let edited = crlf.replace("line one", "edited");
        assert_eq!(classify(&edited, V, BODY), State::Modified);
        assert_eq!(
            classify(&render("0.0.1", BODY).replace('\n', "\r\n"), V, BODY),
            State::Outdated("0.0.1".into())
        );
    }

    #[test]
    fn crlf_file_gets_crlf_block() {
        let crlf = block().replace('\n', "\r\n");
        assert_eq!(line_ending("a\r\nb\r\n"), "\r\n");
        assert_eq!(line_ending("a\nb\n"), "\n");
        assert_eq!(line_ending(""), "\n");

        let text = insert_at_end("# notes\r\n", &block());
        assert_eq!(text, format!("# notes\r\n\r\n{crlf}"));
        assert!(!text.contains("-->\n"), "no bare LF: {text:?}");
        assert_eq!(classify(&text, V, BODY), State::Current);

        // Replace keeps CRLF too.
        let old = render("0.0.1", "old\n").replace('\n', "\r\n");
        let text = format!("head\r\n\r\n{old}\r\ntail\r\n");
        let new = replace_in_place(&text, &block()).unwrap();
        assert_eq!(new, format!("head\r\n\r\n{crlf}\r\ntail\r\n"));
        assert_eq!(classify(&new, V, BODY), State::Current);
    }

    #[test]
    fn crlf_remove_leaves_rest_intact() {
        let text = insert_at_end("# notes\r\nmore\r\n", &block());
        assert_eq!(remove(&text).unwrap(), "# notes\r\nmore\r\n");
        let crlf = block().replace('\n', "\r\n");
        let text = format!("head\r\n\r\n{crlf}\r\ntail\r\n");
        assert_eq!(remove(&text).unwrap(), "head\r\n\r\ntail\r\n");
        assert_eq!(remove(&crlf).unwrap(), "");
    }

    #[test]
    fn version_parsing() {
        assert_eq!(
            parse_version("<!-- symtrace-mcp:begin v=0.5.1 -->"),
            Some("0.5.1")
        );
        assert_eq!(parse_version("<!-- symtrace-mcp:begin -->"), None);
        assert_eq!(parse_version("<!-- symtrace-mcp:begin v= -->"), None);
    }
}
