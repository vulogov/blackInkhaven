//! Spatial templates for composed glyphs (LANG-1 P5.6).
//!
//! Some writing systems build a unit out of several component glyphs arranged
//! in 2D: a Korean syllable square (lead + vowel + tail), an Egyptian quadrat,
//! a stacked Brahmic cluster. A `SpatialTemplate` names the cells of such a
//! unit — each a normalized rectangle in the em square — and a component glyph
//! is placed into each cell. The SAME template feeds two binding times: bake
//! the arrangement into one precomposed glyph at **font-build** (this phase),
//! or position the components at **layout** time in Typst (a later phase).
//!
//! Cell coordinates are SVG-style: `(0,0)` is the top-left of the em, `x`/`y`
//! grow right/down, all in `0.0..=1.0`.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct SpatialTemplate {
    pub name: String,
    pub cells: Vec<Cell>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Cell {
    /// Role this cell fills (e.g. `lead`, `vowel`, `tail`, `tl`).
    pub slot: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl SpatialTemplate {
    pub fn slots(&self) -> Vec<&str> {
        self.cells.iter().map(|c| c.slot.as_str()).collect()
    }
}

/// Names of the built-in templates (config templates of the same name win).
pub const BUILTIN_TEMPLATES: &[&str] = &["lr", "tb", "quad", "stack3"];

/// A generic built-in arrangement, or `None` for an unknown name.
pub fn builtin_template(name: &str) -> Option<SpatialTemplate> {
    let cell = |slot: &str, x: f64, y: f64, w: f64, h: f64| Cell {
        slot: slot.into(),
        x,
        y,
        w,
        h,
    };
    let t = |name: &str, cells: Vec<Cell>| SpatialTemplate { name: name.into(), cells };
    Some(match name {
        // Left / right halves.
        "lr" => t("lr", vec![cell("left", 0.0, 0.0, 0.5, 1.0), cell("right", 0.5, 0.0, 0.5, 1.0)]),
        // Top / bottom halves.
        "tb" => t("tb", vec![cell("top", 0.0, 0.0, 1.0, 0.5), cell("bottom", 0.0, 0.5, 1.0, 0.5)]),
        // A 2×2 quadrat.
        "quad" => t(
            "quad",
            vec![
                cell("tl", 0.0, 0.0, 0.5, 0.5),
                cell("tr", 0.5, 0.0, 0.5, 0.5),
                cell("bl", 0.0, 0.5, 0.5, 0.5),
                cell("br", 0.5, 0.5, 0.5, 0.5),
            ],
        ),
        // Three stacked rows.
        "stack3" => t(
            "stack3",
            vec![
                cell("top", 0.0, 0.0, 1.0, 1.0 / 3.0),
                cell("mid", 0.0, 1.0 / 3.0, 1.0, 1.0 / 3.0),
                cell("bottom", 0.0, 2.0 / 3.0, 1.0, 1.0 / 3.0),
            ],
        ),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_resolve_and_tile() {
        for name in BUILTIN_TEMPLATES {
            let t = builtin_template(name).unwrap();
            assert_eq!(&t.name, name);
            assert!(!t.cells.is_empty());
            // every cell fits inside the em square
            for c in &t.cells {
                assert!(c.x >= 0.0 && c.y >= 0.0 && c.x + c.w <= 1.0001 && c.y + c.h <= 1.0001);
            }
        }
        assert!(builtin_template("nope").is_none());
    }

    #[test]
    fn lr_has_left_and_right() {
        let t = builtin_template("lr").unwrap();
        assert_eq!(t.slots(), vec!["left", "right"]);
    }
}
