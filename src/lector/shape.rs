//! LECTOR-1 LR-P7 — the expected-shape overlay.
//!
//! The Planning Board declares an *intended* shape (a framework's beats carry an
//! `expected_tension` curve); LR-P1 measures the *realized* shape from the prose.
//! This module lines them up: it interpolates the framework's expected intensity
//! across the chapters and flags where the intended shape wants a rise but the
//! prose reads flat — the empirical "saggy middle" the Planning Board's tag-based
//! curve is blind to on an untagged draft.
//!
//! The framework is `lector.framework` when set, else suggested from the project
//! `genre`, else Three-Act — so the overlay works with zero setup.

use crate::config::Config;
use crate::planning::{BeatSpec, Framework};

use super::{ReadThrough, ReaderFinding, Severity};

/// Below this expected intensity a beat isn't "supposed" to be high, so a low
/// measured value there isn't a sag.
const EXPECTED_FLOOR: f32 = 0.5;
/// How far measured must fall below expected to read as a sag.
const SAG_GAP: f32 = 0.28;

/// The framework the read-through measures against: the `lector.framework` override
/// (when it parses), else the genre suggestion, else Three-Act.
pub(crate) fn resolve_framework(cfg: &Config) -> Framework {
    if let Some(name) = cfg.lector.framework.as_deref() {
        if let Some(fw) = Framework::parse(name) {
            return fw;
        }
    }
    match cfg.genre.as_deref() {
        Some(g) if !g.trim().is_empty() => Framework::suggest_for_genre(g),
        _ => Framework::ThreeAct,
    }
}

/// The framework's expected intensity at each of `n` evenly-spaced chapter
/// positions (piecewise-linear between beats, clamped at the ends). Pure.
pub(crate) fn expected_curve(fw: Framework, n: usize) -> Vec<f32> {
    let beats = fw.beats();
    if n == 0 || beats.is_empty() {
        return Vec::new();
    }
    (0..n)
        .map(|i| {
            let pos = if n > 1 { i as f32 / (n - 1) as f32 } else { 0.0 };
            interpolate(beats, pos)
        })
        .collect()
}

/// Linear interpolation of `expected_tension` at `pos` (beats assumed ordered by
/// `target_position`; before the first / after the last clamps to the end value).
fn interpolate(beats: &[BeatSpec], pos: f32) -> f32 {
    if pos <= beats[0].target_position {
        return beats[0].expected_tension;
    }
    for w in beats.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        if pos <= b.target_position {
            let span = (b.target_position - a.target_position).max(f32::EPSILON);
            let t = ((pos - a.target_position) / span).clamp(0.0, 1.0);
            return a.expected_tension + t * (b.expected_tension - a.expected_tension);
        }
    }
    beats[beats.len() - 1].expected_tension
}

/// Flag chapters where the framework wants a rise (`expected ≥ EXPECTED_FLOOR`) but
/// the prose reads flat (`measured` at least `SAG_GAP` below). Pure over the read.
pub(crate) fn scan(rt: &ReadThrough, cfg: &Config) -> Vec<ReaderFinding> {
    let fw = resolve_framework(cfg);
    let expected = expected_curve(fw, rt.chapters.len());
    let mut out = Vec::new();
    for (i, c) in rt.chapters.iter().enumerate() {
        let (Some(exp), Some(meas)) = (expected.get(i).copied(), c.measured_intensity) else {
            continue;
        };
        if exp >= EXPECTED_FLOOR && meas + SAG_GAP < exp {
            let chapter = c.chapter;
            let dedup_key = ReaderFinding::make_dedup_key("shape_sag", &[], chapter);
            out.push(ReaderFinding {
                kind: "shape_sag",
                severity: Severity::Notice,
                chapter,
                anchor: None,
                entities: Vec::new(),
                message: format!(
                    "the {} shape wants rising tension around ch. {chapter} (~{:.0}%) but the prose reads flat (~{:.0}%).",
                    fw.label(),
                    exp * 100.0,
                    meas * 100.0,
                ),
                source: "walk",
                dedup_key,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expected_curve_rises_to_the_climax_then_falls() {
        let c = expected_curve(Framework::ThreeAct, 11);
        assert_eq!(c.len(), 11);
        assert!(c[0] < 0.2, "opens low: {}", c[0]);
        // Climax (~0.90) is the peak; the resolution (1.0) drops.
        let peak = c.iter().cloned().fold(0.0f32, f32::max);
        assert!(peak > 0.9, "peaks high: {peak}");
        assert!(*c.last().unwrap() < peak, "resolves below the peak");
    }

    #[test]
    fn genre_suggestion_and_override() {
        let mut cfg = Config::default();
        assert_eq!(resolve_framework(&cfg), Framework::ThreeAct);
        cfg.genre = Some("epic fantasy".into());
        assert_eq!(resolve_framework(&cfg), Framework::HeroJourney);
        cfg.lector.framework = Some("kishotenketsu".into());
        assert_eq!(resolve_framework(&cfg), Framework::Kishotenketsu, "explicit override wins");
    }

    #[test]
    fn sag_fires_where_expected_high_but_measured_flat() {
        use super::super::ChapterRead;
        let ch = |chapter, m: f32| ChapterRead {
            chapter,
            measured_intensity: Some(m),
            ..Default::default()
        };
        // 5 chapters; the midpoint (i=2, pos 0.5, expected ~0.65) is flat.
        let rt = ReadThrough {
            chapters: vec![ch(1, 0.1), ch(2, 0.4), ch(3, 0.05), ch(4, 0.8), ch(5, 0.9)],
            curve: Vec::new(),
        };
        let cfg = Config::default(); // Three-Act
        let sags = scan(&rt, &cfg);
        assert!(sags.iter().any(|f| f.kind == "shape_sag" && f.chapter == 3), "midpoint sag: {sags:?}");
    }
}
