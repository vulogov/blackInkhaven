//! CANON-READER-1 — canon as a *reader*: the ledger checked against the
//! manuscript, reporting findings the way SENTINEL / KEN / BONDS do.
//!
//! Through 3.15 the ledger only answered questions. This module makes it speak
//! first: a decision whose source paragraph is gone, a paragraph that no longer
//! carries the decision it established, a firm decision resting on a soft one,
//! a foundation nobody has settled — and, opt-in, a passage that contradicts a
//! decision the author committed to.
//!
//! CR-P0 laid the pure substrate: the finding shape, its kinds and how each is
//! routed, the view of the manuscript a detector needs ([`SourceText`]), the
//! word-overlap measure `drifted_source` rests on, and the stable ordering of a
//! report. **CR-P1 adds the four deterministic detectors** behind one entry
//! point, [`check`] — free, no model, any ledger size. The shell arrives in
//! CR-P2, the worklist bridge in CR-P3. Nothing here calls a model, and the
//! manuscript is only ever seen through [`SourceText`].
//!
//! Advisory throughout: a finding never edits prose and never edits the ledger.

use std::collections::BTreeSet;

use anyhow::Result;
use smysl::{Commitment, Uid};
use uuid::Uuid;

use super::grounding::{salient_tokens, stemmer_language_name};
use super::query::CanonView;
use super::CanonLedger;
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

    /// The node's authored tags. A decision harvested from a tag (`rel:…`) is
    /// carried by the TAG, not by the prose, so the drift check consults these
    /// first. Default: none.
    fn tags(&self, _node: Uuid) -> Vec<String> {
        Vec::new()
    }
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

/// How many decisions must rest on an unsettled one before it is reported as a
/// foundation. One dependent is an ordinary edge; two or more is load.
const FOUNDATION_MIN_DEPENDENTS: usize = 2;

/// CR-P1 — read the ledger against the manuscript: the four deterministic
/// detectors, in one sorted report. Free (no model), read-only, and only as
/// large as the ledger. `src` is the caller's view of the manuscript.
///
/// - **orphaned_decision** — the source paragraph no longer exists.
/// - **drifted_source** — the source paragraph exists but no longer carries the
///   decision: none of its content words (stemmed, in `language`) remain, and no
///   authored tag on the paragraph declares it any more.
/// - **built_on_sand** — committed above something it rests on (the support
///   check `canon check` prints).
/// - **unsettled_foundation** — at least two decisions rest on one still
///   unmarked or floated. Reported only once the author has committed
///   *something* (a ledger with no commitments is not using the axis, and
///   flagging every foundation in it would be noise).
pub fn check(
    ledger: &CanonLedger,
    src: &dyn SourceText,
    language: &ProseLanguage,
) -> Result<Vec<CanonFinding>> {
    let decisions = ledger.all_decisions()?;
    let mut out: Vec<CanonFinding> = Vec::new();

    // ── orphaned / drifted: each decision against its own source paragraph ──
    for v in &decisions {
        let Some(node) = v.node else { continue }; // node-less (merged) decisions have no source to check
        match src.text(node) {
            None => out.push(CanonFinding {
                kind: CanonFindingKind::OrphanedDecision,
                decision: v.uid,
                gist: v.gist.clone(),
                node: None,
                locator: v.locator.clone(),
                related: None,
                message: format!(
                    "“{}” was established in a paragraph that no longer exists{} — re-source it, retcon it, or let it stand on its grounds.",
                    v.gist,
                    v.locator.as_deref().map(|l| format!(" ({l})")).unwrap_or_default()
                ),
                weight: 0,
            }),
            Some(text) => {
                if still_declared_by_tag(v, &src.tags(node)) {
                    continue;
                }
                let o = overlap(&v.gist, &text, language);
                if o.is_gone() {
                    let what = if text.trim().is_empty() {
                        "its source paragraph is now empty"
                    } else {
                        "its source paragraph no longer mentions any of it"
                    };
                    out.push(CanonFinding {
                        kind: CanonFindingKind::DriftedSource,
                        decision: v.uid,
                        gist: v.gist.clone(),
                        node: Some(node),
                        locator: v.locator.clone(),
                        related: None,
                        message: format!(
                            "“{}” — {what}. Which is right: the scene as it now reads, or the decision?",
                            v.gist
                        ),
                        weight: o.total as u32,
                    });
                }
            }
        }
    }

    // ── built on sand: the support check, as findings ──
    let mut sand_grounds: std::collections::HashSet<Uid> = std::collections::HashSet::new();
    for w in ledger.commitment_warnings()? {
        sand_grounds.insert(w.weakest_ground.uid);
        out.push(CanonFinding {
            kind: CanonFindingKind::BuiltOnSand,
            decision: w.unit.uid,
            gist: w.unit.gist.clone(),
            node: w.unit.node,
            locator: w.unit.locator.clone(),
            related: Some(w.weakest_ground.uid),
            message: format!(
                "“{}” is «{}» but rests on “{}”, which is only «{}» — settle the ground or soften the claim.",
                w.unit.gist, w.level, w.weakest_ground.gist, w.ground_level
            ),
            // The gap in levels: canonical-on-floated outranks committed-on-drafted.
            weight: (w.level as u32).saturating_sub(w.ground_level as u32),
        });
    }

    // ── unsettled foundations ──
    let author_uses_commitment = decisions.iter().any(|v| v.commitment.is_some());
    if author_uses_commitment {
        for v in &decisions {
            if !matches!(v.commitment, None | Some(Commitment::Floated)) {
                continue;
            }
            if sand_grounds.contains(&v.uid) {
                continue; // already named as the weak ground of a built-on-sand finding
            }
            let dependents = ledger.impact(v.uid)?.len();
            if dependents < FOUNDATION_MIN_DEPENDENTS {
                continue;
            }
            let state = if v.commitment.is_some() { "only floated" } else { "not marked at all" };
            out.push(CanonFinding {
                kind: CanonFindingKind::UnsettledFoundation,
                decision: v.uid,
                gist: v.gist.clone(),
                node: v.node,
                locator: v.locator.clone(),
                related: None,
                message: format!(
                    "{dependents} decisions rest on “{}”, which is {state} — say how settled it is (`c` in the Canon pane, or `canon commit`).",
                    v.gist
                ),
                weight: dependents as u32,
            });
        }
    }

    Ok(sort_findings(out))
}

