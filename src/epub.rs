//! 1.2.18+ R.1 — EPUB 3 export.
//!
//! Turns a user book into a standards-compliant `.epub`
//! — the format readers actually consume — alongside the
//! existing typst → PDF assembly path.
//!
//! ## Why not pandoc?
//!
//! The 1.2.18 plan floated shelling out to `pandoc` for
//! the prose → HTML step.  But pandoc can't read typst,
//! and requiring it would add a heavy soft-dependency.
//! Inkhaven already has the prose as typst markup + the
//! `zip` crate in-tree, so R.1 builds the EPUB container
//! directly and does a lightweight in-house typst →
//! XHTML conversion for the common subset (headings,
//! emphasis, strong, footnotes).  Zero new dependencies.
//!
//! ## Structure produced
//!
//! ```text
//! mimetype                    (stored, first entry — EPUB rule)
//! META-INF/container.xml
//! OEBPS/content.opf           (package: metadata + manifest + spine)
//! OEBPS/nav.xhtml             (EPUB3 navigation)
//! OEBPS/toc.ncx               (EPUB2 back-compat)
//! OEBPS/style.css
//! OEBPS/chapter-001.xhtml
//! OEBPS/chapter-002.xhtml
//! ...
//! ```
//!
//! ## Conversion fidelity (R.1)
//!
//! The typst → XHTML pass handles the markup inkhaven
//! prose actually uses:
//!
//!   * `= …` / `== …` / `=== …` headings → `<h1/2/3>`
//!   * `_emphasis_` → `<em>`
//!   * `*strong*` → `<strong>`
//!   * `#footnote[…]` → inline `<span class="footnote">`
//!     (a documented R.1 limitation; proper popup
//!     endnotes are an R.1.b polish)
//!   * blank-line-separated blocks → `<p>`
//!
//! Each paragraph node's leading `= title` heading is
//! treated as organisational scaffolding and stripped —
//! the reader sees flowing chapter prose, not "001.
//! Approach" scene labels.  (A `--paragraph-headings`
//! opt-in can surface them later.)

use std::io::Write;
use std::path::Path;

use anyhow::Result;
use zip::write::FileOptions;
use zip::{CompressionMethod, ZipWriter};

/// Book-level metadata for the EPUB package document.
#[derive(Debug, Clone)]
pub struct EpubMeta {
    pub title: String,
    pub author: String,
    /// BCP-47 language tag (`en`, `ru`, `fr`).
    pub language: String,
    /// Unique identifier (a UUID urn).  Stable per
    /// export so re-exports replace cleanly in a
    /// reader's library.
    pub identifier: String,
    /// 1.2.20+ R.1.b — optional cover image.  When
    /// present it becomes the reader's library
    /// thumbnail (`properties="cover-image"`) and the
    /// first spine page.  `None` keeps the R.1
    /// text-only output byte-for-byte.
    pub cover: Option<EpubCover>,
}

/// 1.2.20+ R.1.b — a cover image to embed in the EPUB.
#[derive(Debug, Clone)]
pub struct EpubCover {
    /// Raw image bytes, written verbatim into the
    /// archive (stored, not re-deflated — JPEG/PNG are
    /// already compressed).
    pub bytes: Vec<u8>,
    /// MIME type for the OPF manifest, e.g.
    /// `image/jpeg` or `image/png`.
    pub media_type: String,
    /// Extension used for the in-archive filename
    /// (`cover.jpg` / `cover.png`).
    pub file_ext: String,
}

/// One chapter: a heading + pre-converted XHTML body
/// (the inner content of `<body>`, already escaped +
/// marked up), plus any inline images the body's `<img>`
/// tags reference (written into `OEBPS/` + the manifest).
#[derive(Debug, Clone)]
pub struct EpubChapter {
    pub title: String,
    pub body_xhtml: String,
    /// Inline figures referenced by `<img src="…">` in
    /// `body_xhtml`. Empty for image-free chapters.
    pub images: Vec<EpubImage>,
}

