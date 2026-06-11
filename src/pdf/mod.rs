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
}
