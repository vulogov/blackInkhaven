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
        ChorusCommand::Report { book, json } => report(project, book.as_deref(), json),
        ChorusCommand::Stylist { book, coach, suppress, unsuppress, json } => {
            stylist(project, book.as_deref(), coach, suppress, unsuppress, json)
        }
    }
}

/// CH-P8 — the unified voice dashboard: narrator profile + cast + distinctiveness
/// + the Inner Stylist synthesis, over one book.
fn report(project: &Path, book_name: Option<&str>, json: bool) -> Result<()> {
    use crate::chorus::distinct::matrix;
    use crate::chorus::drift::character_drift;
    use crate::chorus::voices::character_profiles;
    use crate::prose::VoiceScope;

    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let h = Hierarchy::load(&store)?;
    let book = super::resolve_user_book(&h, book_name, "chorus").map_err(Error::Store)?;

    // Narrator profile (NARR-1 book aggregate).
    let pstore = ProseStore::open(store.project_root()).map_err(|e| Error::Store(e.to_string()))?;
    let narrator_profiles = crate::prose::refresh_book(
        &pstore, &layout, &h, &cfg, book, None, cfg.prose.deep_metrics, cfg.prose.mattr_window, &now(),
    )
    .map_err(|e| Error::Store(e.to_string()))?;
    let narrator = narrator_profiles.into_iter().find(|p| matches!(p.scope, VoiceScope::Book));

    // Cast voices → distinctiveness + drift.
    let ds = DialogueStore::open(store.project_root()).map_err(|e| Error::Store(e.to_string()))?;
    crate::dialogue::refresh_book(&ds, &layout, &h, &cfg, book, None, &now())
        .map_err(|e| Error::Store(e.to_string()))?;
    let voices = character_profiles(&pstore, &ds, &cfg, book, None, &now())
        .map_err(|e| Error::Store(e.to_string()))?;
    let dm = matrix(&voices, cfg.chorus.distinct_threshold, &cfg.chorus.distinct_ignore_pairs);
    let drifts: Vec<(String, Vec<crate::prose::violations::Violation>)> = voices
        .iter()
        .map(|v| (v.name.clone(), character_drift(v, &cfg.prose.thresholds)))
        .filter(|(_, d)| !d.is_empty())
        .collect();

    // Discipline pillars → the Inner Stylist synthesis.
    let head_hops = crate::chorus::pov::scan_head_hops(&layout, &h, &cfg, book);
    let tense = crate::chorus::tense::scan_tense(&layout, &h, &cfg, book);
    let register = crate::chorus::register::scan_register(&layout, &h, &cfg, book);
    let findings =
        crate::inner_stylist::fast::synthesize(&dm, &drifts, &head_hops, &tense, &register);

    if json {
        let out = serde_json::json!({
            "narrator": narrator.as_ref().map(|p| serde_json::json!({
                "median_sentence_words": p.p50, "cv": p.cv, "mattr": p.mattr,
                "modal_density": p.modal_density, "interiority_ratio": p.interiority_ratio,
            })),
            "voices": voices.iter().map(voice_json).collect::<Vec<_>>(),
            "distinctiveness": distinct_json(&dm),
            "stylist": findings.iter().map(|f| serde_json::json!({
                "severity": f.severity.label(), "kind": f.kind, "key": f.key, "message": f.message,
            })).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
        return Ok(());
    }

    println!("Voice report — `{}` [{}]", book.title, cfg.language);
    println!("{}", "═".repeat(66));
    println!("Narrator");
    match &narrator {
        Some(p) => {
            println!("    sentence length (median)   {:.0} words", p.p50);
            println!("    rhythm variety (CV)        {:.2}", p.cv);
            println!("    lexical diversity (MATTR)  {:.2}", p.mattr);
            println!("    hedging                    {}", opt(p.modal_density));
            println!("    interiority                {}", opt(p.interiority_ratio));
        }
        None => println!("    (no narrator profile — the book has no prose yet)"),
    }
    println!("{}", "─".repeat(66));
    print_cards(&book.title, &cfg.language, &voices.iter().collect::<Vec<_>>());
    print_distinctiveness(&dm);
    println!("{}", "─".repeat(66));
    println!("Inner Stylist");
    if findings.is_empty() {
        println!("  the book's voice reads clean — nothing to raise");
    } else {
        for f in &findings {
            println!("  {} [{}] {}", f.severity.glyph(), f.kind, f.message);
        }
    }
    println!("{}", "═".repeat(66));
    Ok(())
}

fn stylist(
    project: &Path,
    book_name: Option<&str>,
    coach: bool,
    suppress: Option<String>,
    unsuppress: Option<String>,
    json: bool,
) -> Result<()> {
    use crate::inner_stylist::storage::InnerStylistStore;

    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let sstore = InnerStylistStore::open_for_project(store.project_root())
        .map_err(|e| Error::Store(e.to_string()))?;

    // Suppression management short-circuits.
    if let Some(key) = suppress {
        sstore.suppress(&key).map_err(|e| Error::Store(e.to_string()))?;
        println!("silenced `{key}` — the Inner Stylist won't raise it again");
        return Ok(());
    }
    if let Some(key) = unsuppress {
        sstore.unsuppress(&key).map_err(|e| Error::Store(e.to_string()))?;
        println!("restored `{key}`");
        return Ok(());
    }

    let h = Hierarchy::load(&store)?;
    let book = super::resolve_user_book(&h, book_name, "stylist").map_err(Error::Store)?;

    let mut findings = crate::inner_stylist::pipeline::gather(&store, &layout, &h, &cfg, book, &now())
        .map_err(Error::Store)?;
    let silenced = sstore.all_suppressions().map_err(|e| Error::Store(e.to_string()))?;
    findings.retain(|f| !silenced.contains(&f.key));

    if coach {
        let iso = crate::ai::prompts::iso_from_long(cfg.stylist.language.as_deref().unwrap_or(&cfg.language));
        let lang = crate::prose::resolve_prose_language(Some(iso), &cfg.language).0;
        let prompt = crate::inner_stylist::slow::build_coach_prompt(&findings, &lang);
        let coaching = crate::inner_stylist::slow::stylist_llm_call(
            &cfg,
            crate::inner_stylist::slow::STYLIST_SYSTEM,
            &prompt,
        )
        .map_err(Error::Store)?;
        println!("{coaching}");
        return Ok(());
    }

    if json {
        let arr: Vec<serde_json::Value> = findings
            .iter()
            .map(|f| {
                serde_json::json!({
                    "severity": f.severity.label(),
                    "kind": f.kind,
                    "key": f.key,
                    "message": f.message,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr).unwrap_or_default());
        return Ok(());
    }

    println!("Inner Stylist — `{}` [{}]", book.title, cfg.language);
    println!("{}", "─".repeat(66));
    if findings.is_empty() {
        println!("  the book's voice reads clean — nothing to raise");
    } else {
        for f in &findings {
            println!("  {} [{}] {}", f.severity.glyph(), f.kind, f.message);
            println!("        key: {}  ·  silence with `chorus stylist --suppress {}`", f.key, f.key);
        }
    }
    println!("{}", "─".repeat(66));
    println!("`chorus stylist --coach` turns these into grounded LLM coaching.");
    Ok(())
}

fn scan(project: &Path, book_name: Option<&str>, json: bool) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let h = Hierarchy::load(&store)?;
    let book = super::resolve_user_book(&h, book_name, "chorus").map_err(Error::Store)?;

    let findings = crate::chorus::pov::scan_head_hops(&layout, &h, &cfg, book);
    let tense = crate::chorus::tense::scan_tense(&layout, &h, &cfg, book);
    let register = crate::chorus::register::scan_register(&layout, &h, &cfg, book);

    if json {
        let head_hops: Vec<serde_json::Value> = findings
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
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "head_hops": head_hops,
                "tense": tense_json(&tense),
                "register": register_json(&register),
            }))
            .unwrap_or_default()
        );
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
        println!(
            "  · declare a scene's POV with a `pov:<name>` / `pov:first` / `pov:omniscient` tag"
        );
    }
    println!("{}", "─".repeat(64));
    print_tense(&tense);
    println!("{}", "─".repeat(64));
    print_register(&register);
    println!("{}", "─".repeat(64));
    Ok(())
}

