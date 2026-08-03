//! CHORUS-1 — `inkhaven chorus …` commands (voice & style at book scale). CH-P1
//! ships `voices` (character voice fingerprints); CH-P2/P8 add `distinct` /
//! `report` / `scan`. (Distinct from `inkhaven style`, the editor style-warning
//! report.)

use std::path::Path;

use crate::chorus::distinct::{DistinctMatrix, matrix};
use crate::chorus::drift::character_drift;
use crate::chorus::voices::{CharacterVoice, Confidence, character_profiles};
use crate::prose::violations::Violation;
use crate::config::Config;
use crate::dialogue::{DialogueStore, refresh_book};
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::prose::ProseStore;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;

use super::ChorusCommand;

pub fn run(project: &Path, cmd: ChorusCommand) -> Result<()> {
    match cmd {
        ChorusCommand::Voices { book, character, json } => {
            voices(project, book.as_deref(), character.as_deref(), json)
        }
        ChorusCommand::Scan { book, json } => scan(project, book.as_deref(), json),
    }
}

fn scan(project: &Path, book_name: Option<&str>, json: bool) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let h = Hierarchy::load(&store)?;
    let book = super::resolve_user_book(&h, book_name, "chorus").map_err(Error::Store)?;

    let findings = crate::chorus::pov::scan_head_hops(&layout, &h, &cfg, book);

    if json {
        let arr: Vec<serde_json::Value> = findings
            .iter()
            .map(|s| {
                serde_json::json!({
                    "chapter": s.chapter_ord,
                    "scene": s.scene_index,
                    "pov": s.pov.describe(),
                    "first_para": s.first_para.to_string(),
                    "head_hops": s.hops.iter()
                        .map(|hh| serde_json::json!({"experiencer": hh.experiencer, "count": hh.count}))
                        .collect::<Vec<_>>(),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr).unwrap_or_default());
        return Ok(());
    }

    println!("Voice discipline — `{}` [{}]", book.title, cfg.language);
    println!("{}", "─".repeat(64));
    println!("POV / head-hop (advisory)");
    if findings.is_empty() {
        println!("  ✓ no head-hops found — interiority stays with each scene's POV");
    } else {
        for s in &findings {
            println!("  ch.{} · scene {} ({})", s.chapter_ord, s.scene_index, s.pov.describe());
            for hh in &s.hops {
                let times = if hh.count == 1 { String::new() } else { format!(" ({}×)", hh.count) };
                println!(
                    "      ⚠ {}'s interiority leaks{times} — not the scene's POV",
                    hh.experiencer
                );
            }
        }
        println!("{}", "─".repeat(64));
        println!(
            "Declare a scene's POV with a `pov:<name>` (or `pov:first` / `pov:omniscient`)\n\
             paragraph tag to silence false positives."
        );
    }
    println!("{}", "─".repeat(64));
    Ok(())
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn voices(project: &Path, book_name: Option<&str>, character: Option<&str>, json: bool) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let h = Hierarchy::load(&store)?;
    let book = super::resolve_user_book(&h, book_name, "chorus").map_err(Error::Store)?;

    // The character corpus is built from attributed dialogue spans — refresh them
    // first (idempotent; content-hash cached), exactly as `dialogue profile` does.
    let ds = DialogueStore::open(store.project_root()).map_err(|e| Error::Store(e.to_string()))?;
    refresh_book(&ds, &layout, &h, &cfg, book, None, &now())
        .map_err(|e| Error::Store(e.to_string()))?;
    let pstore = ProseStore::open(store.project_root()).map_err(|e| Error::Store(e.to_string()))?;

    let voices = character_profiles(&pstore, &ds, &cfg, book, None, &now())
        .map_err(|e| Error::Store(e.to_string()))?;

    // The distinctiveness matrix (CH-P2) is over the WHOLE cast — compute it
    // before any `--character` narrowing.
    let dm = matrix(&voices, cfg.chorus.distinct_threshold, &cfg.chorus.distinct_ignore_pairs);

    // A `--character` view shows just that card; the full listing adds the matrix.
    let shown: Vec<&CharacterVoice> = match character {
        Some(name) => voices.iter().filter(|v| v.name.to_lowercase() == name.to_lowercase()).collect(),
        None => voices.iter().collect(),
    };

    // Per-character voice drift (CH-P3), over the shown set.
    let drift: Vec<(String, Vec<Violation>)> = shown
        .iter()
        .copied()
        .map(|v| (v.name.clone(), character_drift(v, &cfg.prose.thresholds)))
        .filter(|(_, d)| !d.is_empty())
        .collect();

    if json {
        let arr: Vec<serde_json::Value> = shown.iter().copied().map(voice_json).collect();
        let out = serde_json::json!({
            "voices": arr,
            "distinctiveness": distinct_json(&dm),
            "drift": drift.iter().map(|(name, ds)| serde_json::json!({
                "character": name,
                "violations": ds.iter().map(|v| serde_json::json!({
                    "chapter": v.chapter, "metric": v.metric, "delta": v.delta, "value": v.value,
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
        return Ok(());
    }

    print_cards(&book.title, &cfg.language, &shown);
    if character.is_none() {
        print_distinctiveness(&dm);
    }
    print_drift(&drift);
    Ok(())
}

fn print_drift(drift: &[(String, Vec<Violation>)]) {
    if drift.is_empty() {
        return;
    }
    println!("Voice drift (each character vs. their first chapter)");
    println!("{}", "─".repeat(60));
    for (name, violations) in drift {
        println!("◆ {name}");
        for v in violations {
            let dir = if v.delta >= 0.0 { "rose" } else { "fell" };
            println!("    ch.{}  {} {dir} to {:.2} (Δ {:+.2})", v.chapter, v.metric, v.value, v.delta);
        }
    }
    println!("{}", "─".repeat(60));
}

fn distinct_json(dm: &DistinctMatrix) -> serde_json::Value {
    serde_json::json!({
        "compared": dm.names,
        "indistinguishable": dm.indistinguishable.iter()
            .map(|p| serde_json::json!({"a": p.a, "b": p.b, "distance": p.distance}))
            .collect::<Vec<_>>(),
        "closest": dm.closest().map(|p| serde_json::json!({"a": p.a, "b": p.b, "distance": p.distance})),
        "most_distinct": dm.most_distinct().map(|p| serde_json::json!({"a": p.a, "b": p.b, "distance": p.distance})),
    })
}

fn print_distinctiveness(dm: &DistinctMatrix) {
    println!("Distinctiveness ({} comparable voice(s))", dm.names.len());
    println!("{}", "─".repeat(60));
    if dm.names.len() < 2 {
        println!("  (need at least two well-attributed voices to compare)");
        println!("{}", "─".repeat(60));
        return;
    }
    if dm.indistinguishable.is_empty() {
        println!("  ✓ every comparable voice is distinct");
    } else {
        for p in &dm.indistinguishable {
            println!("  ⚠ {} ≈ {}  (distance {:.2}) — these read alike", p.a, p.b, p.distance);
        }
    }
    if let (Some(c), Some(d)) = (dm.closest(), dm.most_distinct()) {
        println!("    closest pair:  {} ↔ {}  ({:.2})", c.a, c.b, c.distance);
        println!("    most distinct: {} ↔ {}  ({:.2})", d.a, d.b, d.distance);
    }
    println!("{}", "─".repeat(60));
}

fn voice_json(v: &CharacterVoice) -> serde_json::Value {
    let p = &v.profile;
    serde_json::json!({
        "character": v.name,
        "confidence": v.confidence.label(),
        "utterances": v.utterances,
        "median_sentence_words": p.p50,
        "cv": p.cv,
        "mattr": p.mattr,
        "modal_density": p.modal_density,
        "interiority_ratio": p.interiority_ratio,
    })
}

fn print_cards(book_title: &str, language: &str, voices: &[&CharacterVoice]) {
    println!("Character voices — `{book_title}` [{language}]");
    println!("{}", "─".repeat(60));
    if voices.is_empty() {
        println!("  (no attributed dialogue yet — run `inkhaven dialogue scan`)");
        println!("{}", "─".repeat(60));
        return;
    }

    // Cast means over the voices confident enough to compare — a preview of the
    // CH-P2 distinctiveness matrix (Δ-from-cast-mean).
    let confident: Vec<&CharacterVoice> =
        voices.iter().copied().filter(|v| v.confidence != Confidence::Low).collect();
    let mean_cv = mean(confident.iter().map(|v| v.profile.cv));
    let mean_mattr = mean(confident.iter().map(|v| v.profile.mattr));

    for v in voices {
        let p = &v.profile;
        println!(
            "◆ {:<16} confidence {} · {} utterance(s)",
            v.name,
            v.confidence.label(),
            v.utterances
        );
        println!("    sentence length (median)   {:.0} words", p.p50);
        println!("    rhythm variety (CV)        {:.2}{}", p.cv, delta(p.cv, mean_cv));
        println!("    lexical diversity (MATTR)  {:.2}{}", p.mattr, delta(p.mattr, mean_mattr));
        println!("    hedging (modal density)    {}", opt(p.modal_density));
        println!("    interiority                {}", opt(p.interiority_ratio));
    }

    let low: Vec<&str> =
        voices.iter().filter(|v| v.confidence == Confidence::Low).map(|v| v.name.as_str()).collect();
    println!("{}", "─".repeat(60));
    if !low.is_empty() {
        println!(
            "Note: {} have low confidence (too little dialogue) — profiled, but not\n\
             comparable; the distinctiveness pass will not flag them.",
            low.join(", ")
        );
        println!("{}", "─".repeat(60));
    }
}

fn mean(xs: impl Iterator<Item = f32>) -> Option<f32> {
    let (sum, n) = xs.fold((0.0f32, 0u32), |(s, n), x| (s + x, n + 1));
    if n == 0 { None } else { Some(sum / n as f32) }
}

/// `(cast ±0.05)` against the cast mean, when there is a comparable cast.
fn delta(value: f32, cast_mean: Option<f32>) -> String {
    match cast_mean {
        Some(m) => format!("   (cast {:+.2})", value - m),
        None => String::new(),
    }
}

fn opt(x: Option<f32>) -> String {
    match x {
        Some(v) => format!("{v:.3}"),
        None => "n/a".to_string(),
    }
}
