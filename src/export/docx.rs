//! 1.3.1 SUBMISSION-1 — hand-rolled Shunn-format `.docx` writer.
//!
//! A `.docx` is an OOXML package: a zip of a fixed set of XML parts.  We
//! emit it by hand over the in-tree `zip` crate (the same one the EPUB
//! writer uses) rather than pulling `docx-rs`, which hard-depends on a
//! *second* major version of `zip` (0.6 vs the in-tree 2.x) plus its own
//! `flate2` chain — duplicate-dependency tech debt for a format that is,
//! at heart, six small XML files in a zip.
//!
//! Output is **standard manuscript format** (Shunn): a title page (contact
//! corner + rounded word count + centred title/byline), then double-spaced
//! Times New Roman (or Courier) 12 pt body with a 1″ margin, ½″ first-line
//! indent, scene breaks as a centred `#`, each chapter starting a fresh
//! page, and a `Surname / KEYWORD / page#` running header from page 2.
//!
//! Reuses [`ManuscriptMeta`] / [`ManuscriptChapter`] / `round_word_count`
//! / `header_keyword` / `is_scene_break` from [`crate::manuscript`], so the
//! typst and `.docx` paths share one notion of the format.

// P0 lands the writer + its fidelity gate; the public API has no in-binary
// caller until P1 wires the `inkhaven docx` CLI + the `docx` book-take.
// Retire this allow when P1 lands.
#![allow(dead_code)]

use std::io::Write;

use anyhow::Result;
use zip::write::SimpleFileOptions;

use crate::manuscript::{header_keyword, is_scene_break, round_word_count, ManuscriptChapter, ManuscriptMeta};

/// Body typeface.  Shunn accepts either; Courier is the traditional pick,
/// Times the common modern one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocxFont {
    TimesNewRoman,
    Courier,
}

impl DocxFont {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().replace([' ', '-', '_'], "").as_str() {
            "times" | "timesnewroman" | "serif" => Some(Self::TimesNewRoman),
            "courier" | "couriernew" | "mono" | "monospace" => Some(Self::Courier),
            _ => None,
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::TimesNewRoman => "Times New Roman",
            Self::Courier => "Courier New",
        }
    }
}

const HALF_PT_12: &str = "24"; // 12 pt in OOXML half-points
const TWIPS_INCH: u32 = 1440; // 1 inch
const DOUBLE_LINE: &str = "480"; // 240 = single, 480 = double (lineRule=auto)

/// Build a Shunn-format `.docx` for `meta` + `chapters` in `font`.
pub fn build_docx(
    meta: &ManuscriptMeta,
    chapters: &[ManuscriptChapter],
    font: DocxFont,
) -> Result<Vec<u8>> {
    let mut buf: Vec<u8> = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut buf);
        let mut zw = zip::ZipWriter::new(cursor);
        let opts = SimpleFileOptions::default();
        let mut put = |name: &str, body: &str| -> Result<()> {
            zw.start_file(name, opts)?;
            zw.write_all(body.as_bytes())?;
            Ok(())
        };
        put("[Content_Types].xml", CONTENT_TYPES)?;
        put("_rels/.rels", ROOT_RELS)?;
        put("word/_rels/document.xml.rels", DOC_RELS)?;
        put("word/styles.xml", &styles_xml(font))?;
        put("word/header2.xml", &header_xml(&meta.surname, &meta.title))?;
        put("word/document.xml", &document_xml(meta, chapters))?;
        zw.finish()?;
    }
    Ok(buf)
}

// ── document body ───────────────────────────────────────────────────

fn document_xml(meta: &ManuscriptMeta, chapters: &[ManuscriptChapter]) -> String {
    let mut b = String::new();
    b.push_str(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" \
xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">\n<w:body>\n",
    );

    // ── title page (single-spaced) ──────────────────────────────────
    for line in meta.contact.lines() {
        b.push_str(&para(line, &[Prop::SingleSpace, Prop::Left]));
    }
    // word count, right-aligned
    b.push_str(&para(
        &format!("approx. {} words", round_word_count(meta.word_count)),
        &[Prop::SingleSpace, Prop::Right],
    ));
    // vertical drop to ~⅓ page, then centred title + byline
    for _ in 0..8 {
        b.push_str(&para("", &[Prop::SingleSpace, Prop::Center]));
    }
    b.push_str(&para(&meta.title.to_uppercase(), &[Prop::Center]));
    b.push_str(&para("", &[Prop::Center]));
    b.push_str(&para(&format!("by {}", meta.byline), &[Prop::Center]));

    // ── chapters (each starts a fresh page, double-spaced body) ─────
    for ch in chapters {
        b.push_str(&para(&ch.title.to_uppercase(), &[Prop::PageBreakBefore, Prop::Center]));
        b.push_str(&para("", &[Prop::Center]));
        for p in &ch.paragraphs {
            if is_scene_break(p) {
                b.push_str(&para("#", &[Prop::Center]));
            } else {
                b.push_str(&para(p, &[Prop::FirstLineIndent]));
            }
        }
    }

    // ── section: header from page 2, 1" margins, US Letter ──────────
    b.push_str(&format!(
        "<w:sectPr>\
<w:headerReference w:type=\"default\" r:id=\"rIdHeader\"/>\
<w:titlePg/>\
<w:pgSz w:w=\"12240\" w:h=\"15840\"/>\
<w:pgMar w:top=\"{m}\" w:right=\"{m}\" w:bottom=\"{m}\" w:left=\"{m}\" \
w:header=\"720\" w:footer=\"720\" w:gutter=\"0\"/>\
</w:sectPr>\n",
        m = TWIPS_INCH,
    ));
    b.push_str("</w:body>\n</w:document>\n");
    b
}

