//! EPUB import P2 — XHTML → typst prose.
//!
//! The inverse of `crate::epub::typst_to_xhtml`: walks a chapter's
//! XHTML and emits inkhaven's typst-prose subset — headings (`=`…),
//! paragraphs, `*strong*` / `_emph_`, lists, line breaks, and image
//! references. Unknown tags are dropped but their text is kept; text is
//! typst-escaped so imported prose never renders as accidental markup.
//!
//! Lenient + never-panic: a malformed document stops cleanly with
//! whatever was converted so far, rather than erroring out the import.

use quick_xml::events::Event;
use quick_xml::Reader;

/// Convert one XHTML document body to typst prose. `img` `src`s are
/// emitted verbatim as `#image("src")`; the orchestrator rewrites them
/// to the on-disk path after extracting the image.
pub fn xhtml_to_typst(xhtml: &str) -> String {
    let mut reader = Reader::from_str(xhtml);
    reader.config_mut().check_end_names = false; // tolerate sloppy XHTML

    let mut buf = Vec::new();
    let mut blocks: Vec<String> = Vec::new();
    let mut line = String::new();
    let mut heading: Option<usize> = None;
    let mut list_stack: Vec<char> = Vec::new();
    let mut in_head = false;

    // Flush the current inline buffer as a plain block.
    fn flush(blocks: &mut Vec<String>, line: &mut String) {
        let t = line.trim();
        if !t.is_empty() {
            blocks.push(t.to_string());
        }
        line.clear();
    }

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"head" => in_head = true,
                b"h1" | b"h2" | b"h3" | b"h4" | b"h5" | b"h6" => {
                    flush(&mut blocks, &mut line);
                    heading = Some(heading_level(local(e.name().as_ref())));
                }
                b"p" | b"div" | b"blockquote" => flush(&mut blocks, &mut line),
                b"em" | b"i" => line.push('_'),
                b"strong" | b"b" => line.push('*'),
                b"ul" => list_stack.push('-'),
                b"ol" => list_stack.push('+'),
                b"li" => {
                    flush(&mut blocks, &mut line);
                    let marker = list_stack.last().copied().unwrap_or('-');
                    line.push(marker);
                    line.push(' ');
                }
                _ => {}
            },
            Ok(Event::End(e)) => match local(e.name().as_ref()) {
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
                b"p" | b"div" | b"blockquote" | b"li" => flush(&mut blocks, &mut line),
                b"em" | b"i" => line.push('_'),
                b"strong" | b"b" => line.push('*'),
                b"ul" | b"ol" => {
                    list_stack.pop();
                }
                _ => {}
            },
            Ok(Event::Empty(e)) => match local(e.name().as_ref()) {
                b"br" => line.push(' '),
                b"img" => {
                    if let Some(src) = attr(&e, b"src") {
                        line.push_str(&format!("#image(\"{src}\")"));
                    }
                }
                _ => {}
            },
            Ok(Event::Text(t)) => {
                if !in_head {
                    let s = t.unescape().unwrap_or_default();
                    line.push_str(&escape_typst(&s));
                }
            }
            Ok(Event::Eof) => break,
            // Lenient: keep what we have rather than failing the import.
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    flush(&mut blocks, &mut line);
    blocks.join("\n\n")
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
                    "<img src=\"x\"/>", "word", " ", "&amp;", "<", ">",
                ]),
                0..200,
            ),
        ) {
            let _ = xhtml_to_typst(&toks.concat());
        }
    }
}
