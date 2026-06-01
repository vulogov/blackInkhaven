//! 1.2.14+ Phase Q.2 — text snippet expansion.
//!
//! HJSON-driven map of trigger strings to
//! expansion bodies; the editor watches for
//! non-word characters typed AFTER a trigger and
//! replaces the trigger inline with the expansion
//! (the triggering non-word char stays — typing
//! `\dt ` produces `2026-05-31 ` with the trailing
//! space intact).
//!
//! Placeholders supported in expansion bodies:
//!
//! | Placeholder | Replaced with |
//! |-------------|---------------|
//! | `{today}` | Today's date `YYYY-MM-DD` |
//! | `{today:%FMT}` | Today's date in the chrono format string |
//! | `{now}` | Current time `HH:MM` |
//! | `{now:%FMT}` | Current time in the chrono format string |
//! | `{paragraph_title}` | The open paragraph's display title |
//! | `{paragraph_slug}` | The open paragraph's slug |
//! | `{selection}` | The active selection text (empty when none) |
//! | `{author}` | Resolved comment author (mirror of `editor.comment_author`) |
//! | `{cursor}` | Position the editor cursor at this point in the expansion (split-paste) |
//!
//! `{cursor}` is processed by the editor's
//! `maybe_expand_snippet` after `expand_placeholders`
//! returns — the expansion gets split at the
//! marker, the head pasted, cursor position
//! captured, tail pasted, cursor moved back.
//! Picker-based placeholders (`{char_lookup}`,
//! `{place_lookup}`, `{artefact_lookup}`) and the
//! `bund:` prefix for advanced expansion are
//! still queued (the picker route needs a modal
//! state machine the snippet pipeline doesn't yet
//! have).
//!
//! See `Documentation/PROPOSALS/1.2.14_PLAN.md`
//! §6.

use chrono::Local;

/// Context the expander reads to resolve
/// non-static placeholders.  Built once at
/// expansion time; cheap to construct
/// (no parsing, no IO).
#[derive(Debug, Clone, Default)]
pub struct ExpansionContext {
    pub paragraph_title: String,
    pub paragraph_slug: String,
    pub selection: String,
    pub author: String,
}

/// 1.2.14+ Phase Q.2 — find the longest trigger
/// in `triggers` that the chars at the END of
/// `before_trigger_char` match.  `before_trigger_char`
/// is the text up to (but not including) the
/// just-typed non-word character that fired the
/// expansion check.  Returns the matching trigger
/// string + the expansion body, or `None`.
///
/// Longest-match wins so triggers like `\dts`
/// (date with seconds) take precedence over the
/// shorter `\dt`.
pub fn find_trigger<'a>(
    before_trigger_char: &str,
    triggers: &'a std::collections::HashMap<String, String>,
) -> Option<(&'a str, &'a str)> {
    let mut best: Option<(&str, &str)> = None;
    for (k, v) in triggers {
        if !before_trigger_char.ends_with(k.as_str()) {
            continue;
        }
        if best.map_or(true, |(prev, _)| k.chars().count() > prev.chars().count()) {
            best = Some((k.as_str(), v.as_str()));
        }
    }
    best
}

/// 1.2.14+ Phase Q.2 — replace every recognised
/// placeholder in `body` with its resolved value.
/// Unknown placeholders pass through verbatim so
/// the author sees them and can spot typos.
pub fn expand_placeholders(body: &str, ctx: &ExpansionContext) -> String {
    let mut out = String::with_capacity(body.len());
    let mut chars = body.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '{' {
            out.push(c);
            continue;
        }
        // Read until matching '}'.  Bail back to
        // literal '{' if no close brace is found.
        let mut name = String::new();
        let mut found_close = false;
        while let Some(&nc) = chars.peek() {
            chars.next();
            if nc == '}' {
                found_close = true;
                break;
            }
            name.push(nc);
        }
        if !found_close {
            out.push('{');
            out.push_str(&name);
            continue;
        }
        let resolved = resolve_placeholder(&name, ctx);
        match resolved {
            Some(s) => out.push_str(&s),
            None => {
                // Unknown placeholder: pass through
                // verbatim so the typo is visible to
                // the author.
                out.push('{');
                out.push_str(&name);
                out.push('}');
            }
        }
    }
    out
}