/// One inline image resource — raw bytes written verbatim
/// into `OEBPS/<href>` (stored, not re-deflated) plus the
/// metadata the OPF manifest needs. `id` is the manifest
/// item id (a valid XML NCName); `href` is the in-archive
/// filename `<img src>` points at.
#[derive(Debug, Clone)]
pub struct EpubImage {
    pub id: String,
    pub href: String,
    pub media_type: String,
    pub bytes: Vec<u8>,
}

/// Map an image file extension to its IANA media type for
/// the OPF manifest. Mirrors `image_extension_for`'s
/// accepted set; unknown extensions fall back to a generic
/// binary type (still ships, just without a precise hint).
pub fn image_media_type(ext: &str) -> &'static str {
    match ext.to_ascii_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

/// Result summary for stdout reporting.
#[derive(Debug, Clone)]
pub struct EpubReport {
    pub chapters: usize,
    pub bytes: u64,
}

/// Write the assembled EPUB to `dest`.
pub fn write_epub(
    meta: &EpubMeta,
    chapters: &[EpubChapter],
    dest: &Path,
) -> Result<EpubReport> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = build_epub_bytes(meta, chapters)?;
    // M5 (3.0.0 P2) — write to a sibling `.part` and rename on success, so a
    // failed write (disk full, an interrupted write) never truncates a
    // previously-good .epub already at `dest`. Mirrors `export::bundle::write_zip`.
    let mut tmp_os = dest.as_os_str().to_owned();
    tmp_os.push(".part");
    let tmp = std::path::PathBuf::from(tmp_os);
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, dest)?;
    Ok(EpubReport { chapters: chapters.len(), bytes: bytes.len() as u64 })
}

/// I-3 — build the EPUB3 container entirely in memory. Shared by [`write_epub`]
/// (→ disk) and the Bund `ink.export.epub` / TUI batch exporters, which need the
/// bytes as an in-memory artefact. Deterministic given the same inputs.
pub fn build_epub_bytes(meta: &EpubMeta, chapters: &[EpubChapter]) -> Result<Vec<u8>> {
    let mut zip = ZipWriter::new(std::io::Cursor::new(Vec::<u8>::new()));

    // ── mimetype — MUST be first + stored (uncompressed)
    //    per the EPUB OCF spec.  Readers sniff the first
    //    30 bytes; a deflated mimetype fails validation.
    let stored: FileOptions<()> =
        FileOptions::default().compression_method(CompressionMethod::Stored);
    zip.start_file("mimetype", stored)?;
    zip.write_all(b"application/epub+zip")?;

    let deflated: FileOptions<()> =
        FileOptions::default().compression_method(CompressionMethod::Deflated);

    zip.start_file("META-INF/container.xml", deflated)?;
    zip.write_all(CONTAINER_XML.as_bytes())?;

    zip.start_file("OEBPS/style.css", deflated)?;
    zip.write_all(STYLE_CSS.as_bytes())?;

    // ── cover (R.1.b) — the image is `Stored` (already
    //    a compressed format), the wrapper page deflated.
    if let Some(cover) = &meta.cover {
        zip.start_file(format!("OEBPS/cover.{}", cover.file_ext), stored)?;
        zip.write_all(&cover.bytes)?;
        zip.start_file("OEBPS/cover.xhtml", deflated)?;
        zip.write_all(cover_xhtml(&cover.file_ext, &meta.language).as_bytes())?;
    }

    // Chapter documents + their inline images. Image
    // bytes are `stored` (PNG/JPEG/WebP are already
    // compressed); SVG would gain from deflate but the
    // simpler single mode keeps the writer uniform.
    for (i, ch) in chapters.iter().enumerate() {
        let name = chapter_filename(i);
        zip.start_file(format!("OEBPS/{name}"), deflated)?;
        zip.write_all(chapter_xhtml(&ch.title, &ch.body_xhtml, &meta.language).as_bytes())?;
        for img in &ch.images {
            zip.start_file(format!("OEBPS/{}", img.href), stored)?;
            zip.write_all(&img.bytes)?;
        }
    }

    // Navigation + package + ncx.
    zip.start_file("OEBPS/nav.xhtml", deflated)?;
    zip.write_all(nav_xhtml(chapters, &meta.language).as_bytes())?;

    zip.start_file("OEBPS/toc.ncx", deflated)?;
    zip.write_all(toc_ncx(meta, chapters).as_bytes())?;

    zip.start_file("OEBPS/content.opf", deflated)?;
    zip.write_all(content_opf(meta, chapters).as_bytes())?;

    let cursor = zip.finish()?;
    Ok(cursor.into_inner())
}

