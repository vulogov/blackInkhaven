//! LECTOR-1 LR-P3 — the forward reader-state walk (the AUDIENCE value core).
//!
//! Reads the book **forward, once**, carrying the reader's accumulating state —
//! which entities they've met, which threads are open, how the energy is running —
//! and derives the reader-experience problems a first reader would hit, with **no
//! LLM** (LR-P4 adds the synthetic read on top). The discipline is forward-only: a
//! finding is computed from the chapters read *so far*, never from what comes
//! after, so a later payoff never retroactively cancels an earlier dip.
//!
//! It is pure orchestration over pieces already in the tree: SENTINEL's
//! `introduce` (an entity used before it's introduced ⇒ *confusion*), `tension.rs`
//! (a setup never paid off ⇒ *unpaid_setup*), the roster (first mentions ⇒
//! *info_dump*), and LR-P1's measured intensity (a flat, eventless run ⇒
//! *attention_dip* / *put_down_risk*).

use std::collections::HashSet;

use super::intensity;
use super::{ChapterRead, ReadThrough, ReaderFinding, Severity};
use crate::config::Config;
use crate::continuity_intel::introduce;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::Store;
use crate::tension::{TensionKind, TensionLedger};

/// New names in one chapter at/above which the reader is likely overwhelmed.
const INFO_DUMP_MIN: usize = 4;
const INFO_DUMP_CONCERN: usize = 7;
/// Measured intensity below which a chapter reads "low energy".
const DIP_FLOOR: f32 = 0.15;
/// Consecutive flat + eventless chapters that read as a put-down point.
const PUT_DOWN_RUN: usize = 3;

/// Read the manuscript forward and return the whole read-through: the per-chapter
/// reads (state + findings) with the deterministic reader findings attached. The
/// shape `curve` is left to LR-P5 (built from `measured_intensity` + the framework).
pub(crate) fn read_forward(
    store: &Store,
    cfg: &Config,
    layout: &ProjectLayout,
    h: &Hierarchy,
) -> ReadThrough {
    let _ = store; // reserved (roster/threads/intensity are read via layout + h)
    let chapters = h.user_book_chapters();
    let intensities = intensity::measure(layout, h, cfg);
    let ledger = TensionLedger::load(&layout.root).unwrap_or_default();

    // The roster the reader is meeting: character + place entries.
    let roster: Vec<String> = introduce::roster(h, crate::store::SYSTEM_TAG_CHARACTERS)
        .into_iter()
        .chain(introduce::roster(h, crate::store::SYSTEM_TAG_PLACES))
        .map(|(_, name)| name)
        .filter(|n| !n.trim().is_empty())
        .collect();

    // ── the forward walk: build each ChapterRead with its accumulating state ──
    let mut reads: Vec<ChapterRead> = Vec::with_capacity(chapters.len());
    let mut seen: HashSet<String> = HashSet::new();
    for (idx, (chapter_id, title)) in chapters.iter().enumerate() {
        let ordinal = (idx + 1) as u32;
        let raw = crate::cli::book_walk::chapter_raw_prose(layout, h, *chapter_id);
        let text_lc = crate::audiobook::typst_to_plain(&raw).to_lowercase();

        // Names the reader meets for the first time here.
        let mut new_entities = Vec::new();
        for name in &roster {
            let name_lc = name.to_lowercase();
            if !seen.contains(&name_lc) && crate::drift::mentions(&text_lc, &name_lc) {
                seen.insert(name_lc);
                new_entities.push(name.clone());
            }
        }

        let opened_threads: Vec<String> = ledger
            .tags
            .iter()
            .filter(|t| t.kind == TensionKind::Introduce && t.chapter_index == idx)
            .map(|t| t.topic.clone())
            .collect();
        let resolved_threads: Vec<String> = ledger
            .tags
            .iter()
            .filter(|t| t.kind == TensionKind::Resolve && t.chapter_index == idx)
            .map(|t| t.topic.clone())
            .collect();

        reads.push(ChapterRead {
            chapter: ordinal,
            title: title.clone(),
            measured_intensity: intensities.get(idx).and_then(|c| c.intensity),
            new_entities,
            opened_threads,
            resolved_threads,
            findings: Vec::new(),
        });
    }

    // ── derive the deterministic reader findings ──
    confusion_findings(layout, h, &mut reads);
    info_dump_findings(&mut reads);
    dip_and_put_down_findings(&mut reads);
    unpaid_setup_findings(&ledger, &cfg.language, &mut reads);

    // The measured shape curve — `(position, intensity)` per chapter, for the
    // sparkline. (LR-P7 overlays the framework's expected curve.)
    let n = reads.len();
    let curve: Vec<(f32, f32)> = reads
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let pos = if n > 1 { i as f32 / (n - 1) as f32 } else { 0.0 };
            (pos, c.measured_intensity.unwrap_or(0.0))
        })
        .collect();

    ReadThrough { chapters: reads, curve }
}

