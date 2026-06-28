//! DIALOG-1 (D-P7) — `inkhaven dialogue …` commands.
//!
//! - `scan`    — detect + print findings (exits non-zero on zero-attribution).
//! - `profile` — per-character dialogue fingerprints.
//! - `refresh` — force full recomputation (bypasses the content-hash cache).
//! - `suggest` — deterministic, template-driven chapter summary (no LLM).

use std::path::Path;

use crate::config::Config;
use crate::dialogue::{
    DialogueFinding, DialogueFindingKind, DialogueStore, character_names, refresh_book,
};
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;
use crate::store::{NodeKind, Store};

use super::DialogueCommand;

pub fn run(project: &Path, cmd: DialogueCommand) -> Result<()> {
    match cmd {
        DialogueCommand::Scan { book, findings, json } => {
            scan(project, book.as_deref(), findings.as_deref(), json)
        }
        DialogueCommand::Profile { book, character, json } => {
            profile(project, book.as_deref(), character.as_deref(), json)
        }
        DialogueCommand::Refresh { book, chapter } => refresh(project, book.as_deref(), chapter),
        DialogueCommand::Suggest { book, chapter } => suggest(project, book.as_deref(), chapter),
    }
}

fn open(project: &Path) -> Result<(ProjectLayout, Config, Store, Hierarchy)> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let h = Hierarchy::load(&store)?;
    Ok((layout, cfg, store, h))
}

fn computed_now() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn dstore(store: &Store) -> Result<DialogueStore> {
    DialogueStore::open(store.project_root()).map_err(|e| Error::Store(e.to_string()))
}

fn run_refresh(
    layout: &ProjectLayout,
    cfg: &Config,
    ds: &DialogueStore,
    h: &Hierarchy,
    book: &Node,
) -> Result<Vec<DialogueFinding>> {
    refresh_book(ds, layout, h, cfg, book, None, &computed_now())
        .map_err(|e| Error::Store(e.to_string()))
}

/// Force a full recompute (clear every chapter so the hash check misses), then
/// refresh — the explicit-pass surfaces (`scan`, `suggest`) always need the
/// complete current finding set, not just the lazily-recomputed delta.
fn force_refresh(
    layout: &ProjectLayout,
    cfg: &Config,
    ds: &DialogueStore,
    h: &Hierarchy,
    book: &Node,
) -> Result<Vec<DialogueFinding>> {
    let chapters = h
        .children_of(Some(book.id))
        .into_iter()
        .filter(|n| n.kind == NodeKind::Chapter)
        .count();
    for ord in 1..=chapters as u32 {
        ds.clear_chapter(&book.slug, ord).map_err(|e| Error::Store(e.to_string()))?;
    }
    run_refresh(layout, cfg, ds, h, book)
}

fn kind_matches(filter: Option<&str>, k: DialogueFindingKind) -> bool {
    match filter {
        None | Some("all") => true,
        Some("zero-attribution") => k == DialogueFindingKind::ZeroAttribution,
        Some("said-bookism") => k == DialogueFindingKind::SaidBookism,
        Some("talking-heads") => k == DialogueFindingKind::TalkingHead,
        _ => true,
    }
}

