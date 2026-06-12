//! 1.3.0 PDF-1 P1 — printer marks (RFC §8.2.5).  Pure: each mark is a
//! string of PDF content-stream operators appended to a sheet side.  No
//! SVG — vector primitives directly (`m`/`l`/`c`/`re`/`S`/`f`).

use super::super::geometry::{mm_to_pt, Size};

/// Which marks to draw.
#[derive(Debug, Clone, Copy)]
pub struct MarkConfig {
    pub crop: bool,
    pub fold: bool,
    pub registration: bool,
    /// Collation bars: a descending staircase across signatures.
    pub spine_marker: bool,
    pub signature_number: bool,
    pub color_bar: bool,
}

impl Default for MarkConfig {
    fn default() -> Self {
        Self {
            crop: true,
            fold: true,
            registration: true,
            spine_marker: true,
            signature_number: true,
            color_bar: false,
        }
    }
}

impl MarkConfig {
    /// True if any mark needs the Helvetica text font (`/F1`).
    pub fn needs_font(&self) -> bool {
        self.signature_number
    }
}

/// The imposed block on a sheet, for positioning marks (all pt).
#[derive(Debug, Clone, Copy)]
pub struct MarkGeometry {
    pub sheet: Size,
    pub page: Size,
    pub columns: usize,
    pub block_x0: f32,
    pub block_y0: f32,
    pub crop_offset_mm: f32,
    pub fold_len_mm: f32,
}

impl MarkGeometry {
    fn block_w(&self) -> f32 {
        self.columns as f32 * self.page.width
    }
    fn block_x1(&self) -> f32 {
        self.block_x0 + self.block_w()
    }
    fn block_y1(&self) -> f32 {
        self.block_y0 + self.page.height
    }
    /// Spine fold x (centre of a 2-up block).
    fn fold_x(&self) -> f32 {
        self.block_x0 + self.page.width
    }
}

/// y of a collation bar's bottom edge — descends monotonically with the
/// signature index, so the gathered spine shows a staircase.
pub(crate) fn spine_bar_y(block_y0: f32, block_h: f32, bar_h: f32, sig: usize, total: usize) -> f32 {
    let total = total.max(1) as f32;
    let span = (block_h - bar_h).max(0.0);
    block_y0 + block_h - bar_h - (sig as f32 / total) * span
}

/// PDF content ops for the enabled marks on one sheet side.  `is_folded`
/// gates fold / spine / signature (a 1-up stab/concertina sheet has no
/// spine fold).  `signature` is 0-based.
pub fn marks_ops(
    cfg: &MarkConfig,
    g: &MarkGeometry,
    signature: usize,
    total_sigs: usize,
    is_folded: bool,
) -> String {
    let mut s = String::new();
    s.push_str("q 0 G 0 g 0.25 w\n"); // black stroke + fill, 0.25 pt
    if cfg.crop {
        crop(&mut s, g);
    }
    if cfg.fold && is_folded {
        fold(&mut s, g);
    }
    if cfg.registration {
        registration(&mut s, g);
    }
    if cfg.spine_marker && is_folded {
        spine(&mut s, g, signature, total_sigs);
    }
    if cfg.signature_number && is_folded {
        numeral(&mut s, g, signature);
    }
    if cfg.color_bar {
        color_bar(&mut s, g);
    }
    s.push_str("Q\n");
    s
}

fn line(s: &mut String, x1: f32, y1: f32, x2: f32, y2: f32) {
    s.push_str(&format!("{x1:.3} {y1:.3} m {x2:.3} {y2:.3} l S\n"));
}
fn rect_fill(s: &mut String, x: f32, y: f32, w: f32, h: f32) {
    s.push_str(&format!("{x:.3} {y:.3} {w:.3} {h:.3} re f\n"));
}

/// Four corner L-marks at the block's trim box, offset outside it.
fn crop(s: &mut String, g: &MarkGeometry) {
    let off = mm_to_pt(g.crop_offset_mm);
    let len = mm_to_pt(5.0);
    let (x0, y0, x1, y1) = (g.block_x0, g.block_y0, g.block_x1(), g.block_y1());
    // bottom-left
    line(s, x0 - off - len, y0, x0 - off, y0);
    line(s, x0, y0 - off - len, x0, y0 - off);
    // bottom-right
    line(s, x1 + off, y0, x1 + off + len, y0);
    line(s, x1, y0 - off - len, x1, y0 - off);
    // top-left
    line(s, x0 - off - len, y1, x0 - off, y1);
    line(s, x0, y1 + off, x0, y1 + off + len);
    // top-right
    line(s, x1 + off, y1, x1 + off + len, y1);
    line(s, x1, y1 + off, x1, y1 + off + len);
}

/// Dashed segments across the spine fold at the block top + bottom.
fn fold(s: &mut String, g: &MarkGeometry) {
    let fx = g.fold_x();
    let len = mm_to_pt(g.fold_len_mm);
    s.push_str("[2 2] 0 d\n");
    line(s, fx, g.block_y1(), fx, g.block_y1() + len);
    line(s, fx, g.block_y0, fx, g.block_y0 - len);
    s.push_str("[] 0 d\n");
}

/// Printer's cross (crosshair + circle) centred above + below the block.
fn registration(s: &mut String, g: &MarkGeometry) {
    let cx = g.sheet.width / 2.0;
    let r = mm_to_pt(3.0);
    let gap = mm_to_pt(4.0);
    for cy in [g.block_y1() + gap + r, g.block_y0 - gap - r] {
        line(s, cx - r, cy, cx + r, cy);
        line(s, cx, cy - r, cx, cy + r);
        circle(s, cx, cy, r * 0.6);
    }
}