/// Whether one of the paragraph's authored tags still declares this decision —
/// the deterministic tag harvest would produce the same kind and gist from it.
fn still_declared_by_tag(v: &CanonView, tags: &[String]) -> bool {
    if tags.is_empty() {
        return false;
    }
    super::harvest_tags(tags).into_iter().any(|(kind, gist)| Some(kind) == v.kind && gist == v.gist)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canon::{CanonLedger, NarrativeKind};

    /// A manuscript view with text and tags, for the detector tests.
    #[derive(Default)]
    struct Book {
        text: std::collections::HashMap<Uuid, String>,
        tags: std::collections::HashMap<Uuid, Vec<String>>,
    }
    impl SourceText for Book {
        fn text(&self, node: Uuid) -> Option<String> {
            self.text.get(&node).cloned()
        }
        fn tags(&self, node: Uuid) -> Vec<String> {
            self.tags.get(&node).cloned().unwrap_or_default()
        }
    }

    fn kinds(fs: &[CanonFinding]) -> Vec<&'static str> {
        fs.iter().map(|f| f.kind.as_str()).collect()
    }

    #[test]
    fn a_ledger_that_matches_the_book_reads_clean() {
        let dir = tempfile::tempdir().unwrap();
        let led = CanonLedger::new(dir.path().join("canon.cbor").to_str().unwrap());
        let n = Uuid::from_u128(0xA);
        led.record_decision(NarrativeKind::WorldFact, "the harbour freezes each winter", n, "ch1", &[]).unwrap();
        let mut book = Book::default();
        book.text.insert(n, "Every winter the harbour froze, and the fleet waited.".into());
        assert!(check(&led, &book, &ProseLanguage::En).unwrap().is_empty());
        // An empty ledger reads clean too.
        let empty = CanonLedger::new(dir.path().join("empty.cbor").to_str().unwrap());
        assert!(check(&empty, &book, &ProseLanguage::En).unwrap().is_empty());
    }

    #[test]
    fn orphaned_and_drifted_are_told_apart() {
        let dir = tempfile::tempdir().unwrap();
        let led = CanonLedger::new(dir.path().join("canon.cbor").to_str().unwrap());
        let (kept, rewritten, emptied, deleted) =
            (Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3), Uuid::from_u128(4));
        led.record_decision(NarrativeKind::WorldFact, "the harbour freezes each winter", kept, "ch1/a", &[]).unwrap();
        let drift = led.record_decision(NarrativeKind::Reveal, "Tomas knows where the key is hidden", rewritten, "ch1/b", &[]).unwrap();
        led.record_decision(NarrativeKind::WorldFact, "the lighthouse burns whale oil", emptied, "ch1/c", &[]).unwrap();
        let gone = led.record_decision(NarrativeKind::PlotPoint, "the fleet sails at the thaw", deleted, "ch1/d", &[]).unwrap();

        let mut book = Book::default();
        book.text.insert(kept, "The harbour froze again that winter.".into());
        book.text.insert(rewritten, "Mara counted the coins twice and said nothing.".into());
        book.text.insert(emptied, "   ".into());
        // `deleted` is absent from the book.

        let fs = check(&led, &book, &ProseLanguage::En).unwrap();
        assert_eq!(kinds(&fs), ["drifted_source", "drifted_source", "orphaned_decision"]);
        let d = fs.iter().find(|f| f.decision == drift).unwrap();
        assert_eq!(d.node, Some(rewritten), "a drifted finding opens the paragraph that drifted");
        assert!(d.message.contains("no longer mentions"));
        assert!(fs.iter().any(|f| f.message.contains("now empty")));
        let o = fs.iter().find(|f| f.decision == gone).unwrap();
        assert_eq!(o.node, None, "nothing to open for an orphan");
        assert_eq!(o.locator.as_deref(), Some("ch1/d"));
    }

    #[test]
    fn a_tag_declared_decision_drifts_only_when_its_tag_goes() {
        let dir = tempfile::tempdir().unwrap();
        let led = CanonLedger::new(dir.path().join("canon.cbor").to_str().unwrap());
        let n = Uuid::from_u128(7);
        let tags = vec!["rel:mentor:Bob:Cara".to_string()];
        let (kind, gist) = crate::canon::harvest_tags(&tags).remove(0);
        led.record_decision(kind, &gist, n, "ch2", &[]).unwrap();

        // The prose never names them; the TAG carries the decision.
        let mut book = Book::default();
        book.text.insert(n, "Rain fell on the quay all morning.".into());
        book.tags.insert(n, tags);
        assert!(check(&led, &book, &ProseLanguage::En).unwrap().is_empty());

        // Remove the tag and nothing declares it any more.
        book.tags.clear();
        assert_eq!(kinds(&check(&led, &book, &ProseLanguage::En).unwrap()), ["drifted_source"]);
    }

    #[test]
    fn sand_and_unsettled_foundations() {
        let dir = tempfile::tempdir().unwrap();
        let led = CanonLedger::new(dir.path().join("canon.cbor").to_str().unwrap());
        let n = Uuid::from_u128(9);
        let rec = |kind, gist: &str, grounds: &[Uid]| led.record_decision(kind, gist, n, "ch1", grounds).unwrap();
        let gate = rec(NarrativeKind::WorldFact, "there is a hidden sea gate", &[]);
        let ice = rec(NarrativeKind::WorldFact, "the harbour freezes each winter", &[]);
        let escape = rec(NarrativeKind::PlotPoint, "the escape uses the sea gate", &[gate]);
        let wait = rec(NarrativeKind::PlotPoint, "the escape waits for the thaw", &[ice]);
        let fleet = rec(NarrativeKind::PlotPoint, "the fleet is trapped by the harbour ice", &[ice]);

        let mut book = Book::default();
        book.text.insert(
            n,
            "A hidden sea gate; the harbour froze each winter; the escape used the gate and waited for the thaw; the fleet lay trapped in the ice.".into(),
        );
        let en = ProseLanguage::En;

        // Nobody has committed anything: the axis is not in use, nothing is said.
        assert!(check(&led, &book, &en).unwrap().is_empty());

        // The author starts committing. `ice` carries two decisions and is unmarked.
        led.commit(wait, Commitment::Drafted, "author").unwrap();
        let fs = check(&led, &book, &en).unwrap();
        assert_eq!(kinds(&fs), ["unsettled_foundation"]);
        assert_eq!(fs[0].decision, ice);
        assert_eq!(fs[0].weight, 2);
        assert!(fs[0].message.contains("not marked at all"));
        // `gate` carries only one decision — an edge, not a foundation.
        assert!(!fs.iter().any(|f| f.decision == gate));

        // Canonical on floated is built-on-sand — and is not ALSO called unsettled.
        led.commit(escape, Commitment::Canonical, "author").unwrap();
        led.commit(gate, Commitment::Floated, "author").unwrap();
        let fs = check(&led, &book, &en).unwrap();
        assert_eq!(kinds(&fs), ["built_on_sand", "unsettled_foundation"]);
        assert_eq!((fs[0].decision, fs[0].related), (escape, Some(gate)));
        assert!(fs[0].message.contains("canonical") && fs[0].message.contains("floated"));

        // Settle the foundation and it goes quiet.
        led.commit(ice, Commitment::Committed, "author").unwrap();
        let _ = fleet;
        assert_eq!(kinds(&check(&led, &book, &en).unwrap()), ["built_on_sand"]);
    }

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