/// `chapter-001.xhtml`, `chapter-002.xhtml`, … (1-based).
pub fn chapter_filename(index0: usize) -> String {
    format!("chapter-{:03}.xhtml", index0 + 1)
}

// ── typst → XHTML ────────────────────────────────────

/// Convert a paragraph's typst body to escaped XHTML
/// (the inner content of `<body>`).  Pure.  See the
/// module-level fidelity note for the supported subset.
pub fn typst_to_xhtml(body: &str) -> String {
    let stripped = crate::typst_prose::strip_leading_heading(body);
    let blocks = crate::typst_prose::split_blocks(&stripped);
    let mut out = String::new();
    for block in blocks {
        let trimmed = block.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Heading levels (== / ===) inside a body.
        if let Some(rest) = trimmed.strip_prefix("=== ") {
            out.push_str(&format!("<h3>{}</h3>\n", inline(rest)));
        } else if let Some(rest) = trimmed.strip_prefix("== ") {
            out.push_str(&format!("<h2>{}</h2>\n", inline(rest)));
        } else if let Some(rest) = trimmed.strip_prefix("= ") {
            out.push_str(&format!("<h2>{}</h2>\n", inline(rest)));
        } else {
            // Collapse intra-block newlines into spaces
            // (typst treats a single newline as a space).
            let joined = trimmed
                .split('\n')
                .map(str::trim)
                .collect::<Vec<_>>()
                .join(" ");
            out.push_str(&format!("<p>{}</p>\n", inline(&joined)));
        }
    }
    // 1.3.13 — promote inline footnote spans to EPUB 3 popup footnotes:
    // numbered `noteref` links + a collected `<aside>` footnotes section
    // (Apple Books & co. render these as tap-to-pop popups; other readers
    // show the section at the chapter end).
    footnotes_to_asides(&out)
}

/// Convert the `<span class="footnote">[…]</span>` placeholders that `inline`
/// emits into EPUB 3 popup footnotes: each becomes a numbered `noteref`
/// anchor, and the notes collect into a `<section epub:type="footnotes">` at
/// the chapter's end. IDs are per-chapter (one XHTML file each). Pure.
fn footnotes_to_asides(body: &str) -> String {
    let open = "<span class=\"footnote\">[";
    let close = "]</span>";
    let mut out = String::new();
    let mut notes = String::new();
    let mut rest = body;
    let mut n = 0usize;
    while let Some(pos) = rest.find(open) {
        out.push_str(&rest[..pos]);
        let after = &rest[pos + open.len()..];
        let Some(end) = after.find(close) else {
            out.push_str(&rest[pos..]); // malformed — leave the tail verbatim
            return out;
        };
        n += 1;
        let inner = &after[..end];
        out.push_str(&format!(
            "<a epub:type=\"noteref\" role=\"doc-noteref\" id=\"fnref-{n}\" href=\"#fn-{n}\"><sup>{n}</sup></a>"
        ));
        notes.push_str(&format!(
            "<aside epub:type=\"footnote\" role=\"doc-footnote\" id=\"fn-{n}\"><p><sup>{n}</sup> {inner} <a href=\"#fnref-{n}\" role=\"doc-backlink\">\u{21a9}</a></p></aside>\n"
        ));
        rest = &after[end + close.len()..];
    }
    out.push_str(rest);
    if !notes.is_empty() {
        out.push_str("<section epub:type=\"footnotes\" role=\"doc-endnotes\" class=\"footnotes\">\n");
        out.push_str(&notes);
        out.push_str("</section>\n");
    }
    out
}

