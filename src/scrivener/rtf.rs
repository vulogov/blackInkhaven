//! Convert a single Scrivener document's RTF body to Typst
//! markup.
//!
//! Uses `rtf-parser-tt` (0.6+) to parse the RTF into a stream of
//! style blocks (a style state + a text run). We translate that
//! stream into a small Typst subset that mirrors what inkhaven
//! users actually write: paragraph breaks, bold (`*…*`), italic
//! (`_…_`), footnotes (`#footnote[…]`), Unicode-safe text.
//!
//! Character decoding is the parser's job: it reads raw UTF-8 as
//! is, decodes `\'xx` escapes through the document's declared
//! code page (`\ansicpg`) or the current font's `\fcharset`
//! (Windows Scrivener's cp1251 Cyrillic, cp1252 curly quotes,
//! Central-European fonts, Symbol-font bullets), and honours the
//! `\ucN` replacement-character rule after `\uN` (LibreOffice
//! writes every non-ASCII character as `\uN\'xx`). Nothing is
//! pre-processed on our side any more — the old `\'xx` decode +
//! ASCII-placeholder dance (needed by the ASCII-only 0.5 lexer)
//! corrupts text on 0.6 (upstream issue #3).
//!
//! What we deliberately drop:
//! * Custom fonts / colours / sizes — Scrivener users almost
//!   always override these during compile anyway.
//! * Embedded images — handled separately by the import
//!   orchestrator (they live in `Files/data/` alongside the
//!   RTF and need to be copied to the project's image dir).
//! * Hidden text (`\v`), page headers and footers — page
//!   furniture, not prose.
//!
//! When the parser bails on malformed RTF, we fall back to the
//! "extract plain text" path: strip control words, keep the
//! visible characters. Lossy but never crashes on a
//! pathological input.

use std::collections::BTreeMap;

use anyhow::Result;
use rtf_parser_tt::{Painter, Part, RtfDocument, StyleBlock};

/// Convert RTF bytes to a Typst-friendly source string.
/// Lossy by design — see module docs for what we drop.
pub fn rtf_to_typst(rtf_bytes: &[u8]) -> Result<String> {
    // Try the structured parser first. `from_bytes` reads raw UTF-8 as is and
    // decodes anything else through the document's code page. 0.6 returns
    // `Err` on malformed input instead of panicking, but the RTF importer
    // parses untrusted bytes from a Scrivener bundle, so keep the
    // `catch_unwind` as defence in depth against the third-party crate (1.3.36
    // hardening): on Err *or* panic, fall back to a brute-force plain-text
    // extraction so the importer never crashes and the user keeps the body.
    // Suppress the global crash hook for the duration of the guarded call, so a
    // recovered panic doesn't write a crash report / print "inkhaven crashed" /
    // tear the terminal down — the body is recovered, not lost.
    let structured = crate::crash::suppress_panic_report(|| {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            RtfDocument::from_bytes(rtf_bytes)
        }))
    });
    match structured {
        Ok(Ok(doc)) => Ok(emit_body(&doc)),
        Ok(Err(_)) | Err(_) => Ok(strip_to_plain_text(rtf_bytes)),
    }
}