/// 1.2.16+ Phase P.6 — picker placeholder kinds.
///
/// `{char_lookup}`, `{place_lookup}`, and
/// `{artefact_lookup}` each open the corresponding
/// system-book entry picker; the chosen entry's
/// title becomes the substitution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnippetPickerKind {
    Char,
    Place,
    Artefact,
}

impl SnippetPickerKind {
    /// Map the placeholder name (without braces) to
    /// a picker kind, or `None` if this isn't a
    /// picker placeholder.
    pub fn from_placeholder(name: &str) -> Option<Self> {
        match name {
            "char_lookup" => Some(SnippetPickerKind::Char),
            "place_lookup" => Some(SnippetPickerKind::Place),
            "artefact_lookup" => Some(SnippetPickerKind::Artefact),
            _ => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            SnippetPickerKind::Char => "Character",
            SnippetPickerKind::Place => "Place",
            SnippetPickerKind::Artefact => "Artefact",
        }
    }
}

/// 1.2.16+ Phase P.6 — result of analysing a
/// snippet body for expansion.
///
/// Two cases:
///   * `Literal(s)` — the body fully expands to
///     `s`; the editor pastes it directly (modulo
///     the `{cursor}` split-paste handled by the
///     caller).
///   * `Picker { head, kind, tail }` — the body
///     contains a picker placeholder.  The editor
///     pastes `head` immediately, opens a modal
///     for `kind`, and inserts the picked entry
///     followed by `tail` on commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpansionPlan {
    Literal(String),
    Picker {
        head: String,
        kind: SnippetPickerKind,
        tail: String,
    },
}

/// 1.2.16+ Phase P.6 — analyse a snippet body and
/// produce an expansion plan.  Handles two cases
/// beyond the pure-sync placeholder expansion:
///
///   1. **`bund:` prefix.**  When the body starts
///      with `bund:`, the rest is interpreted as a
///      Bund-VM program.  Caller (in the editor)
///      evaluates and substitutes the result.
///      This fn returns the program with the
///      prefix stripped, wrapped in
///      `ExpansionPlan::Literal` carrying a
///      sentinel `\x00bund:` prefix the caller
///      detects.  (Why the sentinel: keeps
///      snippets.rs free of any Bund-VM
///      dependency; the caller owns the eval.)
///
///   2. **Picker placeholders.**  Detects the
///      first `{char_lookup}` / `{place_lookup}` /
///      `{artefact_lookup}` and returns
///      `ExpansionPlan::Picker` with the
///      surrounding text split into head + tail.
///
/// Sync placeholders are expanded normally in
/// every branch.
pub fn analyse(body: &str, ctx: &ExpansionContext) -> ExpansionPlan {
    if let Some(stripped) = body.strip_prefix("bund:") {
        // Expand sync placeholders inside the Bund
        // program first — so `bund: "{author}" ink.print`
        // gets the author injected before eval.
        let expanded = expand_placeholders(stripped, ctx);
        return ExpansionPlan::Literal(format!("\x00bund:{expanded}"));
    }
    let expanded = expand_placeholders(body, ctx);
    // Scan for the first picker placeholder.
    if let Some((head, kind, tail)) = split_at_picker_placeholder(&expanded) {
        return ExpansionPlan::Picker {
            head: head.to_string(),
            kind,
            tail: tail.to_string(),
        };
    }
    ExpansionPlan::Literal(expanded)
}

fn split_at_picker_placeholder(s: &str) -> Option<(&str, SnippetPickerKind, &str)> {
    for placeholder in &["char_lookup", "place_lookup", "artefact_lookup"] {
        let needle = format!("{{{placeholder}}}");
        if let Some(idx) = s.find(&needle) {
            let head = &s[..idx];
            let tail = &s[idx + needle.len()..];
            let kind = SnippetPickerKind::from_placeholder(placeholder)?;
            return Some((head, kind, tail));
        }
    }
    None
}

fn resolve_placeholder(name: &str, ctx: &ExpansionContext) -> Option<String> {
    // Allow chrono-format suffixes: `today:%Y/%m/%d`,
    // `now:%H:%M:%S`.
    let (head, tail) = match name.split_once(':') {
        Some((h, t)) => (h, Some(t)),
        None => (name, None),
    };
    let now = Local::now();
    match head {
        "today" => Some(now.format(tail.unwrap_or("%Y-%m-%d")).to_string()),
        "now" => Some(now.format(tail.unwrap_or("%H:%M")).to_string()),
        "paragraph_title" => Some(ctx.paragraph_title.clone()),
        "paragraph_slug" => Some(ctx.paragraph_slug.clone()),
        "selection" => Some(ctx.selection.clone()),
        "author" => Some(ctx.author.clone()),
        _ => None,
    }
}

