//! 1.3.0 PDF-1 — PDF management & imposition subsystem.
//!
//! [RFC PDF-1](../../Documentation/PROPOSALS/PDF-1.md) +
//! [implementation plan](../../Documentation/PROPOSALS/PDF-1_PLAN.md):
//! imposition, page operations, cover/barcode, preflight, outline
//! injection — all pure-Rust (`lopdf`), single-binary, primarily over
//! inkhaven-authored (typst-pdf) output.  Built in phases P0→P3.
//!
//! ## P0 fidelity gate
//!
//! Before any `src/pdf/` code builds on top of `lopdf`, the corpus test
//! below proves `lopdf` faithfully parses *and* round-trips the exact
//! PDFs inkhaven produces — embedded font subsets, image XObjects,
//! vector content, multiple pages.  This is RFC §14's make-or-break
//! risk: if typst-pdf output doesn't load cleanly in `lopdf`, the whole
//! subsystem is blocked.  `PdfDoc` + `geometry` + `paper` land in this
//! step; `ops` / `outline` and the CLI / TUI / Bund surfaces follow.

// P0 builds the library bottom-up, ahead of its CLI/TUI/Bund wiring
// (`Command::Pdf`, `ink.pdf.*`).  The `#[cfg(test)]` suites exercise the
// surface; this `allow` is removed when the first caller lands.
#![allow(dead_code)]

pub mod doc;
pub mod geometry;
pub mod meta;
pub mod ops;
pub mod paper;

// Re-export the public value type; consumed once `Command::Pdf` lands.
#[allow(unused_imports)]
pub use doc::{PdfDoc, PdfSource};

use std::fmt;

/// Errors from the PDF subsystem.
#[derive(Debug)]
pub enum Error {
    /// An error from the underlying `lopdf` parse/serialize.
    Lopdf(lopdf::Error),
    /// Filesystem I/O.
    Io(std::io::Error),
    /// A tree-aware operation (outline injection, by-chapter ops) was
    /// requested on a PDF inkhaven didn't author.
    NotInkhavenSource,
    /// Anything else, with a message.
    Other(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Lopdf(e) => write!(f, "pdf: {e}"),
            Error::Io(e) => write!(f, "pdf io: {e}"),
            Error::NotInkhavenSource => {
                write!(f, "pdf: operation requires an inkhaven-authored PDF")
            }
            Error::Other(m) => write!(f, "pdf: {m}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<lopdf::Error> for Error {
    fn from(e: lopdf::Error) -> Self {
        Error::Lopdf(e)
    }
}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

/// Result alias for the PDF subsystem.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
pub(crate) mod test_support {
    use lopdf::{Dictionary, Document, Object};

    /// A minimal valid `n`-page PDF, each page `w × h` points — the
    /// shared fixture for the `doc` / `ops` / `meta` unit tests.
    pub fn minimal_pdf(n: usize, w: f32, h: f32) -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let kids: Vec<Object> = (0..n)
            .map(|_| {
                let mut page = Dictionary::new();
                page.set("Type", "Page");
                page.set("Parent", pages_id);
                page.set(
                    "MediaBox",
                    vec![
                        Object::Integer(0),
                        Object::Integer(0),
                        Object::Real(w),
                        Object::Real(h),
                    ],
                );
                Object::Reference(doc.add_object(page))
            })
            .collect();
        let mut pages = Dictionary::new();
        pages.set("Type", "Pages");
        pages.set("Count", n as i64);
        pages.set("Kids", kids);
        doc.objects.insert(pages_id, Object::Dictionary(pages));
        let mut cat = Dictionary::new();
        cat.set("Type", "Catalog");
        cat.set("Pages", pages_id);
        let cat_id = doc.add_object(cat);
        doc.trailer.set("Root", cat_id);
        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        buf
    }
}

#[cfg(test)]
mod corpus_tests {
    use crate::typst_world::{InkhavenWorld, WorldSettings};
    use typst::layout::PagedDocument;
    use typst_pdf::PdfOptions;

    /// Compile an in-memory typst body to real typst-pdf bytes — the same
    /// path `Ctrl+B B` / `inkhaven export pdf` use, with the bundled
    /// fonts so the test is deterministic.
    fn typst_pdf_bytes(root: &std::path::Path, body: &str) -> Vec<u8> {
        let settings = WorldSettings {
            bundle_fonts: true,
            use_system_fonts: false,
            packages_enabled: false,
        };
        let world = InkhavenWorld::in_memory(root.to_path_buf(), body.to_string(), settings);
        let document = typst::compile::<PagedDocument>(&world)
            .output
            .expect("typst compile");
        typst_pdf::pdf(&document, &PdfOptions::default()).expect("typst-pdf emit")
    }

