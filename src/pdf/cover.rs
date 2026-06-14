//! 1.3.0 PDF-1 P2 — cover-and-spine generation (RFC §8.4).
//!
//! A single landscape page laid out `[ bleed | back | spine | front |
//! bleed ]`, generated natively (no Typst pass — cover layout is purely
//! geometric).  The spine width is computed from page count + paper
//! stocks; the front image is embedded as a Flate-compressed image
//! XObject (decoded via the in-tree `image` crate — no lopdf image
//! feature, no duplicate `image` version).

use std::path::{Path, PathBuf};

use lopdf::{Dictionary, Document, Object, Stream};
use serde::{Deserialize, Serialize};

use super::barcode::{render_ean13, BarcodeSpec};
use super::doc::{PdfDoc, PdfSource};
use super::geometry::mm_to_pt;
use super::paper::{self, PaperStock};
use super::{Error, Result};

/// Spine width in mm (RFC §8.4): the interior stack (2 pages per sheet)
/// + the cover wrap on both faces + a binding allowance.
pub fn spine_width_mm(page_count: usize, interior: PaperStock, cover: PaperStock) -> f32 {
    page_count as f32 * interior.thickness_mm * 0.5
        + cover.thickness_mm * 2.0
        + cover.binding_compensation_mm()
}

/// Title / author printed up the spine.
#[derive(Debug, Clone, Default)]
pub struct SpineText {
    pub title: Option<String>,
    pub author: Option<String>,
    pub font_size_pt: f32,
}

/// A cover to generate.  Front/back are `front_width_mm × front_height_mm`;
/// the spine width comes from [`spine_width_mm`].
#[derive(Debug, Clone)]
pub struct CoverSpec {
    pub front_width_mm: f32,
    pub front_height_mm: f32,
    pub spine_width_mm: f32,
    pub bleed_mm: f32,
    pub front_image: Option<std::path::PathBuf>,
    pub spine_text: SpineText,
    pub back_text: Option<String>,
    pub barcode: Option<BarcodeSpec>,
}

/// The `cover:` HJSON block — house defaults for trim size, bleed, paper
/// stocks (for the computed spine), and spine type size.  Merges through
/// the config cascade like every other setting.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CoverConfig {
    pub front_width_mm: f32,
    pub front_height_mm: f32,
    pub bleed_mm: f32,
    pub interior_stock: String,
    pub cover_stock: String,
    pub spine_font_size_pt: f32,
}

impl Default for CoverConfig {
    fn default() -> Self {
        Self {
            front_width_mm: 152.0,  // 6 in
            front_height_mm: 229.0, // 9 in trade
            bleed_mm: 3.0,
            interior_stock: paper::DEFAULT_INTERIOR.into(),
            cover_stock: paper::DEFAULT_COVER.into(),
            spine_font_size_pt: 11.0,
        }
    }
}

/// The per-invocation cover inputs (page count, text, art, ISBN) that the
/// CLI / book-take supplies on top of [`CoverConfig`].
#[derive(Debug, Clone, Default)]
pub struct CoverRequest {
    pub page_count: usize,
    pub title: Option<String>,
    pub author: Option<String>,
    pub back_text: Option<String>,
    pub front_image: Option<PathBuf>,
    pub isbn: Option<String>,
    /// Override the computed spine width (e.g. printer-supplied).
    pub spine_mm_override: Option<f32>,
}

impl CoverConfig {
    fn stock(&self, name: &str, fallback: &str) -> PaperStock {
        paper::paper_stock(name)
            .or_else(|| paper::paper_stock(fallback))
            .unwrap_or_else(|| PaperStock::custom("fallback", 0.1))
    }

    /// Spine width (mm) for `page_count` using this profile's stocks.
    pub fn spine_mm(&self, page_count: usize) -> f32 {
        spine_width_mm(
            page_count,
            self.stock(&self.interior_stock, paper::DEFAULT_INTERIOR),
            self.stock(&self.cover_stock, paper::DEFAULT_COVER),
        )
    }

    /// Combine these defaults with a [`CoverRequest`] into a [`CoverSpec`].
    pub fn build_spec(&self, req: &CoverRequest) -> CoverSpec {
        let spine = req
            .spine_mm_override
            .unwrap_or_else(|| self.spine_mm(req.page_count));
        CoverSpec {
            front_width_mm: self.front_width_mm,
            front_height_mm: self.front_height_mm,
            spine_width_mm: spine,
            bleed_mm: self.bleed_mm,
            front_image: req.front_image.clone(),
            spine_text: SpineText {
                title: req.title.clone(),
                author: req.author.clone(),
                font_size_pt: self.spine_font_size_pt,
            },
            back_text: req.back_text.clone(),
            barcode: req.isbn.as_ref().map(|isbn| BarcodeSpec {
                isbn: isbn.clone(),
                ..Default::default()
            }),
        }
    }
}