/// Drop a single leading `= heading` line (the
/// paragraph's organisational title).  Leaves `==` /
/// `===` subheadings intact.
/// Inline markup conversion on a single block.  Escapes
/// XML first, then applies `_emph_`, `*strong*`,
/// `#footnote[…]` over the escaped text (the markup
/// delimiters aren't escape targets, so order is safe).
fn inline(text: &str) -> String {
    let escaped = escape_xml(text);
    let with_footnotes = convert_footnotes(&escaped);
    let with_strong = convert_delim(&with_footnotes, '*', "strong");
    convert_delim(&with_strong, '_', "em")
}

/// Replace `#footnote[…]` with an inline footnote span.
/// Non-nested (R.1 limitation); the first `]` closes.
fn convert_footnotes(s: &str) -> String {
    let needle = "#footnote[";
    let mut out = String::new();
    let mut rest = s;
    while let Some(pos) = rest.find(needle) {
        out.push_str(&rest[..pos]);
        let after = &rest[pos + needle.len()..];
        if let Some(end) = after.find(']') {
            let inner = &after[..end];
            out.push_str(&format!(
                "<span class=\"footnote\">[{inner}]</span>"
            ));
            rest = &after[end + 1..];
        } else {
            // Unterminated — emit literally + stop.
            out.push_str(&rest[pos..]);
            return out;
        }
    }
    out.push_str(rest);
    out
}

