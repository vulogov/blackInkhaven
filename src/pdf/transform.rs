//! 1.3.0 PDF-1 P3 — grayscale + optimize (RFC §8.7).
//!
//! **grayscale** desaturates without re-rendering: we *neutralize* the
//! colour operands in every page's content stream — `r g b rg` becomes
//! `y y y rg` (luminance `y`), `c m y k k` becomes `0 0 0 (1-y) k` — so
//! the operator and its colour space are untouched and the output stays
//! structurally valid, just neutral.  Raster images are converted
//! DeviceRGB/CMYK → DeviceGray.  Best-effort: JPEG (`DCTDecode`) and
//! exotic colour spaces are left as-is (documented).
//!
//! **optimize** prunes orphan objects and Flate-compresses every
//! uncompressed stream — a lossless size pass after imposition / merge.

use lopdf::content::Content;
use lopdf::{Object, Stream};

use super::doc::PdfDoc;
use super::{Error, Result};

/// Rec. 601 luma of an sRGB triple (components in 0..=1).
fn luma(r: f32, g: f32, b: f32) -> f32 {
    (0.299 * r + 0.587 * g + 0.114 * b).clamp(0.0, 1.0)
}

/// Naive CMYK → RGB, then luma.
fn cmyk_luma(c: f32, m: f32, y: f32, k: f32) -> f32 {
    let r = (1.0 - c) * (1.0 - k);
    let g = (1.0 - m) * (1.0 - k);
    let b = (1.0 - y) * (1.0 - k);
    luma(r, g, b)
}

fn nums(operands: &[Object]) -> Option<Vec<f32>> {
    // `as_float` (not `as_f32`) so Integer operands like `1 0 0 rg` parse.
    operands.iter().map(|o| o.as_float().ok()).collect()
}

/// Neutralize one colour-setting operation's operands.  Returns true if
/// it changed anything.  The operator (and thus the selected colour
/// space) is preserved — only the operand values move onto the gray
/// diagonal, which renders identically in mono.
fn neutralize(op: &mut lopdf::content::Operation) -> bool {
    let gray = |operands: &[Object]| -> Option<Vec<Object>> {
        match op.operator.as_str() {
            // device RGB fill/stroke → equal components
            "rg" | "RG" => {
                let v = nums(operands)?;
                if v.len() != 3 {
                    return None;
                }
                let y = luma(v[0], v[1], v[2]);
                Some(vec![Object::Real(y), Object::Real(y), Object::Real(y)])
            }
            // device CMYK fill/stroke → black-only channel
            "k" | "K" => {
                let v = nums(operands)?;
                if v.len() != 4 {
                    return None;
                }
                let y = cmyk_luma(v[0], v[1], v[2], v[3]);
                Some(vec![
                    Object::Real(0.0),
                    Object::Real(0.0),
                    Object::Real(0.0),
                    Object::Real(1.0 - y),
                ])
            }
            // generic colour-space set: convert by arity, only when every
            // operand is numeric (skip pattern/`scn` with a trailing name).
            "sc" | "scn" | "SC" | "SCN" => {
                let v = nums(operands)?;
                match v.len() {
                    3 => {
                        let y = luma(v[0], v[1], v[2]);
                        Some(vec![Object::Real(y), Object::Real(y), Object::Real(y)])
                    }
                    4 => {
                        let y = cmyk_luma(v[0], v[1], v[2], v[3]);
                        Some(vec![
                            Object::Real(0.0),
                            Object::Real(0.0),
                            Object::Real(0.0),
                            Object::Real(1.0 - y),
                        ])
                    }
                    _ => None, // 1 = already gray; anything else: leave
                }
            }
            _ => None,
        }
    };
    if let Some(replacement) = gray(&op.operands) {
        op.operands = replacement;
        true
    } else {
        false
    }
}

/// Convert a document to grayscale in place (best-effort — see module
/// docs).  Returns the number of raster images converted.
pub fn to_grayscale(doc: &mut PdfDoc) -> Result<usize> {
    let page_ids = doc.page_ids().to_vec();
    let inner = doc.document_mut();

    // 1) neutralize colour operators in each page's content stream.
    for pid in &page_ids {
        let Ok(content) = inner.get_and_decode_page_content(*pid) else {
            continue;
        };
        let mut ops = content.operations;
        let mut changed = false;
        for op in &mut ops {
            if neutralize(op) {
                changed = true;
            }
        }
        if changed {
            let encoded = Content { operations: ops }
                .encode()
                .map_err(|e| Error::Other(format!("grayscale: re-encode content: {e}")))?;
            inner
                .change_page_content(*pid, encoded)
                .map_err(Error::Lopdf)?;
        }
    }

    // 2) convert DeviceRGB / DeviceCMYK image XObjects to DeviceGray.
    let mut converted = 0usize;
    for obj in inner.objects.values_mut() {
        if let Object::Stream(st) = obj {
            if grayscale_image(st) {
                converted += 1;
            }
        }
    }
    Ok(converted)
}

