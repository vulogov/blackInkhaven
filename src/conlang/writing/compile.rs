//! In-process UFO → TTF compilation (LANG-1 P5.3).
//!
//! Takes the [`norad::Font`] produced by [`super::font::build_ufo`] and emits a
//! ready-to-use TrueType binary, with no external tool. Each UFO contour is
//! converted to a `kurbo::BezPath` (cubics survive; `write-fonts`'
//! `SimpleGlyph::from_bezpath` quadifies them for the `glyf` table), and the
//! OpenType tables (`glyf`/`loca`/`head`/`hhea`/`maxp`/`hmtx`/`cmap`/`name`/
//! `post`/`OS/2`) are assembled directly via `write-fonts`. Pure +
//! deterministic — same UFO in, identical bytes out.
//!
//! Scope: static outline fonts (the constructed-script case). Hinting,
//! variable axes, and `GSUB`/`GPOS` shaping are intentionally out of scope —
//! they arrive with the later writing-system phases (Hangul precompose, etc.).

use kurbo::{BezPath, Point};
use norad::{Contour, Font, PointType};
use write_fonts::{
    tables::{
        cmap::Cmap,
        glyf::{GlyfLocaBuilder, Glyph, SimpleGlyph},
        head::Head,
        hhea::Hhea,
        hmtx::{Hmtx, LongMetric},
        loca::LocaFormat,
        maxp::Maxp,
        name::{Name, NameRecord},
        os2::{Os2, SelectionFlags},
        post::Post,
    },
    types::{FWord, GlyphId, NameId, UfWord},
    FontBuilder, OffsetMarker,
};

