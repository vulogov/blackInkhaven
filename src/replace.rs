//! 1.2.22 R.1–R.2 — find/replace primitives for the project-wide,
//! review-gated replace.
//!
//! The matcher is pure: [`find_matches`] / [`apply`] / [`scan_bodies`]
//! do no I/O.  [`scan_project`] is the thin walk that reads paragraph
//! bodies from the store + hierarchy and delegates to `scan_bodies`.
//! The review modal, the snapshots, and the atomic writes stay in the
//! TUI / CLI (R.3–R.5).  Matching is lexical — literal (default),
//! word-boundary, opt-in regex, optional case-insensitive — backed by
//! the `regex` crate (already in-tree).  There is no full-text index: a
//! manuscript is one book, so the scan is linear over paragraph bodies;
//! an inverted index can neither rank-replace nor do
//! substring/regex/word-boundary, so it would not help.
//!
//! ## Why per-hit replacements are precomputed
//!
//! [`find_matches`] expands each regex match's replacement *at find
//! time* (so `$1` capture references resolve against that match's
//! captures) and stores it on the [`Hit`].  [`apply`] then just splices
//! the accepted hits' byte ranges right-to-left — it needs neither the
//! pattern nor the options, and the author can accept any subset.

use std::fmt;

use regex::RegexBuilder;

/// How the pattern is matched.  `Default` encodes the safe default used
/// by the TUI: a literal, whole-word match (no `Will`/`will` surprise).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplaceOpts {
    /// Treat the pattern as a regular expression (else literal text).
    pub regex: bool,
    /// Match only at word boundaries (`\bpattern\b`).
    pub word_boundary: bool,
    /// Case-insensitive match.
    pub ignore_case: bool,
}

impl Default for ReplaceOpts {
    fn default() -> Self {
        Self {
            regex: false,
            word_boundary: true,
            ignore_case: false,
        }
    }
}

/// One match in a text, with everything the review UI + [`apply`] need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    /// Byte range `[start, end)` of the match in the full text.
    pub start: usize,
    pub end: usize,
    /// 1-based line and 1-based char column of the match start.
    pub line: usize,
    pub col: usize,
    /// The full line containing the match start — for KWIC context /
    /// span highlighting in the review UI.
    pub line_text: String,
    /// The matched text.
    pub matched: String,
    /// The replacement for THIS match, with regex captures already
    /// expanded (in literal mode it is the replacement verbatim).
    pub replacement: String,
}

/// Why a find failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplaceError {
    /// An empty pattern matches nothing useful — rejected up front.
    EmptyPattern,
    /// The pattern didn't compile as a regex (only in regex /
    /// word-boundary mode; `e` is the engine's message).
    BadRegex(String),
}

impl fmt::Display for ReplaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReplaceError::EmptyPattern => write!(f, "empty search pattern"),
            ReplaceError::BadRegex(e) => write!(f, "invalid regex: {e}"),
        }
    }
}

impl std::error::Error for ReplaceError {}

/// Find every match of `pattern` in `text`, computing each one's
/// replacement from `repl` (regex captures expand in regex mode;
/// literal mode uses `repl` verbatim).  Matches are non-overlapping and
/// in source order; zero-width matches (e.g. from `a*`) are skipped.
// The single-text primitive: the canonical "find in one string" entry
// + test surface.  The project scan uses `find_with` (compile-once);
// kept for the API and future single-buffer callers.
#[allow(dead_code)]
pub fn find_matches(
    text: &str,
    pattern: &str,
    repl: &str,
    opts: ReplaceOpts,
) -> Result<Vec<Hit>, ReplaceError> {
    if pattern.is_empty() {
        return Err(ReplaceError::EmptyPattern);
    }
    let re = build_regex(pattern, opts)?;
    Ok(find_with(&re, text, repl, opts.regex))
}