/// 1.2.14+ Phase Q.2 — heuristic for "non-word
/// character" that fires snippet expansion.  Whitespace
/// (including newline), ASCII punctuation, and
/// general-category punctuation Unicode classes
/// all count.  Word characters (alphanumeric +
/// underscore) and the typical opening-bracket
/// characters do not (so typing `\dt[` lets the
/// trigger keep accumulating).
pub fn is_expansion_trigger_char(c: char) -> bool {
    if c.is_whitespace() {
        return true;
    }
    // Common ASCII sentence punctuation +
    // grouping closers.
    matches!(
        c,
        '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '}' | '"' | '\''
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn mk_triggers(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn find_trigger_matches_at_end() {
        let triggers = mk_triggers(&[(r"\dt", "{today}")]);
        let m = find_trigger(r"the date is \dt", &triggers);
        assert_eq!(m, Some((r"\dt", "{today}")));
    }

    #[test]
    fn find_trigger_returns_none_when_no_match() {
        let triggers = mk_triggers(&[(r"\dt", "{today}")]);
        assert!(find_trigger("nothing here", &triggers).is_none());
    }

    #[test]
    fn find_trigger_prefers_longer_match() {
        // Both `\dt` and `\dts` would match a buffer
        // ending in `\dts`; the longer one wins.
        let triggers =
            mk_triggers(&[(r"\dt", "{today}"), (r"\dts", "{today:%Y-%m-%dT%H:%M:%S}")]);
        let m = find_trigger(r"foo \dts", &triggers);
        assert_eq!(m, Some((r"\dts", "{today:%Y-%m-%dT%H:%M:%S}")));
    }

    #[test]
    fn expand_static_placeholders() {
        let ctx = ExpansionContext {
            paragraph_title: "Rain on the marketplace".into(),
            paragraph_slug: "03-rain".into(),
            selection: "the river-cult".into(),
            author: "vladimir".into(),
        };
        let out = expand_placeholders(
            "see {paragraph_title} ({paragraph_slug}) — by {author}: {selection}",
            &ctx,
        );
        assert_eq!(
            out,
            "see Rain on the marketplace (03-rain) — by vladimir: the river-cult"
        );
    }

    #[test]
    fn expand_today_default_format() {
        let ctx = ExpansionContext::default();
        let out = expand_placeholders("{today}", &ctx);
        // Just check the shape: YYYY-MM-DD.
        assert_eq!(out.len(), 10);
        let chars: Vec<char> = out.chars().collect();
        assert!(chars[4] == '-' && chars[7] == '-');
    }

    #[test]
    fn expand_chrono_format_suffix() {
        let ctx = ExpansionContext::default();
        // Custom format reaches chrono.
        let out = expand_placeholders("{today:%Y/%m/%d}", &ctx);
        assert_eq!(out.len(), 10);
        assert!(out.contains('/'));
        assert!(!out.contains('-'));
    }

    #[test]
    fn expand_unknown_placeholder_passes_through() {
        let out = expand_placeholders("{nonexistent}", &ExpansionContext::default());
        assert_eq!(out, "{nonexistent}");
    }

    #[test]
    fn expand_unmatched_brace_passes_through() {
        let out = expand_placeholders("{unterminated", &ExpansionContext::default());
        assert_eq!(out, "{unterminated");
    }

    #[test]
    fn expand_cursor_placeholder_passes_through() {
        // `{cursor}` is intentionally an unknown
        // placeholder to `expand_placeholders` so
        // the editor's split-paste logic can find
        // and consume it.  Verify the marker
        // survives expansion verbatim.
        let body = "TODO: {cursor}\nresolved by {author}";
        let ctx = ExpansionContext {
            author: "vladimir".into(),
            ..Default::default()
        };
        let out = expand_placeholders(body, &ctx);
        assert!(out.contains("{cursor}"));
        assert!(out.contains("vladimir"));
    }

    #[test]
    fn expand_literal_braces_in_body() {
        // No placeholder syntax → braces stay.
        let out = expand_placeholders(
            "TODO(vladimir): write {paragraph_title}",
            &ExpansionContext {
                paragraph_title: "Rain".into(),
                ..Default::default()
            },
        );
        assert_eq!(out, "TODO(vladimir): write Rain");
    }

    #[test]
    fn expansion_trigger_char_recognises_common_punctuation() {
        assert!(is_expansion_trigger_char(' '));
        assert!(is_expansion_trigger_char('\n'));
        assert!(is_expansion_trigger_char('\t'));
        assert!(is_expansion_trigger_char('.'));
        assert!(is_expansion_trigger_char(','));
        assert!(is_expansion_trigger_char(';'));
        assert!(is_expansion_trigger_char(')'));
    }

    #[test]
    fn expansion_trigger_char_skips_word_chars() {
        for c in "abcXYZ_0123".chars() {
            assert!(!is_expansion_trigger_char(c), "char {c} should not fire");
        }
    }

    // 1.2.16+ Phase P.6 — analyse() tests.

    #[test]
    fn analyse_plain_body_returns_literal() {
        let plan = analyse("hello world", &ExpansionContext::default());
        assert_eq!(plan, ExpansionPlan::Literal("hello world".into()));
    }

    #[test]
    fn analyse_sync_placeholder_returns_literal_with_substitution() {
        let plan = analyse(
            "hi {paragraph_title}!",
            &ExpansionContext {
                paragraph_title: "Rain".into(),
                ..Default::default()
            },
        );
        assert_eq!(plan, ExpansionPlan::Literal("hi Rain!".into()));
    }

    #[test]
    fn analyse_bund_prefix_returns_sentinel_literal() {
        let plan = analyse("bund:40 2 +", &ExpansionContext::default());
        match plan {
            ExpansionPlan::Literal(s) => {
                assert!(s.starts_with('\x00'), "expected sentinel prefix");
                assert_eq!(s, "\x00bund:40 2 +");
            }
            _ => panic!("expected Literal with bund sentinel"),
        }
    }

    #[test]
    fn analyse_bund_prefix_expands_sync_placeholders_in_program() {
        let plan = analyse(
            "bund:\"{paragraph_title}\" ink.print",
            &ExpansionContext {
                paragraph_title: "Rain".into(),
                ..Default::default()
            },
        );
        match plan {
            ExpansionPlan::Literal(s) => {
                assert_eq!(s, "\x00bund:\"Rain\" ink.print");
            }
            _ => panic!("expected Literal"),
        }
    }

    #[test]
    fn analyse_char_lookup_returns_picker() {
        let plan = analyse(
            "She turned to {char_lookup}.",
            &ExpansionContext::default(),
        );
        match plan {
            ExpansionPlan::Picker { head, kind, tail } => {
                assert_eq!(head, "She turned to ");
                assert_eq!(kind, SnippetPickerKind::Char);
                assert_eq!(tail, ".");
            }
            _ => panic!("expected Picker"),
        }
    }

    #[test]
    fn analyse_place_lookup_returns_picker() {
        let plan = analyse("In {place_lookup}", &ExpansionContext::default());
        match plan {
            ExpansionPlan::Picker { kind, .. } => {
                assert_eq!(kind, SnippetPickerKind::Place);
            }
            _ => panic!("expected Picker"),
        }
    }

    #[test]
    fn analyse_artefact_lookup_returns_picker() {
        let plan = analyse(
            "the {artefact_lookup} fell",
            &ExpansionContext::default(),
        );
        match plan {
            ExpansionPlan::Picker { kind, .. } => {
                assert_eq!(kind, SnippetPickerKind::Artefact);
            }
            _ => panic!("expected Picker"),
        }
    }

    #[test]
    fn analyse_multi_picker_picks_first_only() {
        // Limitation documented in the proposal:
        // only the first picker placeholder fires
        // a modal; subsequent ones currently pass
        // through verbatim into the tail.
        let plan = analyse(
            "{char_lookup} met {place_lookup}",
            &ExpansionContext::default(),
        );
        match plan {
            ExpansionPlan::Picker { head, kind, tail } => {
                assert_eq!(head, "");
                assert_eq!(kind, SnippetPickerKind::Char);
                assert_eq!(tail, " met {place_lookup}");
            }
            _ => panic!("expected Picker"),
        }
    }

    #[test]
    fn picker_kind_label_matches_system_book() {
        assert_eq!(SnippetPickerKind::Char.label(), "Character");
        assert_eq!(SnippetPickerKind::Place.label(), "Place");
        assert_eq!(SnippetPickerKind::Artefact.label(), "Artefact");
    }
}