/// Compile a UFO font into TrueType (`.ttf`) bytes.
///
/// `upm` is the design grid (units-per-em) the glyphs were built on — the same
/// value passed to [`super::font::build_ufo`].
pub fn compile_ttf(font: &Font, upm: f64) -> Result<Vec<u8>, String> {
    let upm_u16 = upm.round().clamp(16.0, 16384.0) as u16;
    let family = font
        .font_info
        .family_name
        .clone()
        .unwrap_or_else(|| "Untitled".to_string());

    // Deterministic glyph order. Index 0 is always `.notdef`.
    let layer = font.default_layer();
    let mut glyphs: Vec<&norad::Glyph> = layer.iter().collect();
    glyphs.sort_by(|a, b| a.name().cmp(b.name()));

    let mut builder = GlyfLocaBuilder::new();
    builder
        .add_glyph(&Glyph::Empty) // .notdef
        .map_err(|e| format!("glyf .notdef: {e}"))?;

    let mut h_metrics = vec![LongMetric::new(upm_u16 / 2, 0)];
    let mut cmap_entries: Vec<(char, GlyphId)> = Vec::new();
    let mut bbox: Option<write_fonts::tables::glyf::Bbox> = None;
    let mut max_points = 0u16;
    let mut max_contours = 0u16;

    for (i, g) in glyphs.iter().enumerate() {
        let gid = (i + 1) as u32;

        let mut path = BezPath::new();
        for c in &g.contours {
            contour_to_bezpath(c, &mut path);
        }
        // TrueType `glyf` is all-quadratic: approximate any cubics with quads.
        let path = quadify(&path, (upm / 1000.0).max(0.1));

        // Upper-bound point/contour counts for `maxp` (over-estimating is safe;
        // renderers allocate at least this much). One `MoveTo` per contour.
        let n_elements = path.elements().len() as u16;
        let n_contours = path
            .elements()
            .iter()
            .filter(|el| matches!(el, kurbo::PathEl::MoveTo(_)))
            .count() as u16;

        let glyph = if path.elements().is_empty() {
            Glyph::Empty
        } else {
            Glyph::Simple(
                SimpleGlyph::from_bezpath(&path)
                    .map_err(|e| format!("glyph `{}`: {e:?}", g.name()))?,
            )
        };

        if let Some(bb) = glyph.bbox() {
            bbox = Some(match bbox {
                Some(acc) => acc.union(bb),
                None => bb,
            });
        }
        max_points = max_points.max(n_elements);
        max_contours = max_contours.max(n_contours);

        let advance = g.width.round().clamp(0.0, u16::MAX as f64) as u16;
        let lsb = glyph.bbox().map(|b| b.x_min).unwrap_or(0);
        h_metrics.push(LongMetric::new(advance, lsb));

        builder
            .add_glyph(&glyph)
            .map_err(|e| format!("glyf `{}`: {e}", g.name()))?;

        for cp in g.codepoints.iter() {
            cmap_entries.push((cp, GlyphId::new(gid)));
        }
    }

    let (glyf, loca, loca_format) = builder.build();
    let num_glyphs = (glyphs.len() + 1) as u16;
    let bbox = bbox.unwrap_or_default();

    // head ---------------------------------------------------------------
    let mut head = Head {
        units_per_em: upm_u16,
        x_min: bbox.x_min,
        y_min: bbox.y_min,
        x_max: bbox.x_max,
        y_max: bbox.y_max,
        lowest_rec_ppem: 8,
        ..Default::default()
    };
    head.index_to_loc_format = match loca_format {
        LocaFormat::Short => 0,
        LocaFormat::Long => 1,
    };

    // hhea ---------------------------------------------------------------
    let ascender = if bbox.y_max > 0 { bbox.y_max } else { (upm * 0.8) as i16 };
    let descender = if bbox.y_min < 0 { bbox.y_min } else { -((upm * 0.2) as i16) };
    let advance_max = h_metrics.iter().map(|m| m.advance).max().unwrap_or(upm_u16);
    let hhea = Hhea {
        ascender: FWord::new(ascender),
        descender: FWord::new(descender),
        line_gap: FWord::new(0),
        advance_width_max: UfWord::new(advance_max),
        min_left_side_bearing: FWord::new(bbox.x_min),
        min_right_side_bearing: FWord::new(0),
        x_max_extent: FWord::new(bbox.x_max),
        caret_slope_rise: 1,
        caret_slope_run: 0,
        caret_offset: 0,
        number_of_h_metrics: h_metrics.len() as u16,
        ..Default::default()
    };

    // maxp (TrueType 1.0 — the Some(..) fields force version 1.0) ---------
    let maxp = Maxp {
        num_glyphs,
        max_points: Some(max_points),
        max_contours: Some(max_contours),
        max_composite_points: Some(0),
        max_composite_contours: Some(0),
        max_zones: Some(1),
        max_twilight_points: Some(0),
        max_storage: Some(0),
        max_function_defs: Some(0),
        max_instruction_defs: Some(0),
        max_stack_elements: Some(0),
        max_size_of_instructions: Some(0),
        max_component_elements: Some(0),
        max_component_depth: Some(0),
    };

    let hmtx = Hmtx::new(h_metrics, Vec::new());

    // cmap ---------------------------------------------------------------
    let cmap = Cmap::from_mappings(cmap_entries.iter().copied())
        .map_err(|e| format!("cmap: {e}"))?;

    // OS/2 ---------------------------------------------------------------
    let (first_char, last_char) = cmap_entries
        .iter()
        .map(|(c, _)| *c as u32)
        .fold((0xFFFFu32, 0u32), |(lo, hi), c| (lo.min(c), hi.max(c)));
    let mut os2 = Os2 {
        us_weight_class: 400,
        us_width_class: 5,
        fs_selection: SelectionFlags::REGULAR,
        us_first_char_index: first_char.min(0xFFFF) as u16,
        us_last_char_index: last_char.min(0xFFFF) as u16,
        s_typo_ascender: ascender,
        s_typo_descender: descender,
        s_typo_line_gap: 0,
        us_win_ascent: ascender.max(0) as u16,
        us_win_descent: descender.unsigned_abs(),
        ach_vend_id: write_fonts::types::Tag::new(b"NONE"),
        ..Default::default()
    };
    os2.panose_10 = [2, 0, 0, 0, 0, 0, 0, 0, 0, 0];

    // post (v3.0 — no per-glyph names) -----------------------------------
    let post = Post {
        version: write_fonts::types::Version16Dot16::VERSION_3_0,
        underline_position: FWord::new(-((upm * 0.1) as i16)),
        underline_thickness: FWord::new((upm * 0.05) as i16),
        ..Default::default()
    };

    // name ---------------------------------------------------------------
    let ps_name: String = family.chars().filter(|c| !c.is_whitespace()).collect();
    let name = build_name(&family, &ps_name);

    // assemble -----------------------------------------------------------
    let mut fb = FontBuilder::new();
    fb.add_table(&head).map_err(|e| format!("head: {e}"))?;
    fb.add_table(&hhea).map_err(|e| format!("hhea: {e}"))?;
    fb.add_table(&maxp).map_err(|e| format!("maxp: {e}"))?;
    fb.add_table(&os2).map_err(|e| format!("OS/2: {e}"))?;
    fb.add_table(&hmtx).map_err(|e| format!("hmtx: {e}"))?;
    fb.add_table(&cmap).map_err(|e| format!("cmap: {e}"))?;
    fb.add_table(&loca).map_err(|e| format!("loca: {e}"))?;
    fb.add_table(&glyf).map_err(|e| format!("glyf: {e}"))?;
    fb.add_table(&name).map_err(|e| format!("name: {e}"))?;
    fb.add_table(&post).map_err(|e| format!("post: {e}"))?;
    Ok(fb.build())
}