fn scan(project: &Path, book_name: Option<&str>, findings_filter: Option<&str>, json: bool) -> Result<()> {
    let (layout, cfg, store, h) = open(project)?;
    let book = super::resolve_user_book(&h, book_name, "dialogue").map_err(Error::Store)?;
    let ds = dstore(&store)?;
    let all = force_refresh(&layout, &cfg, &ds, &h, book)?;
    let shown: Vec<&DialogueFinding> =
        all.iter().filter(|f| kind_matches(findings_filter, f.kind)).collect();

    if json {
        let arr: Vec<serde_json::Value> = shown
            .iter()
            .map(|f| {
                serde_json::json!({
                    "kind": f.kind.as_code(),
                    "chapter": f.chapter_ord,
                    "para_id": f.para_id,
                    "detail": f.detail,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr).unwrap_or_default());
    } else {
        let stats = ds.all_chapter_stats(&book.slug).map_err(|e| Error::Store(e.to_string()))?;
        let total_spans: u32 = stats.iter().map(|s| s.total_spans).sum();
        let zero: u32 = stats.iter().map(|s| s.zero_attribution_count).sum();
        let th: u32 = stats.iter().map(|s| s.talking_head_sequences).sum();
        println!("Dialogue scan — `{}` [{}]", book.title, cfg.language);
        println!("{}", "─".repeat(66));
        println!("  dialogue spans detected   {total_spans}");
        println!("  zero attribution          {zero}");
        println!("  talking-head sequences    {th}");
        println!("{}", "─".repeat(66));
        if shown.is_empty() {
            println!("  no findings");
        } else {
            for f in &shown {
                match &f.para_id {
                    Some(p) => println!("  [ch.{} · {}] {}", f.chapter_ord, &p[..p.len().min(8)], f.detail),
                    None => println!("  [ch.{}] {}", f.chapter_ord, f.detail),
                }
            }
        }
        println!("{}", "─".repeat(66));
    }

    // Exit non-zero if any zero-attribution finding exists (CI gate, RFC §10.1).
    if all.iter().any(|f| f.kind == DialogueFindingKind::ZeroAttribution) {
        return Err(Error::Store("unattributed dialogue found".into()));
    }
    Ok(())
}

fn profile(project: &Path, book_name: Option<&str>, character: Option<&str>, json: bool) -> Result<()> {
    let (layout, cfg, store, h) = open(project)?;
    let book = super::resolve_user_book(&h, book_name, "dialogue").map_err(Error::Store)?;
    let ds = dstore(&store)?;
    run_refresh(&layout, &cfg, &ds, &h, book)?;

    let fps = match character {
        Some(name) => ds
            .fingerprint(&book.slug, name)
            .map_err(|e| Error::Store(e.to_string()))?
            .into_iter()
            .collect(),
        None => ds.all_fingerprints(&book.slug).map_err(|e| Error::Store(e.to_string()))?,
    };

    if json {
        let arr: Vec<serde_json::Value> = fps
            .iter()
            .map(|f| {
                serde_json::json!({
                    "character": f.character_name,
                    "utterances": f.utterance_count,
                    "mean_words": f.mean_utterance_words,
                    "mattr": f.utterance_mattr,
                    "question_ratio": f.question_ratio,
                    "exclamation_ratio": f.exclamation_ratio,
                    "hedge_density": f.hedge_density,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr).unwrap_or_default());
        return Ok(());
    }

    println!("Dialogue fingerprints — `{}` [{}]", book.title, cfg.language);
    println!("{}", "─".repeat(71));
    println!(
        "{:<14}{:>5}{:>11}{:>7}{:>11}{:>7}{:>9}",
        "Character", "Utts", "Avg words", "MATTR", "Questions", "Excl.", "Hedging"
    );
    println!("{}", "─".repeat(71));
    if fps.is_empty() {
        println!("  (no attributed dialogue yet — run `inkhaven dialogue scan`)");
    }
    for f in &fps {
        println!(
            "{:<14}{:>5}{:>11.1}{:>7.2}{:>11.2}{:>7.2}{:>9.3}",
            f.character_name,
            f.utterance_count,
            f.mean_utterance_words,
            f.utterance_mattr,
            f.question_ratio,
            f.exclamation_ratio,
            f.hedge_density
        );
    }
    let low: Vec<&str> = fps
        .iter()
        .filter(|f| f.utterance_count < 5)
        .map(|f| f.character_name.as_str())
        .collect();
    if !low.is_empty() {
        println!("{}", "─".repeat(71));
        println!(
            "Note: {} has < 5 certain attributions — fingerprint may be incomplete.",
            low.join(", ")
        );
    }
    println!("{}", "─".repeat(71));
    Ok(())
}

fn refresh(project: &Path, book_name: Option<&str>, chapter: Option<u32>) -> Result<()> {
    let (layout, cfg, store, h) = open(project)?;
    let book = super::resolve_user_book(&h, book_name, "dialogue").map_err(Error::Store)?;
    let ds = dstore(&store)?;
    // Force: clear stored chapters so the hash check misses and recomputes.
    let chapters: Vec<&Node> = h
        .children_of(Some(book.id))
        .into_iter()
        .filter(|n| n.kind == NodeKind::Chapter)
        .collect();
    for (idx, _) in chapters.iter().enumerate() {
        let ord = (idx + 1) as u32;
        if chapter.is_none() || chapter == Some(ord) {
            ds.clear_chapter(&book.slug, ord).map_err(|e| Error::Store(e.to_string()))?;
        }
    }
    let findings = run_refresh(&layout, &cfg, &ds, &h, book)?;
    eprintln!(
        "dialogue refresh: `{}` recomputed — {} finding(s)",
        book.title,
        findings.len()
    );
    Ok(())
}

fn suggest(project: &Path, book_name: Option<&str>, chapter: Option<u32>) -> Result<()> {
    let (layout, cfg, store, h) = open(project)?;
    let book = super::resolve_user_book(&h, book_name, "dialogue").map_err(Error::Store)?;
    let ds = dstore(&store)?;
    let findings = force_refresh(&layout, &cfg, &ds, &h, book)?;
    let names = character_names(&h);
    let _ = names;

    let stats = ds.all_chapter_stats(&book.slug).map_err(|e| Error::Store(e.to_string()))?;
    for s in &stats {
        if let Some(c) = chapter {
            if s.chapter_ord != c {
                continue;
            }
        }
        let chap_findings: Vec<&DialogueFinding> =
            findings.iter().filter(|f| f.chapter_ord == s.chapter_ord).collect();
        println!("Chapter {} dialogue summary:\n", s.chapter_ord);

        let zero = chap_findings.iter().filter(|f| f.kind == DialogueFindingKind::ZeroAttribution).count();
        if zero > 0 {
            println!(
                "Attribution: {zero} unattributed speech span(s). These are the most\n\
                 structurally risky findings — a reader who loses the speaker must\n\
                 re-read backwards. Consider adding a name or an action beat.\n"
            );
        }
        if chap_findings.iter().any(|f| f.kind == DialogueFindingKind::SaidBookism) {
            println!(
                "Said-bookisms: this chapter's non-neutral tag density ({:.2}) runs\n\
                 above the book baseline. Whether that's intentional texture or an\n\
                 accumulated habit is your call.\n",
                s.said_bookism_density
            );
        }
        if s.talking_head_sequences > 0 {
            println!(
                "Talking heads: {} run(s) of pure dialogue with no action beat — the\n\
                 scene may read as disembodied. A physical beat would ground it.\n",
                s.talking_head_sequences
            );
        }
        if zero == 0 && s.talking_head_sequences == 0
            && !chap_findings.iter().any(|f| f.kind == DialogueFindingKind::SaidBookism)
        {
            println!("No dialogue findings. Attribution, tag discipline, and grounding all read clean.\n");
        }
        println!(
            "Dialogue density: {:.0}% of words are inside speech.\n",
            s.dialogue_density_ratio * 100.0
        );
    }
    Ok(())
}
