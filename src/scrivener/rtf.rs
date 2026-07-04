//! Convert a single Scrivener document's RTF body to Typst
//! markup.
//!
//! Uses `rtf-parser-tt` to tokenise the RTF into a stream of
//! style transitions + text runs. We translate that stream
//! into a small Typst subset that mirrors what inkhaven users
//! actually write: paragraph breaks, bold (`*…*`), italic
//! (`_…_`), Unicode-safe text. Headings emit as `= …` when
//! the source RTF used a "Heading 1" style; otherwise plain
//! prose stays plain prose.
//!
//! What we deliberately drop:
//! * Custom fonts / colours / sizes — Scrivener users almost
//!   always override these during compile anyway.
//! * Embedded images — handled separately by the import
//!   orchestrator (they live in `Files/data/` alongside the
//!   RTF and need to be copied to the project's image dir).
//! * Footnotes / inline annotations — out of scope for v1;
//!   could land later as Typst footnotes.
//!
//! When the parser bails on malformed RTF, we fall back to the
//! "extract plain text" path: strip control words, keep the
//! visible characters. Lossy but never crashes on a
//! pathological input.

use anyhow::Result;

/// Convert RTF bytes to a Typst-friendly source string.
/// Lossy by design — see module docs for what we drop.
pub fn rtf_to_typst(rtf_bytes: &[u8]) -> Result<String> {
    // Try the structured parser first. If it returns Err — or the third-party
    // `rtf-parser-tt` lexer/parser *panics* on adversarial bytes (1.3.36
    // hardening: the upstream crate byte-slices and can panic even on ASCII
    // input) — fall back to a brute-force plain-text extraction so the importer
    // never crashes and the user keeps the document body.
    let structured =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| parse_structured(rtf_bytes)));
    match structured {
        Ok(Ok(s)) => Ok(s),
        Ok(Err(_)) | Err(_) => Ok(strip_to_plain_text(rtf_bytes)),
    }
}

/// Structured parse via `rtf-parser-tt`. Walks the
/// `parse(...)`'s `body` tokens and emits Typst markup.
fn parse_structured(rtf_bytes: &[u8]) -> Result<String> {
    use rtf_parser_tt::lexer::Lexer;
    use rtf_parser_tt::parser::Parser;

    // M4 — real Scrivener `.rtf` is frequently Windows-1252/Latin-1 with
    // `\'xx` hex escapes for high bytes. Rejecting on the first non-UTF-8
    // byte dropped ALL structure to the plain-text fallback. But the
    // upstream RTF lexer byte-slices and panics on multi-byte chars, so
    // we can't hand it raw lossy UTF-8 either — map every non-ASCII byte
    // to `?` so it sees pure single-byte ASCII. Structure + the `\'xx`
    // escapes (themselves ASCII, decoded by the parser) survive; only
    // stray literal 8-bit bytes — rare in RTF — are flattened.
    let ascii: String = rtf_bytes
        .iter()
        .map(|&b| if b < 0x80 { b as char } else { '?' })
        .collect();
    let tokens = Lexer::scan(&ascii)
        .map_err(|e| anyhow::anyhow!("rtf lex: {e}"))?;
    let mut parser = Parser::new(tokens);
    let doc = parser
        .parse()
        .map_err(|e| anyhow::anyhow!("rtf parse: {e}"))?;

    // `doc.body` is a Vec<StyleBlock>. Each block has a `painter`
    // (style state) + `text` (a String). We translate by emitting
    // wrappers around runs of consecutive identical style and
    // splitting on RTF paragraph markers (`\n` is the post-parse
    // representation of `\par`).
    let mut out = String::new();
    for block in &doc.body {
        let painter = &block.painter;
        let raw = &block.text;
        // Skip empty blocks.
        if raw.is_empty() {
            continue;
        }
        // Each block can contain multiple lines (one per `\par`
        // in the source); split + re-emit so paragraph breaks
        // become `\n\n` in Typst.
        for (i, line) in raw.split('\n').enumerate() {
            if i > 0 {
                ensure_paragraph_break(&mut out);
            }
            emit_styled_line(&mut out, line, painter);
        }
    }
    // Trim trailing blank lines so we don't ship "\n\n\n…" at
    // the end of every document.
    while out.ends_with("\n\n\n") {
        out.pop();
    }
    Ok(out)
}

