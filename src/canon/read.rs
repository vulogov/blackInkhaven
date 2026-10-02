//! CANON-READER-1 — canon as a *reader*: the ledger checked against the
//! manuscript, reporting findings the way SENTINEL / KEN / BONDS do.
//!
//! Through 3.15 the ledger only answered questions. This module makes it speak
//! first: a decision whose source paragraph is gone, a paragraph that no longer
//! carries the decision it established, a firm decision resting on a soft one,
//! a foundation nobody has settled — and, opt-in, a passage that contradicts a
//! decision the author committed to.
//!
//! **CR-P0 (this file's first cut) is the pure substrate**: the finding shape,
//! its kinds and how each is routed, the view of the manuscript a detector
//! needs ([`SourceText`]), the word-overlap measure `drifted_source` rests on,
//! and the stable ordering of a report. The detectors arrive in CR-P1, the shell
//! in CR-P2, the worklist bridge in CR-P3. Nothing here reads a store or calls a
//! model; everything is unit-tested on plain values.
//!
//! Advisory throughout: a finding never edits prose and never edits the ledger.

// CR-P0 lays types the later phases consume; remove once CR-P2 wires the shell.
#![allow(dead_code)]

use std::collections::BTreeSet;

use smysl::Uid;
use uuid::Uuid;

use super::grounding::{salient_tokens, stemmer_language_name};
use crate::prose::ProseLanguage;

/// What kind of disagreement between ledger and manuscript a finding reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CanonFindingKind {
    /// A committed/canonical decision that a passage of the book contradicts
    /// (the opt-in `--deep` model pass, CR-P5).
    Contradicted,
    /// A decision committed above something it rests on (the support check).
    BuiltOnSand,
    /// The paragraph that established a decision no longer carries it.
    DriftedSource,
    /// A live decision whose source paragraph no longer exists.
    OrphanedDecision,
    /// Other decisions rest on one that is still unmarked or floated.
    UnsettledFoundation,
}

impl CanonFindingKind {
    /// Every kind, in report order (most serious first — also the `Ord`).
    pub const ALL: [CanonFindingKind; 5] = [
        CanonFindingKind::Contradicted,
        CanonFindingKind::BuiltOnSand,
        CanonFindingKind::DriftedSource,
        CanonFindingKind::OrphanedDecision,
        CanonFindingKind::UnsettledFoundation,
    ];

    /// The stable machine name — the Editorial Pass `category`, the `--json`
    /// field, the CHRONICLE fingerprint. Never rename one.
    pub fn as_str(self) -> &'static str {
        match self {
            CanonFindingKind::Contradicted => "contradicted",
            CanonFindingKind::BuiltOnSand => "built_on_sand",
            CanonFindingKind::DriftedSource => "drifted_source",
            CanonFindingKind::OrphanedDecision => "orphaned_decision",
            CanonFindingKind::UnsettledFoundation => "unsettled_foundation",
        }
    }

    /// The heading a report groups this kind under.
    pub fn heading(self) -> &'static str {
        match self {
            CanonFindingKind::Contradicted => "Contradicted by the prose",
            CanonFindingKind::BuiltOnSand => "Built on sand",
            CanonFindingKind::DriftedSource => "Drifted from its source",
            CanonFindingKind::OrphanedDecision => "Orphaned (source paragraph gone)",
            CanonFindingKind::UnsettledFoundation => "Unsettled foundations",
        }
    }

    /// The short marker shown beside a decision in the Canon pane / dashboard.
    pub fn marker(self) -> &'static str {
        match self {
            CanonFindingKind::Contradicted => "⚠ contradicted",
            CanonFindingKind::BuiltOnSand => "⚠ on sand",
            CanonFindingKind::DriftedSource => "⚠ drifted",
            CanonFindingKind::OrphanedDecision => "⚠ orphaned",
            CanonFindingKind::UnsettledFoundation => "⚠ unsettled",
        }
    }

    /// Whether resolving it is an authorial *choice between two truths* — the
    /// scene or the ledger (an Editorial-Pass **Decision**) — rather than
    /// ledger housekeeping the author does by hand (a **Brief**).
    pub fn is_decision(self) -> bool {
        matches!(self, CanonFindingKind::Contradicted | CanonFindingKind::DriftedSource)
    }

    /// Whether the kind is produced by the deterministic core (free, always on)
    /// rather than the opt-in model pass.
    pub fn is_deterministic(self) -> bool {
        !matches!(self, CanonFindingKind::Contradicted)
    }
}