/// Attach a finding to its chapter's read (1-based; clamped to the valid range).
fn attach(reads: &mut [ChapterRead], chapter: u32, finding: ReaderFinding) {
    let i = (chapter.max(1) as usize - 1).min(reads.len().saturating_sub(1));
    if let Some(r) = reads.get_mut(i) {
        r.findings.push(finding);
    }
}

/// *confusion* — an entity used before it's introduced. Reuses SENTINEL's
/// `introduce` scan wholesale; the too-early reference is exactly a "who is this?"
/// moment for a first reader.
fn confusion_findings(layout: &ProjectLayout, h: &Hierarchy, reads: &mut [ChapterRead]) {
    for f in introduce::scan(layout, h, 0) {
        let dedup_key = ReaderFinding::make_dedup_key("confusion", &f.entities, f.chapter);
        attach(
            reads,
            f.chapter,
            ReaderFinding {
                kind: "confusion",
                severity: Severity::Notice,
                chapter: f.chapter,
                anchor: f.anchor,
                entities: f.entities,
                message: f.message,
                source: "walk",
                dedup_key,
            },
        );
    }
}

/// *info_dump* — too many names to meet in one chapter.
fn info_dump_findings(reads: &mut [ChapterRead]) {
    let dumps: Vec<(u32, Vec<String>)> = reads
        .iter()
        .filter(|r| r.new_entities.len() >= INFO_DUMP_MIN)
        .map(|r| (r.chapter, r.new_entities.clone()))
        .collect();
    for (chapter, names) in dumps {
        let sev = if names.len() >= INFO_DUMP_CONCERN { Severity::Concern } else { Severity::Notice };
        let dedup_key = ReaderFinding::make_dedup_key("info_dump", &[], chapter);
        attach(
            reads,
            chapter,
            ReaderFinding {
                kind: "info_dump",
                severity: sev,
                chapter,
                anchor: None,
                message: format!(
                    "ch. {chapter} introduces {} new names at once ({}) — hard for a reader to hold.",
                    names.len(),
                    names.join(", "),
                ),
                entities: names,
                source: "walk",
                dedup_key,
            },
        );
    }
}

/// A chapter reads flat + eventless: low measured intensity, no new names, no
/// thread movement.
fn is_dead(r: &ChapterRead) -> bool {
    r.measured_intensity.is_some_and(|i| i < DIP_FLOOR)
        && r.new_entities.is_empty()
        && r.opened_threads.is_empty()
        && r.resolved_threads.is_empty()
}

/// *attention_dip* (isolated flat chapters) and *put_down_risk* (a run of them —
/// flagged at the run's end, where the reader has endured it: forward-only).
fn dip_and_put_down_findings(reads: &mut [ChapterRead]) {
    let dead: Vec<u32> = reads.iter().filter(|r| is_dead(r)).map(|r| r.chapter).collect();
    let dead_set: HashSet<u32> = dead.iter().copied().collect();

    // Maximal runs of consecutive dead chapters.
    let mut i = 0;
    while i < reads.len() {
        if !dead_set.contains(&reads[i].chapter) {
            i += 1;
            continue;
        }
        let start = i;
        while i < reads.len() && dead_set.contains(&reads[i].chapter) {
            i += 1;
        }
        let run = &reads[start..i];
        if run.len() >= PUT_DOWN_RUN {
            let (a, b) = (run[0].chapter, run[run.len() - 1].chapter);
            let dedup_key = ReaderFinding::make_dedup_key("put_down_risk", &[], b);
            let f = ReaderFinding {
                kind: "put_down_risk",
                severity: Severity::Concern,
                chapter: b,
                anchor: None,
                entities: Vec::new(),
                message: format!(
                    "ch. {a}\u{2013}{b} run flat and eventless ({} chapters) — a likely put-down point.",
                    run.len(),
                ),
                source: "walk",
                dedup_key,
            };
            attach(reads, b, f);
        } else {
            for r in run.iter().map(|r| r.chapter).collect::<Vec<_>>() {
                let dedup_key = ReaderFinding::make_dedup_key("attention_dip", &[], r);
                attach(
                    reads,
                    r,
                    ReaderFinding {
                        kind: "attention_dip",
                        severity: Severity::Info,
                        chapter: r,
                        anchor: None,
                        entities: Vec::new(),
                        message: format!(
                            "ch. {r} reads low-energy with nothing new — the reader's attention may drift."
                        ),
                        source: "walk",
                        dedup_key,
                    },
                );
            }
        }
    }
}

