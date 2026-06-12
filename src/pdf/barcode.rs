//! 1.3.0 PDF-1 P2 — ISBN EAN-13 barcode (RFC §8.5).
//!
//! `barcoders` encodes the 95-module bar/space pattern (validating the
//! ISBN + check digit); we turn each run of dark modules into a filled
//! PDF rectangle — no PNG round-trip, so it scales to any DPI.  Output is
//! a content-op string drawn at the origin (bars above, the human-
//! readable digits below) plus the block size, which the cover / a
//! standalone barcode page positions.

use barcoders::sym::ean13::EAN13;
use lopdf::{Dictionary, Document, Object, Stream};

use super::doc::{PdfDoc, PdfSource};
use super::geometry::mm_to_pt;
use super::{Error, Result};

/// What to render.
#[derive(Debug, Clone)]
pub struct BarcodeSpec {
    /// 12- or 13-digit ISBN (hyphens/spaces ignored).
    pub isbn: String,
    /// Bar height in mm (EAN-13 nominal ≈ 22.85).
    pub height_mm: f32,
    /// X-dimension (single module width) in mm (EAN-13 SC2 ≈ 0.33).
    pub module_width_mm: f32,
    pub include_human_readable: bool,
}

impl Default for BarcodeSpec {
    fn default() -> Self {
        Self {
            isbn: String::new(),
            height_mm: 22.85,
            module_width_mm: 0.33,
            include_human_readable: true,
        }
    }
}

/// The barcode as PDF content ops at the origin, with its block size.
#[derive(Debug, Clone)]
pub struct RenderedBarcode {
    pub ops: String,
    pub width_pt: f32,
    pub height_pt: f32,
}

/// EAN-13 check digit for the first 12 digits (mod-10).
pub fn check_digit(first12: &[u8]) -> u8 {
    let sum: u32 = first12
        .iter()
        .enumerate()
        .map(|(i, &d)| d as u32 * if i % 2 == 0 { 1 } else { 3 })
        .sum();
    ((10 - (sum % 10)) % 10) as u8
}

fn sanitize(isbn: &str) -> Result<Vec<u8>> {
    let digits: Vec<u8> = isbn
        .chars()
        .filter(|c| !matches!(c, '-' | ' ' | '_'))
        .map(|c| {
            c.to_digit(10)
                .map(|d| d as u8)
                .ok_or_else(|| Error::Other(format!("barcode: `{c}` is not a digit")))
        })
        .collect::<Result<_>>()?;
    if digits.len() != 12 && digits.len() != 13 {
        return Err(Error::Other(format!(
            "barcode: ISBN must be 12 or 13 digits, got {}",
            digits.len()
        )));
    }
    Ok(digits)
}

/// Render the EAN-13 barcode for `spec.isbn`.  Validates the ISBN +
/// check digit (via `barcoders`).
pub fn render_ean13(spec: &BarcodeSpec) -> Result<RenderedBarcode> {
    let digits = sanitize(&spec.isbn)?;
    // Full 13-digit display string (compute the check digit when 12 given).
    let display: String = if digits.len() == 13 {
        digits.iter().map(|d| (b'0' + d) as char).collect()
    } else {
        let mut d = digits.clone();
        d.push(check_digit(&digits));
        d.iter().map(|x| (b'0' + x) as char).collect()
    };
    let ean = EAN13::new(&display)
        .map_err(|e| Error::Other(format!("barcode: invalid ISBN `{}`: {e:?}", spec.isbn)))?;
    let modules = ean.encode();

    let mw = mm_to_pt(spec.module_width_mm.max(0.1));
    let bar_h = mm_to_pt(spec.height_mm.max(1.0));
    let text_h = if spec.include_human_readable {
        mm_to_pt(3.5)
    } else {
        0.0
    };
    let width_pt = modules.len() as f32 * mw;

    let mut s = String::from("0 g\n");
    let mut i = 0;
    while i < modules.len() {
        if modules[i] == 1 {
            let start = i;
            while i < modules.len() && modules[i] == 1 {
                i += 1;
            }
            s.push_str(&format!(
                "{:.3} {:.3} {:.3} {:.3} re\n",
                start as f32 * mw,
                text_h,
                (i - start) as f32 * mw,
                bar_h
            ));
        } else {
            i += 1;
        }
    }
    s.push_str("f\n");

    if spec.include_human_readable {
        let size = (text_h * 0.85).max(4.0);
        let tx = ((width_pt - size * 0.5 * display.len() as f32) / 2.0).max(0.0);
        s.push_str(&format!("BT /F1 {size:.1} Tf {tx:.3} 0.5 Td ({display}) Tj ET\n"));
    }

    Ok(RenderedBarcode {
        ops: s,
        width_pt,
        height_pt: bar_h + text_h,
    })
}

