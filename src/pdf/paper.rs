//! 1.3.0 PDF-1 — paper-stock presets + caliper (thickness).  Pure.
//!
//! Thickness (caliper) per single sheet drives creep compensation
//! (`impose::creep`) and spine-width calculation (`cover`).  The table is
//! the RFC §8.2.4 starting set; HJSON can override a stock's thickness or
//! add new ones via `thickness_mm_override` / a custom entry.

/// A paper stock: a name and its single-sheet caliper in millimetres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaperStock {
    pub name: &'static str,
    /// Single-sheet thickness (caliper), mm.
    pub thickness_mm: f32,
}

impl PaperStock {
    /// Construct an ad-hoc stock (e.g. from a `thickness_mm_override`).
    pub const fn custom(name: &'static str, thickness_mm: f32) -> Self {
        Self { name, thickness_mm }
    }

    /// Extra spine allowance for a perfect-bound cover wrap — the glue
    /// layer + score/hinge that a flat stack calculation misses, in mm.
    /// A small fixed allowance; exotic binders override in config.
    pub fn binding_compensation_mm(&self) -> f32 {
        1.0
    }
}

/// The preset table (RFC §8.2.4).  Names are lowercase; lookup is
/// case-insensitive.
const STOCKS: &[PaperStock] = &[
    PaperStock::custom("bible_70gsm", 0.060),
    PaperStock::custom("uncoated_70gsm", 0.085),
    PaperStock::custom("uncoated_80gsm", 0.100),
    PaperStock::custom("uncoated_90gsm", 0.115),
    PaperStock::custom("uncoated_100gsm", 0.130),
    PaperStock::custom("uncoated_120gsm", 0.160),
    PaperStock::custom("coated_matte_80gsm", 0.080),
    PaperStock::custom("coated_matte_100gsm", 0.105),
    PaperStock::custom("coated_gloss_100gsm", 0.100),
    PaperStock::custom("cover_250gsm", 0.300),
    PaperStock::custom("cover_300gsm", 0.360),
];

/// Default interior stock — standard novel paper.
pub const DEFAULT_INTERIOR: &str = "uncoated_80gsm";
/// Default cover stock.
pub const DEFAULT_COVER: &str = "cover_250gsm";

/// Look up a preset stock by name (case-insensitive).
pub fn paper_stock(name: &str) -> Option<PaperStock> {
    let n = name.trim().to_ascii_lowercase();
    STOCKS.iter().copied().find(|s| s.name == n)
}

/// All preset names (for `--help` / config error messages).
pub fn stock_names() -> impl Iterator<Item = &'static str> {
    STOCKS.iter().map(|s| s.name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_is_case_insensitive_and_trims() {
        assert_eq!(paper_stock("uncoated_80gsm").unwrap().thickness_mm, 0.100);
        assert_eq!(paper_stock("  Uncoated_80GSM  ").unwrap().thickness_mm, 0.100);
        assert!(paper_stock("papyrus").is_none());
    }

    #[test]
    fn defaults_exist_and_are_ordered() {
        let interior = paper_stock(DEFAULT_INTERIOR).unwrap();
        let cover = paper_stock(DEFAULT_COVER).unwrap();
        // cover stock is much thicker than interior — sanity on the table.
        assert!(cover.thickness_mm > interior.thickness_mm * 2.0);
        assert!(cover.binding_compensation_mm() > 0.0);
    }

    #[test]
    fn preset_names_are_unique() {
        let mut names: Vec<&str> = stock_names().collect();
        let n = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), n, "duplicate stock name in the preset table");
    }

    #[test]
    fn custom_override() {
        let s = PaperStock::custom("exotic", 0.42);
        assert_eq!(s.thickness_mm, 0.42);
    }
}