/// Paragraph property flags.
enum Prop {
    SingleSpace,
    Left,
    Right,
    Center,
    FirstLineIndent,
    PageBreakBefore,
}

/// One `<w:p>` with the given text + properties.  Empty text → a blank
/// (spacer) paragraph.
fn para(text: &str, props: &[Prop]) -> String {
    let mut ppr = String::new();
    if props.iter().any(|p| matches!(p, Prop::PageBreakBefore)) {
        ppr.push_str("<w:pageBreakBefore/>");
    }
    if props.iter().any(|p| matches!(p, Prop::SingleSpace)) {
        ppr.push_str("<w:spacing w:line=\"240\" w:lineRule=\"auto\"/>");
    }
    if props.iter().any(|p| matches!(p, Prop::FirstLineIndent)) {
        ppr.push_str("<w:ind w:firstLine=\"720\"/>");
    }
    let jc = props.iter().find_map(|p| match p {
        Prop::Left => Some("left"),
        Prop::Right => Some("right"),
        Prop::Center => Some("center"),
        _ => None,
    });
    if let Some(jc) = jc {
        ppr.push_str(&format!("<w:jc w:val=\"{jc}\"/>"));
    }
    let ppr = if ppr.is_empty() {
        String::new()
    } else {
        format!("<w:pPr>{ppr}</w:pPr>")
    };
    let run = if text.is_empty() {
        String::new()
    } else {
        format!("<w:r><w:t xml:space=\"preserve\">{}</w:t></w:r>", xml_escape(text))
    };
    format!("<w:p>{ppr}{run}</w:p>\n")
}

// ── header part (running header, page 2+) ───────────────────────────

fn header_xml(surname: &str, title: &str) -> String {
    let label = format!("{} / {} / ", surname, header_keyword(title));
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
<w:hdr xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\
<w:p><w:pPr><w:jc w:val=\"right\"/></w:pPr>\
<w:r><w:t xml:space=\"preserve\">{label}</w:t></w:r>\
<w:r><w:fldChar w:fldCharType=\"begin\"/></w:r>\
<w:r><w:instrText xml:space=\"preserve\"> PAGE </w:instrText></w:r>\
<w:r><w:fldChar w:fldCharType=\"end\"/></w:r>\
</w:p></w:hdr>\n",
        label = xml_escape(&label),
    )
}

// ── styles (font + global double-spacing via docDefaults) ───────────

fn styles_xml(font: DocxFont) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
<w:styles xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\
<w:docDefaults><w:rPrDefault><w:rPr>\
<w:rFonts w:ascii=\"{f}\" w:hAnsi=\"{f}\" w:cs=\"{f}\"/>\
<w:sz w:val=\"{sz}\"/><w:szCs w:val=\"{sz}\"/></w:rPr></w:rPrDefault>\
<w:pPrDefault><w:pPr><w:spacing w:line=\"{line}\" w:lineRule=\"auto\"/></w:pPr></w:pPrDefault>\
</w:docDefaults>\
<w:style w:type=\"paragraph\" w:default=\"1\" w:styleId=\"Normal\"><w:name w:val=\"Normal\"/></w:style>\
</w:styles>\n",
        f = font.name(),
        sz = HALF_PT_12,
        line = DOUBLE_LINE,
    )
}

// ── static parts ────────────────────────────────────────────────────

const CONTENT_TYPES: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
<Default Extension=\"xml\" ContentType=\"application/xml\"/>\
<Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/>\
<Override PartName=\"/word/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml\"/>\
<Override PartName=\"/word/header2.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml\"/>\
</Types>\n";

const ROOT_RELS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/>\
</Relationships>\n";