/// The match loop over a pre-compiled regex — lets a project scan
/// compile once and reuse across every paragraph.  `expand` is
/// `opts.regex`: when set, `$1` capture references in `repl` resolve;
/// otherwise `repl` is verbatim.
fn find_with(re: &regex::Regex, text: &str, repl: &str, expand: bool) -> Vec<Hit> {
    let mut hits = Vec::new();
    for caps in re.captures_iter(text) {
        let m = caps.get(0).expect("group 0 always present");
        let (start, end) = (m.start(), m.end());
        if start == end {
            // A zero-width match isn't a meaningful replace target.
            continue;
        }
        let replacement = if expand {
            let mut out = String::new();
            caps.expand(repl, &mut out);
            out
        } else {
            repl.to_string()
        };
        let (line, col, line_text) = locate(text, start);
        hits.push(Hit {
            start,
            end,
            line,
            col,
            line_text,
            matched: m.as_str().to_string(),
            replacement,
        });
    }
    hits
}

/// Apply an accepted subset of hits to `text`, splicing right-to-left
/// by byte offset so earlier replacements don't invalidate later
/// positions.  The hits must be non-overlapping (as produced by
/// [`find_matches`]); a subset of them is fine.
pub fn apply(text: &str, accepted: &[Hit]) -> String {
    let mut hits: Vec<&Hit> = accepted.iter().collect();
    hits.sort_by(|a, b| b.start.cmp(&a.start));
    let mut out = text.to_string();
    for h in hits {
        // Guard against stale ranges (the caller's text must match the
        // one the hits were found in); skip rather than panic.
        if h.start <= h.end && h.end <= out.len() && out.is_char_boundary(h.start) && out.is_char_boundary(h.end) {
            out.replace_range(h.start..h.end, &h.replacement);
        }
    }
    out
}

// ── project scan (R.2) ────────────────────────────────────
//
// `scan_bodies` is pure (matches over supplied bodies — unit-tested);
// `scan_project` is the thin I/O layer that gathers paragraph bodies
// from the store + hierarchy and delegates.  The regex is compiled once
// and reused across every paragraph.

/// Which books a project scan covers.
#[derive(Debug, Clone)]
pub enum ScanScope {
    /// Every user (non-system) book — the default; a manuscript rename
    /// shouldn't touch Notes/Facts/Characters.
    UserBooks,
    /// User books **plus** the system books (Notes / Research / Facts /
    /// Characters / …).
    IncludingSystem,
    /// A single book (its whole subtree), by id.
    Book(uuid::Uuid),
}

/// One paragraph that has at least one match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParaMatches {
    pub para_id: uuid::Uuid,
    pub title: String,
    pub slug_path: String,
    /// The paragraph body the hits were found in — `apply` splices into
    /// exactly this, so the byte ranges stay valid.
    pub body: String,
    pub hits: Vec<Hit>,
}

/// Run the matcher over an iterator of `(id, title, slug_path, body)`
/// paragraphs, returning only those with ≥1 hit, in input order.  Pure
/// — the caller does the I/O.  Compiles the regex once.
pub fn scan_bodies<I>(
    bodies: I,
    pattern: &str,
    repl: &str,
    opts: ReplaceOpts,
) -> Result<Vec<ParaMatches>, ReplaceError>
where
    I: IntoIterator<Item = (uuid::Uuid, String, String, String)>,
{
    if pattern.is_empty() {
        return Err(ReplaceError::EmptyPattern);
    }
    let re = build_regex(pattern, opts)?;
    let mut out = Vec::new();
    for (para_id, title, slug_path, body) in bodies {
        let hits = find_with(&re, &body, repl, opts.regex);
        if !hits.is_empty() {
            out.push(ParaMatches {
                para_id,
                title,
                slug_path,
                body,
                hits,
            });
        }
    }
    Ok(out)
}

