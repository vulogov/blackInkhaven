//! CHORUS-1 — `inkhaven chorus …` commands (voice & style at book scale). CH-P1
//! ships `voices` (character voice fingerprints); CH-P2/P8 add `distinct` /
//! `report` / `scan`. (Distinct from `inkhaven style`, the editor style-warning
//! report.)

use std::path::Path;

use crate::chorus::voices::{CharacterVoice, Confidence, character_profiles};
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

    let mut voices = character_profiles(&pstore, &ds, &cfg, book, None, &now())
        .map_err(|e| Error::Store(e.to_string()))?;
    if let Some(name) = character {
        voices.retain(|v| v.name.eq_ignore_ascii_case(name));
    }

    if json {
        let arr: Vec<serde_json::Value> = voices.iter().map(voice_json).collect();
        println!("{}", serde_json::to_string_pretty(&arr).unwrap_or_default());
        return Ok(());
    }

    print_cards(&book.title, &cfg.language, &voices);
    Ok(())
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

fn print_cards(book_title: &str, language: &str, voices: &[CharacterVoice]) {
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
        voices.iter().filter(|v| v.confidence != Confidence::Low).collect();
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
