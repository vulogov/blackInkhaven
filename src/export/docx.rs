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
        let (doc, footnotes) = document_xml(meta, chapters);
        put("word/document.xml", &doc)?;
        put("word/footnotes.xml", &footnotes_xml(&footnotes))?;
        zw.finish()?;
    }
    Ok(buf)
}

// ── document body ───────────────────────────────────────────────────

/// Returns `(document.xml, footnote_bodies)`. Footnote bodies are collected
/// document-wide (id = 1-based position) so `footnotes.xml` and the in-body
/// `<w:footnoteReference>` ids agree.
fn document_xml(meta: &ManuscriptMeta, chapters: &[ManuscriptChapter]) -> (String, Vec<String>) {
    let mut footnotes: Vec<String> = Vec::new();
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
                b.push_str(&body_para(p, &[Prop::FirstLineIndent], &mut footnotes));
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
    (b, footnotes)
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

/// The `<w:pPr>…</w:pPr>` block for the given properties (empty string if none).
fn ppr_xml(props: &[Prop]) -> String {
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
    if ppr.is_empty() {
        String::new()
    } else {
        format!("<w:pPr>{ppr}</w:pPr>")
    }
}

/// One `<w:p>` with the given text + properties as a single **plain** run —
/// no emphasis parsing.  A6 — title-page fields (contact / title / byline) and
/// chapter headings must stay literal, matching the typst path's `escape_typst`,
/// so an email or name with `_`/`*` doesn't render italic/bold and lose the
/// delimiter.  Body prose uses [`body_para`] (emphasis + footnotes) instead.
/// Empty text → a blank (spacer) paragraph.
fn para(text: &str, props: &[Prop]) -> String {
    let run = if text.is_empty() {
        String::new()
    } else {
        format!(
            "<w:r><w:t xml:space=\"preserve\">{}</w:t></w:r>",
            xml_escape(text)
        )
    };
    format!("<w:p>{}{run}</w:p>\n", ppr_xml(props))
}

/// A2 — a body paragraph whose `#footnote[…]` markers become real Word footnote
/// references, their bodies pushed onto `footnotes` (id = 1-based position).
fn body_para(text: &str, props: &[Prop], footnotes: &mut Vec<String>) -> String {
    format!("<w:p>{}{}</w:p>\n", ppr_xml(props), runs_with_footnotes(text, footnotes))
}

/// Like [`runs`], but extracts `#footnote[body]` spans into `footnotes` and emits
/// a superscript `<w:footnoteReference>` in their place.
fn runs_with_footnotes(text: &str, footnotes: &mut Vec<String>) -> String {
    const FN_OPEN: &str = "#footnote[";
    let mut out = String::with_capacity(text.len() + 16);
    let mut rest = text;
    while let Some(pos) = rest.find(FN_OPEN) {
        out.push_str(&runs(&rest[..pos]));
        let after = &rest[pos + FN_OPEN.len()..];
        match crate::manuscript::matching_bracket(after) {
            Some(end) => {
                footnotes.push(after[..end].to_string());
                let id = footnotes.len(); // 1-based; -1/0 reserved for separators
                out.push_str(&format!(
                    "<w:r><w:rPr><w:rStyle w:val=\"FootnoteReference\"/>\
<w:vertAlign w:val=\"superscript\"/></w:rPr><w:footnoteReference w:id=\"{id}\"/></w:r>"
                ));
                rest = &after[end + 1..];
            }
            None => {
                // Unterminated marker — emit it literally, no note.
                out.push_str(&runs(FN_OPEN));
                rest = after;
            }
        }
    }
    out.push_str(&runs(rest));
    out
}

/// The `word/footnotes.xml` part: the two mandatory separator notes (ids -1/0)
/// plus one `<w:footnote>` per collected body (ids 1..). The reference glyph is a
/// superscript `<w:footnoteRef/>`; the body renders emphasis like ordinary prose.
fn footnotes_xml(bodies: &[String]) -> String {
    let mut b = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
<w:footnotes xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\
<w:footnote w:type=\"separator\" w:id=\"-1\"><w:p><w:r><w:separator/></w:r></w:p></w:footnote>\
<w:footnote w:type=\"continuationSeparator\" w:id=\"0\"><w:p><w:r><w:continuationSeparator/></w:r></w:p></w:footnote>",
    );
    for (i, body) in bodies.iter().enumerate() {
        let id = i + 1;
        b.push_str(&format!(
            "<w:footnote w:id=\"{id}\"><w:p><w:pPr><w:pStyle w:val=\"FootnoteText\"/></w:pPr>\
<w:r><w:rPr><w:rStyle w:val=\"FootnoteReference\"/><w:vertAlign w:val=\"superscript\"/></w:rPr>\
<w:footnoteRef/></w:r>{run}</w:p></w:footnote>",
            run = runs(&format!(" {body}")),
        ));
    }
    b.push_str("</w:footnotes>\n");
    b
}