/// Walk the in-scope paragraphs, read each body from the store, and
/// scan it.  Linear, no index.  Validates the pattern up front so a bad
/// regex / empty pattern errors even on an empty project.
pub fn scan_project(
    store: &crate::store::Store,
    hierarchy: &crate::store::hierarchy::Hierarchy,
    scope: &ScanScope,
    pattern: &str,
    repl: &str,
    opts: ReplaceOpts,
) -> Result<Vec<ParaMatches>, ReplaceError> {
    if pattern.is_empty() {
        return Err(ReplaceError::EmptyPattern);
    }
    build_regex(pattern, opts)?; // validate before any walk

    let ids = paragraph_ids_in_scope(hierarchy, scope);
    let bodies = ids.into_iter().filter_map(|id| {
        let node = hierarchy.get(id)?;
        let bytes = store.get_content(id).ok().flatten()?;
        let body = String::from_utf8_lossy(&bytes).into_owned();
        Some((id, node.title.clone(), hierarchy.slug_path(node), body))
    });
    scan_bodies(bodies, pattern, repl, opts)
}

/// Outcome of applying a set of `ParaMatches`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApplyReport {
    pub paragraphs: usize,
    pub occurrences: usize,
    pub snapshots: usize,
}

/// Apply every hit in `matches` to its paragraph: **snapshot the
/// current body first** (annotated, for undo), then write the new body
/// to the `.typ` file + the store.  I/O.  The caller has already gated
/// this — the CLI behind `--yes`, the TUI behind the review's accepted
/// set — so this just executes.  File-backed paragraphs only.
pub fn apply_project(
    store: &crate::store::Store,
    hierarchy: &crate::store::hierarchy::Hierarchy,
    matches: &[ParaMatches],
    annotation: &str,
) -> Result<ApplyReport, String> {
    let mut report = ApplyReport::default();
    for pm in matches {
        if pm.hits.is_empty() {
            continue;
        }
        let Some(node) = hierarchy.get(pm.para_id) else {
            continue;
        };
        let Some(rel) = node.file.clone() else {
            continue;
        };
        let new_body = apply(&pm.body, &pm.hits);
        store
            .create_snapshot_annotated(node, pm.body.as_bytes(), annotation)
            .map_err(|e| format!("snapshot `{}`: {e}", pm.title))?;
        report.snapshots += 1;
        let abs = store.project_root().join(&rel);
        std::fs::write(&abs, new_body.as_bytes())
            .map_err(|e| format!("write `{}`: {e}", abs.display()))?;
        let mut node_mut = node.clone();
        store
            .update_paragraph_content(&mut node_mut, new_body.as_bytes())
            .map_err(|e| format!("update `{}`: {e}", pm.title))?;
        report.paragraphs += 1;
        report.occurrences += pm.hits.len();
    }
    Ok(report)
}

/// Paragraph ids in `scope`, in document order.
fn paragraph_ids_in_scope(
    hierarchy: &crate::store::hierarchy::Hierarchy,
    scope: &ScanScope,
) -> Vec<uuid::Uuid> {
    use crate::store::node::NodeKind;
    let is_paragraph = |id: &uuid::Uuid| {
        hierarchy
            .get(*id)
            .map(|n| n.kind == NodeKind::Paragraph)
            .unwrap_or(false)
    };
    match scope {
        ScanScope::Book(book_id) => hierarchy
            .collect_subtree(*book_id)
            .into_iter()
            .filter(is_paragraph)
            .collect(),
        _ => {
            let include_system = matches!(scope, ScanScope::IncludingSystem);
            let mut ids = Vec::new();
            for book in hierarchy.children_of(None) {
                if book.kind != NodeKind::Book {
                    continue;
                }
                if !include_system && book.system_tag.is_some() {
                    continue;
                }
                ids.extend(
                    hierarchy
                        .collect_subtree(book.id)
                        .into_iter()
                        .filter(is_paragraph),
                );
            }
            ids
        }
    }
}