/// Walk the parsed document's `body` blocks and emit Typst markup.
///
/// `doc.body` is a `Vec<StyleBlock>`. Each block has a `painter` (style
/// state), a `part` (body / footnote reference / footnote text / header /
/// footer) and `text`. A block's text can span several paragraphs (`\n` is
/// the post-parse representation of `\par`), so each block is split on
/// `\n` and paragraph breaks become `\n\n` in Typst.
///
/// Footnotes: the parser lists the note text (`Part::Note(n)`) after the
/// body and marks where it was referenced (`Part::NoteReference(n)`). We
/// splice the note back in at its reference as `#footnote[…]`, which
/// Typst numbers itself, so the reference block's own number text is
/// not emitted.
fn emit_body(doc: &RtfDocument) -> String {
    let mut notes: BTreeMap<u32, Vec<&StyleBlock>> = BTreeMap::new();
    for block in &doc.body {
        if let Part::Note(n) = block.part {
            notes.entry(n).or_default().push(block);
        }
    }

    let mut out = String::new();
    for block in &doc.body {
        // Hidden text (`\v`) is not prose — same filter as the parser's
        // `get_text()`.
        if block.painter.hidden {
            continue;
        }
        match block.part {
            Part::Body => emit_block(&mut out, block, false),
            Part::NoteReference(n) => {
                if let Some(blocks) = notes.remove(&n) {
                    emit_footnote(&mut out, n, &blocks);
                }
            }
            // Note text is emitted at its reference; headers / footers are
            // page furniture.
            Part::Note(_) | Part::Header | Part::Footer => {}
            // `Part` is `#[non_exhaustive]` — treat anything new as body
            // prose rather than silently dropping it.
            _ => emit_block(&mut out, block, false),
        }
    }
    // A footnote whose reference never appeared in the body (no `\chftn`)
    // has no anchor; append its text as plain paragraphs rather than lose it.
    for (n, blocks) in notes {
        let mut note = String::new();
        emit_note_text(&mut note, n, &blocks, false);
        let note = note.trim();
        if !note.is_empty() {
            ensure_paragraph_break(&mut out);
            out.push_str(note);
        }
    }
    // Trim the trailing paragraph break every `\par`-terminated document
    // ends with so we don't ship "…\n\n" at the end of every document.
    while out.ends_with('\n') {
        out.pop();
    }
    out
}

/// Emit one block: split on paragraph breaks and emit each line styled
/// by the block's painter. `in_content_block` escapes `[` / `]` as well,
/// for text that lands inside a Typst `[…]` content argument.
fn emit_block(out: &mut String, block: &StyleBlock, in_content_block: bool) {
    emit_text(out, &block.text, &block.painter, in_content_block);
}

/// Emit `text` (possibly several paragraphs) styled by `painter`.
fn emit_text(out: &mut String, text: &str, painter: &Painter, in_content_block: bool) {
    if text.is_empty() {
        return;
    }
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            ensure_paragraph_break(out);
        }
        emit_styled_line(out, line, painter, in_content_block);
    }
}

/// Emit footnote `n` at its reference point as `#footnote[…]`.
fn emit_footnote(out: &mut String, n: u32, blocks: &[&StyleBlock]) {
    let mut note = String::new();
    emit_note_text(&mut note, n, blocks, true);
    let note = note.trim();
    if note.is_empty() {
        return;
    }
    out.push_str("#footnote[");
    out.push_str(note);
    out.push(']');
}

/// Emit the text of footnote `n`. The parser renders the note's own
/// `\chftn` number as text at the start of the note — on its own block
/// when `\super` styles it, otherwise fused onto the first text block
/// ("1 First note."). Typst numbers footnotes itself, so that leading
/// number is dropped (only when nothing digit-like follows it, so a note
/// that genuinely opens with "10 ships" under a `\chftn`-less writer
/// keeps its digits).
fn emit_note_text(out: &mut String, n: u32, blocks: &[&StyleBlock], in_content_block: bool) {
    let number = n.to_string();
    let mut first = true;
    for block in blocks {
        if block.painter.hidden {
            continue;
        }
        let mut text = block.text.as_str();
        if first {
            first = false;
            if let Some(rest) = text.trim_start().strip_prefix(&number) {
                if !rest.starts_with(|c: char| c.is_ascii_digit()) {
                    text = rest;
                }
            }
        }
        emit_text(out, text, &block.painter, in_content_block);
    }
}

