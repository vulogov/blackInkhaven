//! SENTINEL-1 CT-P2 — the unification engine.
//!
//! One `run` that fans out to every deterministic continuity detector already in
//! the tree, maps each native finding into a [`ContinuityFinding`] through a thin
//! adapter, then [`rank`]s (most-severe first) and [`dedupe`]s (folding two
//! detectors' reports of the same break). No detection logic is re-implemented —
//! the adapters *call* the existing engines:
//!
//! - `co_location` — [`crate::world::timeline_context::co_location_conflicts`]
//!   (a character in two places at overlapping times), magic-ledger suppressed.
//! - `timeline` — [`crate::timeline::critique::run`] (orphans + fuzzy overlaps).
//! - `numeric` — [`crate::continuity::detect_contradictions`] (direction
//!   reversal / conflicting durations), per user-book chapter.
//! - `char_facts` — [`crate::continuity_bible::detect_drift`] over the extracted
//!   `.inkhaven/continuity.json` (an established fact changed across chapters).
//! - `introduce` — CT-P1's [`super::introduce::scan`] (referenced-before-introduced).
//!
//! The LLM detectors (drift/coherence/tension) are deliberately *not* here — they
//! stay their own explicit, cost-capped commands (CT-P7 invokes them on demand).

use super::{ContinuityFinding, Severity, dedupe, introduce, rank};
use crate::config::Config;
use crate::continuity_bible::ContinuityBible;
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;
use crate::timeline::critique;

/// Every detector key, in a stable display order. `--only`/`--skip` and the
/// per-detector config (CT-P3) name these.
pub(crate) const DETECTORS: &[&str] =
    &["co_location", "timeline", "numeric", "char_facts", "introduce"];

/// Run every enabled deterministic detector, normalise, rank, and dedupe.
///
/// A detector runs only when BOTH its `continuity:` config toggle
/// ([`crate::config::ContinuityConfig`]) and the caller's `enabled(key)`
/// predicate allow it. The config is the standing gate (a detector switched off
/// there never runs anywhere); `enabled` is the caller-specific narrowing — the
/// CLI's `--only`/`--skip`, or the review pass excluding `timeline` (which it
/// surfaces on its own line). All detectors are deterministic and cheap, so this
/// never touches the network.
pub(crate) fn run(
    store: &Store,
    cfg: &Config,
    layout: &ProjectLayout,
    h: &Hierarchy,
    enabled: &dyn Fn(&str) -> bool,
) -> Vec<ContinuityFinding> {
    let ct = &cfg.continuity;
    let on = |key: &str| ct.detector_enabled(key) && enabled(key);

    let mut out: Vec<ContinuityFinding> = Vec::new();
    if on("co_location") {
        out.extend(co_location(layout, h));
    }
    if on("timeline") {
        out.extend(timeline(cfg, h));
    }
    if on("numeric") {
        out.extend(numeric(cfg, layout, h));
    }
    if on("char_facts") {
        out.extend(char_facts(cfg, layout));
    }
    if on("introduce") {
        out.extend(introduce::scan(layout, h, ct.introduce_tolerance));
    }
    let _ = store; // reserved for CT-P5's scoped re-check
    // Rank before dedupe so the survivor of a folded group is the most severe.
    rank(&mut out);
    dedupe(out)
}

/// Split a `--only` / `--skip` pair into an `enabled` predicate. `only` (when
/// non-empty) is an allow-list; `skip` always subtracts. Unknown keys are ignored
/// by construction (they simply never match a detector).
pub(crate) fn selector(only: &[String], skip: &[String]) -> impl Fn(&str) -> bool + use<> {
    let only: Vec<String> = only.iter().map(|s| s.to_ascii_lowercase()).collect();
    let skip: Vec<String> = skip.iter().map(|s| s.to_ascii_lowercase()).collect();
    move |key: &str| {
        if skip.iter().any(|s| s == key) {
            return false;
        }
        only.is_empty() || only.iter().any(|o| o == key)
    }
}

// ── adapters ───────────────────────────────────────────────────────────────

/// Load the magic ledger from `world.hjson` (empty when absent) so a declared
/// `teleportation`-style rule can excuse a co-location.
fn magic_ledger(layout: &ProjectLayout) -> crate::world::types::MagicLedger {
    std::fs::read_to_string(layout.root.join("world.hjson"))
        .ok()
        .and_then(|raw| crate::world::types::WorldDefinition::from_hjson(&raw).ok())
        .and_then(|d| d.magic)
        .unwrap_or_default()
}