/// Convert paired single-char delimiters (`*x*`, `_x_`)
/// into `<tag>x</tag>`.  Pairs are matched greedily on
/// the same logical line; an unpaired delimiter passes
/// through literally.
fn convert_delim(s: &str, delim: char, tag: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == delim {
            // Find the closing delimiter.
            if let Some(close) = (i + 1..chars.len()).find(|&j| chars[j] == delim) {
                let inner: String = chars[i + 1..close].iter().collect();
                if !inner.is_empty() {
                    out.push_str(&format!("<{tag}>{inner}</{tag}>"));
                    i = close + 1;
                    continue;
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Escape the five XML special characters.
pub fn escape_xml(s: &str) -> String {
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

// ── document templates ───────────────────────────────

const CONTAINER_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>
"#;

const STYLE_CSS: &str = r#"body { font-family: serif; line-height: 1.5; margin: 1em; }
h1, h2, h3 { font-family: sans-serif; line-height: 1.2; }
p { margin: 0 0 0.8em 0; text-indent: 1.5em; }
p:first-of-type { text-indent: 0; }
.footnote { font-size: 0.85em; color: #555; }
a[role~="doc-noteref"] { text-decoration: none; }
.footnotes { margin-top: 2em; border-top: 1px solid #ccc; padding-top: 0.5em; font-size: 0.85em; color: #444; }
.footnotes aside { margin: 0.4em 0; }
.footnotes p { text-indent: 0; }
"#;

fn chapter_xhtml(title: &str, body: &str, lang: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" xml:lang="{lang}" lang="{lang}">
<head>
  <meta charset="UTF-8"/>
  <title>{title}</title>
  <link rel="stylesheet" type="text/css" href="style.css"/>
</head>
<body>
  <section epub:type="chapter">
    <h1>{title}</h1>
{body}  </section>
</body>
</html>
"#,
        title = escape_xml(title),
        body = body,
        lang = escape_xml(lang),
    )
}

/// 1.2.20+ R.1.b — full-page cover wrapper.  A reader
/// that honours `epub:type="cover"` shows this as the
/// opening page; the image scales to the viewport.
fn cover_xhtml(file_ext: &str, lang: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" xml:lang="{lang}" lang="{lang}">
<head>
  <meta charset="UTF-8"/>
  <title>Cover</title>
  <style>html, body {{ margin: 0; padding: 0; }} img {{ max-width: 100%; height: auto; display: block; margin: 0 auto; }}</style>
</head>
<body>
  <section epub:type="cover">
    <img src="cover.{ext}" alt="Cover"/>
  </section>
</body>
</html>
"#,
        ext = file_ext,
        lang = escape_xml(lang),
    )
}

fn nav_xhtml(chapters: &[EpubChapter], lang: &str) -> String {
    let mut items = String::new();
    for (i, ch) in chapters.iter().enumerate() {
        items.push_str(&format!(
            "      <li><a href=\"{}\">{}</a></li>\n",
            chapter_filename(i),
            escape_xml(&ch.title),
        ));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" xml:lang="{lang}" lang="{lang}">
<head>
  <meta charset="UTF-8"/>
  <title>Contents</title>
</head>
<body>
  <nav epub:type="toc" id="toc">
    <h1>Contents</h1>
    <ol>
{items}    </ol>
  </nav>
</body>
</html>
"#,
        items = items,
        lang = escape_xml(lang),
    )
}

fn toc_ncx(meta: &EpubMeta, chapters: &[EpubChapter]) -> String {
    let mut points = String::new();
    for (i, ch) in chapters.iter().enumerate() {
        points.push_str(&format!(
            r#"    <navPoint id="navpoint-{n}" playOrder="{n}">
      <navLabel><text>{title}</text></navLabel>
      <content src="{file}"/>
    </navPoint>
"#,
            n = i + 1,
            title = escape_xml(&ch.title),
            file = chapter_filename(i),
        ));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1">
  <head>
    <meta name="dtb:uid" content="{id}"/>
  </head>
  <docTitle><text>{title}</text></docTitle>
  <navMap>
{points}  </navMap>
</ncx>
"#,
        id = escape_xml(&meta.identifier),
        title = escape_xml(&meta.title),
        points = points,
    )
}

fn content_opf(meta: &EpubMeta, chapters: &[EpubChapter]) -> String {
    // ── cover (R.1.b) — EPUB3 `cover-image` property
    //    drives the library thumbnail; the EPUB2
    //    `<meta name="cover">` keeps older readers happy;
    //    the wrapper page leads the spine.
    let (cover_meta, cover_manifest, cover_spine) = match &meta.cover {
        Some(c) => (
            "    <meta name=\"cover\" content=\"cover-image\"/>\n".to_string(),
            format!(
                "    <item id=\"cover-image\" href=\"cover.{ext}\" media-type=\"{mt}\" properties=\"cover-image\"/>\n\
                 \x20   <item id=\"cover\" href=\"cover.xhtml\" media-type=\"application/xhtml+xml\"/>\n",
                ext = c.file_ext,
                mt = escape_xml(&c.media_type),
            ),
            "    <itemref idref=\"cover\" linear=\"yes\"/>\n".to_string(),
        ),
        None => (String::new(), String::new(), String::new()),
    };

    let mut manifest = String::new();
    let mut spine = String::new();
    for (i, ch) in chapters.iter().enumerate() {
        let id = format!("ch{:03}", i + 1);
        manifest.push_str(&format!(
            "    <item id=\"{id}\" href=\"{file}\" media-type=\"application/xhtml+xml\"/>\n",
            id = id,
            file = chapter_filename(i),
        ));
        // Inline-image resources (R-images): one manifest
        // entry per `<img>` the chapter body references.
        for img in &ch.images {
            manifest.push_str(&format!(
                "    <item id=\"{id}\" href=\"{href}\" media-type=\"{mt}\"/>\n",
                id = escape_xml(&img.id),
                href = escape_xml(&img.href),
                mt = escape_xml(&img.media_type),
            ));
        }
        spine.push_str(&format!("    <itemref idref=\"{id}\"/>\n", id = id));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="pub-id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="pub-id">{id}</dc:identifier>
    <dc:title>{title}</dc:title>
    <dc:creator>{author}</dc:creator>
    <dc:language>{lang}</dc:language>
{cover_meta}    <meta property="dcterms:modified">2026-01-01T00:00:00Z</meta>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>
    <item id="css" href="style.css" media-type="text/css"/>
{cover_manifest}{manifest}  </manifest>
  <spine toc="ncx">
{cover_spine}{spine}  </spine>
</package>
"#,
        id = escape_xml(&meta.identifier),
        title = escape_xml(&meta.title),
        author = escape_xml(&meta.author),
        lang = escape_xml(&meta.language),
        cover_meta = cover_meta,
        cover_manifest = cover_manifest,
        cover_spine = cover_spine,
        manifest = manifest,
        spine = spine,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── escape_xml ────────────────────────────────────

    #[test]
    fn escape_handles_all_five() {
        assert_eq!(
            escape_xml(r#"a&b<c>d"e'f"#),
            "a&amp;b&lt;c&gt;d&quot;e&apos;f",
        );
    }

    #[test]
    fn escape_passes_unicode() {
        assert_eq!(escape_xml("Русский — 中文"), "Русский — 中文");
    }

    // ── inline markup ─────────────────────────────────

    #[test]
    fn inline_converts_emphasis() {
        assert_eq!(inline("a _word_ b"), "a <em>word</em> b");
    }

    #[test]
    fn inline_converts_strong() {
        assert_eq!(inline("a *word* b"), "a <strong>word</strong> b");
    }

    #[test]
    fn inline_unpaired_delim_passes_through() {
        // A lone underscore (e.g. a filename) must not
        // open an unterminated <em>.
        assert_eq!(inline("file_name only"), "file_name only");
    }

    #[test]
    fn inline_escapes_then_marks_up() {
        // The `<` must be escaped, the `_` must convert.
        assert_eq!(
            inline("x < y and _z_"),
            "x &lt; y and <em>z</em>",
        );
    }

    #[test]
    fn inline_converts_footnote() {
        assert_eq!(
            inline("text#footnote[a note]more"),
            "text<span class=\"footnote\">[a note]</span>more",
        );
    }

    #[test]
    fn footnotes_become_epub3_noterefs_and_asides() {
        let body = "A claim.#footnote[the source] More.#footnote[second]";
        let xhtml = typst_to_xhtml(body);
        // two numbered noterefs in the flow
        assert!(xhtml.contains("epub:type=\"noteref\""));
        assert!(xhtml.contains("href=\"#fn-1\"") && xhtml.contains("href=\"#fn-2\""));
        // a collected footnotes section with asides + backlinks
        assert!(xhtml.contains("epub:type=\"footnotes\""));
        assert!(xhtml.contains("epub:type=\"footnote\"") && xhtml.contains("id=\"fn-2\""));
        assert!(xhtml.contains("the source") && xhtml.contains("href=\"#fnref-1\""));
    }

    #[test]
    fn no_footnotes_means_no_footnotes_section() {
        assert!(!typst_to_xhtml("plain prose").contains("footnotes"));
    }

    #[test]
    fn inline_unterminated_footnote_is_literal() {
        let got = inline("text#footnote[oops");
        assert!(got.contains("#footnote[oops"));
    }

    // ── typst_to_xhtml ────────────────────────────────

    #[test]
    fn xhtml_wraps_paragraphs() {
        let body = "= Title\n\nFirst para.\n\nSecond para.";
        let got = typst_to_xhtml(body);
        assert!(got.contains("<p>First para.</p>"));
        assert!(got.contains("<p>Second para.</p>"));
        assert!(!got.contains("Title"), "leading heading should be stripped");
    }

    #[test]
    fn xhtml_converts_subheadings() {
        let body = "Lead.\n\n== A scene\n\nMore.";
        let got = typst_to_xhtml(body);
        assert!(got.contains("<h2>A scene</h2>"));
        assert!(got.contains("<p>Lead.</p>"));
        assert!(got.contains("<p>More.</p>"));
    }

    #[test]
    fn xhtml_collapses_intra_block_newlines() {
        let body = "Line one\nline two";
        let got = typst_to_xhtml(body);
        assert!(got.contains("<p>Line one line two</p>"));
    }

    #[test]
    fn xhtml_empty_body_is_empty() {
        assert_eq!(typst_to_xhtml("= Title\n\n"), "");
    }

    // ── chapter_filename ──────────────────────────────

    #[test]
    fn chapter_filenames_are_zero_padded_one_based() {
        assert_eq!(chapter_filename(0), "chapter-001.xhtml");
        assert_eq!(chapter_filename(9), "chapter-010.xhtml");
    }

    // ── write_epub (real zip) ─────────────────────────

    fn sample_meta() -> EpubMeta {
        EpubMeta {
            title: "The Harbor Code".into(),
            author: "A. Writer".into(),
            language: "en".into(),
            identifier: "urn:uuid:test-1234".into(),
            cover: None,
        }
    }

    fn sample_chapters() -> Vec<EpubChapter> {
        vec![
            EpubChapter {
                title: "Arrivals".into(),
                body_xhtml: "<p>Helena paused.</p>\n".into(),
                images: Vec::new(),
            },
            EpubChapter {
                title: "The Wharf".into(),
                body_xhtml: "<p>Marcus waited.</p>\n".into(),
                images: Vec::new(),
            },
        ]
    }

    #[test]
    fn inline_images_are_written_and_manifested() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("book.epub");
        let chapters = vec![EpubChapter {
            title: "Plates".into(),
            body_xhtml: "<figure><img src=\"img-x.png\" alt=\"a map\"/></figure>\n".into(),
            images: vec![EpubImage {
                id: "img-x".into(),
                href: "img-x.png".into(),
                media_type: "image/png".into(),
                // minimal PNG signature — bytes are written verbatim.
                bytes: vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a],
            }],
        }];
        write_epub(&sample_meta(), &chapters, &dest).unwrap();

        let file = std::fs::File::open(&dest).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let names: Vec<String> = archive.file_names().map(String::from).collect();
        assert!(names.iter().any(|n| n == "OEBPS/img-x.png"), "image bytes not in archive");

        // image is `stored` (already-compressed bytes).
        let img = archive.by_name("OEBPS/img-x.png").unwrap();
        assert_eq!(img.compression(), zip::CompressionMethod::Stored);
        drop(img);

        let mut opf = archive.by_name("OEBPS/content.opf").unwrap();
        let mut s = String::new();
        std::io::Read::read_to_string(&mut opf, &mut s).unwrap();
        assert!(
            s.contains("href=\"img-x.png\"") && s.contains("media-type=\"image/png\""),
            "image not in OPF manifest:\n{s}",
        );
    }

    #[test]
    fn write_epub_produces_valid_container() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("book.epub");
        let report =
            write_epub(&sample_meta(), &sample_chapters(), &dest).unwrap();
        assert_eq!(report.chapters, 2);
        assert!(report.bytes > 0);
        assert!(dest.exists());

        // Re-open the zip + assert the EPUB invariants.
        let file = std::fs::File::open(&dest).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();

        // mimetype must be the FIRST entry + stored.
        let first = archive.by_index(0).unwrap();
        assert_eq!(first.name(), "mimetype");
        assert_eq!(first.compression(), zip::CompressionMethod::Stored);
        drop(first);

        // Required members present.
        let names: Vec<String> =
            archive.file_names().map(String::from).collect();
        for required in [
            "mimetype",
            "META-INF/container.xml",
            "OEBPS/content.opf",
            "OEBPS/nav.xhtml",
            "OEBPS/toc.ncx",
            "OEBPS/chapter-001.xhtml",
            "OEBPS/chapter-002.xhtml",
        ] {
            assert!(
                names.iter().any(|n| n == required),
                "missing EPUB member: {required}",
            );
        }
    }

    #[test]
    fn write_epub_mimetype_content_is_exact() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("b.epub");
        write_epub(&sample_meta(), &sample_chapters(), &dest).unwrap();
        let file = std::fs::File::open(&dest).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut mt = archive.by_name("mimetype").unwrap();
        use std::io::Read;
        let mut s = String::new();
        mt.read_to_string(&mut s).unwrap();
        assert_eq!(s, "application/epub+zip");
    }

    #[test]
    fn content_opf_lists_every_chapter_in_spine() {
        let opf = content_opf(&sample_meta(), &sample_chapters());
        assert!(opf.contains("<dc:title>The Harbor Code</dc:title>"));
        assert!(opf.contains("<dc:creator>A. Writer</dc:creator>"));
        assert!(opf.contains("<dc:language>en</dc:language>"));
        assert!(opf.contains("idref=\"ch001\""));
        assert!(opf.contains("idref=\"ch002\""));
    }

    #[test]
    fn nav_lists_every_chapter() {
        let nav = nav_xhtml(&sample_chapters(), "en");
        assert!(nav.contains("chapter-001.xhtml"));
        assert!(nav.contains("Arrivals"));
        assert!(nav.contains("The Wharf"));
    }

    #[test]
    fn metadata_with_xml_specials_is_escaped() {
        let meta = EpubMeta {
            title: "Tom & Jerry <draft>".into(),
            author: "A \"Quoted\" Name".into(),
            language: "en".into(),
            identifier: "urn:uuid:x".into(),
            cover: None,
        };
        let opf = content_opf(&meta, &sample_chapters());
        assert!(opf.contains("Tom &amp; Jerry &lt;draft&gt;"));
        assert!(opf.contains("A &quot;Quoted&quot; Name"));
    }

    // ── cover (R.1.b) ─────────────────────────────────

    fn meta_with_cover() -> EpubMeta {
        EpubMeta {
            cover: Some(EpubCover {
                bytes: vec![0x89, 0x50, 0x4e, 0x47, 1, 2, 3, 4],
                media_type: "image/png".into(),
                file_ext: "png".into(),
            }),
            ..sample_meta()
        }
    }

    #[test]
    fn no_cover_keeps_opf_cover_free() {
        // The default (no cover) path must not emit any
        // cover manifest / spine / meta entries.
        let opf = content_opf(&sample_meta(), &sample_chapters());
        assert!(!opf.contains("cover-image"));
        assert!(!opf.contains("name=\"cover\""));
        assert!(!opf.contains("cover.xhtml"));
    }

    #[test]
    fn cover_opf_declares_image_and_leads_spine() {
        let opf = content_opf(&meta_with_cover(), &sample_chapters());
        // EPUB3 cover-image property + media type.
        assert!(opf.contains(
            "<item id=\"cover-image\" href=\"cover.png\" media-type=\"image/png\" properties=\"cover-image\"/>"
        ));
        // EPUB2 back-compat meta.
        assert!(opf.contains("<meta name=\"cover\" content=\"cover-image\"/>"));
        // The cover page leads the spine, ahead of ch001.
        let cover_at = opf.find("idref=\"cover\"").expect("cover in spine");
        let ch1_at = opf.find("idref=\"ch001\"").expect("ch001 in spine");
        assert!(cover_at < ch1_at, "cover must precede the first chapter");
    }

    #[test]
    fn cover_image_and_page_land_in_archive() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("with-cover.epub");
        write_epub(&meta_with_cover(), &sample_chapters(), &dest).unwrap();

        let file = std::fs::File::open(&dest).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let names: Vec<String> = archive.file_names().map(String::from).collect();
        assert!(names.iter().any(|n| n == "OEBPS/cover.png"), "cover image missing");
        assert!(names.iter().any(|n| n == "OEBPS/cover.xhtml"), "cover page missing");

        // The image bytes are stored verbatim (not deflated).
        let entry = archive.by_name("OEBPS/cover.png").unwrap();
        assert_eq!(entry.compression(), zip::CompressionMethod::Stored);
        use std::io::Read;
        let mut got = Vec::new();
        let mut e = entry;
        e.read_to_end(&mut got).unwrap();
        assert_eq!(got, vec![0x89, 0x50, 0x4e, 0x47, 1, 2, 3, 4]);
    }
}
