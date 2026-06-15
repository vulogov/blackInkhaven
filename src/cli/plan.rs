//! 1.3.2 PLANNING-1 P0 — `inkhaven plan` subcommand.
//!
//! `plan init` scaffolds a story-structure framework's beats into the
//! `Planning` system book as HJSON-fronted paragraphs (the Threads
//! pattern).  The deterministic coverage/pacing report (`plan check`) and
//! the AI analyze pass arrive in later phases.

use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::planning::Framework;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::NodeKind;
use crate::store::{InsertPosition, Store, SYSTEM_TAG_PLANNING};

use super::PlanCommand;

pub fn run(project: &Path, cmd: PlanCommand) -> Result<()> {
    match cmd {
        PlanCommand::Init { framework } => init(project, framework.as_deref()),
        PlanCommand::Check {
            book_name,
            json,
            drift,
        } => check(project, book_name.as_deref(), json, drift),
    }
}

fn check(project: &Path, book_name: Option<&str>, json: bool, drift_pct: Option<u32>) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| Error::Store(e.to_string()))?;
    let h = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;

    let book = super::resolve_user_book(&h, book_name, "plan check")
        .map_err(Error::Store)?
        .clone();
    let drift = (drift_pct.unwrap_or(10) as f32) / 100.0;
    let (report, fw, n_chapters) = build_report(&store, &layout, &h, &book, drift)?;

    if json {
        let out = serde_json::to_string_pretty(&report)
            .map_err(|e| Error::Store(format!("plan check: {e}")))?;
        println!("{out}");
    } else {
        render(&report, &book.title, n_chapters, &fw, drift);
    }
    Ok(())
}

/// Build the structure report for `book`: load the Planning-book beats,
/// measure them against the book's chapter positions, and analyze.
/// Returns `(report, framework_slug, chapter_count)`.  Shared by `plan
/// check` and the TUI structure-outline view.
pub(crate) fn build_report(
    store: &Store,
    layout: &ProjectLayout,
    h: &Hierarchy,
    book: &crate::store::node::Node,
    drift: f32,
) -> Result<(crate::planning::PlanReport, String, usize)> {
    let planning = planning_book(h)?;
    let beats: Vec<crate::planning::Beat> = h
        .children_of(Some(planning.id))
        .iter()
        .filter(|n| n.kind == NodeKind::Paragraph)
        .filter_map(|n| store.get_content(n.id).ok().flatten())
        .filter_map(|bytes| crate::planning::parse_beat(&String::from_utf8_lossy(&bytes)))
        .collect();
    if beats.is_empty() {
        return Err(Error::Store(
            "plan: no beats yet — run `inkhaven plan init` first".into(),
        ));
    }
    let chapters = chapter_positions(layout, h, book);
    if chapters.is_empty() {
        return Err(Error::Store(format!(
            "plan: `{}` has no chapters to measure against",
            book.title
        )));
    }
    let fw = beats.first().map(|b| b.framework.clone()).unwrap_or_default();
    let n = chapters.len();
    Ok((crate::planning::analyze(&beats, &chapters, drift), fw, n))
}

/// Each chapter's slug + start position as a fraction of the book's total
/// words (scene-break markers excluded, as the manuscript export does).
fn chapter_positions(
    layout: &ProjectLayout,
    h: &Hierarchy,
    book: &crate::store::node::Node,
) -> Vec<crate::planning::ChapterPos> {
    let mut raw: Vec<(String, usize)> = Vec::new();
    for ch in h.children_of(Some(book.id)) {
        if ch.kind != NodeKind::Chapter {
            continue;
        }
        let words: usize = crate::cli::book_walk::chapter_paragraphs_raw(layout, h, ch.id)
            .iter()
            .map(|p| crate::audiobook::typst_to_plain(p))
            .filter(|p| !crate::manuscript::is_scene_break(p))
            .map(|p| crate::progress::count_words(&p).max(0) as usize)
            .sum();
        raw.push((ch.slug.clone(), words));
    }
    let total = raw.iter().map(|(_, w)| *w).sum::<usize>().max(1);
    let mut cum = 0usize;
    raw.into_iter()
        .map(|(slug, w)| {
            let start = cum as f32 / total as f32;
            cum += w;
            crate::planning::ChapterPos { slug, start }
        })
        .collect()
}

