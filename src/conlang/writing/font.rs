//! Font source compilation (LANG-1 P5.2).
//!
//! Turn a collection of glyph SVGs into a **UFO** (Unified Font Object) — the
//! standard interchange font source — via `norad`. Outlines come from the
//! in-tree `usvg` parser: each filled path is transformed to absolute coords,
//! the SVG y-down viewBox is flipped + scaled into the em square, and segments
//! become UFO contour points (lines + cubic/quadratic off-curves). The UFO is
//! a complete, externally-compilable artifact (fontc / fontmake / FontForge);
//! P5.3 compiles it to TTF/OTF in-process. Pure conversion; deterministic.

use norad::{Contour, ContourPoint, PointType};
use resvg::{tiny_skia, usvg};

/// Convert a glyph SVG to UFO contours (font coordinate space: y-up, scaled to
/// `upm`). Only filled paths contribute; the viewBox bottom maps to the
/// baseline.
pub fn svg_to_contours(svg: &str, upm: f64) -> Result<Vec<Contour>, String> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default())
        .map_err(|e| format!("svg parse: {e}"))?;
    let height = tree.size().height() as f64;
    if height <= 0.0 {
        return Err("svg has zero height".into());
    }
    let scale = upm / height;
    let mut out = Vec::new();
    collect(tree.root(), scale, height, &mut out);
    Ok(out)
}

fn collect(group: &usvg::Group, scale: f64, height: f64, out: &mut Vec<Contour>) {
    for node in group.children() {
        match node {
            usvg::Node::Group(g) => collect(g, scale, height, out),
            usvg::Node::Path(p) if p.fill().is_some() => {
                if let Some(abs) = p.data().clone().transform(p.abs_transform()) {
                    out.extend(path_to_contours(&abs, scale, height));
                }
            }
            _ => {}
        }
    }
}

fn path_to_contours(path: &tiny_skia::Path, scale: f64, height: f64) -> Vec<Contour> {
    let pt = |x: f32, y: f32, typ: PointType| {
        ContourPoint::new((x as f64) * scale, (height - y as f64) * scale, typ, false, None, None)
    };
    let mut contours = Vec::new();
    let mut pts: Vec<ContourPoint> = Vec::new();
    let flush = |pts: &mut Vec<ContourPoint>, contours: &mut Vec<Contour>| {
        if !pts.is_empty() {
            contours.push(Contour::new(std::mem::take(pts), None));
        }
    };

    for seg in path.segments() {
        match seg {
            tiny_skia::PathSegment::MoveTo(p) => {
                flush(&mut pts, &mut contours);
                // A glyph contour is closed; the start point is on-curve.
                pts.push(pt(p.x, p.y, PointType::Line));
            }
            tiny_skia::PathSegment::LineTo(p) => pts.push(pt(p.x, p.y, PointType::Line)),
            tiny_skia::PathSegment::QuadTo(c, p) => {
                pts.push(pt(c.x, c.y, PointType::OffCurve));
                pts.push(pt(p.x, p.y, PointType::QCurve));
            }
            tiny_skia::PathSegment::CubicTo(c1, c2, p) => {
                pts.push(pt(c1.x, c1.y, PointType::OffCurve));
                pts.push(pt(c2.x, c2.y, PointType::OffCurve));
                pts.push(pt(p.x, p.y, PointType::Curve));
            }
            // Closure is implicit for UFO contours (last point joins first).
            tiny_skia::PathSegment::Close => flush(&mut pts, &mut contours),
        }
    }
    flush(&mut pts, &mut contours);
    contours
}

/// One glyph to compile: a UFO glyph name, an optional Unicode codepoint, and
/// the glyph's SVG source.
pub struct GlyphSource {
    pub name: String,
    pub codepoint: Option<char>,
    pub svg: String,
}

/// Build a UFO font from glyph sources.
pub fn build_ufo(family: &str, upm: f64, glyphs: &[GlyphSource]) -> Result<norad::Font, String> {
    let mut font = norad::Font::default();
    font.font_info.family_name = Some(family.to_string());
    font.font_info.units_per_em = norad::fontinfo::NonNegativeIntegerOrFloat::new(upm);

    let layer = font.default_layer_mut();
    for g in glyphs {
        let contours = svg_to_contours(&g.svg, upm)?;
        let mut glyph = norad::Glyph::new(g.name.as_str());
        glyph.width = upm;
        if let Some(c) = g.codepoint {
            glyph.codepoints = norad::Codepoints::new([c]);
        }
        glyph.contours = contours;
        layer
            .insert_glyph(glyph);
    }
    Ok(font)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_square_becomes_four_line_points() {
        // viewBox 0..100; upm 1000 → scale 10; y-flip.
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
            <path d="M10 10 H 90 V 90 H 10 Z" fill="black"/></svg>"#;
        let cs = svg_to_contours(svg, 1000.0).unwrap();
        assert_eq!(cs.len(), 1);
        let pts = &cs[0].points;
        assert_eq!(pts.len(), 4);
        assert!(pts.iter().all(|p| p.typ == PointType::Line));
        // (10,10) → (100, 900) after scale 10 + y-flip (100-10)*10.
        assert!((pts[0].x - 100.0).abs() < 0.5 && (pts[0].y - 900.0).abs() < 0.5);
    }

    #[test]
    fn build_ufo_inserts_glyphs_with_codepoints() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
            <path d="M10 10 H 90 V 90 H 10 Z" fill="black"/></svg>"#;
        let font = build_ufo(
            "Test",
            1000.0,
            &[GlyphSource { name: "a".into(), codepoint: Some('a'), svg: svg.into() }],
        )
        .unwrap();
        assert_eq!(font.font_info.family_name.as_deref(), Some("Test"));
        let layer = font.default_layer();
        let g = layer.get_glyph("a").expect("glyph a present");
        assert_eq!(g.contours.len(), 1);
        assert!(g.codepoints.contains('a'));
    }
}
