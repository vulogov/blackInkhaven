//! 1.3.0 PDF-1 P3 — watermark / stamp (RFC §8.7).
//!
//! Stamp text (`DRAFT`, a name, a date) and/or an image (a logo) onto a
//! page range.  Each stamp is appended as a self-contained `q … Q` block
//! after the page's existing content — wrapped in its own graphics state
//! with a constant-alpha `ExtGState` for translucency — so it never
//! disturbs the body.  Text is centred and rotatable; the image is
//! centred and scaled to a fraction of the page width.

use std::path::PathBuf;

use lopdf::{Dictionary, Object};

use super::doc::PdfDoc;
use super::geometry::mm_to_pt;
use super::ops::PageSpec;
use super::{Error, Result};

/// Where on the page the stamp anchors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WmPosition {
    Center,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl WmPosition {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().replace(['-', '_'], "").as_str() {
            "center" | "centre" | "middle" => Some(Self::Center),
            "topleft" | "tl" => Some(Self::TopLeft),
            "topright" | "tr" => Some(Self::TopRight),
            "bottomleft" | "bl" => Some(Self::BottomLeft),
            "bottomright" | "br" => Some(Self::BottomRight),
            _ => None,
        }
    }
}

/// What to stamp.  At least one of `text` / `image` should be set.
#[derive(Debug, Clone)]
pub struct WatermarkSpec {
    pub text: Option<String>,
    pub image: Option<PathBuf>,
    /// Constant alpha 0..=1 (fill + stroke).
    pub opacity: f32,
    /// Text rotation in degrees (counter-clockwise; e.g. 45 for a
    /// diagonal `DRAFT`).
    pub rotation_deg: f32,
    pub font_size_pt: f32,
    /// Text colour (0..=1 RGB).
    pub color: (f32, f32, f32),
    pub position: WmPosition,
    /// Image width as a fraction of the page width (aspect preserved).
    pub image_scale: f32,
    /// Page range; `None` = every page.
    pub pages: Option<PageSpec>,
}

impl Default for WatermarkSpec {
    fn default() -> Self {
        Self {
            text: None,
            image: None,
            opacity: 0.18,
            rotation_deg: 45.0,
            font_size_pt: 72.0,
            color: (0.5, 0.5, 0.5),
            position: WmPosition::Center,
            image_scale: 0.5,
            pages: None,
        }
    }
}

/// Inset from the page edge for the corner anchors.
const MARGIN: f32 = 24.0; // pt (~8.5 mm)

/// Stamp `spec` onto the selected pages.  Returns the number stamped.
pub fn apply_watermark(doc: &mut PdfDoc, spec: &WatermarkSpec) -> Result<usize> {
    if spec.text.is_none() && spec.image.is_none() {
        return Err(Error::Other("watermark: nothing to stamp (no text or image)".into()));
    }
    let page_ids = doc.page_ids().to_vec();
    let count = page_ids.len();
    let selected: Vec<u32> = spec
        .pages
        .as_ref()
        .map(|s| s.resolve(count))
        .unwrap_or_else(|| (1..=count as u32).collect());

    // Image pixel size (to preserve aspect) + the shared XObject, added once.
    let image_obj = match &spec.image {
        Some(path) => {
            let (stream, w, h) = super::cover::image_xobject(path)?;
            let id = doc.document_mut().add_object(stream);
            Some((id, (w.max(1) as f32), (h.max(1) as f32)))
        }
        None => None,
    };

    // Shared ExtGState for translucency.
    let alpha = spec.opacity.clamp(0.0, 1.0);
    let mut gs = Dictionary::new();
    gs.set("Type", Object::Name(b"ExtGState".to_vec()));
    gs.set("ca", Object::Real(alpha));
    gs.set("CA", Object::Real(alpha));
    let gs_id = doc.document_mut().add_object(Object::Dictionary(gs));

    let inner = doc.document_mut();
    let mut stamped = 0usize;
    for &page_no in &selected {
        let idx = (page_no - 1) as usize;
        let Some(&pid) = page_ids.get(idx) else { continue };
        // Page geometry from MediaBox.
        let (pw, ph) = page_box(inner, pid).unwrap_or((mm_to_pt(210.0), mm_to_pt(297.0)));
        let aspect = image_obj.map(|(_, w, h)| h / w);
        let ops = build_stamp_ops(spec, pw, ph, aspect);

        // Wire resources: ExtGState/WmGS, Font/WmF, XObject/WmImg.
        inner.add_graphics_state(pid, "WmGS", gs_id).map_err(Error::Lopdf)?;
        ensure_font(inner, pid)?;
        if let Some((img_id, _, _)) = image_obj {
            inner.add_xobject(pid, "WmImg", img_id).map_err(Error::Lopdf)?;
        }

        // Append the stamp after existing content (collapsed to one stream).
        let mut content = inner.get_and_decode_page_content(pid).map_err(Error::Lopdf)?;
        let extra = lopdf::content::Content::decode(ops.as_bytes())
            .map_err(Error::Lopdf)?;
        content.operations.extend(extra.operations);
        let encoded = content.encode().map_err(Error::Lopdf)?;
        inner.change_page_content(pid, encoded).map_err(Error::Lopdf)?;
        stamped += 1;
    }
    Ok(stamped)
}

