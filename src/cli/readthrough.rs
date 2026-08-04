//! LECTOR-1 LR-P5 — `inkhaven readthrough`: the read-through report.
//!
//! The one artifact that unifies the whole read: the measured shape curve, the
//! per-chapter scene/sequel beat, and the ranked reader findings (the LR-P3
//! deterministic walk + LR-P2 arrhythmia, plus the LR-P4 synthetic first-read
//! under `--deep`). Advisory; deterministic + free unless `--deep`.

use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::lector::scene_sequel::SceneKind;
use crate::lector::{scene_sequel, synthetic, walk, ReaderFinding, Severity};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::Store;

/// Run the read-through report.
pub fn run(project: &Path, deep: bool, max_cost: usize, force: bool, json: bool) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| Error::Store(e.to_string()))?;
    let h = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;

    // The forward walk (state + deterministic findings) and the scene/sequel beat.
    let rt = walk::read_forward(&store, &cfg, &layout, &h);
    let kinds = scene_sequel::chapter_kinds(&layout, &h, &cfg);
    let kind_pairs: Vec<(u32, SceneKind)> = kinds.iter().map(|(c, _, k)| (*c, *k)).collect();

    // Gather findings: the walk's (ranked/deduped) + arrhythmia + (opt) synthetic.
    let mut findings: Vec<ReaderFinding> = rt.ranked_findings();
    findings.extend(scene_sequel::arrhythmia(&kind_pairs));
    if deep {
        eprintln!("readthrough: running the synthetic first-read (LLM, cost-capped)…");
        findings.extend(synthetic::run(project, max_cost, force).map_err(Error::Store)?);
    }
    crate::lector::rank(&mut findings);
    let findings = crate::lector::dedupe(findings);

    if json {
        return print_json(&rt, &kinds, &findings);
    }

    // ── the shape curve ──
    let n = rt.chapters.len();
    println!("Read-through — {n} chapter(s)");
    if n > 0 {
        println!("  intensity  {}", crate::planning::intensity_sparkline(&rt.curve, n.max(1)));
    }

    // ── per-chapter beats ──
    for (i, c) in rt.chapters.iter().enumerate() {
        let kind = kinds.get(i).map(|(_, _, k)| kind_glyph(*k)).unwrap_or("  ");
        let bar = intensity_cell(c.measured_intensity);
        println!("  ch {:>2}  {bar} {kind}  {}", c.chapter, c.title);
    }

    // ── findings ──
    if findings.is_empty() {
        println!("\n\u{2713} the read holds — no reader problems flagged");
    } else {
        println!();
        for f in &findings {
            let icon = severity_icon(f.severity);
            let src = if f.source == "reader" { " · first-read" } else { "" };
            println!("{icon} [{}{src}] {}", f.kind, f.message);
        }
        let concerns = findings.iter().filter(|f| f.severity == Severity::Concern).count();
        println!("\n{} reader finding(s): {concerns} concern(s).", findings.len());
    }
    Ok(())
}

fn severity_icon(s: Severity) -> &'static str {
    match s {
        Severity::Concern => "\u{2297}", // ⊗
        Severity::Notice => "\u{26a0}",  // ⚠
        Severity::Info => "\u{25cf}",    // ●
    }
}

fn kind_glyph(k: SceneKind) -> &'static str {
    match k {
        SceneKind::Scene => "\u{25b6} ", // ▶ forward
        SceneKind::Sequel => "\u{25c9} ", // ◉ reflective
        SceneKind::Mixed => "\u{00b7} ",  // ·
    }
}

/// A one-cell sparkbar for a chapter's intensity.
pub(crate) fn intensity_cell(intensity: Option<f32>) -> char {
    match intensity {
        None => ' ',
        Some(v) => {
            let bars = ['\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2586}', '\u{2587}', '\u{2588}'];
            let idx = ((v.clamp(0.0, 1.0) * (bars.len() - 1) as f32).round()) as usize;
            bars[idx.min(bars.len() - 1)]
        }
    }
}

fn print_json(
    rt: &crate::lector::ReadThrough,
    kinds: &[(u32, String, SceneKind)],
    findings: &[ReaderFinding],
) -> Result<()> {
    let chapters: Vec<serde_json::Value> = rt
        .chapters
        .iter()
        .enumerate()
        .map(|(i, c)| {
            serde_json::json!({
                "chapter": c.chapter,
                "title": c.title,
                "intensity": c.measured_intensity,
                "kind": kinds.get(i).map(|(_, _, k)| k.label()),
                "new_entities": c.new_entities,
                "opened_threads": c.opened_threads,
                "resolved_threads": c.resolved_threads,
            })
        })
        .collect();
    let finds: Vec<serde_json::Value> = findings
        .iter()
        .map(|f| {
            serde_json::json!({
                "kind": f.kind,
                "severity": f.severity.label(),
                "chapter": f.chapter,
                "anchor": f.anchor.map(|a| a.to_string()),
                "source": f.source,
                "entities": f.entities,
                "message": f.message,
            })
        })
        .collect();
    let out = serde_json::json!({ "chapters": chapters, "findings": finds });
    println!("{}", serde_json::to_string_pretty(&out).unwrap_or_else(|_| "{}".into()));
    Ok(())
}