/// One thing the ledger and the manuscript disagree about.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonFinding {
    pub kind: CanonFindingKind,
    /// The decision the finding is about.
    pub decision: Uid,
    /// That decision's gist (so a report needs no second ledger read).
    pub gist: String,
    /// The paragraph to jump to: the decision's source, or — for a
    /// contradiction — the passage that contradicts it. `None` when there is no
    /// paragraph to open (an orphaned decision).
    pub node: Option<Uuid>,
    /// The manuscript breadcrumb of `node`, when known.
    pub locator: Option<String>,
    /// The other decision involved, if any (the weaker ground of a
    /// built-on-sand finding).
    pub related: Option<Uid>,
    /// One human sentence: what is wrong and what the author can do.
    pub message: String,
    /// Rank within the kind, larger first (e.g. how many decisions rest on an
    /// unsettled foundation).
    pub weight: u32,
}

/// What a detector needs to know about the manuscript: whether a node still
/// exists, and its text. The reader never touches the store itself — the shell
/// and the editor each hand it this view (and the tests hand it a map).
pub trait SourceText {
    /// The current text of `node`: `None` when the node no longer exists,
    /// `Some("")` when it exists but is empty.
    fn text(&self, node: Uuid) -> Option<String>;
}

impl SourceText for std::collections::HashMap<Uuid, String> {
    fn text(&self, node: Uuid) -> Option<String> {
        self.get(&node).cloned()
    }
}

/// How much of a decision still shows in a piece of prose: the decision's
/// content words (stemmed, stop-words dropped, in the project language) and how
/// many of them the prose still contains.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Overlap {
    /// Content words of the decision found in the prose.
    pub matched: usize,
    /// Content words the decision has.
    pub total: usize,
}

impl Overlap {
    /// The decision has content words and the prose carries **none** of them —
    /// the conservative "this paragraph is no longer about that" signal. A
    /// decision with no content words (too short to judge) never drifts.
    pub fn is_gone(self) -> bool {
        self.total > 0 && self.matched == 0
    }
}

/// Measure [`Overlap`] between a decision's gist and a paragraph. Multilingual
/// by construction: both sides go through the same stemmer and stop-word list
/// grounding inference uses, so "freezes" ~ "froze"-class variants and Russian
/// case endings match, and function words never count.
pub fn overlap(gist: &str, prose: &str, language: &ProseLanguage) -> Overlap {
    let lang = stemmer_language_name(language);
    let wanted: BTreeSet<String> = salient_tokens(gist, lang);
    if wanted.is_empty() {
        return Overlap { matched: 0, total: 0 };
    }
    let have: BTreeSet<String> = salient_tokens(prose, lang);
    Overlap { matched: wanted.intersection(&have).count(), total: wanted.len() }
}