/// Build the `q … Q` stamp content for one page of size `pw × ph`.
/// `image_aspect` is the stamp image's height/width (None when no image).
fn build_stamp_ops(spec: &WatermarkSpec, pw: f32, ph: f32, image_aspect: Option<f32>) -> String {
    let (ax, ay) = anchor(spec.position, pw, ph);
    let mut s = String::from("q\n/WmGS gs\n");

    if let Some(aspect) = image_aspect {
        // Centre the scaled image on the anchor, preserving aspect.
        let tw = pw * spec.image_scale.clamp(0.02, 1.0);
        let th = tw * aspect;
        let ix = ax - tw / 2.0;
        let iy = ay - th / 2.0;
        s.push_str(&format!("q {tw:.3} 0 0 {th:.3} {ix:.3} {iy:.3} cm /WmImg Do Q\n"));
    }

    if let Some(text) = &spec.text {
        let size = spec.font_size_pt.max(4.0);
        let (r, g, b) = spec.color;
        let rad = spec.rotation_deg.to_radians();
        let (cos, sin) = (rad.cos(), rad.sin());
        // Rough Helvetica advance for centring.
        let tw = size * 0.5 * text.chars().count() as f32;
        s.push_str(&format!("{r:.3} {g:.3} {b:.3} rg\n"));
        s.push_str(&format!(
            "BT /WmF {size:.1} Tf {cos:.5} {sin:.5} {nsin:.5} {cos:.5} {ax:.3} {ay:.3} Tm\n",
            nsin = -sin
        ));
        // Shift along the rotated baseline to centre the run on the anchor.
        s.push_str(&format!("{:.3} {:.3} Td ({}) Tj ET\n", -tw / 2.0, -size * 0.35, esc(text)));
    }

    s.push_str("Q\n");
    s
}

fn anchor(pos: WmPosition, pw: f32, ph: f32) -> (f32, f32) {
    match pos {
        WmPosition::Center => (pw / 2.0, ph / 2.0),
        WmPosition::TopLeft => (MARGIN, ph - MARGIN),
        WmPosition::TopRight => (pw - MARGIN, ph - MARGIN),
        WmPosition::BottomLeft => (MARGIN, MARGIN),
        WmPosition::BottomRight => (pw - MARGIN, MARGIN),
    }
}

fn page_box(doc: &lopdf::Document, page_id: lopdf::ObjectId) -> Option<(f32, f32)> {
    let dict = doc.get_dictionary(page_id).ok()?;
    let mb = dict.get(b"MediaBox").ok()?.as_array().ok()?;
    if mb.len() != 4 {
        return None;
    }
    let v: Vec<f32> = mb.iter().map(|o| o.as_float().unwrap_or(0.0)).collect();
    Some(((v[2] - v[0]).abs(), (v[3] - v[1]).abs()))
}