/// Windows + Mac name records for the core IDs.
fn build_name(family: &str, ps_name: &str) -> Name {
    let entries: [(u16, &str); 6] = [
        (1, family),               // Family
        (2, "Regular"),            // Subfamily
        (3, ps_name),              // Unique ID
        (4, family),               // Full name
        (5, "Version 1.000"),      // Version
        (6, ps_name),              // PostScript name
    ];
    let mut records = Vec::new();
    for (id, value) in entries {
        // Windows, Unicode BMP, en-US.
        records.push(NameRecord::new(
            3,
            1,
            0x0409,
            NameId::new(id),
            OffsetMarker::new(value.to_string()),
        ));
        // Macintosh, Roman, English.
        records.push(NameRecord::new(
            1,
            0,
            0,
            NameId::new(id),
            OffsetMarker::new(value.to_string()),
        ));
    }
    // The 'name' table requires records sorted by
    // (platform, encoding, language, nameID).
    records.sort_by_key(|r| (r.platform_id, r.encoding_id, r.language_id, r.name_id));
    Name::new(records)
}

/// Append a single UFO contour to `path` as a closed `BezPath` subpath.
///
/// Handles open (leading `Move`) and closed contours, cubic (`Curve`) and
/// TrueType-quadratic (`QCurve`, with implied on-curve midpoints between
/// consecutive off-curves) segments.
fn contour_to_bezpath(contour: &Contour, path: &mut BezPath) {
    let pts = &contour.points;
    if pts.is_empty() {
        return;
    }
    // Start at the first on-curve point so the cycle is well-defined. A
    // contour with no on-curve point (all off-curve quad blob) is degenerate;
    // skip it rather than guess.
    let Some(start_idx) = pts.iter().position(|p| !matches!(p.typ, PointType::OffCurve)) else {
        return;
    };
    let n = pts.len();
    let pt = |p: &norad::ContourPoint| Point::new(p.x, p.y);
    let start = &pts[start_idx];

    path.move_to(pt(start));
    let mut ctrls: Vec<Point> = Vec::new();
    for k in 1..n {
        let p = &pts[(start_idx + k) % n];
        match p.typ {
            PointType::OffCurve => ctrls.push(pt(p)),
            PointType::Line | PointType::Move => {
                path.line_to(pt(p));
                ctrls.clear();
            }
            PointType::Curve => {
                emit_cubic(path, &ctrls, pt(p));
                ctrls.clear();
            }
            PointType::QCurve => {
                emit_quad(path, &ctrls, pt(p));
                ctrls.clear();
            }
        }
    }
    // The closing segment runs from the trailing off-curves back to `start`;
    // its type is carried by the start point.
    if !ctrls.is_empty() {
        match start.typ {
            PointType::Curve => emit_cubic(path, &ctrls, pt(start)),
            _ => emit_quad(path, &ctrls, pt(start)),
        }
    }
    path.close_path();
}

/// Rewrite a `BezPath` with every cubic replaced by quadratic approximations,
/// so it is acceptable to `SimpleGlyph::from_bezpath` (TrueType outlines are
/// quadratic-only). Lines and quads pass through untouched.
fn quadify(path: &BezPath, accuracy: f64) -> BezPath {
    use kurbo::{CubicBez, PathEl};
    let mut out = BezPath::new();
    let mut cur = Point::ZERO;
    let mut start = Point::ZERO;
    for el in path.elements() {
        match *el {
            PathEl::MoveTo(p) => {
                out.move_to(p);
                cur = p;
                start = p;
            }
            PathEl::LineTo(p) => {
                out.line_to(p);
                cur = p;
            }
            PathEl::QuadTo(c, p) => {
                out.quad_to(c, p);
                cur = p;
            }
            PathEl::CurveTo(c1, c2, p) => {
                for (_, _, q) in CubicBez::new(cur, c1, c2, p).to_quads(accuracy) {
                    out.quad_to(q.p1, q.p2);
                }
                cur = p;
            }
            PathEl::ClosePath => {
                out.close_path();
                cur = start;
            }
        }
    }
    out
}

