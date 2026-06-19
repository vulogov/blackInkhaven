//! Glyph suitability preflight (LANG-1 P5.1).
//!
//! Before a glyph SVG can be bound or compiled into a font, it passes a
//! deterministic suitability check — for AI-drafted *and* hand-authored
//! artwork alike. Mirrors the PDF preflight (`src/pdf/preflight.rs`) in shape,
//! and uses the in-tree `usvg` parser (no new dependency) to normalize the SVG
//! into a path tree, then reports what would make it a bad font glyph: doesn't
//! parse, no fill outline (stroke-only / empty), raster image content, or
//! non-monochrome paint (gradients / patterns). Geometry-level checks (closed
//! contours, self-intersection) join with the compiler in P5.2.

use resvg::usvg;

#[derive(Debug, Default, serde::Serialize)]
pub struct GlyphReport {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub info: Vec<String>,
}

impl GlyphReport {
    pub fn is_usable(&self) -> bool {
        self.errors.is_empty()
    }
}

#[derive(Default)]
struct Counts {
    fill_paths: usize,
    stroke_only: usize,
    images: usize,
    non_solid_paint: usize,
}

/// Lint a glyph SVG (as a string) for font suitability.
pub fn lint_svg(svg: &str) -> GlyphReport {
    let mut r = GlyphReport::default();

    let opts = usvg::Options::default();
    let tree = match usvg::Tree::from_str(svg, &opts) {
        Ok(t) => t,
        Err(e) => {
            r.errors.push(format!("does not parse as SVG: {e}"));
            return r;
        }
    };

    let size = tree.size();
    r.info.push(format!("viewBox {:.0} × {:.0}", size.width(), size.height()));

    let mut c = Counts::default();
    walk(tree.root(), &mut c);

    if c.images > 0 {
        r.errors
            .push(format!("{} raster <image> element(s) — a font glyph must be vector outlines", c.images));
    }
    if c.fill_paths == 0 {
        if c.stroke_only > 0 {
            r.errors.push(format!(
                "{} stroke-only path(s) and no filled outline — strokes must be outlined for a font",
                c.stroke_only
            ));
        } else {
            r.errors.push("no fillable outline found".into());
        }
    } else {
        r.info.push(format!("{} filled contour(s)", c.fill_paths));
        if c.stroke_only > 0 {
            r.warnings
                .push(format!("{} stroke-only path(s) will be dropped (only fills compile)", c.stroke_only));
        }
    }
    if c.non_solid_paint > 0 {
        r.warnings.push(format!(
            "{} gradient/pattern fill(s) — a font glyph is monochrome; these flatten to a single colour",
            c.non_solid_paint
        ));
    }
    r
}

fn walk(group: &usvg::Group, c: &mut Counts) {
    for node in group.children() {
        match node {
            usvg::Node::Group(g) => walk(g, c),
            usvg::Node::Path(p) => {
                match p.fill() {
                    Some(fill) => {
                        c.fill_paths += 1;
                        if !matches!(fill.paint(), usvg::Paint::Color(_)) {
                            c.non_solid_paint += 1;
                        }
                    }
                    None => {
                        if p.stroke().is_some() {
                            c.stroke_only += 1;
                        }
                    }
                }
            }
            usvg::Node::Image(_) => c.images += 1,
            usvg::Node::Text(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_filled_path_is_usable() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
            <path d="M10 10 H 90 V 90 H 10 Z" fill="black"/></svg>"#;
        let r = lint_svg(svg);
        assert!(r.is_usable(), "errors: {:?}", r.errors);
        assert!(r.info.iter().any(|i| i.contains("filled contour")));
    }

    #[test]
    fn stroke_only_has_no_outline() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
            <path d="M10 10 H 90" stroke="black" fill="none"/></svg>"#;
        let r = lint_svg(svg);
        assert!(!r.is_usable());
        assert!(r.errors.iter().any(|e| e.contains("stroke-only")));
    }

    #[test]
    fn empty_svg_has_no_outline() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"></svg>"#;
        assert!(!lint_svg(svg).is_usable());
    }

    #[test]
    fn garbage_does_not_parse() {
        let r = lint_svg("not an svg at all");
        assert!(!r.is_usable());
        assert!(r.errors.iter().any(|e| e.contains("does not parse")));
    }

    #[test]
    fn gradient_fill_warns_not_monochrome() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
            <defs><linearGradient id="g"><stop offset="0" stop-color="black"/><stop offset="1" stop-color="white"/></linearGradient></defs>
            <path d="M10 10 H 90 V 90 H 10 Z" fill="url(#g)"/></svg>"#;
        let r = lint_svg(svg);
        assert!(r.is_usable()); // a fill exists, just not monochrome
        assert!(r.warnings.iter().any(|w| w.contains("gradient")));
    }
}