/// Build the cover PDF (one page).
pub fn build_cover(spec: &CoverSpec) -> Result<PdfDoc> {
    let bleed = mm_to_pt(spec.bleed_mm);
    let fw = mm_to_pt(spec.front_width_mm);
    let fh = mm_to_pt(spec.front_height_mm);
    let sw = mm_to_pt(spec.spine_width_mm);
    let total_w = 2.0 * fw + sw + 2.0 * bleed;
    let total_h = fh + 2.0 * bleed;
    let back_x = bleed;
    let spine_x = bleed + fw;
    let front_x = bleed + fw + sw;
    let region_y = bleed;

    let mut doc = Document::with_version("1.5");
    let mut xobj = Dictionary::new();
    let mut c = String::new();

    // Front image — fills the front region out to the right bleed edge.
    if let Some(path) = &spec.front_image {
        let stream = image_xobject(path)?;
        let id = doc.add_object(stream);
        xobj.set("CoverImg", Object::Reference(id));
        let w = total_w - front_x;
        c.push_str(&format!(
            "q {w:.3} 0 0 {total_h:.3} {front_x:.3} 0 cm /CoverImg Do Q\n"
        ));
    }

    // Spine text, rotated 90° CCW (reads bottom-to-top).
    let mut label = String::new();
    if let Some(t) = &spec.spine_text.title {
        label.push_str(t);
    }
    if let Some(a) = &spec.spine_text.author {
        if !label.is_empty() {
            label.push_str("   ·   ");
        }
        label.push_str(a);
    }
    if !label.is_empty() {
        let size = spec.spine_text.font_size_pt.max(6.0);
        let cx = spine_x + sw / 2.0 + size * 0.35;
        let y = region_y + fh * 0.18;
        c.push_str(&format!(
            "0 g\nBT /F1 {size:.1} Tf 0 1 -1 0 {cx:.3} {y:.3} Tm ({}) Tj ET\n",
            esc(&label)
        ));
    }

    // Back text (top-left of the back region).
    if let Some(back) = &spec.back_text {
        let x = back_x + mm_to_pt(15.0);
        let y = region_y + fh - mm_to_pt(20.0);
        c.push_str(&format!("0 g\nBT /F1 10 Tf {x:.3} {y:.3} Td ({}) Tj ET\n", esc(back)));
    }

    // Barcode (back, bottom-right).
    if let Some(b) = &spec.barcode {
        let r = render_ean13(b)?;
        let x = back_x + fw - r.width_pt - mm_to_pt(12.0);
        let y = region_y + mm_to_pt(12.0);
        c.push_str(&format!("q 1 0 0 1 {x:.3} {y:.3} cm\n{}Q\n", r.ops));
    }

    // Crop marks at the trim corners (inside the bleed).
    crop_marks(&mut c, bleed, bleed, total_w - bleed, total_h - bleed);

    let content_id = doc.add_object(Stream::new(Dictionary::new(), c.into_bytes()));

    let mut helv = Dictionary::new();
    helv.set("Type", "Font");
    helv.set("Subtype", "Type1");
    helv.set("BaseFont", "Helvetica");
    let mut font = Dictionary::new();
    font.set("F1", Object::Dictionary(helv));
    let mut res = Dictionary::new();
    res.set("XObject", Object::Dictionary(xobj));
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
            Object::Real(total_w),
            Object::Real(total_h),
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

/// Decode an image to RGB8 and wrap it as a Flate-compressed PDF image
/// XObject (the in-tree `image` crate + lopdf's `Stream::compress`; no
/// lopdf image feature, no `flate2` dep).
pub(crate) fn image_xobject(path: &Path) -> Result<Stream> {
    let img =
        image::open(path).map_err(|e| Error::Other(format!("cover image `{}`: {e}", path.display())))?;
    let rgb = img.to_rgb8();
    let (w, h) = (rgb.width(), rgb.height());
    let mut d = Dictionary::new();
    d.set("Type", "XObject");
    d.set("Subtype", "Image");
    d.set("Width", w as i64);
    d.set("Height", h as i64);
    d.set("ColorSpace", Object::Name(b"DeviceRGB".to_vec()));
    d.set("BitsPerComponent", 8i64);
    let mut stream = Stream::new(d, rgb.into_raw());
    let _ = stream.compress(); // flate + sets /Filter /FlateDecode
    Ok(stream)
}

fn tick(c: &mut String, ax: f32, ay: f32, bx: f32, by: f32) {
    c.push_str(&format!("{ax:.3} {ay:.3} m {bx:.3} {by:.3} l S\n"));
}

fn crop_marks(c: &mut String, x0: f32, y0: f32, x1: f32, y1: f32) {
    let off = mm_to_pt(3.0);
    let len = mm_to_pt(5.0);
    c.push_str("0 G 0.25 w\n");
    // bottom-left
    tick(c, x0 - off - len, y0, x0 - off, y0);
    tick(c, x0, y0 - off - len, x0, y0 - off);
    // bottom-right
    tick(c, x1 + off, y0, x1 + off + len, y0);
    tick(c, x1, y0 - off - len, x1, y0 - off);
    // top-left
    tick(c, x0 - off - len, y1, x0 - off, y1);
    tick(c, x0, y1 + off, x0, y1 + off + len);
    // top-right
    tick(c, x1 + off, y1, x1 + off + len, y1);
    tick(c, x1, y1 + off, x1, y1 + off + len);
}

/// Escape a string for a PDF literal `(...)`.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if matches!(ch, '(' | ')' | '\\') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::paper::paper_stock;
    use crate::pdf::PdfDoc;

    #[test]
    fn spine_width_for_a_novel() {
        let interior = paper_stock("uncoated_80gsm").unwrap();
        let cover = paper_stock("cover_250gsm").unwrap();
        // 200 pages: 200·0.10·0.5 + 0.30·2 + 1.0 = 10 + 0.6 + 1.0 = 11.6 mm
        let w = spine_width_mm(200, interior, cover);
        assert!((w - 11.6).abs() < 1e-3, "got {w}");
        // thicker book → wider spine
        assert!(spine_width_mm(400, interior, cover) > w);
    }

    fn spec() -> CoverSpec {
        CoverSpec {
            front_width_mm: 152.0,
            front_height_mm: 229.0, // 6×9 trade
            spine_width_mm: 12.0,
            bleed_mm: 3.0,
            front_image: None,
            spine_text: SpineText {
                title: Some("The Lantern Room".into()),
                author: Some("V. Ulogov".into()),
                font_size_pt: 11.0,
            },
            back_text: Some("A novel.".into()),
            barcode: Some(BarcodeSpec {
                isbn: "9780306406157".into(),
                ..Default::default()
            }),
        }
    }

    #[test]
    fn builds_cover_with_text_and_barcode() {
        let mut doc = build_cover(&spec()).unwrap();
        assert_eq!(doc.page_count(), 1);
        // page = 2·front + spine + 2·bleed wide
        let sz = doc.page_size(0).unwrap();
        let expect_w = mm_to_pt(2.0 * 152.0 + 12.0 + 2.0 * 3.0);
        assert!((sz.width() - expect_w).abs() < 1.0, "got {}", sz.width());
        assert_eq!(PdfDoc::load_mem(&doc.to_bytes().unwrap()).unwrap().page_count(), 1);
    }

    #[test]
    fn config_build_spec_computes_spine_from_pages() {
        let cfg = CoverConfig::default();
        let req = CoverRequest {
            page_count: 200,
            title: Some("The Lantern Room".into()),
            isbn: Some("9780306406157".into()),
            ..Default::default()
        };
        let spec = cfg.build_spec(&req);
        // same 11.6 mm as the standalone spine_width_mm novel case
        assert!((spec.spine_width_mm - 11.6).abs() < 1e-2, "got {}", spec.spine_width_mm);
        assert_eq!(spec.front_width_mm, 152.0);
        assert!(spec.barcode.is_some(), "isbn → barcode");
        // override wins
        let forced = cfg.build_spec(&CoverRequest {
            spine_mm_override: Some(20.0),
            ..req
        });
        assert_eq!(forced.spine_width_mm, 20.0);
    }

    #[test]
    fn embeds_front_image() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("front.png");
        image::RgbImage::from_pixel(16, 24, image::Rgb([180, 60, 60]))
            .save(&path)
            .unwrap();
        let mut s = spec();
        s.front_image = Some(path);
        let mut doc = build_cover(&s).unwrap();
        let reloaded = lopdf::Document::load_mem(&doc.to_bytes().unwrap()).unwrap();
        let has_image = reloaded.objects.values().any(|o| match o {
            lopdf::Object::Stream(st) => {
                st.dict.get(b"Subtype").ok().and_then(|x| x.as_name().ok()) == Some(&b"Image"[..])
            }
            _ => false,
        });
        assert!(has_image, "cover embeds the front image XObject");
    }
}
