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

// The `Command::Pdf` CLI now consumes `doc`/`ops`/`meta`/`outline`; the
// remaining unused surface (`paper`, imposition-only `geometry`,
// `inject_outline`) is built ahead of P1/P2 (imposition, cover) + the
// `ink.pdf.*` Bund layer.  This `allow` retires when those land.
#![allow(dead_code)]

pub mod doc;
pub mod geometry;
pub mod meta;
pub mod ops;
pub mod outline;
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

/// Encode a Rust string as a PDF text string — ASCII as a literal,
/// otherwise UTF-16BE with a BOM.  Shared by `meta` + `outline`.
pub(crate) fn pdf_string(s: &str) -> lopdf::Object {
    use lopdf::{Object, StringFormat};
    if s.is_ascii() {
        Object::String(s.as_bytes().to_vec(), StringFormat::Literal)
    } else {
        let mut bytes = vec![0xFE, 0xFF];
        for u in s.encode_utf16() {
            bytes.extend_from_slice(&u.to_be_bytes());
        }
        Object::String(bytes, StringFormat::Literal)
    }
}

/// Decode a PDF text string — UTF-16BE-with-BOM, else lossy UTF-8/Latin.
pub(crate) fn decode_pdf_string(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let u16s: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&u16s)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

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

    /// PDF-1 — end-to-end feature-proof: compile a synthetic multi-chapter
    /// `.typ` (with the additive `#metadata` marker the assemble step now
    /// emits) and exercise the *whole* manipulation set against the real
    /// PDF — meta, outline inject/read, extract, split, merge, rotate,
    /// delete, reorder, and an atomic save→reload.
    #[test]
    #[ignore = "compiles typst; PDF-1 end-to-end feature-proof on real output"]
    fn full_pdf_feature_proof() {
        use crate::pdf::{meta, ops, outline, PdfDoc};

        let dir = tempfile::tempdir().unwrap();
        image::RgbImage::from_pixel(8, 8, image::Rgb([30, 90, 200]))
            .save(dir.path().join("px.png"))
            .unwrap();

        let body = r#"#set page(width: 300pt, height: 400pt)
#metadata((node_id: "11111111-1111-1111-1111-111111111111"))
= Chapter One
Opening prose with an image. #image("px.png", width: 50pt)
#pagebreak()
Chapter one continues on a second page.
#pagebreak()
= Chapter Two
Second chapter prose.
#pagebreak()
== Section 2.1
A subsection of chapter two.
#pagebreak()
= Chapter Three
The third and final chapter.
#pagebreak()
The end."#;
        let bytes = typst_pdf_bytes(dir.path(), body);
        assert!(bytes.starts_with(b"%PDF-"), "the #metadata marker compiles inertly");

        let base = PdfDoc::load_mem(&bytes).unwrap();
        let pages = base.page_count();
        assert!(pages >= 5, "synthetic doc paginates to >=5 pages (got {pages})");

        // Reading an outline never panics (typst may emit its own).
        let _ = outline::read_outline(&base);

        // metadata: write → read-back → strip.
        {
            let mut doc = PdfDoc::load_mem(&bytes).unwrap();
            let m = meta::PdfMetadata {
                title: Some("Proof".into()),
                author: Some("Tester".into()),
                keywords: vec!["a".into(), "b".into()],
                ..Default::default()
            };
            meta::write_metadata(&mut doc, &m).unwrap();
            let rt = PdfDoc::load_mem(&doc.to_bytes().unwrap()).unwrap();
            let read = meta::read_metadata(&rt);
            assert_eq!(read.title.as_deref(), Some("Proof"));
            assert_eq!(read.keywords, vec!["a".to_string(), "b".to_string()]);
            let mut d2 = PdfDoc::load_mem(&doc.to_bytes().unwrap()).unwrap();
            meta::strip_metadata(&mut d2).unwrap();
            assert_eq!(meta::read_metadata(&d2).title, None);
        }

        // outline: inject a custom tree → read it back off the real PDF.
        {
            let mut doc = PdfDoc::load_mem(&bytes).unwrap();
            let items = vec![
                outline::OutlineItem::new("Chapter One", 0),
                outline::OutlineItem::new("Chapter Two", 2)
                    .with_children(vec![outline::OutlineItem::new("Section 2.1", 3)]),
                outline::OutlineItem::new("Chapter Three", 4),
            ];
            outline::inject_outline(&mut doc, &items).unwrap();
            let rt = PdfDoc::load_mem(&doc.to_bytes().unwrap()).unwrap();
            assert_eq!(outline::read_outline(&rt), items, "injected outline round-trips");
        }

        // extract a contiguous range.
        assert_eq!(
            ops::extract(&base, &ops::PageSpec::parse("2-4").unwrap())
                .unwrap()
                .page_count(),
            3
        );

        // split into 2-page pieces (counts sum back to the whole).
        {
            let parts = ops::split(&base, &ops::SplitMode::EveryNPages(2)).unwrap();
            assert_eq!(parts.iter().map(|p| p.page_count()).sum::<usize>(), pages);
            assert_eq!(parts.len(), pages.div_ceil(2));
        }

        // merge two copies; the image survives the concat.
        {
            let a = PdfDoc::load_mem(&bytes).unwrap();
            let b = PdfDoc::load_mem(&bytes).unwrap();
            let mut merged = ops::merge(&[a, b]).unwrap();
            assert_eq!(merged.page_count(), pages * 2);
            let reloaded = lopdf::Document::load_mem(&merged.to_bytes().unwrap()).unwrap();
            assert!(dicts(&reloaded).any(|d| name_eq(d, b"Subtype", b"Image")));
        }

        // rotate page 1 by 90°.
        {
            let mut doc = PdfDoc::load_mem(&bytes).unwrap();
            ops::rotate(&mut doc, &ops::PageSpec::Single(1), ops::Rotation::D90).unwrap();
            let id = doc.document().get_pages()[&1];
            let r = match doc.document().get_dictionary(id).unwrap().get(b"Rotate") {
                Ok(lopdf::Object::Integer(i)) => *i,
                _ => 0,
            };
            assert_eq!(r, 90);
        }

        // delete two pages.
        {
            let mut doc = PdfDoc::load_mem(&bytes).unwrap();
            ops::delete(&mut doc, &ops::PageSpec::parse("1,2").unwrap()).unwrap();
            assert_eq!(doc.page_count(), pages - 2);
        }

        // reverse the page order.
        {
            let mut doc = PdfDoc::load_mem(&bytes).unwrap();
            let rev: Vec<usize> = (0..pages).rev().collect();
            ops::reorder(&mut doc, &rev).unwrap();
            assert_eq!(doc.page_count(), pages);
        }

        // atomic save to disk → reload.
        {
            let mut doc = PdfDoc::load_mem(&bytes).unwrap();
            let out_path = dir.path().join("out.pdf");
            doc.save(&out_path).unwrap();
            assert_eq!(PdfDoc::load(&out_path).unwrap().page_count(), pages);
        }
    }
}