/// Character-in-two-places-at-once, from the timeline alone.
fn co_location(layout: &ProjectLayout, h: &Hierarchy) -> Vec<ContinuityFinding> {
    use crate::world::timeline_context as tc;
    let events = tc::gather_events(h);
    if events.is_empty() {
        return Vec::new();
    }
    let ledger = magic_ledger(layout);
    let name = |id: uuid::Uuid| h.get(id).map(|n| n.title.clone()).unwrap_or_else(|| "?".into());

    tc::co_location_conflicts(&events)
        .into_iter()
        .map(|c| {
            let ctx = crate::world::types::magic::CheckContext {
                category: "co_location",
                ..Default::default()
            };
            let suppressed = ledger.find_suppressor(&ctx).map(|r| r.kind.clone());
            let (severity, tail) = match &suppressed {
                Some(rule) => (Severity::Info, format!(" (ok — magic rule `{rule}`)")),
                None => (Severity::Contradiction, String::new()),
            };
            let ch = name(c.character);
            let entities = vec![ch.clone()];
            let chapter = 0;
            ContinuityFinding {
                kind: "co_location",
                severity,
                chapter,
                anchor: None,
                dedup_key: ContinuityFinding::make_dedup_key("co_location", &entities, chapter),
                entities,
                message: format!(
                    "{ch} is in {} (\u{201c}{}\u{201d}) and {} (\u{201c}{}\u{201d}) at overlapping times.{tail}",
                    name(c.place_a), c.title_a, name(c.place_b), c.title_b,
                ),
                source: "co_location",
            }
        })
        .collect()
}

fn crit_severity(s: critique::CritSeverity) -> Severity {
    match s {
        critique::CritSeverity::Info => Severity::Info,
        critique::CritSeverity::Warning => Severity::Warning,
        critique::CritSeverity::Contradiction => Severity::Contradiction,
    }
}

/// Timeline-internal breaks: orphaned events + fuzzy-precision overlaps.
fn timeline(cfg: &Config, h: &Hierarchy) -> Vec<ContinuityFinding> {
    use crate::timeline::Calendar;
    let calendar = Calendar::from_config(cfg.timeline.calendar.clone());
    let default_track = cfg.timeline.default_track.clone();
    let now = chrono::Utc::now();
    let events: Vec<critique::CritiqueEvent> = h
        .flatten()
        .into_iter()
        .filter_map(|(n, _)| n.event.as_ref().map(|e| (n, e)))
        .map(|(n, ev)| critique::CritiqueEvent {
            id: n.id,
            title: n.title.clone(),
            start_ticks: ev.start_ticks,
            end_ticks: ev.end_ticks,
            precision: ev.precision,
            track: ev.track.clone().unwrap_or_else(|| default_track.clone()),
            is_orphan: ev.is_orphan(&n.linked_paragraphs),
            linked_paragraph_count: n.linked_paragraphs.len(),
            characters: ev.characters.clone(),
            places: ev.places.clone(),
            age_days: Some((now - n.modified_at).num_days().max(0)),
        })
        .collect();
    if events.is_empty() {
        return Vec::new();
    }
    let cc = &cfg.timeline.critique;
    let fuzz = critique::fuzz_windows(&calendar);
    let mut report = critique::run(
        &events,
        &fuzz,
        cc.min_significance(),
        cc.min_suspicion(),
        cc.fuzzy_overlap.cluster_min_size.max(2),
        critique::DEFAULT_STALENESS_DAYS,
    );
    if !cc.orphan.enabled {
        report.orphans.clear();
    }
    if !cc.fuzzy_overlap.enabled {
        report.overlaps.clear();
    }

    let mut out = Vec::new();
    for f in &report.orphans {
        let entities = vec![f.title.clone()];
        out.push(ContinuityFinding {
            kind: "timeline",
            severity: crit_severity(f.severity),
            chapter: 0,
            anchor: None,
            dedup_key: ContinuityFinding::make_dedup_key("timeline_orphan", &entities, 0),
            entities,
            message: format!("orphan event \u{201c}{}\u{201d} — {}", f.title, f.reasons.join(" ")),
            source: "timeline",
        });
    }
    for f in &report.overlaps {
        let entities = f.titles.clone();
        out.push(ContinuityFinding {
            kind: "timeline",
            severity: crit_severity(f.severity),
            chapter: 0,
            anchor: None,
            dedup_key: ContinuityFinding::make_dedup_key("timeline_overlap", &entities, 0),
            entities,
            message: format!("overlapping events: {} — {}", f.titles.join(" + "), f.reasons.join(" ")),
            source: "timeline",
        });
    }
    out
}

