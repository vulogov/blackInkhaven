//! EPUB import P2 — XHTML → typst prose.
//!
//! The inverse of `crate::epub::typst_to_xhtml`: walks a chapter's
//! XHTML and emits inkhaven's typst-prose subset — headings (`=`…),
//! paragraphs, `*strong*` / `_emph_`, lists, line breaks, image
//! references, and (I-2) **footnotes** — an inline `<a epub:type="noteref">`
//! is rebuilt as `#footnote[body]` from its collected `<aside>` (the epub →
//! import round-trip; the collected footnotes section is suppressed). Unknown
//! tags are dropped but their text is kept; text is typst-escaped so imported
//! prose never renders as accidental markup.
//!
//! Lenient + never-panic: a malformed document stops cleanly with
//! whatever was converted so far, rather than erroring out the import.

use quick_xml::events::Event;
use quick_xml::Reader;

/// Convert one XHTML document body to typst prose. `img` `src`s are
/// emitted verbatim as `#image("src")`; the orchestrator rewrites them
/// to the on-disk path after extracting the image.
///
/// A5 — `<blockquote>` is preserved as `#quote[…]` and `<table>` as a real
/// `#table(columns: N, …)` (its cells were previously concatenated into a
/// run-on paragraph); both were formerly flattened / garbled.
pub fn xhtml_to_typst(xhtml: &str) -> String {
    // I-2 — first pass: gather the collected footnote bodies so an inline
    // `noteref` can be rebuilt as `#footnote[body]` (the epub → import round-trip).
    let footnotes = collect_footnotes(xhtml);

    let mut reader = Reader::from_str(xhtml);
    reader.config_mut().check_end_names = false; // tolerate sloppy XHTML

    let mut buf = Vec::new();
    let mut blocks: Vec<String> = Vec::new();
    let mut line = String::new();
    let mut heading: Option<usize> = None;
    let mut list_stack: Vec<char> = Vec::new();
    let mut in_head = false;
    // Depth inside a footnote body / footnotes section — suppress its content in
    // the main flow (it's been hoisted inline as `#footnote[…]`).
    let mut fn_suppress = 0usize;
    // Inside an inline `<a noteref>` — suppress the superscript marker text.
    let mut in_noteref = false;
    // A5 — blockquote nesting; inner blocks accumulate here and are wrapped as
    // `#quote[…]` when the outermost blockquote closes.
    let mut quote_depth = 0usize;
    let mut quote_buf: Vec<String> = Vec::new();
    // A5 — table capture: rows of cells; `cell` is the buffer for the open td/th.
    let mut table_depth = 0usize;
    let mut table_rows: Vec<Vec<String>> = Vec::new();
    let mut cur_row: Vec<String> = Vec::new();
    let mut cell: Option<String> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                // Enter (and suppress) a footnote body / footnotes section.
                if (name == b"aside" && is_footnote_body(&e)) || is_footnotes_section(&e) {
                    fn_suppress += 1;
                    buf.clear();
                    continue;
                }
                if fn_suppress > 0 {
                    buf.clear();
                    continue;
                }
                match name {
                    b"head" => in_head = true,
                    b"h1" | b"h2" | b"h3" | b"h4" | b"h5" | b"h6" => {
                        flush_block(&mut blocks, &mut quote_buf, quote_depth, &mut line);
                        heading = Some(heading_level(name));
                    }
                    b"table" => {
                        flush_block(&mut blocks, &mut quote_buf, quote_depth, &mut line);
                        table_depth += 1;
                        table_rows.clear();
                        cur_row.clear();
                        cell = None;
                    }
                    b"tr" if table_depth > 0 => cur_row.clear(),
                    b"td" | b"th" if table_depth > 0 => cell = Some(String::new()),
                    b"p" | b"div" => {
                        flush_block(&mut blocks, &mut quote_buf, quote_depth, &mut line)
                    }
                    b"blockquote" => {
                        flush_block(&mut blocks, &mut quote_buf, quote_depth, &mut line);
                        quote_depth += 1;
                    }
                    b"em" | b"i" => push_inline(&mut cell, table_depth, &mut line, '_'),
                    b"strong" | b"b" => push_inline(&mut cell, table_depth, &mut line, '*'),
                    // Inline footnote reference → rebuild the note here.
                    b"a" if is_noteref(&e) => {
                        if let Some(body) = attr(&e, b"href")
                            .and_then(|h| h.strip_prefix('#').map(str::to_string))
                            .and_then(|id| footnotes.get(&id))
                        {
                            // Guard the content brackets so a `]` in the body
                            // can't close the `#footnote[…]` early.
                            let safe = body.replace('[', "\\[").replace(']', "\\]");
                            push_inline_str(
                                &mut cell,
                                table_depth,
                                &mut line,
                                &format!("#footnote[{safe}]"),
                            );
                        }
                        in_noteref = true;
                    }
                    b"ul" => list_stack.push('-'),
                    b"ol" => list_stack.push('+'),
                    b"li" => {
                        flush_block(&mut blocks, &mut quote_buf, quote_depth, &mut line);
                        let marker = list_stack.last().copied().unwrap_or('-');
                        line.push(marker);
                        line.push(' ');
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if fn_suppress > 0 {
                    if name == b"aside" || name == b"section" {
                        fn_suppress = fn_suppress.saturating_sub(1);
                    }
                    buf.clear();
                    continue;
                }
                match name {
                    b"head" => in_head = false,
                    b"h1" | b"h2" | b"h3" | b"h4" | b"h5" | b"h6" => {
                        if let Some(lvl) = heading.take() {
                            let t = line.trim();
                            if !t.is_empty() {
                                blocks.push(format!("{} {}", "=".repeat(lvl), t));
                            }
                            line.clear();
                        }
                    }
                    b"td" | b"th" if table_depth > 0 => {
                        if let Some(c) = cell.take() {
                            cur_row.push(c.trim().to_string());
                        }
                    }
                    b"tr" if table_depth > 0 => {
                        if !cur_row.is_empty() {
                            table_rows.push(std::mem::take(&mut cur_row));
                        }
                    }
                    b"table" => {
                        table_depth = table_depth.saturating_sub(1);
                        if table_depth == 0 {
                            if let Some(tbl) = emit_table(&table_rows) {
                                blocks.push(tbl);
                            }
                            table_rows.clear();
                        }
                    }
                    b"p" | b"div" | b"li" => {
                        flush_block(&mut blocks, &mut quote_buf, quote_depth, &mut line)
                    }
                    b"blockquote" => {
                        flush_block(&mut blocks, &mut quote_buf, quote_depth, &mut line);
                        quote_depth = quote_depth.saturating_sub(1);
                        if quote_depth == 0 {
                            let body = quote_buf.join("\n\n");
                            quote_buf.clear();
                            if !body.trim().is_empty() {
                                blocks.push(format!("#quote[{body}]"));
                            }
                        }
                    }
                    b"em" | b"i" => push_inline(&mut cell, table_depth, &mut line, '_'),
                    b"strong" | b"b" => push_inline(&mut cell, table_depth, &mut line, '*'),
                    b"a" if in_noteref => in_noteref = false,
                    b"ul" | b"ol" => {
                        list_stack.pop();
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) => {
                if fn_suppress > 0 {
                    buf.clear();
                    continue;
                }
                match local(e.name().as_ref()) {
                    b"br" => push_inline(&mut cell, table_depth, &mut line, ' '),
                    b"img" => {
                        if let Some(src) = attr(&e, b"src") {
                            push_inline_str(
                                &mut cell,
                                table_depth,
                                &mut line,
                                &format!("#image(\"{src}\")"),
                            );
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(t)) => {
                // Suppress footnote-section text and the noteref's marker digit.
                if !in_head && fn_suppress == 0 && !in_noteref {
                    let s = t.unescape().unwrap_or_default();
                    let esc = escape_typst(&s);
                    if let Some(c) = cell.as_mut() {
                        c.push_str(&esc);
                    } else if table_depth == 0 {
                        // Between table cells stray text/whitespace is dropped.
                        line.push_str(&esc);
                    }
                }
            }
            Ok(Event::Eof) => break,
            // Lenient: keep what we have rather than failing the import.
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    // Drain any content left open by malformed / unclosed markup so nothing is lost.
    flush_block(&mut blocks, &mut quote_buf, quote_depth, &mut line);
    if !quote_buf.is_empty() {
        blocks.push(format!("#quote[{}]", quote_buf.join("\n\n")));
    }
    if !cur_row.is_empty() {
        table_rows.push(cur_row);
    }
    if let Some(tbl) = emit_table(&table_rows) {
        blocks.push(tbl);
    }
    blocks.join("\n\n")
}

/// Append inline `ch` to the active sink: the open table cell if any, else the
/// main line (content between table cells is discarded).
fn push_inline(cell: &mut Option<String>, table_depth: usize, line: &mut String, ch: char) {
    if let Some(c) = cell.as_mut() {
        c.push(ch);
    } else if table_depth == 0 {
        line.push(ch);
    }
}

/// String form of [`push_inline`].
fn push_inline_str(cell: &mut Option<String>, table_depth: usize, line: &mut String, s: &str) {
    if let Some(c) = cell.as_mut() {
        c.push_str(s);
    } else if table_depth == 0 {
        line.push_str(s);
    }
}

/// Flush the current inline buffer as a block — into the quote buffer while
/// inside a `<blockquote>`, otherwise straight into the block list.
fn flush_block(
    blocks: &mut Vec<String>,
    quote_buf: &mut Vec<String>,
    quote_depth: usize,
    line: &mut String,
) {
    let t = line.trim();
    if !t.is_empty() {
        if quote_depth > 0 {
            quote_buf.push(t.to_string());
        } else {
            blocks.push(t.to_string());
        }
    }
    line.clear();
}

/// Render captured table rows as a typst `#table(columns: N, …)`. Short rows are
/// padded to the widest so the column count stays consistent. `None` if empty.
fn emit_table(rows: &[Vec<String>]) -> Option<String> {
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    if cols == 0 {
        return None;
    }
    let mut s = format!("#table(\n  columns: {cols},");
    for row in rows {
        s.push_str("\n ");
        for c in row {
            s.push_str(&format!(" [{}],", c.trim()));
        }
        for _ in row.len()..cols {
            s.push_str(" [],");
        }
    }
    s.push_str("\n)");
    Some(s)
}

fn heading_level(local: &[u8]) -> usize {
    match local {
        b"h1" => 1,
        b"h2" => 2,
        b"h3" => 3,
        b"h4" => 4,
        b"h5" => 5,
        _ => 6,
    }
}

/// Escape the typst-markup-significant characters in plain text so an
/// imported sentence containing `*`, `_`, `#`, … doesn't render as
/// accidental markup. The delimiters we emit ourselves (for em/strong)
/// are written separately and aren't escaped.
fn escape_typst(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '\\' | '#' | '*' | '_' | '`' | '$' | '@' | '<' | '>') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Strip an XML namespace prefix (`xhtml:p` → `p`).
fn local(name: &[u8]) -> &[u8] {
    match name.iter().rposition(|&b| b == b':') {
        Some(i) => &name[i + 1..],
        None => name,
    }
}

fn attr(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<String> {
    for a in e.attributes().flatten() {
        if local(a.key.as_ref()) == key {
            return Some(String::from_utf8_lossy(&a.value).into_owned());
        }
    }
    None
}

/// True for a footnote *body* element — an `<aside epub:type="footnote">` (or
/// `role="doc-footnote"`), the form `crate::epub` emits and readers popup.
fn is_footnote_body(e: &quick_xml::events::BytesStart) -> bool {
    attr(e, b"type").as_deref() == Some("footnote")
        || attr(e, b"role").as_deref() == Some("doc-footnote")
}

/// True for the collected footnotes container (`<section epub:type="footnotes">`).
fn is_footnotes_section(e: &quick_xml::events::BytesStart) -> bool {
    attr(e, b"type").as_deref() == Some("footnotes")
        || attr(e, b"role").as_deref() == Some("doc-endnotes")
}

/// True for an inline footnote *reference* (`<a epub:type="noteref">`).
fn is_noteref(e: &quick_xml::events::BytesStart) -> bool {
    attr(e, b"type").as_deref() == Some("noteref")
        || attr(e, b"role").as_deref() == Some("doc-noteref")
}

/// I-2 — pre-scan for the collected footnote bodies, keyed by their `id`, so a
/// `noteref` encountered inline can be rebuilt as a typst `#footnote[body]`. The
/// body is the aside's prose minus the leading superscript marker and the trailing
/// back-link anchor (both `crate::epub` adds on export). Keeps `*`/`_` emphasis.
fn collect_footnotes(xhtml: &str) -> std::collections::BTreeMap<String, String> {
    let mut reader = Reader::from_str(xhtml);
    reader.config_mut().check_end_names = false;
    let mut buf = Vec::new();
    let mut map = std::collections::BTreeMap::new();
    let mut cur_id: Option<String> = None;
    let mut body = String::new();
    let mut skip_inline = 0usize; // inside <sup> (marker) or the doc-backlink <a>
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if name == b"aside" && is_footnote_body(&e) {
                    cur_id = attr(&e, b"id");
                    body.clear();
                    skip_inline = 0;
                } else if cur_id.is_some() {
                    if name == b"sup"
                        || (name == b"a" && attr(&e, b"role").as_deref() == Some("doc-backlink"))
                    {
                        skip_inline += 1;
                    } else if name == b"em" || name == b"i" {
                        body.push('_');
                    } else if name == b"strong" || name == b"b" {
                        body.push('*');
                    }
                }
            }
            Ok(Event::End(e)) => {
                if cur_id.is_none() {
                    buf.clear();
                    continue;
                }
                match local(e.name().as_ref()) {
                    b"aside" => {
                        if let Some(id) = cur_id.take() {
                            let t = body.trim().to_string();
                            if !t.is_empty() {
                                map.insert(id, t);
                            }
                        }
                    }
                    b"sup" | b"a" => skip_inline = skip_inline.saturating_sub(1),
                    b"em" | b"i" => body.push('_'),
                    b"strong" | b"b" => body.push('*'),
                    _ => {}
                }
            }
            Ok(Event::Text(t)) => {
                if cur_id.is_some() && skip_inline == 0 {
                    let s = t.unescape().unwrap_or_default();
                    body.push_str(&escape_typst(&s));
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_paragraphs_and_inline() {
        let xhtml = "<html><body>\
            <h1>Chapter One</h1>\
            <p>The road was <strong>long</strong> and <em>cold</em>.</p>\
            <p>Second paragraph.</p>\
            </body></html>";
        let typ = xhtml_to_typst(xhtml);
        assert!(typ.contains("= Chapter One"));
        assert!(typ.contains("The road was *long* and _cold_."));
        // Blocks separated by a blank line.
        assert!(typ.contains("\n\nSecond paragraph."));
    }

    #[test]
    fn lists_and_images() {
        let xhtml = "<body><ul><li>alpha</li><li>beta</li></ul>\
            <p>see <img src=\"img/x.png\"/> here</p></body>";
        let typ = xhtml_to_typst(xhtml);
        assert!(typ.contains("- alpha"));
        assert!(typ.contains("- beta"));
        assert!(typ.contains("#image(\"img/x.png\")"));
    }

    #[test]
    fn text_is_typst_escaped() {
        // A sentence with literal markup chars must be escaped.
        let typ = xhtml_to_typst("<body><p>cost is #5 *not* a list_item</p></body>");
        assert!(typ.contains("\\#5"), "got: {typ}");
        assert!(typ.contains("\\*not\\*"), "got: {typ}");
        assert!(typ.contains("list\\_item"), "got: {typ}");
    }

    #[test]
    fn footnote_noteref_is_rebuilt_and_section_suppressed() {
        // I-2 — the exact shape `crate::epub` emits: an inline noteref + a
        // collected <aside> in a footnotes <section>.
        let xhtml = "<body><p>The bell rang<a epub:type=\"noteref\" role=\"doc-noteref\" id=\"fnref-1\" href=\"#fn-1\"><sup>1</sup></a> once.</p><section epub:type=\"footnotes\" role=\"doc-endnotes\"><aside epub:type=\"footnote\" role=\"doc-footnote\" id=\"fn-1\"><p><sup>1</sup> At dawn. <a href=\"#fnref-1\" role=\"doc-backlink\">\u{21a9}</a></p></aside></section></body>";
        let typ = xhtml_to_typst(xhtml);
        assert!(typ.contains("The bell rang#footnote[At dawn.] once."), "got: {typ:?}");
        // The collected footnotes section must NOT leak as a stray paragraph, and
        // the noteref marker digit must be gone.
        assert!(!typ.contains("At dawn.\n\n"), "note body not duplicated: {typ:?}");
        assert!(!typ.contains('\u{21a9}'), "backlink arrow dropped: {typ:?}");
    }

    #[test]
    fn blockquote_becomes_quote() {
        // A5 — a <blockquote> keeps quote semantics as #quote[…], not a flat para.
        let xhtml = "<body><p>He said:</p>\
            <blockquote><p>The sea is <em>wide</em>.</p><p>And deep.</p></blockquote>\
            <p>Then left.</p></body>";
        let typ = xhtml_to_typst(xhtml);
        assert!(typ.contains("#quote[The sea is _wide_.\n\nAnd deep.]"), "got: {typ}");
        // The surrounding prose is untouched.
        assert!(typ.contains("He said:") && typ.contains("Then left."), "got: {typ}");
    }

    #[test]
    fn table_becomes_typst_table() {
        // A5 — <table> cells become a real #table, not a run-on paragraph.
        let xhtml = "<body><table>\
            <tr><th>Name</th><th>Age</th></tr>\
            <tr><td>Ada</td><td>36</td></tr>\
            </table></body>";
        let typ = xhtml_to_typst(xhtml);
        assert!(typ.contains("#table("), "got: {typ}");
        assert!(typ.contains("columns: 2"), "got: {typ}");
        assert!(typ.contains("[Name],") && typ.contains("[Age],"), "got: {typ}");
        assert!(typ.contains("[Ada],") && typ.contains("[36],"), "got: {typ}");
        // No run-on paragraph gluing the cells together.
        assert!(!typ.contains("NameAge"), "cells not concatenated: {typ}");
    }

    #[test]
    fn blockquote_round_trips_through_export() {
        // typst #quote → xhtml <blockquote> → typst #quote.
        let xhtml = crate::epub::typst_to_xhtml("#quote[A borrowed line.]\n");
        assert!(xhtml.contains("<blockquote>"), "export emits blockquote: {xhtml}");
        let back = xhtml_to_typst(&xhtml);
        assert!(back.contains("#quote[A borrowed line.]"), "round-trip: {xhtml:?} -> {back:?}");
    }

    #[test]
    fn footnote_round_trips_through_export() {
        // The true round-trip: a #footnote survives typst → xhtml → typst.
        let xhtml = crate::epub::typst_to_xhtml("A claim#footnote[The source.] stands.\n");
        let back = xhtml_to_typst(&xhtml);
        assert!(back.contains("#footnote[The source.]"), "round-trip: {xhtml:?} -> {back:?}");
    }

    #[test]
    fn head_content_is_dropped() {
        let xhtml = "<html><head><title>meta</title></head><body><p>body text</p></body></html>";
        let typ = xhtml_to_typst(xhtml);
        assert!(!typ.contains("meta"));
        assert!(typ.contains("body text"));
    }

    use proptest::prelude::*;
    proptest! {
        /// Arbitrary input must never panic the converter (untrusted
        /// XHTML from an imported file).
        #[test]
        fn never_panics(s in "\\PC{0,400}") {
            let _ = xhtml_to_typst(&s);
        }

        /// A tag-salad of the elements we key on, interleaved with
        /// prose, must also stay panic-free.
        #[test]
        fn tag_salad_never_panics(
            toks in proptest::collection::vec(
                proptest::sample::select(vec![
                    "<p>", "</p>", "<h1>", "</h1>", "<strong>", "</strong>",
                    "<em>", "<ul>", "<li>", "</li>", "</ul>", "<br/>",
                    "<blockquote>", "</blockquote>", "<table>", "</table>",
                    "<tr>", "</tr>", "<td>", "</td>", "<th>", "</th>",
                    "<img src=\"x\"/>", "word", " ", "&amp;", "<", ">",
                ]),
                0..200,
            ),
        ) {
            let _ = xhtml_to_typst(&toks.concat());
        }
    }
}