/// Returns true if it converted `st` (an 8-bpc DeviceRGB/CMYK image) to
/// DeviceGray.  Leaves anything it can't safely handle untouched.
fn grayscale_image(st: &mut Stream) -> bool {
    let is_image = st.dict.get(b"Subtype").ok().and_then(|o| o.as_name().ok()) == Some(&b"Image"[..]);
    if !is_image {
        return false;
    }
    if st.dict.get(b"BitsPerComponent").ok().and_then(|o| o.as_i64().ok()) != Some(8) {
        return false;
    }
    if st.dict.has(b"ImageMask") || st.dict.has(b"SMask") {
        // (the SMask itself, when reached as its own object, is already
        // DeviceGray and is skipped by the colour-space check.)
    }
    let comps = match st.dict.get(b"ColorSpace").ok().and_then(|o| o.as_name().ok()) {
        Some(b"DeviceRGB") => 3,
        Some(b"DeviceCMYK") => 4,
        _ => return false, // DeviceGray (noop), Indexed, ICCBased, … skip
    };
    let Ok(data) = st.decompressed_content() else {
        return false; // e.g. DCTDecode (JPEG) — leave as-is
    };
    if data.len() % comps != 0 {
        return false;
    }
    let mut gray = Vec::with_capacity(data.len() / comps);
    for px in data.chunks_exact(comps) {
        let y = if comps == 3 {
            luma(px[0] as f32 / 255.0, px[1] as f32 / 255.0, px[2] as f32 / 255.0)
        } else {
            cmyk_luma(
                px[0] as f32 / 255.0,
                px[1] as f32 / 255.0,
                px[2] as f32 / 255.0,
                px[3] as f32 / 255.0,
            )
        };
        gray.push((y * 255.0).round() as u8);
    }
    st.dict.set("ColorSpace", Object::Name(b"DeviceGray".to_vec()));
    st.set_plain_content(gray); // drops the old /Filter
    let _ = st.compress(); // re-flate
    true
}

/// What an [`optimize`] pass did.
#[derive(Debug, Clone, Copy)]
pub struct OptimizeReport {
    pub objects_before: usize,
    pub objects_after: usize,
    pub pruned: usize,
}

/// Lossless slim-down: prune orphan objects, Flate-compress every
/// uncompressed stream.  Safe to run after imposition / merge.
pub fn optimize(doc: &mut PdfDoc) -> Result<OptimizeReport> {
    let inner = doc.document_mut();
    let before = inner.objects.len();
    let pruned = inner.prune_objects().len();
    inner.compress();
    Ok(OptimizeReport {
        objects_before: before,
        objects_after: inner.objects.len(),
        pruned,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::test_support::minimal_pdf;
    use crate::pdf::PdfDoc;

    #[test]
    fn luma_of_pure_colors() {
        assert!((luma(1.0, 1.0, 1.0) - 1.0).abs() < 1e-6);
        assert!(luma(0.0, 0.0, 0.0) < 1e-6);
        // green is the brightest primary under Rec.601
        assert!(luma(0.0, 1.0, 0.0) > luma(1.0, 0.0, 0.0));
        assert!(luma(1.0, 0.0, 0.0) > luma(0.0, 0.0, 1.0));
    }

    #[test]
    fn neutralize_rgb_fill_goes_diagonal() {
        let mut op = lopdf::content::Operation::new(
            "rg",
            vec![Object::Real(1.0), Object::Real(0.0), Object::Real(0.0)],
        );
        assert!(neutralize(&mut op));
        let v = nums(&op.operands).unwrap();
        assert_eq!(v.len(), 3);
        assert!((v[0] - v[1]).abs() < 1e-6 && (v[1] - v[2]).abs() < 1e-6, "neutral");
        assert!((v[0] - 0.299).abs() < 1e-3, "red luma");
        assert_eq!(op.operator, "rg", "operator (colour space) preserved");
    }

    #[test]
    fn neutralize_cmyk_becomes_black_only() {
        let mut op = lopdf::content::Operation::new(
            "k",
            vec![
                Object::Real(0.0),
                Object::Real(1.0),
                Object::Real(1.0),
                Object::Real(0.0),
            ],
        );
        assert!(neutralize(&mut op));
        let v = nums(&op.operands).unwrap();
        assert_eq!(v.len(), 4);
        assert!(v[0] == 0.0 && v[1] == 0.0 && v[2] == 0.0);
        assert!(v[3] > 0.0, "carries some black");
    }

    #[test]
    fn neutralize_leaves_non_color_ops() {
        let mut op = lopdf::content::Operation::new("Tj", vec![Object::string_literal("x")]);
        assert!(!neutralize(&mut op));
    }

    #[test]
    fn grayscale_rewrites_a_colored_page() {
        // Build a 1-page doc whose content sets a red fill + draws a box.
        let mut pdf = PdfDoc::load_mem(&minimal_pdf(1, 200.0, 200.0)).unwrap();
        let pid = pdf.page_ids()[0];
        let inner = pdf.document_mut();
        let cid = inner.add_object(Stream::new(
            lopdf::Dictionary::new(),
            b"1 0 0 rg 10 10 50 50 re f\n".to_vec(),
        ));
        if let Ok(Object::Dictionary(p)) = inner.get_object_mut(pid) {
            p.set("Contents", cid);
        }
        to_grayscale(&mut pdf).unwrap();
        let decoded = pdf.document().get_and_decode_page_content(pid).unwrap();
        let rg = decoded
            .operations
            .iter()
            .find(|o| o.operator == "rg")
            .expect("rg op survives");
        let v = nums(&rg.operands).unwrap();
        assert!((v[0] - v[1]).abs() < 1e-6 && (v[1] - v[2]).abs() < 1e-6, "now neutral");
    }

    #[test]
    fn optimize_prunes_and_reports() {
        let mut pdf = PdfDoc::load_mem(&minimal_pdf(2, 200.0, 200.0)).unwrap();
        // add an orphan object that nothing references
        let _orphan = pdf
            .document_mut()
            .add_object(Object::string_literal("orphan"));
        let r = optimize(&mut pdf).unwrap();
        assert!(r.pruned >= 1, "the orphan is pruned ({} pruned)", r.pruned);
        assert!(r.objects_after <= r.objects_before);
        // still loads + same page count
        assert_eq!(
            PdfDoc::load_mem(&pdf.to_bytes().unwrap()).unwrap().page_count(),
            2
        );
    }
}
