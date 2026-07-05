//! 1.3.0 PDF-1 — units + page geometry.  Pure; no I/O.
//!
//! PDF's native unit is the **point** (1 pt = 1/72 inch).  All sizes in
//! this subsystem are points internally; `mm`/`in` are converted at the
//! edges (HJSON config is in mm, the way print shops talk).  `f32` to
//! match `lopdf::Object::Real` and the RFC's public structs — ample
//! precision at mm scale.

/// Points per inch (PDF's native unit is the point).
pub const PT_PER_IN: f32 = 72.0;
/// Millimetres per inch.
pub const MM_PER_IN: f32 = 25.4;

pub fn mm_to_pt(mm: f32) -> f32 {
    mm / MM_PER_IN * PT_PER_IN
}
pub fn pt_to_mm(pt: f32) -> f32 {
    pt / PT_PER_IN * MM_PER_IN
}
pub fn in_to_pt(inch: f32) -> f32 {
    inch * PT_PER_IN
}
#[allow(dead_code)] // unit conversion, paired with the in→pt helper above
pub fn pt_to_in(pt: f32) -> f32 {
    pt / PT_PER_IN
}

/// A page / sheet size in **points**.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
    /// Build from millimetres (how stock/sheet sizes are specified).
    pub fn from_mm(w_mm: f32, h_mm: f32) -> Self {
        Self::new(mm_to_pt(w_mm), mm_to_pt(h_mm))
    }
    pub fn is_landscape(&self) -> bool {
        self.width > self.height
    }
    /// Wider-than-tall orientation (swap if currently portrait).
    pub fn landscape(self) -> Self {
        if self.is_landscape() {
            self
        } else {
            Self::new(self.height, self.width)
        }
    }
    /// Taller-than-wide orientation (swap if currently landscape).
    pub fn portrait(self) -> Self {
        if self.is_landscape() {
            Self::new(self.height, self.width)
        } else {
            self
        }
    }
}

/// A rectangle in **points**, in PDF `MediaBox` order (lower-left origin,
/// `[x0 y0 x1 y1]`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl Rect {
    pub const fn new(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        Self { x0, y0, x1, y1 }
    }
    /// A rect at the origin with the given size.
    pub fn from_size(s: Size) -> Self {
        Self::new(0.0, 0.0, s.width, s.height)
    }
    pub fn width(&self) -> f32 {
        self.x1 - self.x0
    }
    pub fn height(&self) -> f32 {
        self.y1 - self.y0
    }
    pub fn size(&self) -> Size {
        Size::new(self.width(), self.height())
    }
    /// PDF `MediaBox` array `[x0 y0 x1 y1]`.
    #[allow(dead_code)] // imposition geometry surface (PDF-1 P2 sheet placement)
    pub fn to_mediabox(&self) -> [f32; 4] {
        [self.x0, self.y0, self.x1, self.y1]
    }
    /// Parse a `MediaBox`, normalising so `x0 <= x1` and `y0 <= y1`
    /// (PDFs in the wild sometimes store corners in either order).
    pub fn from_mediabox(b: [f32; 4]) -> Self {
        Self::new(
            b[0].min(b[2]),
            b[1].min(b[3]),
            b[0].max(b[2]),
            b[1].max(b[3]),
        )
    }
}

/// Named page / sheet sizes (case-insensitive).  ISO A/B series, US
/// office sizes, and common trade-book trims.  Custom sizes come from
/// HJSON as `{ width_mm, height_mm }` (handled by the config layer, not
/// here).
pub fn page_size(name: &str) -> Option<Size> {
    let s = match name.trim().to_ascii_uppercase().as_str() {
        "A3" => Size::from_mm(297.0, 420.0),
        "A4" => Size::from_mm(210.0, 297.0),
        "A5" => Size::from_mm(148.0, 210.0),
        "A6" => Size::from_mm(105.0, 148.0),
        "B5" => Size::from_mm(176.0, 250.0),
        "LETTER" => Size::new(in_to_pt(8.5), in_to_pt(11.0)),
        "LEGAL" => Size::new(in_to_pt(8.5), in_to_pt(14.0)),
        "TABLOID" => Size::new(in_to_pt(11.0), in_to_pt(17.0)),
        // common US trade-book trims
        "TRADE_6X9" | "6X9" => Size::new(in_to_pt(6.0), in_to_pt(9.0)),
        "DIGEST_5.5X8.5" | "5.5X8.5" => Size::new(in_to_pt(5.5), in_to_pt(8.5)),
        "POCKET_4.25X6.87" => Size::new(in_to_pt(4.25), in_to_pt(6.87)),
        _ => return None,
    };
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) {
        assert!((a - b).abs() < 0.01, "{a} !≈ {b}");
    }

    #[test]
    fn unit_conversions_round_trip() {
        approx(mm_to_pt(25.4), 72.0); // 1 inch
        approx(pt_to_mm(72.0), 25.4);
        approx(in_to_pt(1.0), 72.0);
        approx(pt_to_in(72.0), 1.0);
        approx(pt_to_mm(mm_to_pt(123.4)), 123.4);
    }

    #[test]
    fn a4_and_letter_in_points() {
        let a4 = page_size("a4").unwrap(); // case-insensitive
        approx(a4.width, 595.276);
        approx(a4.height, 841.890);
        let letter = page_size("Letter").unwrap();
        approx(letter.width, 612.0);
        approx(letter.height, 792.0);
        assert!(page_size("nope").is_none());
    }

    #[test]
    fn orientation_swaps() {
        let a4 = page_size("A4").unwrap();
        assert!(!a4.is_landscape());
        let land = a4.landscape();
        assert!(land.is_landscape());
        approx(land.width, a4.height);
        approx(land.portrait().width, a4.width); // back to portrait
        assert_eq!(a4.landscape().landscape(), a4.landscape()); // idempotent
    }

    #[test]
    fn rect_size_and_mediabox() {
        let r = Rect::from_size(Size::new(612.0, 792.0));
        approx(r.width(), 612.0);
        approx(r.height(), 792.0);
        assert_eq!(r.to_mediabox(), [0.0, 0.0, 612.0, 792.0]);
        // reversed corners normalise
        let n = Rect::from_mediabox([612.0, 792.0, 0.0, 0.0]);
        assert_eq!(n, Rect::new(0.0, 0.0, 612.0, 792.0));
        approx(n.width(), 612.0);
    }
}