/// *unpaid_setup* — a tension/question/goal raised but never paid off. Reuses
/// `tension::detect_unresolved`; anchored at the chapter it was raised.
fn unpaid_setup_findings(ledger: &TensionLedger, language: &str, reads: &mut [ChapterRead]) {
    let unresolved = crate::tension::detect_unresolved(ledger, language);
    if unresolved.is_empty() {
        return;
    }
    let open_topics: HashSet<String> =
        unresolved.iter().map(|u| u.topic.to_lowercase()).collect();
    for tag in ledger.tags.iter().filter(|t| t.kind == TensionKind::Introduce) {
        if !open_topics.contains(&tag.topic.to_lowercase()) {
            continue;
        }
        let chapter = (tag.chapter_index + 1) as u32;
        let entities = vec![tag.topic.clone()];
        let dedup_key = ReaderFinding::make_dedup_key("unpaid_setup", &entities, chapter);
        attach(
            reads,
            chapter,
            ReaderFinding {
                kind: "unpaid_setup",
                severity: Severity::Notice,
                chapter,
                anchor: None,
                message: format!(
                    "setup \u{201c}{}\u{201d} (ch. {chapter}) is never paid off — the reader is left waiting.",
                    tag.topic,
                ),
                entities,
                source: "walk",
                dedup_key,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(chapter: u32, intensity: Option<f32>, new: &[&str], opened: &[&str]) -> ChapterRead {
        ChapterRead {
            chapter,
            title: format!("ch{chapter}"),
            measured_intensity: intensity,
            new_entities: new.iter().map(|s| s.to_string()).collect(),
            opened_threads: opened.iter().map(|s| s.to_string()).collect(),
            resolved_threads: Vec::new(),
            findings: Vec::new(),
        }
    }

    #[test]
    fn info_dump_fires_on_a_crowded_chapter() {
        let mut reads = vec![
            read(1, Some(0.5), &["Mara", "Joren", "Aldous", "Sella", "Cael"], &[]),
            read(2, Some(0.5), &["Bram"], &[]),
        ];
        info_dump_findings(&mut reads);
        assert_eq!(reads[0].findings.len(), 1);
        assert_eq!(reads[0].findings[0].kind, "info_dump");
        assert!(reads[1].findings.is_empty(), "one new name is fine");
    }

    #[test]
    fn a_flat_run_is_a_put_down_risk_not_three_dips() {
        let mut reads = vec![
            read(1, Some(0.6), &["Mara"], &[]),
            read(2, Some(0.05), &[], &[]),
            read(3, Some(0.08), &[], &[]),
            read(4, Some(0.10), &[], &[]),
            read(5, Some(0.7), &[], &["the duel"]),
        ];
        dip_and_put_down_findings(&mut reads);
        let all: Vec<&ReaderFinding> = reads.iter().flat_map(|r| r.findings.iter()).collect();
        assert_eq!(all.len(), 1, "the 3-chapter flat run is one put-down risk");
        assert_eq!(all[0].kind, "put_down_risk");
        assert_eq!(all[0].chapter, 4, "flagged at the run's end (forward-only)");
    }

    #[test]
    fn an_isolated_flat_chapter_is_a_dip() {
        let mut reads = vec![
            read(1, Some(0.6), &["Mara"], &[]),
            read(2, Some(0.05), &[], &[]),
            read(3, Some(0.7), &["Joren"], &[]),
        ];
        dip_and_put_down_findings(&mut reads);
        assert_eq!(reads[1].findings.len(), 1);
        assert_eq!(reads[1].findings[0].kind, "attention_dip");
    }

    #[test]
    fn a_chapter_with_movement_is_not_dead() {
        // Low intensity but a thread opens ⇒ not eventless.
        let mut reads = vec![read(1, Some(0.05), &[], &["a mystery"])];
        dip_and_put_down_findings(&mut reads);
        assert!(reads[0].findings.is_empty());
    }
}