/// A standalone single-page PDF containing just the barcode (a quiet-zone
/// margin around it), with a `/F1` Helvetica for the digits.
pub fn build_barcode_pdf(spec: &BarcodeSpec) -> Result<PdfDoc> {
    let b = render_ean13(spec)?;
    let margin = mm_to_pt(4.0);
    let page_w = b.width_pt + 2.0 * margin;
    let page_h = b.height_pt + 2.0 * margin;

    let mut doc = Document::with_version("1.5");
    let content = format!("q 1 0 0 1 {margin:.3} {margin:.3} cm\n{}Q\n", b.ops);
    let content_id = doc.add_object(Stream::new(Dictionary::new(), content.into_bytes()));

    let mut helv = Dictionary::new();
    helv.set("Type", "Font");
    helv.set("Subtype", "Type1");
    helv.set("BaseFont", "Helvetica");
    let mut font = Dictionary::new();
    font.set("F1", Object::Dictionary(helv));
    let mut res = Dictionary::new();
    res.set("Font", Object::Dictionary(font));

    let pages_id = doc.new_object_id();
    let mut page = Dictionary::new();
    page.set("Type", "Page");
    page.set("Parent", pages_id);
    page.set(
        "MediaBox",
        vec![
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(page_w),
            Object::Real(page_h),
        ],
    );
    page.set("Resources", Object::Dictionary(res));
    page.set("Contents", content_id);
    let page_id = doc.add_object(page);

    let mut pages = Dictionary::new();
    pages.set("Type", "Pages");
    pages.set("Kids", vec![Object::Reference(page_id)]);
    pages.set("Count", 1);
    doc.objects.insert(pages_id, Object::Dictionary(pages));
    let mut cat = Dictionary::new();
    cat.set("Type", "Catalog");
    cat.set("Pages", pages_id);
    let cat_id = doc.add_object(cat);
    doc.trailer.set("Root", cat_id);

    Ok(PdfDoc::from_document(doc, PdfSource::External))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::PdfDoc;

    #[test]
    fn check_digit_matches_known_isbn() {
        // 978-0-306-40615-7 → check digit 7
        let first12: Vec<u8> = "978030640615".bytes().map(|b| b - b'0').collect();
        assert_eq!(check_digit(&first12), 7);
    }

    #[test]
    fn renders_bars_and_text() {
        let spec = BarcodeSpec {
            isbn: "978-0-306-40615-7".into(),
            ..Default::default()
        };
        let r = render_ean13(&spec).unwrap();
        assert!(r.width_pt > 0.0);
        // 95 modules × X-dim
        assert!((r.width_pt - 95.0 * mm_to_pt(0.33)).abs() < 0.1);
        assert!(r.ops.contains(" re\n") && r.ops.contains("f\n")); // filled rects
        assert!(r.ops.contains("(9780306406157) Tj")); // human-readable
    }

    #[test]
    fn rejects_bad_isbn() {
        assert!(render_ean13(&BarcodeSpec {
            isbn: "123".into(),
            ..Default::default()
        })
        .is_err());
        // wrong check digit (…6 instead of …7)
        assert!(render_ean13(&BarcodeSpec {
            isbn: "9780306406156".into(),
            ..Default::default()
        })
        .is_err());
    }

    #[test]
    fn builds_a_one_page_pdf() {
        let spec = BarcodeSpec {
            isbn: "9780306406157".into(),
            ..Default::default()
        };
        let mut doc = build_barcode_pdf(&spec).unwrap();
        assert_eq!(doc.page_count(), 1);
        assert_eq!(
            PdfDoc::load_mem(&doc.to_bytes().unwrap()).unwrap().page_count(),
            1
        );
    }
}