/// Emit one styled run. `line` is unstyled text; `painter`
/// holds bold/italic flags.
fn emit_styled_line(
    out: &mut String,
    line: &str,
    painter: &Painter,
    in_content_block: bool,
) {
    let trimmed = line.trim_end_matches('\r');
    if trimmed.is_empty() {
        return;
    }
    // `Painter` exposes `bold` / `italic` / `underline` (and more). We
    // honour bold + italic; underline isn't a first-class Typst markup so
    // we drop it.
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
    // prose is `*`, `_`, `#` at line start. Inside a `[…]` content
    // argument an unbalanced `]` would end the argument, so square
    // brackets are escaped there too.
    for c in trimmed.chars() {
        match c {
            '*' | '_' | '#' | '@' | '<' | '>' | '$' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            '[' | ']' if in_content_block => {
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
    while out.ends_with('\n') && !out.is_empty() {
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
    fn raw_utf8_content_is_kept_not_flattened() {
        // Audit regression — a Scrivener .rtf storing prose as raw UTF-8 bytes
        // must keep its Cyrillic text, not have it destroyed to `?`.
        let mut rtf = b"{\\rtf1\\ansi\\pard ".to_vec();
        rtf.extend_from_slice("Москва".as_bytes());
        rtf.extend_from_slice(b"\\par}");
        let out = rtf_to_typst(&rtf).unwrap();
        assert!(out.contains("Москва"), "Cyrillic must survive: {out:?}");
        assert!(!out.contains('?'), "must not flatten to '?': {out:?}");
    }

    #[test]
    fn plain_paragraph() {
        let rtf = b"{\\rtf1\\ansi The quick brown fox.}";
        let out = rtf_to_typst(rtf).unwrap();
        assert!(out.contains("The quick brown fox"));
    }

    #[test]
    fn cyrillic_bold_keeps_text_and_structure() {
        // I-1 — Windows Scrivener stores cp1251 Cyrillic as `\'xx`; here inside a
        // bold run. Both the text ("Москва") AND the bold markup must survive.
        let rtf = b"{\\rtf1\\ansi\\ansicpg1251 \\b \\'cc\\'ee\\'f1\\'ea\\'e2\\'e0\\b0}";
        let out = rtf_to_typst(rtf).unwrap();
        assert!(out.contains("Москва"), "Cyrillic text survives: {out:?}");
        assert!(out.contains("*Москва*"), "bold structure survives: {out:?}");
        assert!(!out.contains('\u{2}') && !out.contains('\u{3}'), "no control chars: {out:?}");
    }

    #[test]
    fn accented_italic_keeps_text_and_structure() {
        // cp1252 é (`\'e9`) inside an italic run → "café" in typst italics.
        let rtf = b"{\\rtf1\\ansi\\ansicpg1252 \\i caf\\'e9\\i0}";
        let out = rtf_to_typst(rtf).unwrap();
        assert!(out.contains("caf\u{e9}"), "accent survives: {out:?}");
        assert!(out.contains("_caf\u{e9}_"), "italic structure survives: {out:?}");
    }

    #[test]
    fn cp1251_hex_escapes_decode_to_cyrillic() {
        // 3.0.8 regression — Scrivener-for-Windows stores Cyrillic as `\'xx`
        // under \ansicpg1251. Must decode to "Москва", not the raw-codepoint
        // mojibake "Ìñâ".
        let rtf = b"{\\rtf1\\ansi\\ansicpg1251 \\'cc\\'ee\\'f1\\'ea\\'e2\\'e0}";
        let out = rtf_to_typst(rtf).unwrap();
        assert!(out.contains("Москва"), "cp1251 must decode: {out:?}");
    }

    #[test]
    fn cp1252_smart_punctuation_decodes_not_c1_controls() {
        // `\'92` is a curly apostrophe in cp1252, not U+0092. Must be `’`.
        let rtf = b"{\\rtf1\\ansi\\ansicpg1252 don\\'92t}";
        let out = rtf_to_typst(rtf).unwrap();
        assert!(out.contains("don\u{2019}t"), "cp1252 apostrophe: {out:?}");
        assert!(!out.contains('\u{92}'), "must not emit a C1 control: {out:?}");
    }

    #[test]
    fn cp1252_latin1_accents_still_decode() {
        // The common French/German/Spanish accents (0xA0-0xFF ≡ Latin-1) too.
        let rtf = b"{\\rtf1\\ansi\\ansicpg1252 caf\\'e9}";
        let out = rtf_to_typst(rtf).unwrap();
        assert!(out.contains("caf\u{e9}"), "é must decode: {out:?}");
    }

    #[test]
    fn unicode_escape_with_hex_replacement_is_not_doubled_or_corrupted() {
        // Issue #3 — LibreOffice writes every non-ASCII character as `\uN`
        // followed by its `\'xx` replacement. The replacement must be skipped
        // (`\uc1` default), leaving exactly one `é`. On 0.5 this lost the
        // character and the following space ("cafok"); pre-decoding `\'xx`
        // ourselves on 0.6 produced "café0\u{3} ok".
        let rtf = b"{\\rtf1\\ansi\\ansicpg1252 caf\\u233\\'e9 ok}";
        let out = rtf_to_typst(rtf).unwrap();
        assert_eq!(out, "caf\u{e9} ok", "{out:?}");
    }

    #[test]
    fn unicode_escape_with_question_mark_replacement() {
        // Word's form: `舒?` → em dash, no stray `?`.
        let rtf = b"{\\rtf1\\ansi a\\u8212?b}";
        let out = rtf_to_typst(rtf).unwrap();
        assert_eq!(out, "a\u{2014}b", "{out:?}");
    }

    #[test]
    fn cocoa_uc0_unicode_escapes() {
        // Mac Scrivener (Cocoa) writes `\uc0ၒ ` — no replacement char.
        let rtf = b"{\\rtf1\\ansi\\ansicpg1252\\cocoartf2709 \\uc0\\u1052 \\u1086 \\u1089  ok}";
        let out = rtf_to_typst(rtf).unwrap();
        assert_eq!(out, "Мос ok", "{out:?}");
    }

    #[test]
    fn par_becomes_a_typst_paragraph_break() {
        // Windows Scrivener ends paragraphs with `\par`. On 0.5 these ran
        // together ("café oknext").
        let rtf = b"{\\rtf1\\ansi\\ansicpg1252 caf\\'e9 ok\\par next\\par}";
        let out = rtf_to_typst(rtf).unwrap();
        assert_eq!(out, "caf\u{e9} ok\n\nnext", "{out:?}");
    }

    #[test]
    fn cocoa_backslash_newline_paragraphs() {
        // Mac Scrivener writes `\` + newline for a paragraph break.
        let rtf = b"{\\rtf1\\ansi\\ansicpg1252\\cocoartf2709\n\\pard\n\\f0\\fs24 \\cf0 First para.\\\nSecond para.\\\n\\b Bold third.\\b0 }";
        let out = rtf_to_typst(rtf).unwrap();
        assert_eq!(out, "First para.\n\nSecond para.\n\n*Bold third.*", "{out:?}");
    }

    #[test]
    fn punctuation_after_control_word_is_kept_and_bold_closes() {
        // On 0.5 `\b0,` swallowed the comma and bold leaked over "rest".
        let rtf = b"{\\rtf1\\ansi \\b bold\\b0, rest}";
        let out = rtf_to_typst(rtf).unwrap();
        assert_eq!(out, "*bold*, rest", "{out:?}");
    }

    #[test]
    fn raw_8bit_bytes_decode_through_declared_codepage() {
        // A file that stores cp1251 bytes unescaped (not valid UTF-8) is
        // decoded through `\ansicpg`, not replaced with U+FFFD.
        let rtf = b"{\\rtf1\\ansi\\ansicpg1251 \xcc\xee\xf1 ok}";
        let out = rtf_to_typst(rtf).unwrap();
        assert_eq!(out, "Мос ok", "{out:?}");
    }

    #[test]
    fn font_charset_selects_the_codepage() {
        // Polish in a Central-European (`\fcharset238`) font under a
        // cp1252 document: `\'b3` is `ł` in cp1250, `³` in cp1252.
        let rtf = b"{\\rtf1\\ansi\\ansicpg1252{\\fonttbl{\\f1\\fcharset238 Arial;}}\\f1 \\'b3\\'f3d\\'bf}";
        let out = rtf_to_typst(rtf).unwrap();
        assert_eq!(out, "\u{142}\u{f3}d\u{17c}", "{out:?}");
    }

    #[test]
    fn hidden_text_is_omitted() {
        let rtf = b"{\\rtf1\\ansi shown {\\v hidden }still shown}";
        let out = rtf_to_typst(rtf).unwrap();
        assert_eq!(out, "shown still shown", "{out:?}");
    }

    #[test]
    fn footnote_maps_to_typst_footnote_at_its_reference() {
        let rtf = b"{\\rtf1\\ansi Body one{\\super\\chftn{\\footnote{\\super\\chftn} First *note*.}}.\\par Body two{\\super\\chftn{\\footnote{\\super\\chftn} Second [note].}}\\par}";
        let out = rtf_to_typst(rtf).unwrap();
        assert_eq!(
            out,
            "Body one#footnote[First \\*note\\*.].\n\nBody two#footnote[Second \\[note\\].]",
            "{out:?}"
        );
    }

    #[test]
    fn footnote_number_on_its_own_block_is_dropped() {
        // `\pard\plain` before the number puts it in a block of its own.
        let rtf = b"{\\rtf1\\ansi Body{\\super\\chftn}{\\footnote\\pard\\plain{\\super\\chftn} Note.} end.\\par}";
        let out = rtf_to_typst(rtf).unwrap();
        assert_eq!(out, "Body#footnote[Note.] end.", "{out:?}");
    }

    #[test]
    fn footnote_keeps_inline_style() {
        let rtf = b"{\\rtf1\\ansi A{\\chftn{\\footnote\\chftn \\i italic note\\i0}} B}";
        let out = rtf_to_typst(rtf).unwrap();
        assert_eq!(out, "A#footnote[_italic note_] B", "{out:?}");
    }

    #[test]
    fn headers_and_footers_are_dropped() {
        let rtf = b"{\\rtf1\\ansi{\\header Page head\\par}{\\footer Foot\\par}Body text\\par}";
        let out = rtf_to_typst(rtf).unwrap();
        assert_eq!(out, "Body text", "{out:?}");
    }

    #[test]
    fn no_trailing_paragraph_break() {
        let rtf = b"{\\rtf1\\ansi one\\par two\\par\\par}";
        let out = rtf_to_typst(rtf).unwrap();
        assert!(!out.ends_with('\n'), "{out:?}");
        assert_eq!(out, "one\n\ntwo");
    }

    #[test]
    fn malformed_input_falls_back_to_plain_text() {
        // Unbalanced braces + garbage control words: the structured parser
        // must return Err (not panic) and the fallback keeps the words.
        let rtf = b"{\\rtf1\\ansi {{{\\b text \\par more";
        let out = rtf_to_typst(rtf).unwrap();
        assert!(out.contains("text") && out.contains("more"), "{out:?}");
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
                    "{", "}", "\\rtf1", "\\ansi", "\\ansicpg1251", "\\b", "\\i", "\\v",
                    "\\par", "\\pard", "\\u8217", "\\u-1", "\\uc0", "\\uc5", "\\'92", "\\'",
                    "\\fonttbl", "\\fcharset2", "\\f1", "\\*", "\\footnote", "\\chftn",
                    "\\header", "\\footer", " ", "word", "\n", "\u{e9}", "\\",
                ]),
                0..160,
            ),
        ) {
            let joined = toks.concat();
            let _ = rtf_to_typst(joined.as_bytes());
        }
    }
}