/// Ensure the page's Resources carry an inline Helvetica as `/WmF`.
fn ensure_font(doc: &mut lopdf::Document, page_id: lopdf::ObjectId) -> Result<()> {
    let res = doc
        .get_or_create_resources(page_id)
        .map_err(Error::Lopdf)?
        .as_dict_mut()
        .map_err(Error::Lopdf)?;
    if !res.has(b"Font") {
        res.set("Font", Dictionary::new());
    }
    let fonts = res.get_mut(b"Font").and_then(Object::as_dict_mut).map_err(Error::Lopdf)?;
    if !fonts.has(b"WmF") {
        let mut helv = Dictionary::new();
        helv.set("Type", "Font");
        helv.set("Subtype", "Type1");
        helv.set("BaseFont", "Helvetica");
        fonts.set("WmF", Object::Dictionary(helv));
    }
    Ok(())
}

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
    use crate::pdf::PdfDoc;
    use lopdf::Stream;

    /// A small helper PDF whose pages carry a trivial content stream.
    fn doc_with_content(n: usize) -> PdfDoc {
        let mut pdf = PdfDoc::load_mem(&crate::pdf::test_support::minimal_pdf(n, 300.0, 400.0)).unwrap();
        let ids = pdf.page_ids().to_vec();
        let inner = pdf.document_mut();
        for pid in ids {
            let cid = inner.add_object(Stream::new(Dictionary::new(), b"0 0 0 rg\n".to_vec()));
            if let Ok(Object::Dictionary(p)) = inner.get_object_mut(pid) {
                p.set("Contents", cid);
            }
        }
        pdf
    }

    #[test]
    fn position_parses() {
        assert_eq!(WmPosition::parse("center"), Some(WmPosition::Center));
        assert_eq!(WmPosition::parse("bottom-right"), Some(WmPosition::BottomRight));
        assert_eq!(WmPosition::parse("TL"), Some(WmPosition::TopLeft));
        assert!(WmPosition::parse("sideways").is_none());
    }

    #[test]
    fn anchors_land_in_the_right_corners() {
        let (pw, ph) = (300.0, 400.0);
        assert_eq!(anchor(WmPosition::Center, pw, ph), (150.0, 200.0));
        assert_eq!(anchor(WmPosition::TopRight, pw, ph), (pw - MARGIN, ph - MARGIN));
        assert_eq!(anchor(WmPosition::BottomLeft, pw, ph), (MARGIN, MARGIN));
    }

    #[test]
    fn empty_spec_errors() {
        let mut pdf = doc_with_content(1);
        assert!(apply_watermark(&mut pdf, &WatermarkSpec::default()).is_err());
    }

    #[test]
    fn stamps_text_on_all_pages_and_round_trips() {
        let mut pdf = doc_with_content(3);
        let spec = WatermarkSpec {
            text: Some("DRAFT".into()),
            ..Default::default()
        };
        let n = apply_watermark(&mut pdf, &spec).unwrap();
        assert_eq!(n, 3);
        // every page now contains the DRAFT Tj + the gs reference
        let reloaded = PdfDoc::load_mem(&pdf.to_bytes().unwrap()).unwrap();
        assert_eq!(reloaded.page_count(), 3);
        for pid in reloaded.page_ids() {
            let c = reloaded.document().get_and_decode_page_content(*pid).unwrap();
            assert!(
                c.operations.iter().any(|o| o.operator == "gs"),
                "page carries the watermark ExtGState"
            );
            assert!(
                c.operations.iter().any(|o| o.operator == "Tj"),
                "page carries the watermark text"
            );
        }
    }

    #[test]
    fn page_range_limits_the_stamp() {
        let mut pdf = doc_with_content(4);
        let spec = WatermarkSpec {
            text: Some("X".into()),
            pages: Some(PageSpec::parse("2-3").unwrap()),
            ..Default::default()
        };
        assert_eq!(apply_watermark(&mut pdf, &spec).unwrap(), 2);
        let count_gs = |pdf: &PdfDoc, idx: usize| {
            let pid = pdf.page_ids()[idx];
            pdf.document()
                .get_and_decode_page_content(pid)
                .unwrap()
                .operations
                .iter()
                .filter(|o| o.operator == "gs")
                .count()
        };
        assert_eq!(count_gs(&pdf, 0), 0, "page 1 untouched");
        assert_eq!(count_gs(&pdf, 1), 1, "page 2 stamped");
        assert_eq!(count_gs(&pdf, 3), 0, "page 4 untouched");
    }
}
