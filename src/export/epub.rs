//! Minimal EPUB3 writer.
//!
//! Builds an EPUB3-compliant zip from a markdown source string,
//! using the same `zip` crate already pulled in for backup
//! archives. Single-chapter layout — the markdown becomes one
//! XHTML file, the nav lists it under `title`, and OPF / NCX
//! metadata is generated from `title` + a deterministic UUID
//! derived from the content hash. No epub-specific dependency
//! means we don't bloat the binary just to write this one
//! format, but it also means the converter is on the conservative
//! side: rich features (cover image, multiple chapters,
//! cross-references) are explicitly out of scope.

use std::io::Write;

use anyhow::Result;
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use zip::write::SimpleFileOptions;

/// Write a single-chapter EPUB3 archive. `title` lands in the
/// metadata / nav; `markdown_src` becomes the body XHTML; `lang` is the BCP-47
/// language tag (`en`, `ru`, …) stamped on the OPF metadata and the content /
/// nav documents so e-reader hyphenation, TTS, and font selection are correct.
pub fn write_epub(markdown_src: &str, title: &str, lang: &str) -> Result<Vec<u8>> {
    let mut buf: Vec<u8> = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut buf);
        let mut zw = zip::ZipWriter::new(cursor);
        // mimetype MUST be the first entry, stored (not deflated),
        // with no extra fields. EPUB readers reject archives that
        // violate this. The zip crate's "stored" method is the
        // way to skip deflate for a single entry.
        zw.start_file(
            "mimetype",
            SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored),
        )?;
        zw.write_all(b"application/epub+zip")?;

        let html_body = markdown_to_xhtml_body(markdown_src);
        let chapter_xhtml = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE html>\n\
<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\" xml:lang=\"{lang}\" lang=\"{lang}\">\n\
<head><meta charset=\"utf-8\"/><title>{title_esc}</title></head>\n\
<body>{body}</body>\n\
</html>\n",
            title_esc = xml_escape(title),
            body = html_body,
            lang = xml_escape(lang),
        );

        let nav_xhtml = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE html>\n\
<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\" xml:lang=\"{lang}\" lang=\"{lang}\">\n\
<head><meta charset=\"utf-8\"/><title>Navigation</title></head>\n\
<body><nav epub:type=\"toc\"><ol><li><a href=\"chapter.xhtml\">{title_esc}</a></li></ol></nav></body>\n\
</html>\n",
            title_esc = xml_escape(title),
            lang = xml_escape(lang),
        );

        // Stable UUID-ish identifier derived from the title +
        // content length so repeated exports of the same project
        // are byte-stable (ignoring zip metadata timestamps that
        // the zip crate fixes anyway).
        let identifier = format!(
            "urn:inkhaven:{}-{}-{}",
            slug::slugify(title),
            markdown_src.len(),
            crude_hash(markdown_src),
        );
        let opf = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<package version=\"3.0\" xmlns=\"http://www.idpf.org/2007/opf\" unique-identifier=\"book-id\">\n\
  <metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\">\n\
    <dc:identifier id=\"book-id\">{id_esc}</dc:identifier>\n\
    <dc:title>{title_esc}</dc:title>\n\
    <dc:language>{lang_esc}</dc:language>\n\
    <meta property=\"dcterms:modified\">2025-01-01T00:00:00Z</meta>\n\
  </metadata>\n\
  <manifest>\n\
    <item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>\n\
    <item id=\"chap\" href=\"chapter.xhtml\" media-type=\"application/xhtml+xml\"/>\n\
  </manifest>\n\
  <spine>\n\
    <itemref idref=\"chap\"/>\n\
  </spine>\n\
</package>\n",
            id_esc = xml_escape(&identifier),
            title_esc = xml_escape(title),
            lang_esc = xml_escape(lang),
        );

        let container_xml = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<container version=\"1.0\" xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\">\n\
  <rootfiles>\n\
    <rootfile full-path=\"OEBPS/content.opf\" media-type=\"application/oebps-package+xml\"/>\n\
  </rootfiles>\n\
</container>\n";

        // Everything else gets deflated.
        let opts = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zw.start_file("META-INF/container.xml", opts)?;
        zw.write_all(container_xml.as_bytes())?;
        zw.start_file("OEBPS/content.opf", opts)?;
        zw.write_all(opf.as_bytes())?;
        zw.start_file("OEBPS/nav.xhtml", opts)?;
        zw.write_all(nav_xhtml.as_bytes())?;
        zw.start_file("OEBPS/chapter.xhtml", opts)?;
        zw.write_all(chapter_xhtml.as_bytes())?;

        zw.finish()?;
    }
    Ok(buf)
}

