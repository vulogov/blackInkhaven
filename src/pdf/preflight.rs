//! 1.3.0 PDF-1 P2 — preflight (RFC §8.6): verify a PDF is print-ready.
//!
//! The highest-value check is **effective image DPI** — catching the
//! 72-dpi screenshot pasted at full page size.  That needs the placed
//! size, so we walk each page's content stream tracking the CTM
//! (`q`/`Q`/`cm`) and, at each image `Do`, divide the pixel size by the
//! placed size.  Plus: font embedding, page-size consistency, blank
//! pages, (image-)colour usage, and the press hazards that silently change
//! on output — overprint, transparency, and spot colours.

use std::collections::{BTreeSet, HashMap, HashSet};

use lopdf::{Dictionary, Document, Object, ObjectId};
use serde::{Deserialize, Serialize};

use super::doc::PdfDoc;

#[derive(Debug, Clone, Copy)]
pub enum PreflightProfile {
    HandBinding { target_dpi: u32 },
    PrintShop { target_dpi: u32 },
    Strict,
}

impl PreflightProfile {
    pub fn target_dpi(&self) -> u32 {
        match self {
            PreflightProfile::HandBinding { target_dpi }
            | PreflightProfile::PrintShop { target_dpi } => *target_dpi,
            PreflightProfile::Strict => 300,
        }
    }
}

/// The `preflight:` HJSON block — house DPI targets, selectable by name.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PreflightConfig {
    pub default_profile: String,
    pub hand_binding_dpi: u32,
    pub print_shop_dpi: u32,
}

impl Default for PreflightConfig {
    fn default() -> Self {
        Self {
            default_profile: "hand_binding".into(),
            hand_binding_dpi: 300,
            print_shop_dpi: 300,
        }
    }
}

impl PreflightConfig {
    /// Resolve a profile name (`hand_binding` | `print_shop` | `strict`)
    /// to a [`PreflightProfile`], applying an optional `--dpi` override.
    pub fn resolve(&self, name: &str, dpi_override: Option<u32>) -> Result<PreflightProfile, String> {
        let p = match name.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "hand_binding" | "handbinding" | "hand" => PreflightProfile::HandBinding {
                target_dpi: dpi_override.unwrap_or(self.hand_binding_dpi),
            },
            "print_shop" | "printshop" | "shop" | "print" => PreflightProfile::PrintShop {
                target_dpi: dpi_override.unwrap_or(self.print_shop_dpi),
            },
            "strict" => match dpi_override {
                Some(d) => PreflightProfile::PrintShop { target_dpi: d },
                None => PreflightProfile::Strict,
            },
            other => {
                return Err(format!(
                    "preflight: unknown profile `{other}` (have: hand_binding, print_shop, strict)"
                ))
            }
        };
        Ok(p)
    }
}

#[derive(Debug, Clone)]
pub struct FontReport {
    pub name: String,
    pub embedded: bool,
}

#[derive(Debug, Clone)]
pub struct ImageReport {
    pub page: usize,
    pub name: String,
    pub pixel_w: u32,
    pub pixel_h: u32,
    pub effective_dpi: u32,
    pub colorspace: String,
}

