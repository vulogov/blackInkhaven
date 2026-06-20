//! Glyph composition (LANG-1 P5.6).
//!
//! Place component glyph SVGs into the cells of a [`SpatialTemplate`] and emit
//! ONE combined SVG — a precomposed block (a syllable square, a quadrat). Each
//! component is wrapped in a `<g transform>` that scales its viewBox into the
//! cell rectangle; `font::svg_to_contours` folds those group transforms into
//! each path's absolute transform, so the composite flows through the existing
//! preflight → UFO → TTF pipeline unchanged (a block is just a glyph with more
//! contours). Pure + deterministic.

use std::collections::BTreeMap;

use resvg::usvg;

use crate::conlang::types::spatial::SpatialTemplate;

/// The composite's coordinate space (cells are normalized into this square).
const COMPOSE_UPM: f64 = 1000.0;

/// Compose `components` (slot → component SVG) into a single block SVG per the
/// template. Every cell's slot must be present in `components`.
pub fn compose_block(
    template: &SpatialTemplate,
    components: &BTreeMap<String, String>,
) -> Result<String, String> {
    if template.cells.is_empty() {
        return Err(format!("template `{}` has no cells", template.name));
    }
    let mut body = String::new();
    for cell in &template.cells {
        let svg = components
            .get(&cell.slot)
            .ok_or_else(|| format!("no component for slot `{}`", cell.slot))?;
        let tree = usvg::Tree::from_str(svg, &usvg::Options::default())
            .map_err(|e| format!("slot `{}`: {e}", cell.slot))?;
        let (vw, vh) = (tree.size().width() as f64, tree.size().height() as f64);
        if vw <= 0.0 || vh <= 0.0 {
            return Err(format!("slot `{}` has a zero-size viewBox", cell.slot));
        }
        let inner = inner_svg(svg)
            .ok_or_else(|| format!("slot `{}` has no drawable content", cell.slot))?;
        // Scale the component's viewBox into the cell rectangle, in em units.
        let (x, y) = (cell.x * COMPOSE_UPM, cell.y * COMPOSE_UPM);
        let (sx, sy) = (cell.w * COMPOSE_UPM / vw, cell.h * COMPOSE_UPM / vh);
        body.push_str(&format!(
            "<g transform=\"translate({x} {y}) scale({sx} {sy})\">{inner}</g>"
        ));
    }
    Ok(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {COMPOSE_UPM} {COMPOSE_UPM}\">{body}</svg>"
    ))
}

/// The markup between a single SVG document's root `<svg …>` and `</svg>`.
fn inner_svg(svg: &str) -> Option<String> {
    let lower = svg.to_ascii_lowercase();
    let open = lower.find("<svg")?;
    let gt = svg[open..].find('>')? + open + 1;
    let close = lower.rfind("</svg>")?;
    if gt > close {
        return None;
    }
    Some(svg[gt..close].trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::super::font::svg_to_contours;
    use super::*;
    use crate::conlang::types::spatial::builtin_template;

    fn square(view: u32) -> String {
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {view} {view}\">\
             <path d=\"M0 0 H {view} V {view} H 0 Z\" fill=\"black\"/></svg>"
        )
    }

    #[test]
    fn lr_places_components_left_and_right() {
        let t = builtin_template("lr").unwrap();
        let mut comps = BTreeMap::new();
        comps.insert("left".to_string(), square(100));
        comps.insert("right".to_string(), square(100));
        let composite = compose_block(&t, &comps).unwrap();

        // Two filled squares → two contours when converted (at upm 1000).
        let contours = svg_to_contours(&composite, 1000.0).unwrap();
        assert_eq!(contours.len(), 2);
        // Each contour sits entirely in its half of the em (x in 0..500 or 500..1000).
        for c in &contours {
            let xs: Vec<f64> = c.points.iter().map(|p| p.x).collect();
            let (lo, hi) = (xs.iter().cloned().fold(f64::MAX, f64::min), xs.iter().cloned().fold(f64::MIN, f64::max));
            let in_left = hi <= 501.0;
            let in_right = lo >= 499.0;
            assert!(in_left || in_right, "contour spans the midline: {lo}..{hi}");
        }
        // One in each half.
        let mins: Vec<f64> = contours
            .iter()
            .map(|c| c.points.iter().map(|p| p.x).fold(f64::MAX, f64::min))
            .collect();
        assert!(mins.iter().any(|&m| m < 1.0) && mins.iter().any(|&m| m >= 499.0));
    }

    #[test]
    fn handles_differing_component_viewboxes() {
        // A 100-unit and a 1000-unit square both scale into their cells.
        let t = builtin_template("tb").unwrap();
        let mut comps = BTreeMap::new();
        comps.insert("top".to_string(), square(100));
        comps.insert("bottom".to_string(), square(1000));
        let composite = compose_block(&t, &comps).unwrap();
        assert_eq!(svg_to_contours(&composite, 1000.0).unwrap().len(), 2);
    }

    #[test]
    fn missing_slot_errors() {
        let t = builtin_template("lr").unwrap();
        let mut comps = BTreeMap::new();
        comps.insert("left".to_string(), square(100));
        let err = compose_block(&t, &comps).unwrap_err();
        assert!(err.contains("right"));
    }

    #[test]
    fn inner_svg_strips_wrapper() {
        let s = "<svg viewBox=\"0 0 1 1\"><path d=\"M0 0\"/></svg>";
        assert_eq!(inner_svg(s).unwrap(), "<path d=\"M0 0\"/>");
    }
}