/// Numeric / directional self-contradiction, per user-book chapter.
fn numeric(cfg: &Config, layout: &ProjectLayout, h: &Hierarchy) -> Vec<ContinuityFinding> {
    use crate::continuity as num;
    let Some(lex) = num::built_in_lexicon(&cfg.language) else {
        return Vec::new(); // no numeric lexicon for this language — skip cleanly
    };
    let ccfg = num::ContradictionConfig::default();
    let mut out = Vec::new();
    for (chapter_idx, (chapter_id, _title)) in h.user_book_chapters().into_iter().enumerate() {
        let raw = crate::cli::book_walk::chapter_raw_prose(layout, h, chapter_id);
        let plain = crate::audiobook::typst_to_plain(&raw);
        if plain.trim().is_empty() {
            continue;
        }
        let sentences = num::split_sentences(&plain);
        let quantities = num::extract_quantities(&sentences, &lex);
        let chapter = (chapter_idx + 1) as u32;
        for c in num::detect_contradictions(&quantities, &ccfg) {
            let entities = vec![c.a_raw.clone(), c.b_raw.clone()];
            let message = match c.kind {
                num::ContradictionKind::DirectionReversal => format!(
                    "direction reversal: \u{201c}{}\u{201d} then \u{201c}{}\u{201d}",
                    c.a_raw, c.b_raw
                ),
                num::ContradictionKind::TemporalMismatch => format!(
                    "conflicting durations: \u{201c}{}\u{201d} vs \u{201c}{}\u{201d}",
                    c.a_raw, c.b_raw
                ),
            };
            out.push(ContinuityFinding {
                kind: "numeric",
                severity: Severity::Warning,
                chapter,
                anchor: None,
                dedup_key: ContinuityFinding::make_dedup_key("numeric", &entities, chapter),
                entities,
                message,
                source: "numeric",
            });
        }
    }
    out
}

/// A character's established fact changed across chapters (over the extracted
/// bible — SENTINEL reads it, never re-extracts).
fn char_facts(cfg: &Config, layout: &ProjectLayout) -> Vec<ContinuityFinding> {
    let Ok(bible) = ContinuityBible::load(&layout.root) else {
        return Vec::new();
    };
    if bible.facts.is_empty() {
        return Vec::new();
    }
    let lang = if bible.language.trim().is_empty() { &cfg.language } else { &bible.language };
    crate::continuity_bible::detect_drift(&bible, lang)
        .into_iter()
        .map(|d| {
            let entities = vec![d.character.clone(), d.attribute.clone()];
            let values = d
                .conflicts
                .iter()
                .map(|(chapter, value)| format!("{value} ({chapter})"))
                .collect::<Vec<_>>()
                .join(" \u{2192} ");
            ContinuityFinding {
                kind: "char_facts",
                severity: Severity::Warning,
                chapter: 0,
                anchor: None,
                dedup_key: ContinuityFinding::make_dedup_key("char_facts", &entities, 0),
                message: format!("{}'s {} changes: {values}", d.character, d.attribute),
                entities,
                source: "char_facts",
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_only_and_skip() {
        let sel = selector(&["timeline".into(), "numeric".into()], &["numeric".into()]);
        assert!(sel("timeline"));
        assert!(!sel("numeric"), "skip subtracts from only");
        assert!(!sel("co_location"), "not in only");

        let none = selector(&[], &["introduce".into()]);
        assert!(none("timeline"), "empty only = all but skipped");
        assert!(!none("introduce"));
    }

    #[test]
    fn detector_keys_cover_every_adapter() {
        // The public key list must stay in sync with what `run` dispatches.
        assert_eq!(
            DETECTORS,
            &["co_location", "timeline", "numeric", "char_facts", "introduce"]
        );
    }
}