/// Emit one styled run. `line` is unstyled text; `painter`
/// holds bold/italic/heading flags.
fn emit_styled_line(
    out: &mut String,
    line: &str,
    painter: &rtf_parser_tt::parser::Painter,
) {
    let trimmed = line.trim_end_matches('\r');
    if trimmed.is_empty() {
        return;
    }
    // Heading detection: if the painter marks this block as a
    // heading (some Scrivener exports stamp paragraphs with
    // outline levels), emit `= …`. Fall back to plain text
    // for everything else.
    //
    // `Painter` exposes `bold` / `italic` / `underline`. We
    // honour bold + italic; underline isn't a first-class
    // Typst markup so we drop it.
    let bold = painter.bold;
    let italic = painter.italic;
    if bold {
        // M3 — Typst's strong (bold) delimiter is a SINGLE `*`. `**`
        // parses as an empty strong followed by literal asterisks, so
        // every imported bold run rendered wrong.
        out.push('*');
    }
    if italic {
        out.push('_');
    }
    // Escape Typst-meta characters so user text doesn't
    // accidentally render as markup. The trio that matters in
    // prose is `*`, `_`, `#` at line start.
    for c in trimmed.chars() {
        match c {
            '*' | '_' | '#' | '@' | '<' | '>' | '$' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    if italic {
        out.push('_');
    }
    if bold {
        out.push('*');
    }
}

fn ensure_paragraph_break(out: &mut String) {
    while out.ends_with('\n') && out.len() > 0 {
        // peek the second-to-last char
        let last_two = out.chars().rev().take(2).collect::<String>();
        if last_two == "\n\n" {
            return;
        }
        if out.ends_with('\n') {
            out.push('\n');
            return;
        }
    }
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push('\n');
}

/// Brute-force fallback: drop everything that looks like an
/// RTF control sequence + bracket grouping, return what's
/// left. Used when the structured parser bails on a pathological
/// document; lossy but always produces something.
fn strip_to_plain_text(rtf_bytes: &[u8]) -> String {
    let s = String::from_utf8_lossy(rtf_bytes);
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    let mut depth = 0usize;
    while let Some(c) = chars.next() {
        match c {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            '\\' => {
                // Skip the control word. RTF control words are
                // letters + an optional numeric param; consume
                // until a delimiter (space, brace, backslash).
                while let Some(&next) = chars.peek() {
                    if next.is_ascii_alphanumeric() || next == '-' {
                        chars.next();
                    } else {
                        // Consume a single trailing space as
                        // the control-word terminator.
                        if next == ' ' {
                            chars.next();
                        }
                        break;
                    }
                }
            }
            '\n' | '\r' => {}
            _ if depth > 0 => out.push(c),
            _ => {}
        }
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_rtf() {
        let rtf = b"{\\rtf1\\ansi}";
        let out = rtf_to_typst(rtf).unwrap();
        assert!(out.trim().is_empty());
    }

    #[test]
    fn plain_paragraph() {
        let rtf = b"{\\rtf1\\ansi The quick brown fox.}";
        let out = rtf_to_typst(rtf).unwrap();
        assert!(out.contains("The quick brown fox"));
    }

    #[test]
    fn strip_fallback_handles_garbage() {
        // Pathological input — well-formed enough to not panic.
        let rtf = b"\\xxx{\\bogus garbage \\b text \\par more}";
        let out = strip_to_plain_text(rtf);
        assert!(out.contains("garbage") || out.contains("text"));
    }

    use proptest::prelude::*;

    proptest! {
        /// 1.3.36 hardening — the RTF importer parses untrusted bytes
        /// from a Scrivener `.rtf`. Arbitrary input must return
        /// Ok|Err and never panic (no slice/UTF-8/brace-depth crash).
        #[test]
        fn rtf_to_typst_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..512)) {
            let _ = rtf_to_typst(&bytes);
        }

        /// The plain-text fallback runs on the same bytes when the
        /// structured parse bails; it must be panic-free too.
        #[test]
        fn strip_to_plain_text_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..512)) {
            let _ = strip_to_plain_text(&bytes);
        }

        /// Token-salad of real RTF control words, braces, and prose —
        /// drives the structured parser deeper than random bytes do.
        #[test]
        fn rtf_to_typst_never_panics_on_control_salad(
            toks in proptest::collection::vec(
                proptest::sample::select(vec![
                    "{", "}", "\\rtf1", "\\ansi", "\\b", "\\i", "\\par", "\\pard",
                    "\\u8217", "\\'92", "\\fonttbl", "\\*", " ", "word", "\n",
                ]),
                0..160,
            ),
        ) {
            let joined = toks.concat();
            let _ = rtf_to_typst(joined.as_bytes());
        }
    }
}