/// Put a report in its stable order: by kind (most serious first), then weight
/// (largest first), then location, then gist — and drop exact repeats. Stable
/// order matters beyond tidiness: CHRONICLE diffs findings between milestones.
pub fn sort_findings(mut findings: Vec<CanonFinding>) -> Vec<CanonFinding> {
    findings.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then(b.weight.cmp(&a.weight))
            .then(a.locator.cmp(&b.locator))
            .then(a.gist.cmp(&b.gist))
            .then(a.message.cmp(&b.message))
    });
    findings.dedup_by(|a, b| {
        a.kind == b.kind && a.decision == b.decision && a.node == b.node && a.related == b.related
    });
    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canon::{CanonLedger, NarrativeKind};

    fn uid(led: &CanonLedger, gist: &str) -> Uid {
        led.record_decision(NarrativeKind::WorldFact, gist, Uuid::from_u128(1), "ch1", &[]).unwrap()
    }

    fn finding(kind: CanonFindingKind, decision: Uid, gist: &str, weight: u32, loc: &str) -> CanonFinding {
        CanonFinding {
            kind,
            decision,
            gist: gist.into(),
            node: Some(Uuid::from_u128(1)),
            locator: Some(loc.into()),
            related: None,
            message: format!("{gist} — {}", kind.as_str()),
            weight,
        }
    }

    #[test]
    fn kinds_have_stable_unique_names_and_a_routing() {
        let names: Vec<&str> = CanonFindingKind::ALL.iter().map(|k| k.as_str()).collect();
        assert_eq!(
            names,
            ["contradicted", "built_on_sand", "drifted_source", "orphaned_decision", "unsettled_foundation"]
        );
        let unique: std::collections::HashSet<&str> = names.iter().copied().collect();
        assert_eq!(unique.len(), names.len());
        for k in CanonFindingKind::ALL {
            assert!(!k.heading().is_empty() && k.marker().starts_with('⚠'));
        }
        // The scene-or-ledger choices are Decisions; the rest are Briefs.
        assert!(CanonFindingKind::DriftedSource.is_decision());
        assert!(CanonFindingKind::Contradicted.is_decision());
        assert!(!CanonFindingKind::OrphanedDecision.is_decision());
        assert!(!CanonFindingKind::BuiltOnSand.is_decision());
        // Only the contradiction pass needs a model.
        assert_eq!(CanonFindingKind::ALL.iter().filter(|k| !k.is_deterministic()).count(), 1);
        // `ALL` is in `Ord` order — the order a report is sorted in.
        let mut sorted = CanonFindingKind::ALL.to_vec();
        sorted.sort();
        assert_eq!(sorted, CanonFindingKind::ALL.to_vec());
    }

    #[test]
    fn overlap_sees_a_decision_through_inflection_and_misses_an_unrelated_scene() {
        let en = ProseLanguage::En;
        let gist = "the harbour freezes each winter";
        let still = "By midwinter the harbour had frozen solid, and the winters were long.";
        let o = overlap(gist, still, &en);
        assert!(o.total >= 2 && o.matched >= 1, "{o:?}");
        assert!(!o.is_gone());

        let rewritten = "Mara counted the coins twice and said nothing to her brother.";
        assert!(overlap(gist, rewritten, &en).is_gone());

        // Too little to judge never drifts.
        assert!(!overlap("it is", rewritten, &en).is_gone());
        assert!(!overlap("", rewritten, &en).is_gone());
    }

    #[test]
    fn overlap_works_in_russian() {
        let ru = ProseLanguage::Ru;
        let gist = "гавань замерзает каждую зиму";
        let still = "К середине зимы гавань замёрзла, и корабли встали у причалов.";
        let o = overlap(gist, still, &ru);
        assert!(o.matched >= 1, "inflected Russian forms should match: {o:?}");
        let other = "Мара дважды пересчитала монеты и ничего не сказала брату.";
        assert!(overlap(gist, other, &ru).is_gone());
    }

    #[test]
    fn a_report_sorts_by_kind_then_weight_and_drops_repeats() {
        let dir = tempfile::tempdir().unwrap();
        let led = CanonLedger::new(dir.path().join("canon.cbor").to_str().unwrap());
        let (a, b, c) = (uid(&led, "alpha fact"), uid(&led, "beta fact"), uid(&led, "gamma fact"));
        let sorted = sort_findings(vec![
            finding(CanonFindingKind::UnsettledFoundation, a, "alpha fact", 2, "ch1"),
            finding(CanonFindingKind::OrphanedDecision, b, "beta fact", 0, "ch2"),
            finding(CanonFindingKind::UnsettledFoundation, c, "gamma fact", 9, "ch3"),
            finding(CanonFindingKind::BuiltOnSand, a, "alpha fact", 0, "ch1"),
            finding(CanonFindingKind::OrphanedDecision, b, "beta fact", 0, "ch2"), // a repeat
        ]);
        let got: Vec<(&str, &str)> = sorted.iter().map(|f| (f.kind.as_str(), f.gist.as_str())).collect();
        assert_eq!(
            got,
            [
                ("built_on_sand", "alpha fact"),
                ("orphaned_decision", "beta fact"),
                ("unsettled_foundation", "gamma fact"), // weight 9 before weight 2
                ("unsettled_foundation", "alpha fact"),
            ]
        );
    }

    #[test]
    fn the_manuscript_view_distinguishes_gone_from_empty() {
        let mut m: std::collections::HashMap<Uuid, String> = Default::default();
        let (here, empty, gone) = (Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3));
        m.insert(here, "prose".into());
        m.insert(empty, String::new());
        assert_eq!(SourceText::text(&m, here).as_deref(), Some("prose"));
        assert_eq!(SourceText::text(&m, empty).as_deref(), Some(""));
        assert_eq!(SourceText::text(&m, gone), None);
    }
}