fn register_json(r: &crate::chorus::register::RegisterReport) -> serde_json::Value {
    serde_json::json!({
        "chapters": r.chapters.iter().map(|c| serde_json::json!({
            "chapter": c.chapter_ord,
            "contraction_rate": c.register.contraction_rate,
            "archaism_density": c.register.archaism_density,
            "formality": c.register.formality,
            "latinate_density": c.register.latinate_density,
        })).collect::<Vec<_>>(),
        "drifts": r.drifts.iter().map(|d| serde_json::json!({
            "chapter": d.chapter_ord, "metric": d.metric,
            "baseline": d.baseline, "value": d.value, "delta": d.delta,
        })).collect::<Vec<_>>(),
    })
}

fn print_register(r: &crate::chorus::register::RegisterReport) {
    println!("Register & diction (advisory, vs. chapter 1)");
    if r.chapters.len() < 2 {
        println!("  (need at least two substantial chapters to compare register)");
        return;
    }
    if r.drifts.is_empty() {
        println!("  ✓ register holds across the chapters");
    } else {
        for d in &r.drifts {
            let dir = if d.delta >= 0.0 { "rose" } else { "fell" };
            println!(
                "  ⚠ ch.{}  {} {dir} to {:.3} (ch.1 {:.3}, Δ {:+.3})",
                d.chapter_ord, d.metric, d.value, d.baseline, d.delta
            );
        }
    }
}

fn tense_json(t: &crate::chorus::tense::TenseSummary) -> serde_json::Value {
    use crate::chorus::tense::TenseSummary;
    match t {
        TenseSummary::Unsupported(reason) => serde_json::json!({"supported": false, "reason": reason}),
        TenseSummary::Scanned(scenes) => serde_json::json!({
            "supported": true,
            "slips": scenes.iter().map(|s| serde_json::json!({
                "chapter": s.chapter_ord,
                "scene": s.scene_index,
                "first_para": s.first_para.to_string(),
                "dominant": s.dominant.label(),
                "sentences": s.slips.iter().map(|sl| serde_json::json!({
                    "tense": sl.tense.label(), "excerpt": sl.excerpt,
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        }),
    }
}

fn print_tense(t: &crate::chorus::tense::TenseSummary) {
    use crate::chorus::tense::TenseSummary;
    println!("Tense discipline (advisory, English-only)");
    match t {
        TenseSummary::Unsupported(reason) => println!("  — not analysed: {reason}"),
        TenseSummary::Scanned(scenes) if scenes.is_empty() => {
            println!("  ✓ narration holds a consistent tense")
        }
        TenseSummary::Scanned(scenes) => {
            for s in scenes {
                println!(
                    "  ch.{} · scene {} (dominant: {})",
                    s.chapter_ord,
                    s.scene_index,
                    s.dominant.label()
                );
                for sl in &s.slips {
                    println!("      ⚠ {}-tense slip: \"{}\"", sl.tense.label(), sl.excerpt);
                }
            }
        }
    }
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