    /// Every dictionary in the document (plain dicts + stream dicts).
    fn dicts(doc: &lopdf::Document) -> impl Iterator<Item = &lopdf::Dictionary> {
        doc.objects.values().filter_map(|o| match o {
            lopdf::Object::Dictionary(d) => Some(d),
            lopdf::Object::Stream(s) => Some(&s.dict),
            _ => None,
        })
    }

    fn name_eq(d: &lopdf::Dictionary, key: &[u8], val: &[u8]) -> bool {
        d.get(key).ok().and_then(|o| o.as_name().ok()) == Some(val)
    }

    /// PDF-1 P0 fidelity gate — `lopdf` must parse + round-trip the exact
    /// PDFs inkhaven produces.  Heavy (compiles typst + loads fonts), so
    /// `#[ignore]`d like the other typst-compiling tests; run with
    /// `cargo test --bin inkhaven -- --ignored lopdf_round_trips`.
    #[test]
    #[ignore = "compiles typst; run explicitly as the PDF-1 fidelity gate"]
    fn lopdf_round_trips_typst_pdf_output() {
        let dir = tempfile::tempdir().unwrap();
        // A real raster image → exercises image XObjects, the riskiest
        // feature for a PDF parser.
        let img = image::RgbImage::from_pixel(8, 8, image::Rgb([200, 40, 40]));
        img.save(dir.path().join("px.png")).unwrap();

        let body = r#"#set page(width: 300pt, height: 400pt)
= A Heading
Body text with *bold* and _italic_, long enough to embed a real font
subset rather than a trivial one.
#line(length: 120pt, stroke: 1pt + black)
#rect(width: 80pt, height: 40pt, fill: rgb("4488cc"))
#circle(radius: 18pt, fill: green)
#image("px.png", width: 60pt)
#pagebreak()
== Second Page
More prose on a second page so the page tree has real depth."#;

        let bytes = typst_pdf_bytes(dir.path(), body);
        assert!(bytes.starts_with(b"%PDF-"), "typst produced a PDF");

        // 1. lopdf parses typst-pdf output without error.
        let mut doc = lopdf::Document::load_mem(&bytes)
            .expect("lopdf must parse inkhaven's typst-pdf output");
        assert_eq!(doc.get_pages().len(), 2, "both pages survive parsing");

        // 2. Embedded font subset is visible (typst embeds + subsets).
        let has_embedded_font = dicts(&doc).any(|d| {
            name_eq(d, b"Type", b"FontDescriptor")
                && (d.has(b"FontFile") || d.has(b"FontFile2") || d.has(b"FontFile3"))
        });
        assert!(has_embedded_font, "embedded font subset readable by lopdf");

        // 3. The raster image became an image XObject lopdf can see.
        let has_image = dicts(&doc).any(|d| name_eq(d, b"Subtype", b"Image"));
        assert!(has_image, "image XObject readable by lopdf");

        // 4. Round-trip: load → save → reload preserves the page tree.
        let mut out = Vec::new();
        doc.save_to(&mut out)
            .expect("lopdf re-serializes typst-pdf output");
        let reloaded = lopdf::Document::load_mem(&out)
            .expect("lopdf reloads its own re-serialized output");
        assert_eq!(
            reloaded.get_pages().len(),
            2,
            "round-trip preserves page count"
        );
    }

    /// PDF-1 P0.3 — `merge` reparents each source's page subtree rather
    /// than flattening, so inherited Resources (the image XObject) survive
    /// on *real* typst output.  The riskiest op, validated end-to-end.
    #[test]
    #[ignore = "compiles typst; PDF-1 merge fidelity on real output"]
    fn merge_preserves_typst_resources() {
        let dir = tempfile::tempdir().unwrap();
        image::RgbImage::from_pixel(8, 8, image::Rgb([10, 150, 40]))
            .save(dir.path().join("px.png"))
            .unwrap();
        let body = r#"#set page(width: 200pt, height: 260pt)
= Doc
Some prose and #image("px.png", width: 40pt)."#;
        let bytes = typst_pdf_bytes(dir.path(), body);
        let a = crate::pdf::PdfDoc::load_mem(&bytes).unwrap();
        let b = crate::pdf::PdfDoc::load_mem(&bytes).unwrap();
        let mut merged = crate::pdf::ops::merge(&[a, b]).unwrap();
        assert_eq!(merged.page_count(), 2, "merged page count");
        let out = merged.to_bytes().unwrap();
        let reloaded = lopdf::Document::load_mem(&out).expect("merged output reloads");
        assert_eq!(reloaded.get_pages().len(), 2);
        // The image XObject survives — proves inherited Resources weren't
        // dropped by the merge.
        assert!(
            dicts(&reloaded).any(|d| name_eq(d, b"Subtype", b"Image")),
            "image XObjects survive the merge"
        );
    }
}