/// Compile the matcher: literal → escaped; word-boundary → `\b(?:…)\b`;
/// case-insensitivity via the builder flag.
fn build_regex(pattern: &str, opts: ReplaceOpts) -> Result<regex::Regex, ReplaceError> {
    let body = if opts.regex {
        pattern.to_string()
    } else {
        regex::escape(pattern)
    };
    let body = if opts.word_boundary {
        format!(r"\b(?:{body})\b")
    } else {
        body
    };
    RegexBuilder::new(&body)
        .case_insensitive(opts.ignore_case)
        .build()
        .map_err(|e| ReplaceError::BadRegex(e.to_string()))
}

/// `(1-based line, 1-based char column, full line text)` of byte
/// offset `at` in `text`.
fn locate(text: &str, at: usize) -> (usize, usize, String) {
    let before = &text[..at];
    let line0 = before.bytes().filter(|&b| b == b'\n').count();
    let line_start = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let col0 = text[line_start..at].chars().count();
    let line_end = text[at..]
        .find('\n')
        .map(|i| at + i)
        .unwrap_or(text.len());
    (line0 + 1, col0 + 1, text[line_start..line_end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(regex: bool, word_boundary: bool, ignore_case: bool) -> ReplaceOpts {
        ReplaceOpts {
            regex,
            word_boundary,
            ignore_case,
        }
    }

    #[test]
    fn literal_substring_match() {
        let hits =
            find_matches("Anne went home", "Anne", "Anna", opts(false, false, false)).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].matched, "Anne");
        assert_eq!(hits[0].replacement, "Anna");
        assert_eq!((hits[0].start, hits[0].end), (0, 4));
        assert_eq!((hits[0].line, hits[0].col), (1, 1));
    }

    #[test]
    fn word_boundary_excludes_partials() {
        // "Anne" must not match inside "Anneliese".
        let text = "Anne met Anneliese";
        let wb = find_matches(text, "Anne", "Anna", opts(false, true, false)).unwrap();
        assert_eq!(wb.len(), 1, "word-boundary should match only the standalone Anne");
        assert_eq!(wb[0].start, 0);
        // Without word-boundary it's a substring → both.
        let sub = find_matches(text, "Anne", "Anna", opts(false, false, false)).unwrap();
        assert_eq!(sub.len(), 2);
    }

    #[test]
    fn the_will_will_footgun() {
        let text = "Will will go.";
        // Case-sensitive whole-word → only the capitalised name.
        let cs = find_matches(text, "Will", "Bill", opts(false, true, false)).unwrap();
        assert_eq!(cs.len(), 1);
        assert_eq!(cs[0].start, 0);
        // Case-insensitive → both.
        let ci = find_matches(text, "Will", "Bill", opts(false, true, true)).unwrap();
        assert_eq!(ci.len(), 2);
    }

    #[test]
    fn regex_with_captures_expands_replacement() {
        let hits =
            find_matches("1999-2000", r"(\d{4})-(\d{4})", "$2/$1", opts(true, false, false))
                .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].replacement, "2000/1999");
    }

    #[test]
    fn literal_mode_does_not_expand_dollar() {
        // In literal mode `$1` is part of the replacement, not a capture.
        let hits = find_matches("a a", "a", "$1b", opts(false, false, false)).unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].replacement, "$1b");
    }

    #[test]
    fn apply_splices_right_to_left() {
        let text = "aXaXa";
        let hits = find_matches(text, "X", "YY", opts(false, false, false)).unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(apply(text, &hits), "aYYaYYa");
    }

    #[test]
    fn apply_accepts_a_subset() {
        let text = "aXaXa";
        let hits = find_matches(text, "X", "YY", opts(false, false, false)).unwrap();
        // Accept only the second match.
        assert_eq!(apply(text, &hits[1..]), "aXaYYa");
        // Accept none → unchanged.
        assert_eq!(apply(text, &[]), text);
    }

    #[test]
    fn line_and_col_are_one_based() {
        let text = "first line\nsecond Anne here\nthird";
        let hits = find_matches(text, "Anne", "Anna", opts(false, true, false)).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].line, 2);
        assert_eq!(hits[0].col, 8); // "second " = 7 chars, match at col 8
        assert_eq!(hits[0].line_text, "second Anne here");
    }

    #[test]
    fn unicode_columns_count_chars_not_bytes() {
        let text = "Café Anne"; // 'é' is 2 bytes
        let hits = find_matches(text, "Anne", "Anna", opts(false, true, false)).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].col, 6); // C a f é space = 5 chars, match at 6
        // apply must still splice on byte boundaries correctly.
        assert_eq!(apply(text, &hits), "Café Anna");
    }

    #[test]
    fn ignore_case_literal() {
        let hits = find_matches("ANNE", "anne", "x", opts(false, true, true)).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].matched, "ANNE");
    }

    #[test]
    fn zero_width_matches_are_skipped() {
        // `a*` matches empty strings around the b's; only the real "aa"
        // run should survive.
        let hits = find_matches("baab", "a*", "Z", opts(true, false, false)).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].matched, "aa");
    }

    #[test]
    fn empty_pattern_is_rejected() {
        assert_eq!(
            find_matches("text", "", "x", ReplaceOpts::default()),
            Err(ReplaceError::EmptyPattern),
        );
    }

    #[test]
    fn invalid_regex_errors_cleanly() {
        let err = find_matches("text", "(unclosed", "x", opts(true, false, false));
        assert!(matches!(err, Err(ReplaceError::BadRegex(_))));
    }

    #[test]
    fn default_opts_are_literal_whole_word() {
        let d = ReplaceOpts::default();
        assert!(!d.regex);
        assert!(d.word_boundary);
        assert!(!d.ignore_case);
    }

    // ── scan_bodies ───────────────────────────────────

    fn para(n: u8, title: &str, body: &str) -> (uuid::Uuid, String, String, String) {
        (
            uuid::Uuid::from_u128(n as u128),
            title.into(),
            format!("book/{title}"),
            body.into(),
        )
    }

    #[test]
    fn scan_bodies_keeps_only_paragraphs_with_hits() {
        let bodies = vec![
            para(1, "ch1", "Anne walked in."),
            para(2, "ch2", "Nothing here."),
            para(3, "ch3", "Anne and Anne again."),
        ];
        let res =
            scan_bodies(bodies, "Anne", "Anna", ReplaceOpts::default()).unwrap();
        assert_eq!(res.len(), 2, "the empty paragraph is dropped");
        assert_eq!(res[0].para_id, uuid::Uuid::from_u128(1));
        assert_eq!(res[0].title, "ch1");
        assert_eq!(res[0].slug_path, "book/ch1");
        assert_eq!(res[0].hits.len(), 1);
        // Order preserved; ch3 has two hits.
        assert_eq!(res[1].para_id, uuid::Uuid::from_u128(3));
        assert_eq!(res[1].hits.len(), 2);
    }

    #[test]
    fn scan_bodies_word_boundary_respected_per_paragraph() {
        let bodies = vec![para(1, "ch1", "Anneliese only")];
        // Whole-word "Anne" must not match inside "Anneliese".
        let res = scan_bodies(bodies, "Anne", "Anna", ReplaceOpts::default()).unwrap();
        assert!(res.is_empty());
    }

    #[test]
    fn scan_bodies_propagates_pattern_errors() {
        let bodies = vec![para(1, "ch1", "text")];
        assert_eq!(
            scan_bodies(bodies.clone(), "", "x", ReplaceOpts::default()),
            Err(ReplaceError::EmptyPattern),
        );
        let bad = scan_bodies(
            bodies,
            "(unclosed",
            "x",
            ReplaceOpts {
                regex: true,
                ..ReplaceOpts::default()
            },
        );
        assert!(matches!(bad, Err(ReplaceError::BadRegex(_))));
    }
}