#[derive(Debug, Clone)]
pub struct PreflightReport {
    pub page_count: usize,
    pub consistent_page_size: bool,
    pub fonts: Vec<FontReport>,
    pub images: Vec<ImageReport>,
    /// 1-based.
    pub blank_pages: Vec<usize>,
    /// 1-based pages carrying a non-grayscale image.
    pub color_pages: Vec<usize>,
    /// 1-based pages with an overprint ExtGState (`/OP` or `/op` true).
    pub overprint_pages: Vec<usize>,
    /// 1-based pages using transparency — constant alpha < 1, a non-Normal
    /// blend mode, a soft mask, or a soft-masked image.
    pub transparency_pages: Vec<usize>,
    /// Spot-colour names found (Separation / DeviceN colorants), deduped.
    pub spot_colors: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn preflight(doc: &PdfDoc, profile: PreflightProfile) -> PreflightReport {
    let inner = doc.document();
    let target = profile.target_dpi() as f32;
    let page_ids = doc.page_ids().to_vec();
    let page_count = page_ids.len();

    // Page-size consistency (tolerance 1 pt).
    let first = doc.page_size(0).map(|r| (r.width(), r.height()));
    let mut consistent = true;
    for i in 1..page_count {
        if let (Some((fw, fh)), Some(r)) = (first, doc.page_size(i)) {
            if (r.width() - fw).abs() > 1.0 || (r.height() - fh).abs() > 1.0 {
                consistent = false;
            }
        }
    }

    // Fonts (object walk, deduped by BaseFont).
    let mut fonts = Vec::new();
    let mut seen = HashSet::new();
    for obj in inner.objects.values() {
        let Some(d) = dict_of(obj) else { continue };
        if !name_is(d, b"Type", b"Font") {
            continue;
        }
        let name = d
            .get(b"BaseFont")
            .ok()
            .and_then(|o| o.as_name().ok())
            .map(|n| String::from_utf8_lossy(n).into_owned())
            .unwrap_or_else(|| "<font>".into());
        if seen.insert(name.clone()) {
            fonts.push(FontReport {
                embedded: font_embedded(inner, d),
                name,
            });
        }
    }

    // Images (with effective DPI) + blank pages + colour pages.
    let mut images = Vec::new();
    let mut blank_pages = Vec::new();
    let mut color_pages = BTreeSet::new();
    for (idx, &pid) in page_ids.iter().enumerate() {
        let page_no = idx + 1;
        let xobjs = page_image_xobjects(inner, pid);
        let mut painted = false;
        if let Ok(content) = inner.get_and_decode_page_content(pid) {
            let mut stack: Vec<[f32; 6]> = Vec::new();
            let mut ctm = [1.0f32, 0.0, 0.0, 1.0, 0.0, 0.0];
            for op in &content.operations {
                match op.operator.as_str() {
                    "q" => stack.push(ctm),
                    "Q" => {
                        if let Some(m) = stack.pop() {
                            ctm = m;
                        }
                    }
                    "cm" => {
                        if let Some(m) = read6(&op.operands) {
                            ctm = mat_mul(m, ctm);
                        }
                    }
                    "Do" => {
                        painted = true;
                        if let Some(name) = op.operands.first().and_then(|o| o.as_name().ok()) {
                            if let Some((pw, ph, cs)) = xobjs.get(name) {
                                let placed_w = (ctm[0] * ctm[0] + ctm[1] * ctm[1]).sqrt();
                                let placed_h = (ctm[2] * ctm[2] + ctm[3] * ctm[3]).sqrt();
                                let dpi_x = dpi(*pw, placed_w);
                                let dpi_y = dpi(*ph, placed_h);
                                let eff = dpi_x.min(dpi_y).round() as u32;
                                if cs != "DeviceGray" {
                                    color_pages.insert(page_no);
                                }
                                images.push(ImageReport {
                                    page: page_no,
                                    name: String::from_utf8_lossy(name).into_owned(),
                                    pixel_w: *pw,
                                    pixel_h: *ph,
                                    effective_dpi: eff,
                                    colorspace: cs.clone(),
                                });
                            }
                        }
                    }
                    "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" | "Tj" | "TJ" | "sh" => {
                        painted = true
                    }
                    _ => {}
                }
            }
        }
        if !painted {
            blank_pages.push(page_no);
        }
    }

    // Print-hazard scan: overprint, transparency, and spot colours — the
    // things that silently change on press (or force a flatten / extra
    // plate). Read from each page's resources.
    let mut overprint_pages = Vec::new();
    let mut transparency_pages = Vec::new();
    let mut spot = BTreeSet::new();
    for (idx, &pid) in page_ids.iter().enumerate() {
        let page_no = idx + 1;
        let Some(res) = page_resources(inner, pid) else {
            continue;
        };
        let (op, tr) = scan_extgstate(inner, res);
        let smask_image = res
            .get(b"XObject")
            .ok()
            .and_then(|o| deref(inner, o))
            .and_then(|o| o.as_dict().ok())
            .map(|xo| {
                xo.iter().any(|(_, v)| {
                    matches!(deref(inner, v), Some(Object::Stream(st))
                        if name_is(&st.dict, b"Subtype", b"Image") && st.dict.has(b"SMask"))
                })
            })
            .unwrap_or(false);
        if op {
            overprint_pages.push(page_no);
        }
        if tr || smask_image {
            transparency_pages.push(page_no);
        }
        scan_spot_colors(inner, res, &mut spot);
    }
    let spot_colors: Vec<String> = spot.into_iter().collect();

    // Warnings.
    let mut warnings = Vec::new();
    if !consistent {
        warnings.push("page sizes are inconsistent".into());
    }
    for f in &fonts {
        if !f.embedded {
            warnings.push(format!("font `{}` is not embedded", f.name));
        }
    }
    for img in &images {
        if (img.effective_dpi as f32) < target {
            warnings.push(format!(
                "page {}: image `{}` at {} dpi (below {} target)",
                img.page, img.name, img.effective_dpi, target as u32
            ));
        }
    }
    for &p in &blank_pages {
        warnings.push(format!("page {p} is blank"));
    }
    if !overprint_pages.is_empty() {
        warnings.push(format!(
            "overprint set on page(s) {overprint_pages:?} — verify this is intentional (knockout vs trap)"
        ));
    }
    if !transparency_pages.is_empty() {
        warnings.push(format!(
            "transparency on page(s) {transparency_pages:?} — flatten before sending to a non-PDF/X workflow"
        ));
    }
    if !spot_colors.is_empty() {
        warnings.push(format!(
            "spot colour(s) {spot_colors:?} — each is a separate plate; convert to process if not intended"
        ));
    }

    PreflightReport {
        page_count,
        consistent_page_size: consistent,
        fonts,
        images,
        blank_pages,
        color_pages: color_pages.into_iter().collect(),
        overprint_pages,
        transparency_pages,
        spot_colors,
        warnings,
    }
}

fn dpi(pixels: u32, placed_pt: f32) -> f32 {
    if placed_pt > 0.01 {
        pixels as f32 / (placed_pt / 72.0)
    } else {
        0.0
    }
}

/// PDF matrix product `m · n` for the `[a b c d e f]` form.
fn mat_mul(m: [f32; 6], n: [f32; 6]) -> [f32; 6] {
    [
        m[0] * n[0] + m[1] * n[2],
        m[0] * n[1] + m[1] * n[3],
        m[2] * n[0] + m[3] * n[2],
        m[2] * n[1] + m[3] * n[3],
        m[4] * n[0] + m[5] * n[2] + n[4],
        m[4] * n[1] + m[5] * n[3] + n[5],
    ]
}

fn read6(ops: &[Object]) -> Option<[f32; 6]> {
    if ops.len() != 6 {
        return None;
    }
    let mut m = [0.0f32; 6];
    for (i, o) in ops.iter().enumerate() {
        // `as_float` so Integer operands (e.g. `1 0 0 1 0 0 cm`) parse too.
        m[i] = o.as_float().ok()?;
    }
    Some(m)
}

fn dict_of(o: &Object) -> Option<&Dictionary> {
    match o {
        Object::Dictionary(d) => Some(d),
        Object::Stream(s) => Some(&s.dict),
        _ => None,
    }
}

fn name_is(d: &Dictionary, key: &[u8], val: &[u8]) -> bool {
    d.get(key).ok().and_then(|o| o.as_name().ok()) == Some(val)
}

fn deref<'a>(doc: &'a Document, o: &'a Object) -> Option<&'a Object> {
    match o {
        Object::Reference(id) => doc.get_object(*id).ok(),
        other => Some(other),
    }
}

fn font_embedded(doc: &Document, font: &Dictionary) -> bool {
    let has_file = |fd: &Dictionary| {
        fd.has(b"FontFile") || fd.has(b"FontFile2") || fd.has(b"FontFile3")
    };
    if let Some(fd) = font
        .get(b"FontDescriptor")
        .ok()
        .and_then(|o| deref(doc, o))
        .and_then(|o| o.as_dict().ok())
    {
        if has_file(fd) {
            return true;
        }
    }
    // Type0 → DescendantFonts[*] → FontDescriptor
    if let Some(arr) = font
        .get(b"DescendantFonts")
        .ok()
        .and_then(|o| deref(doc, o))
        .and_then(|o| o.as_array().ok())
    {
        for d in arr {
            if let Some(df) = deref(doc, d).and_then(|o| o.as_dict().ok()) {
                if let Some(fd) = df
                    .get(b"FontDescriptor")
                    .ok()
                    .and_then(|o| deref(doc, o))
                    .and_then(|o| o.as_dict().ok())
                {
                    if has_file(fd) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// `name → (pixel_w, pixel_h, colorspace)` for the image XObjects in a
/// page's resources.
fn page_image_xobjects(doc: &Document, page_id: ObjectId) -> HashMap<Vec<u8>, (u32, u32, String)> {
    let mut map = HashMap::new();
    let res = page_resources(doc, page_id);
    let Some(res) = res else { return map };
    let Some(xobj) = res
        .get(b"XObject")
        .ok()
        .and_then(|o| deref(doc, o))
        .and_then(|o| o.as_dict().ok())
    else {
        return map;
    };
    for (name, val) in xobj.iter() {
        let Some(Object::Stream(st)) = deref(doc, val) else {
            continue;
        };
        if !name_is(&st.dict, b"Subtype", b"Image") {
            continue;
        }
        // Width/Height are Integers — `as_float` casts them.
        let w = st.dict.get(b"Width").ok().and_then(|o| o.as_float().ok()).unwrap_or(0.0) as u32;
        let h = st.dict.get(b"Height").ok().and_then(|o| o.as_float().ok()).unwrap_or(0.0) as u32;
        let cs = st
            .dict
            .get(b"ColorSpace")
            .ok()
            .and_then(|o| o.as_name().ok())
            .map(|n| String::from_utf8_lossy(n).into_owned())
            .unwrap_or_else(|| "other".into());
        map.insert(name.clone(), (w, h, cs));
    }
    map
}

/// Scan a resource dict's `/ExtGState` graphics states for overprint and
/// transparency. Returns `(overprint, transparency)`.
fn scan_extgstate(doc: &Document, res: &Dictionary) -> (bool, bool) {
    let Some(gs) = res
        .get(b"ExtGState")
        .ok()
        .and_then(|o| deref(doc, o))
        .and_then(|o| o.as_dict().ok())
    else {
        return (false, false);
    };
    let (mut overprint, mut transparency) = (false, false);
    for (_, v) in gs.iter() {
        let Some(g) = deref(doc, v).and_then(|o| o.as_dict().ok()) else {
            continue;
        };
        if matches!(g.get(b"OP"), Ok(Object::Boolean(true)))
            || matches!(g.get(b"op"), Ok(Object::Boolean(true)))
        {
            overprint = true;
        }
        for key in [b"CA".as_ref(), b"ca".as_ref()] {
            if let Some(a) = g.get(key).ok().and_then(|o| o.as_float().ok()) {
                if a < 0.999 {
                    transparency = true;
                }
            }
        }
        // A blend mode other than Normal/Compatible is transparency.
        let bm_name = match g.get(b"BM") {
            Ok(Object::Name(n)) => Some(n.clone()),
            Ok(Object::Array(a)) => a.first().and_then(|o| o.as_name().ok().map(<[u8]>::to_vec)),
            _ => None,
        };
        if let Some(n) = bm_name {
            if n != b"Normal" && n != b"Compatible" {
                transparency = true;
            }
        }
        // A soft mask other than the explicit /None is transparency.
        if let Ok(sm) = g.get(b"SMask") {
            if !matches!(sm, Object::Name(n) if n.as_slice() == b"None") {
                transparency = true;
            }
        }
    }
    (overprint, transparency)
}

/// Collect Separation / DeviceN colorant names from a resource dict's
/// `/ColorSpace` entries into `out`.
fn scan_spot_colors(doc: &Document, res: &Dictionary, out: &mut BTreeSet<String>) {
    let Some(cs) = res
        .get(b"ColorSpace")
        .ok()
        .and_then(|o| deref(doc, o))
        .and_then(|o| o.as_dict().ok())
    else {
        return;
    };
    for (_, v) in cs.iter() {
        let Some(arr) = deref(doc, v).and_then(|o| o.as_array().ok()) else {
            continue;
        };
        let Some(head) = arr.first().and_then(|o| o.as_name().ok()) else {
            continue;
        };
        match head {
            b"Separation" => {
                if let Some(name) = arr.get(1).and_then(|o| o.as_name().ok()) {
                    out.insert(String::from_utf8_lossy(name).into_owned());
                }
            }
            b"DeviceN" => {
                if let Some(names) = arr
                    .get(1)
                    .and_then(|o| deref(doc, o))
                    .and_then(|o| o.as_array().ok())
                {
                    for n in names {
                        if let Ok(nm) = n.as_name() {
                            out.insert(String::from_utf8_lossy(nm).into_owned());
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn page_resources<'a>(doc: &'a Document, page_id: ObjectId) -> Option<&'a Dictionary> {
    let (inline, ids) = doc.get_page_resources(page_id).ok()?;
    inline.or_else(|| ids.first().and_then(|&id| doc.get_dictionary(id).ok()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::test_support::minimal_pdf;
    use crate::pdf::PdfDoc;

    #[test]
    fn matrix_product_matches_pdf_convention() {
        // translate then scale: cm [2 0 0 2 0 0] under [1 0 0 1 10 20]
        let m = mat_mul([2.0, 0.0, 0.0, 2.0, 0.0, 0.0], [1.0, 0.0, 0.0, 1.0, 10.0, 20.0]);
        assert_eq!(m, [2.0, 0.0, 0.0, 2.0, 10.0, 20.0]);
    }

    #[test]
    fn config_resolves_profiles_and_dpi_override() {
        let cfg = PreflightConfig::default();
        assert_eq!(
            cfg.resolve("hand_binding", None).unwrap().target_dpi(),
            300
        );
        // hyphen + override
        assert_eq!(cfg.resolve("print-shop", Some(150)).unwrap().target_dpi(), 150);
        assert!(matches!(
            cfg.resolve("strict", None).unwrap(),
            PreflightProfile::Strict
        ));
        assert!(cfg.resolve("bogus", None).is_err());
    }

    #[test]
    fn minimal_pdf_pages_are_blank() {
        let doc = PdfDoc::load_mem(&minimal_pdf(3, 612.0, 792.0)).unwrap();
        let r = preflight(&doc, PreflightProfile::HandBinding { target_dpi: 300 });
        assert_eq!(r.page_count, 3);
        assert!(r.consistent_page_size);
        assert_eq!(r.blank_pages, vec![1, 2, 3]); // no content
        assert!(r.images.is_empty() && r.fonts.is_empty());
        assert!(r.warnings.iter().any(|w| w.contains("blank")));
    }

    #[test]
    fn flags_overprint_transparency_and_spot_colours() {
        use lopdf::{Dictionary, Object};
        let mut pdf = PdfDoc::load_mem(&minimal_pdf(1, 200.0, 200.0)).unwrap();
        let pid = pdf.page_ids()[0];
        let inner = pdf.document_mut();

        // ExtGState GS0: overprint on + 50% constant alpha.
        let mut gs0 = Dictionary::new();
        gs0.set("OP", Object::Boolean(true));
        gs0.set("ca", Object::Real(0.5));
        let mut egs = Dictionary::new();
        egs.set("GS0", Object::Dictionary(gs0));
        // ColorSpace CS0: a PANTONE Separation (tint fn placeholder).
        let sep = Object::Array(vec![
            Object::Name(b"Separation".to_vec()),
            Object::Name(b"PANTONE_021_C".to_vec()),
            Object::Name(b"DeviceCMYK".to_vec()),
            Object::Null,
        ]);
        let mut cs = Dictionary::new();
        cs.set("CS0", sep);
        let mut res = Dictionary::new();
        res.set("ExtGState", Object::Dictionary(egs));
        res.set("ColorSpace", Object::Dictionary(cs));
        if let Ok(Object::Dictionary(p)) = inner.get_object_mut(pid) {
            p.set("Resources", Object::Dictionary(res));
        }

        let r = preflight(&pdf, PreflightProfile::Strict);
        assert_eq!(r.overprint_pages, vec![1], "overprint detected");
        assert_eq!(r.transparency_pages, vec![1], "ca < 1 → transparency");
        assert_eq!(r.spot_colors, vec!["PANTONE_021_C".to_string()]);
        assert!(r.warnings.iter().any(|w| w.contains("overprint")));
        assert!(r.warnings.iter().any(|w| w.contains("transparency")));
        assert!(r.warnings.iter().any(|w| w.contains("spot colour")));
    }

    #[test]
    fn clean_pdf_has_no_press_hazards() {
        let doc = PdfDoc::load_mem(&minimal_pdf(2, 612.0, 792.0)).unwrap();
        let r = preflight(&doc, PreflightProfile::Strict);
        assert!(r.overprint_pages.is_empty());
        assert!(r.transparency_pages.is_empty());
        assert!(r.spot_colors.is_empty());
    }

    #[test]
    fn low_dpi_image_is_flagged() {
        // A 16×24-px image stretched across a cover region → tiny DPI.
        use crate::pdf::cover::{build_cover, CoverSpec, SpineText};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f.png");
        image::RgbImage::from_pixel(16, 24, image::Rgb([200, 30, 30]))
            .save(&path)
            .unwrap();
        let mut doc = build_cover(&CoverSpec {
            front_width_mm: 152.0,
            front_height_mm: 229.0,
            spine_width_mm: 12.0,
            bleed_mm: 3.0,
            front_image: Some(path),
            image_fit: crate::pdf::cover::ImageFit::Stretch,
            spine_text: SpineText::default(),
            back_text: None,
            barcode: None,
        })
        .unwrap();
        // reload so the content stream is the serialized form
        let reloaded = PdfDoc::load_mem(&doc.to_bytes().unwrap()).unwrap();
        let r = preflight(&reloaded, PreflightProfile::PrintShop { target_dpi: 300 });
        assert_eq!(r.images.len(), 1, "the front image is found");
        assert_eq!((r.images[0].pixel_w, r.images[0].pixel_h), (16, 24), "pixel size read");
        assert!(r.images[0].effective_dpi < 50, "16px stretched → very low dpi");
        assert!(r.color_pages.contains(&1), "RGB image → colour page");
        assert!(r.warnings.iter().any(|w| w.contains("dpi")), "low-dpi warning");
    }
}