/// Render the supplied markdown source to an XHTML body fragment
/// (`<h1>` / `<p>` / etc — no wrapping `<html>` or `<body>`).
/// Uses `pulldown-cmark` which is already pulled in for the AI
/// pane's markdown rendering, so no extra dep.
fn markdown_to_xhtml_body(src: &str) -> String {
    let parser = Parser::new(src);
    let mut out = String::with_capacity(src.len() + 64);
    let mut in_code = false;
    for ev in parser {
        match ev {
            Event::Start(Tag::Heading { level, .. }) => {
                out.push_str(&format!("<{}>", heading_tag(level)));
            }
            Event::End(TagEnd::Heading(level)) => {
                out.push_str(&format!("</{}>", heading_tag(level)));
            }
            Event::Start(Tag::Paragraph) => out.push_str("<p>"),
            Event::End(TagEnd::Paragraph) => out.push_str("</p>"),
            Event::Start(Tag::BlockQuote(_)) => out.push_str("<blockquote>"),
            Event::End(TagEnd::BlockQuote(_)) => out.push_str("</blockquote>"),
            Event::Start(Tag::Emphasis) => out.push_str("<em>"),
            Event::End(TagEnd::Emphasis) => out.push_str("</em>"),
            Event::Start(Tag::Strong) => out.push_str("<strong>"),
            Event::End(TagEnd::Strong) => out.push_str("</strong>"),
            Event::Start(Tag::List(None)) => out.push_str("<ul>"),
            Event::Start(Tag::List(Some(_))) => out.push_str("<ol>"),
            Event::End(TagEnd::List(false)) => out.push_str("</ul>"),
            Event::End(TagEnd::List(true)) => out.push_str("</ol>"),
            Event::Start(Tag::Item) => out.push_str("<li>"),
            Event::End(TagEnd::Item) => out.push_str("</li>"),
            Event::Start(Tag::CodeBlock(_)) => {
                in_code = true;
                out.push_str("<pre><code>");
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code = false;
                out.push_str("</code></pre>");
            }
            Event::Start(Tag::Image { dest_url, title, .. }) => {
                out.push_str(&format!(
                    "<img src=\"{}\" alt=\"{}\"/>",
                    xml_escape(&dest_url),
                    xml_escape(&title)
                ));
            }
            Event::Text(t) => {
                if in_code {
                    out.push_str(&xml_escape(&t));
                } else {
                    out.push_str(&xml_escape(&t));
                }
            }
            Event::Code(t) => {
                out.push_str("<code>");
                out.push_str(&xml_escape(&t));
                out.push_str("</code>");
            }
            Event::SoftBreak | Event::HardBreak => out.push('\n'),
            Event::Rule => out.push_str("<hr/>"),
            _ => {} // Drop everything we don't model — links, footnotes, tables stay textual.
        }
    }
    out
}

fn heading_tag(level: HeadingLevel) -> &'static str {
    match level {
        HeadingLevel::H1 => "h1",
        HeadingLevel::H2 => "h2",
        HeadingLevel::H3 => "h3",
        HeadingLevel::H4 => "h4",
        HeadingLevel::H5 => "h5",
        HeadingLevel::H6 => "h6",
    }
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            // XML 1.0 forbids C0 control chars (even escaped) except tab/LF/CR;
            // drop them so a stray form-feed / NUL from a paste can't produce a
            // not-well-formed document that readers silently reject.
            c if is_xml_illegal(c) => {}
            other => out.push(other),
        }
    }
    out
}

/// True for a codepoint XML 1.0 disallows anywhere in a document.
fn is_xml_illegal(c: char) -> bool {
    matches!(c, '\u{0}'..='\u{8}' | '\u{B}' | '\u{C}' | '\u{E}'..='\u{1F}')
}

/// Lightweight content-hash used to make the EPUB identifier
/// stable across repeated exports of the same project. Not
/// cryptographic — fxhash-ish via XOR + rotate.
fn crude_hash(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325; // FNV-1a offset
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn entry(zip_bytes: &[u8], name: &str) -> String {
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)).expect("valid zip");
        let mut f = zip.by_name(name).unwrap_or_else(|_| panic!("entry {name}"));
        let mut s = String::new();
        f.read_to_string(&mut s).expect("utf-8 entry");
        s
    }

    #[test]
    fn epub_stamps_the_book_language_not_hardcoded_en() {
        // A Russian book must not claim English (multilingual — a hard bar).
        let bytes = write_epub("# Глава\n\nПривет.\n", "Тест", "ru").expect("write");
        let opf = entry(&bytes, "OEBPS/content.opf");
        assert!(opf.contains("<dc:language>ru</dc:language>"), "OPF language: {opf}");
        let chap = entry(&bytes, "OEBPS/chapter.xhtml");
        assert!(chap.contains("lang=\"ru\""), "content-doc lang: {chap}");
        assert!(!chap.contains("lang=\"en\""), "must not hardcode en");
    }

    #[test]
    fn xml_escape_drops_illegal_control_chars() {
        // A form-feed / NUL from a paste must not reach the XML (would be
        // not-well-formed even escaped). Tab/newline are legal and kept.
        let out = xml_escape("a\u{0}b\u{c}c\td");
        assert_eq!(out, "abc\td");
        assert_eq!(xml_escape("<&>"), "&lt;&amp;&gt;");
    }
}