const DOC_RELS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rIdStyles\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/>\
<Relationship Id=\"rIdHeader\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/header\" Target=\"header2.xml\"/>\
</Relationships>\n";

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn sample() -> (ManuscriptMeta, Vec<ManuscriptChapter>) {
        let meta = ManuscriptMeta {
            title: "The Harbor Code".into(),
            contact: "Jane Writer\n12 Wharf Rd\njane@example.com".into(),
            byline: "Jane Writer".into(),
            surname: "Writer".into(),
            word_count: 80_123,
        };
        let chapters = vec![
            ManuscriptChapter {
                title: "Chapter One".into(),
                paragraphs: vec![
                    "The harbor was quiet that morning.".into(),
                    "* * *".into(),
                    "By noon it was not.".into(),
                ],
            },
            ManuscriptChapter {
                title: "Chapter Two".into(),
                paragraphs: vec!["A new day, & a new <tide>.".into()],
            },
        ];
        (meta, chapters)
    }

    /// Read one part out of the generated zip as a string.
    fn part(bytes: &[u8], name: &str) -> String {
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("valid zip");
        let mut f = zip.by_name(name).unwrap_or_else(|_| panic!("missing part {name}"));
        let mut s = String::new();
        f.read_to_string(&mut s).unwrap();
        s
    }

    #[test]
    fn package_has_every_required_part() {
        let (m, c) = sample();
        let bytes = build_docx(&m, &c, DocxFont::TimesNewRoman).unwrap();
        for p in [
            "[Content_Types].xml",
            "_rels/.rels",
            "word/_rels/document.xml.rels",
            "word/styles.xml",
            "word/header2.xml",
            "word/document.xml",
        ] {
            let _ = part(&bytes, p); // panics if absent
        }
    }

    #[test]
    fn styles_carry_font_and_double_spacing() {
        let (m, c) = sample();
        let times = build_docx(&m, &c, DocxFont::TimesNewRoman).unwrap();
        let s = part(&times, "word/styles.xml");
        assert!(s.contains("Times New Roman"));
        assert!(s.contains("w:line=\"480\""), "double spacing (480 twips)");
        assert!(s.contains("w:sz w:val=\"24\""), "12 pt");
        // font switch
        let cour = build_docx(&m, &c, DocxFont::Courier).unwrap();
        assert!(part(&cour, "word/styles.xml").contains("Courier New"));
    }

    #[test]
    fn header_has_keyword_and_live_page_field() {
        let (m, c) = sample();
        let h = part(&build_docx(&m, &c, DocxFont::TimesNewRoman).unwrap(), "word/header2.xml");
        // "The Harbor Code" → keyword HARBOR; surname Writer
        assert!(h.contains("Writer / HARBOR / "), "running-header label");
        assert!(h.contains("instrText") && h.contains(" PAGE "), "live page field");
    }

    #[test]
    fn document_has_titlepage_header_pagebreaks_and_scene_break() {
        let (m, c) = sample();
        let d = part(&build_docx(&m, &c, DocxFont::TimesNewRoman).unwrap(), "word/document.xml");
        assert!(d.contains("<w:titlePg/>"), "title page suppresses p1 header");
        assert!(d.contains("rIdHeader"), "section references the header");
        assert!(d.contains("approx. 80000 words"), "rounded word count on title page");
        // two chapters → two page breaks
        assert_eq!(d.matches("<w:pageBreakBefore/>").count(), 2);
        // scene break rendered as a centred #
        assert!(d.contains("<w:t xml:space=\"preserve\">#</w:t>"));
        // XML-escaping of body prose
        assert!(d.contains("&amp; a new &lt;tide&gt;"));
        // well-formed: parses as XML
        assert!(quick_xml_well_formed(&d), "document.xml is well-formed");
    }

    fn quick_xml_well_formed(xml: &str) -> bool {
        use quick_xml::events::Event;
        use quick_xml::reader::Reader;
        let mut r = Reader::from_str(xml);
        loop {
            match r.read_event() {
                Ok(Event::Eof) => return true,
                Err(_) => return false,
                _ => {}
            }
        }
    }

    /// Fidelity gate: emit a sample to /tmp for a manual Word /
    /// LibreOffice / Google-Docs open.  Run with:
    ///   cargo test --bin inkhaven export::docx -- --ignored --nocapture
    #[test]
    #[ignore = "writes a file for manual inspection"]
    fn emit_sample_docx_for_manual_word_check() {
        let (m, c) = sample();
        let bytes = build_docx(&m, &c, DocxFont::TimesNewRoman).unwrap();
        let path = "/tmp/inkhaven-shunn-sample.docx";
        std::fs::write(path, &bytes).unwrap();
        println!("wrote {} ({} bytes) — open in Word and check:", path, bytes.len());
        println!("  - page 1 (title page) has NO running header");
        println!("  - page 2+ header reads 'Writer / HARBOR / <n>' (live page #)");
        println!("  - body is double-spaced 12pt Times New Roman");
        println!("  - each chapter starts on a fresh page; scene break is a centred #");
    }
}