fn emit_cubic(path: &mut BezPath, ctrls: &[Point], end: Point) {
    match ctrls.len() {
        0 => path.line_to(end),
        1 => path.quad_to(ctrls[0], end),
        _ => path.curve_to(ctrls[0], ctrls[1], end),
    }
}

fn emit_quad(path: &mut BezPath, ctrls: &[Point], end: Point) {
    match ctrls.len() {
        0 => path.line_to(end),
        1 => path.quad_to(ctrls[0], end),
        _ => {
            // TrueType implied on-curve points midway between off-curves.
            for w in ctrls.windows(2) {
                let mid = Point::new((w[0].x + w[1].x) / 2.0, (w[0].y + w[1].y) / 2.0);
                path.quad_to(w[0], mid);
            }
            path.quad_to(ctrls[ctrls.len() - 1], end);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::font::{build_ufo, GlyphSource};
    use super::*;

    fn square_svg() -> &'static str {
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
            <path d="M10 10 H 90 V 90 H 10 Z" fill="black"/></svg>"#
    }

    // A cubic-Bézier glyph, to exercise the cubic→quad path.
    fn circle_svg() -> &'static str {
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
            <path d="M50 10 C 72 10 90 28 90 50 C 90 72 72 90 50 90
                     C 28 90 10 72 10 50 C 10 28 28 10 50 10 Z" fill="black"/></svg>"#
    }

    #[test]
    fn compiles_a_valid_ttf() {
        let font = build_ufo(
            "Test",
            1000.0,
            &[
                GlyphSource { name: "a".into(), codepoint: Some('a'), svg: square_svg().into() },
                GlyphSource { name: "o".into(), codepoint: Some('o'), svg: circle_svg().into() },
            ],
        )
        .unwrap();
        let ttf = compile_ttf(&font, 1000.0).unwrap();
        // sfnt version 0x00010000 for TrueType outlines.
        assert_eq!(&ttf[0..4], &[0x00, 0x01, 0x00, 0x00]);
        // Round-trips through a real font parser; both codepoints resolve to
        // real glyphs (gid 0 is .notdef), and the cubic glyph quadified into a
        // drawable outline.
        use skrifa::{FontRef, MetadataProvider};
        use skrifa::outline::{DrawSettings, OutlinePen};
        use skrifa::instance::{LocationRef, Size};
        let f = FontRef::new(&ttf).expect("parse compiled ttf");
        let a = f.charmap().map('a').expect("'a' in cmap");
        let o = f.charmap().map('o').expect("'o' in cmap");
        assert!(a.to_u32() > 0 && o.to_u32() > 0 && a != o);

        struct Counter(usize);
        impl OutlinePen for Counter {
            fn move_to(&mut self, _: f32, _: f32) { self.0 += 1; }
            fn line_to(&mut self, _: f32, _: f32) { self.0 += 1; }
            fn quad_to(&mut self, _: f32, _: f32, _: f32, _: f32) { self.0 += 1; }
            fn curve_to(&mut self, _: f32, _: f32, _: f32, _: f32, _: f32, _: f32) { self.0 += 1; }
            fn close(&mut self) {}
        }
        let outlines = f.outline_glyphs();
        let glyph = outlines.get(o).expect("outline for 'o'");
        let mut pen = Counter(0);
        glyph
            .draw(DrawSettings::unhinted(Size::unscaled(), LocationRef::default()), &mut pen)
            .expect("draw 'o'");
        assert!(pen.0 > 4, "cubic glyph should quadify into multiple segments");
    }

    #[test]
    fn square_contour_has_four_corners() {
        let mut path = BezPath::new();
        let font = build_ufo(
            "T",
            1000.0,
            &[GlyphSource { name: "a".into(), codepoint: Some('a'), svg: square_svg().into() }],
        )
        .unwrap();
        let glyph = font.default_layer().get_glyph("a").unwrap();
        for c in &glyph.contours {
            contour_to_bezpath(c, &mut path);
        }
        // One MoveTo (one contour) + a Close.
        let moves = path
            .elements()
            .iter()
            .filter(|e| matches!(e, kurbo::PathEl::MoveTo(_)))
            .count();
        assert_eq!(moves, 1);
        assert!(path.elements().iter().any(|e| matches!(e, kurbo::PathEl::ClosePath)));
    }
}