/// A stroked circle as four cubic Bézier arcs (`κ = 0.5523`).
fn circle(s: &mut String, cx: f32, cy: f32, r: f32) {
    let k = 0.5523 * r;
    s.push_str(&format!("{:.3} {:.3} m\n", cx + r, cy));
    s.push_str(&format!(
        "{:.3} {:.3} {:.3} {:.3} {:.3} {:.3} c\n",
        cx + r, cy + k, cx + k, cy + r, cx, cy + r
    ));
    s.push_str(&format!(
        "{:.3} {:.3} {:.3} {:.3} {:.3} {:.3} c\n",
        cx - k, cy + r, cx - r, cy + k, cx - r, cy
    ));
    s.push_str(&format!(
        "{:.3} {:.3} {:.3} {:.3} {:.3} {:.3} c\n",
        cx - r, cy - k, cx - k, cy - r, cx, cy - r
    ));
    s.push_str(&format!(
        "{:.3} {:.3} {:.3} {:.3} {:.3} {:.3} c\n",
        cx + k, cy - r, cx + r, cy - k, cx + r, cy
    ));
    s.push_str("S\n");
}

/// Collation bar across the spine fold, descending with the signature.
fn spine(s: &mut String, g: &MarkGeometry, sig: usize, total: usize) {
    let bar_w = mm_to_pt(3.0);
    let bar_h = mm_to_pt(6.0);
    let y = spine_bar_y(g.block_y0, g.page.height, bar_h, sig, total);
    rect_fill(s, g.fold_x() - bar_w / 2.0, y, bar_w, bar_h);
}

/// The signature number (1-based) printed near the spine fold.  Uses the
/// `/F1` Helvetica resource the emitter adds when this mark is on.
fn numeral(s: &mut String, g: &MarkGeometry, sig: usize) {
    s.push_str(&format!(
        "BT /F1 8 Tf {:.3} {:.3} Td ({}) Tj ET\n",
        g.fold_x() + mm_to_pt(2.0),
        g.block_y0 + mm_to_pt(2.0),
        sig + 1
    ));
}

/// A grayscale calibration strip below the block (off by default).
fn color_bar(s: &mut String, g: &MarkGeometry) {
    let n = 5usize;
    let pw = mm_to_pt(6.0);
    let ph = mm_to_pt(4.0);
    let y = g.block_y0 - mm_to_pt(8.0);
    for i in 0..n {
        let gray = i as f32 / (n - 1) as f32;
        s.push_str(&format!("{gray:.2} g\n"));
        rect_fill(s, g.block_x0 + i as f32 * (pw + 2.0), y, pw, ph);
    }
    s.push_str("0 g\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geom() -> MarkGeometry {
        MarkGeometry {
            sheet: Size::new(600.0, 400.0),
            page: Size::new(280.0, 380.0),
            columns: 2,
            block_x0: 20.0,
            block_y0: 10.0,
            crop_offset_mm: 5.0,
            fold_len_mm: 8.0,
        }
    }

    fn only(crop: bool, fold: bool, reg: bool, spine: bool, sig: bool, bar: bool) -> MarkConfig {
        MarkConfig {
            crop,
            fold,
            registration: reg,
            spine_marker: spine,
            signature_number: sig,
            color_bar: bar,
        }
    }

    #[test]
    fn crop_draws_eight_segments() {
        let s = marks_ops(&only(true, false, false, false, false, false), &geom(), 0, 1, true);
        // 4 corners × 2 L-segments = 8 stroked lines
        assert_eq!(s.matches(" l S").count(), 8);
    }

    #[test]
    fn fold_is_dashed_and_folded_only() {
        let folded = marks_ops(&only(false, true, false, false, false, false), &geom(), 0, 1, true);
        assert!(folded.contains("[2 2] 0 d"));
        assert_eq!(folded.matches(" l S").count(), 2);
        // 1-up sheet has no fold marks
        let flat = marks_ops(&only(false, true, false, false, false, false), &geom(), 0, 1, false);
        assert!(!flat.contains("[2 2] 0 d"));
    }

    #[test]
    fn registration_has_crosshair_and_circle() {
        let s = marks_ops(&only(false, false, true, false, false, false), &geom(), 0, 1, true);
        assert!(s.matches(" c\n").count() >= 8); // 2 crosses × 4 bézier arcs
        assert!(s.matches(" l S").count() >= 4); // 2 crosses × 2 lines
    }

    #[test]
    fn spine_bar_staircase_is_monotonic() {
        let y0 = spine_bar_y(10.0, 380.0, 17.0, 0, 4);
        let y1 = spine_bar_y(10.0, 380.0, 17.0, 1, 4);
        let y2 = spine_bar_y(10.0, 380.0, 17.0, 2, 4);
        assert!(y0 > y1 && y1 > y2, "collation bar must descend with signature");
        let s = marks_ops(&only(false, false, false, true, false, false), &geom(), 1, 4, true);
        assert!(s.contains(" re f"));
    }

    #[test]
    fn signature_numeral_emits_text() {
        let s = marks_ops(&only(false, false, false, false, true, false), &geom(), 2, 4, true);
        assert!(s.contains("/F1 8 Tf"));
        assert!(s.contains("(3) Tj")); // 0-based sig 2 → printed 3
        assert!(only(false, false, false, false, true, false).needs_font());
    }

    #[test]
    fn default_config_omits_color_bar() {
        let d = MarkConfig::default();
        assert!(d.crop && d.fold && d.registration && d.spine_marker && d.signature_number);
        assert!(!d.color_bar);
    }
}