/// XP-2 — split `text` into `<w:r>` runs on authored `*bold*` / `_italic_`
/// emphasis (shared tokenizer), so fiction italics render in Word instead of
/// leaking as literal `*bold*`. Plain text with no delimiters yields one run,
/// identical to the old output.
fn runs(text: &str) -> String {
    use crate::manuscript::Emphasis;
    let mut out = String::with_capacity(text.len() + 16);
    for span in crate::manuscript::parse_emphasis(text) {
        let rpr = match span.emphasis {
            Emphasis::Bold => "<w:rPr><w:b/></w:rPr>",
            Emphasis::Italic => "<w:rPr><w:i/></w:rPr>",
            Emphasis::None => "",
        };
        out.push_str(&format!(
            "<w:r>{rpr}<w:t xml:space=\"preserve\">{}</w:t></w:r>",
            xml_escape(&span.text)
        ));
    }
    out
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
<w:style w:type=\"paragraph\" w:styleId=\"FootnoteText\"><w:name w:val=\"footnote text\"/>\
<w:pPr><w:spacing w:line=\"240\" w:lineRule=\"auto\"/></w:pPr><w:rPr><w:sz w:val=\"20\"/><w:szCs w:val=\"20\"/></w:rPr></w:style>\
<w:style w:type=\"character\" w:styleId=\"FootnoteReference\"><w:name w:val=\"footnote reference\"/>\
<w:rPr><w:vertAlign w:val=\"superscript\"/></w:rPr></w:style>\
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
<Override PartName=\"/word/footnotes.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml\"/>\
</Types>\n";

const ROOT_RELS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/>\
</Relationships>\n";

const DOC_RELS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rIdStyles\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/>\
<Relationship Id=\"rIdHeader\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/header\" Target=\"header2.xml\"/>\
<Relationship Id=\"rIdFootnotes\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes\" Target=\"footnotes.xml\"/>\
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
            // XML 1.0 forbids C0 control chars (even escaped) except tab/LF/CR;
            // drop them so a stray control char can't make word/document.xml
            // not-well-formed ("unreadable content").
            '\u{0}'..='\u{8}' | '\u{B}' | '\u{C}' | '\u{E}'..='\u{1F}' => {}
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn xml_escape_drops_illegal_control_chars() {
        // A form-feed / NUL must not reach word/document.xml (would be
        // not-well-formed → "unreadable content"). Tab stays (legal).
        assert_eq!(xml_escape("a\u{0}b\u{c}c\td"), "abc\td");
        assert_eq!(xml_escape("<&>"), "&lt;&amp;&gt;");
    }

    #[test]
    fn body_emphasis_becomes_word_runs_not_literal_markers() {
        // XP-2 — *bold* / _italic_ emit <w:b>/<w:i> runs in body prose.
        let mut fns = Vec::new();
        let p = body_para("she was *very* _tired_ now", &[Prop::FirstLineIndent], &mut fns);
        assert!(p.contains("<w:b/>"), "bold run: {p}");
        assert!(p.contains("<w:i/>"), "italic run: {p}");
        assert!(p.contains("<w:t xml:space=\"preserve\">very</w:t>"), "bold text: {p}");
        assert!(!p.contains('*') && !p.contains('_'), "no literal delimiters: {p}");
        // Plain text with no emphasis is a single unchanged run.
        let plain = body_para("just plain prose", &[], &mut fns);
        assert_eq!(plain.matches("<w:r>").count(), 1, "one run: {plain}");
    }

    #[test]
    fn title_page_fields_are_plain_not_emphasis_parsed() {
        // A6 — a contact email / byline with `_`/`*` must stay literal (the
        // title-page path is plain, matching build_typst's escape_typst), not
        // get italicised with the delimiters eaten.
        let meta = ManuscriptMeta {
            title: "T".into(),
            contact: "jane_q_writer@example.com".into(),
            byline: "Jane_Q".into(),
            surname: "X".into(),
            word_count: 100,
        };
        let chapters = vec![ManuscriptChapter {
            title: "One".into(),
            paragraphs: vec!["plain body".into()],
        }];
        let doc = part(&build_docx(&meta, &chapters, DocxFont::TimesNewRoman).unwrap(), "word/document.xml");
        // Underscores survive literally in the contact + byline.
        assert!(doc.contains("jane_q_writer@example.com"), "contact underscores: {doc}");
        assert!(doc.contains("Jane_Q"), "byline underscore: {doc}");
        // The body is plain here, so nothing in the whole document is italic.
        assert!(!doc.contains("<w:i/>"), "no stray italic run: {doc}");
    }

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
            "word/footnotes.xml",
        ] {
            let _ = part(&bytes, p); // panics if absent
        }
    }

    #[test]
    fn footnotes_become_real_word_notes_with_matching_ids() {
        // A2 — #footnote[…] in body prose → a <w:footnoteReference> in the body
        // plus a <w:footnote> in footnotes.xml, ids agreeing (1-based).
        let meta = ManuscriptMeta {
            title: "T".into(), contact: "X".into(), byline: "X".into(),
            surname: "X".into(), word_count: 100,
        };
        let chapters = vec![ManuscriptChapter {
            title: "One".into(),
            paragraphs: vec![
                "A claim#footnote[first note] stands.".into(),
                "Another#footnote[second note].".into(),
            ],
        }];
        let bytes = build_docx(&meta, &chapters, DocxFont::TimesNewRoman).unwrap();
        let doc = part(&bytes, "word/document.xml");
        let fns = part(&bytes, "word/footnotes.xml");
        // Two references in the body, ids 1 and 2.
        assert!(doc.contains("<w:footnoteReference w:id=\"1\"/>"), "{doc}");
        assert!(doc.contains("<w:footnoteReference w:id=\"2\"/>"), "{doc}");
        // The marker text itself is gone from the body.
        assert!(!doc.contains("#footnote["), "marker consumed: {doc}");
        // Both bodies live in footnotes.xml under matching ids + the separators.
        assert!(fns.contains("w:type=\"separator\" w:id=\"-1\""), "{fns}");
        assert!(fns.contains("<w:footnote w:id=\"1\">") && fns.contains("first note"), "{fns}");
        assert!(fns.contains("<w:footnote w:id=\"2\">") && fns.contains("second note"), "{fns}");
        assert!(quick_xml_well_formed(&fns), "footnotes.xml well-formed");
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