fn render(
    report: &crate::planning::PlanReport,
    book_title: &str,
    chapter_count: usize,
    framework_slug: &str,
    drift: f32,
) {
    let fw = Framework::parse(framework_slug)
        .map(|f| f.label().to_string())
        .unwrap_or_else(|| framework_slug.to_string());
    println!("plan check · {book_title} · {fw} · {chapter_count} chapter(s)");
    println!("\nBEATS");
    for b in &report.beats {
        let (icon, detail) = match (&b.mapped_chapter, b.actual_position, b.drift) {
            (None, _, _) => ('✗', "(unmapped)".to_string()),
            (Some(ch), Some(a), Some(d)) => {
                let icon = if d.abs() > drift { '⚠' } else { '✓' };
                (icon, format!("→ {ch} ({:.0}%, {:+.0}%)", a * 100.0, d * 100.0))
            }
            (Some(ch), _, _) => ('?', format!("→ {ch} (chapter not found)")),
        };
        println!(
            "  {icon} {:<28} act {}  target {:>3.0}%  {detail}",
            b.beat,
            b.act,
            b.target_position * 100.0,
        );
    }
    println!("\nPACING (act word-share)");
    for p in &report.acts {
        let actual = p.actual.map(|a| format!("{:.0}%", a * 100.0)).unwrap_or_else(|| "?".into());
        let flag = match p.actual {
            Some(a) if (a - p.expected).abs() > drift => {
                if a > p.expected {
                    " ⚠ long"
                } else {
                    " ⚠ short"
                }
            }
            _ => "",
        };
        println!(
            "  Act {}  expected {:>3.0}%  actual {:>4}{flag}",
            p.act,
            p.expected * 100.0,
            actual,
        );
    }
    println!();
    if report.warnings.is_empty() {
        println!("✓ no structural warnings");
    } else {
        println!("{} finding(s):", report.warnings.len());
        for w in &report.warnings {
            println!("  ⚠ {w}");
        }
    }
}

fn init(project: &Path, framework: Option<&str>) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| Error::Store(e.to_string()))?;

    let fw = match framework {
        Some(s) => Framework::parse(s).ok_or_else(|| {
            Error::Store(format!(
                "plan init: unknown framework `{s}` \
                 (three_act|save_the_cat|story_circle|hero_journey|seven_point)"
            ))
        })?,
        None => Framework::ThreeAct,
    };

    let h = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;
    let planning = planning_book(&h)?;
    // Refuse to clobber an existing structure.
    if !h.children_of(Some(planning.id)).is_empty() {
        return Err(Error::Store(
            "plan init: the Planning book already has beats — remove them to re-init".into(),
        ));
    }

    let beats = fw.seed_beats();
    for beat in &beats {
        // Reload before each create so later beats see the earlier ones
        // (slug + order) — the same pattern facts/threads use.
        let h = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;
        let planning = planning_book(&h)?;
        let mut node = store.create_node(
            &cfg,
            &h,
            NodeKind::Paragraph,
            &beat.beat,
            Some(&planning),
            None,
            InsertPosition::End,
        )?;
        let body = crate::planning::beat_body(beat);
        node.content_type = Some("hjson".to_string());
        // Disk write first (the editor reads the .typ off disk), then the
        // bdslib-only metadata/content update — the Threads pattern.
        if let Some(rel) = &node.file {
            let abs = store.project_root().join(rel);
            std::fs::write(&abs, body.as_bytes()).map_err(Error::Io)?;
        }
        store
            .update_paragraph_content(&mut node, body.as_bytes())
            .map_err(|e| Error::Store(format!("plan init: seed beat: {e}")))?;
    }

    println!(
        "plan init: seeded {} {} beats into the Planning book",
        beats.len(),
        fw.label(),
    );
    eprintln!("  next: map each beat to a chapter, then `inkhaven plan check` (P1)");
    Ok(())
}

fn planning_book(h: &Hierarchy) -> Result<crate::store::node::Node> {
    h.iter()
        .find(|n| {
            n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_PLANNING)
        })
        .cloned()
        .ok_or_else(|| {
            Error::Store("plan init: Planning book missing — reopen the project to seed it".into())
        })
}
