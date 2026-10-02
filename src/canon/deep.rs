//! CANON-READER-1 (CR-P5) — the opt-in **contradiction** pass.
//!
//! The deterministic detectors ([`super::read::check`]) can tell that a scene no
//! longer *mentions* a decision. They cannot tell that a scene *says the
//! opposite*. This pass asks a model, once, and only about the decisions the
//! author has marked **committed** or **canonical** — the ones a contradiction
//! actually matters for, which also caps cost and noise.
//!
//! Shape: for each such decision the caller retrieves the passages most related
//! to it (the existing semantic search); [`build_prompt`] packs as many cases as
//! fit a token budget into ONE prompt; the model answers a JSON list;
//! [`parse_reply`] turns the answers into [`DeepFinding`]s. They are written to
//! an advisory sidecar (`.inkhaven/canon-contradictions.json`) so the worklist,
//! CHRONICLE and the editor can show them without calling the model again.
//!
//! A sidecar finding **expires on its own** ([`live_findings`]): when its
//! decision is no longer live or no longer committed, or when the passage it
//! cites has changed since it was checked (the author may have fixed it). So a
//! stale accusation never lingers — re-run the pass to check the new text.
//!
//! This module holds the pure parts (no store, no model call); the retrieval
//! and the call live with the shell and the editor.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use smysl::{Commitment, Uid};
use uuid::Uuid;

use super::read::{sort_findings, CanonFinding, CanonFindingKind, SourceText};
use super::CanonLedger;
use crate::project::ProjectLayout;

/// The contradiction-checker's system prompt. (`slow_llm_call` appends the
/// project-language directive for the explanations.)
pub const DEEP_SYSTEM: &str = "You are a meticulous continuity editor for a work of fiction. You \
are given DECISIONS the author has committed to about the story (labelled D1, D2, …), each followed \
by numbered PASSAGES from the manuscript (P1, P2, …) that may bear on it. For each decision, find \
any passage that CONTRADICTS it — one that states the opposite, or shows something that could not \
happen if the decision is true. Do NOT flag: a passage that merely does not mention the decision; a \
passage about something else; a character who is lying, mistaken, guessing or dreaming; a rumour; a \
hypothetical. Be conservative — when in doubt, do not flag. Respond ONLY with a JSON array; each \
item is {\"category\": \"contradicted\", \"severity\": \"contradiction\", \"explanation\": \
\"D<n> P<m>: one sentence saying how that passage contradicts that decision\"}. The explanation \
MUST begin with the decision label and the passage label, e.g. \"D2 P5: …\". Return [] if no \
passage contradicts any decision.";

/// One decision to check, with the passages retrieved for it.
#[derive(Debug, Clone)]
pub struct DeepCase {
    pub decision: Uid,
    pub gist: String,
    /// `world-fact`, `plot-point`, … — context for the model.
    pub kind: String,
    pub commitment: Commitment,
    pub passages: Vec<DeepPassage>,
}

/// A manuscript passage offered as evidence for a case.
#[derive(Debug, Clone)]
pub struct DeepPassage {
    pub node: Uuid,
    pub locator: String,
    pub text: String,
}

/// A contradiction the model reported, as stored in the sidecar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeepFinding {
    /// The decision's canonical id (`Uid` as text).
    pub decision: String,
    pub gist: String,
    /// The passage that contradicts it.
    pub node: Uuid,
    pub locator: String,
    /// The model's one-sentence reason (in the project language).
    pub explanation: String,
    /// Fingerprint of the passage text when it was checked — the finding
    /// expires when the passage changes.
    pub text_fp: u64,
}

/// The advisory sidecar the pass writes.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeepSidecar {
    pub findings: Vec<DeepFinding>,
    /// How many committed/canonical decisions the last run checked…
    #[serde(default)]
    pub checked: usize,
    /// …out of how many there were (the rest did not fit the budget).
    #[serde(default)]
    pub of: usize,
}

impl DeepSidecar {
    fn path(layout: &ProjectLayout) -> std::path::PathBuf {
        layout.root.join(".inkhaven").join("canon-contradictions.json")
    }

    /// Load the sidecar; absent or unreadable reads as empty (it is advisory and
    /// rebuildable — a corrupt file must not break a read).
    pub fn load(layout: &ProjectLayout) -> DeepSidecar {
        std::fs::read(Self::path(layout))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, layout: &ProjectLayout) -> Result<()> {
        let p = Self::path(layout);
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir).map_err(|e| anyhow!("create {dir:?}: {e}"))?;
        }
        let json = serde_json::to_vec_pretty(self).map_err(|e| anyhow!("serialize: {e}"))?;
        crate::io_atomic::write(&p, &json).map_err(|e| anyhow!("write {p:?}: {e}"))
    }
}

/// A stable fingerprint of a passage (FNV-1a over its trimmed text) — stable
/// across runs and versions, unlike the std hasher.
pub fn fingerprint(text: &str) -> u64 {
    text.trim().bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3))
}

/// Whether a decision at this commitment is worth a model's attention.
pub fn is_checkable(commitment: Option<Commitment>) -> bool {
    matches!(commitment, Some(Commitment::Committed) | Some(Commitment::Canonical))
}

fn tokens(s: &str) -> usize {
    s.chars().count().div_ceil(4)
}

/// Pack cases into one prompt, in the order given, stopping before the budget
/// (system prompt included) would be exceeded. Returns the prompt and how many
/// cases it holds — always at least one when there is one, so a tight budget
/// degrades to "checked 1 of N", never to nothing. Passage labels are global
/// (`P1…` across the whole prompt), which is what [`parse_reply`] expects.
pub fn build_prompt(cases: &[DeepCase], budget_tokens: usize) -> (String, usize) {
    let mut out = String::new();
    let mut used = tokens(DEEP_SYSTEM);
    let mut included = 0usize;
    let mut p = 0usize;
    for (i, c) in cases.iter().enumerate() {
        if c.passages.is_empty() {
            included += 1; // nothing to send; counted as checked (nothing related exists)
            continue;
        }
        let mut block = format!("D{} ({}, {}): {}\n", i + 1, c.kind, c.commitment, c.gist.trim());
        let mut local = p;
        for passage in &c.passages {
            local += 1;
            block.push_str(&format!("  P{local} [{}]: {}\n", passage.locator, passage.text.trim()));
        }
        block.push('\n');
        let cost = tokens(&block);
        let sent_any = !out.is_empty();
        if sent_any && used + cost > budget_tokens {
            break;
        }
        out.push_str(&block);
        used += cost;
        p = local;
        included += 1;
    }
    (out, included)
}

/// Turn the model's explanations (`"D2 P5: …"`) into findings, using the same
/// case order and global passage numbering as [`build_prompt`]. An explanation
/// whose labels are missing, out of range, or name a passage that was not
/// offered for that decision is dropped — the model does not get to invent a
/// citation.
pub fn parse_reply(explanations: &[String], cases: &[DeepCase], included: usize) -> Vec<DeepFinding> {
    // Global passage number → (case index, passage).
    let mut by_number: Vec<(usize, &DeepPassage)> = Vec::new();
    for (i, c) in cases.iter().enumerate().take(included) {
        for passage in &c.passages {
            by_number.push((i, passage));
        }
    }
    let mut out: Vec<DeepFinding> = Vec::new();
    for raw in explanations {
        let Some((d, p, reason)) = split_labels(raw) else { continue };
        let Some(case) = d.checked_sub(1).and_then(|i| cases.get(i)).filter(|_| d <= included) else { continue };
        let Some((owner, passage)) = p.checked_sub(1).and_then(|i| by_number.get(i)) else { continue };
        if *owner != d - 1 || reason.is_empty() {
            continue;
        }
        let finding = DeepFinding {
            decision: case.decision.to_string(),
            gist: case.gist.clone(),
            node: passage.node,
            locator: passage.locator.clone(),
            explanation: reason.to_string(),
            text_fp: fingerprint(&passage.text),
        };
        if !out.iter().any(|f| f.decision == finding.decision && f.node == finding.node) {
            out.push(finding);
        }
    }
    out
}

/// `"D2 P5: because…"` → `(2, 5, "because…")`. Tolerant of `D2, P5 —`, `D2/P5:`
/// and surrounding whitespace; `None` when either label is missing.
fn split_labels(raw: &str) -> Option<(usize, usize, &str)> {
    let s = raw.trim_start();
    let rest = s.strip_prefix(['D', 'd'])?;
    let d_end = rest.find(|c: char| !c.is_ascii_digit())?;
    let d: usize = rest[..d_end].parse().ok()?;
    let rest = rest[d_end..].trim_start_matches(|c: char| c == ',' || c == '/' || c == '-' || c.is_whitespace());
    let rest = rest.strip_prefix(['P', 'p'])?;
    let p_end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
    let p: usize = rest[..p_end].parse().ok()?;
    let reason = rest[p_end..]
        .trim_start_matches(|c: char| c == ':' || c == '—' || c == '–' || c == '-' || c == '.' || c.is_whitespace())
        .trim();
    Some((d, p, reason))
}

/// The sidecar's findings that still stand: the decision is live and still
/// committed/canonical, and the passage exists with the text that was checked.
pub fn live_findings(
    ledger: &CanonLedger,
    src: &dyn SourceText,
    sidecar: &DeepSidecar,
) -> Result<Vec<CanonFinding>> {
    if sidecar.findings.is_empty() {
        return Ok(Vec::new());
    }
    let decisions = ledger.all_decisions()?;
    let mut out = Vec::new();
    for f in &sidecar.findings {
        let Some(v) = decisions.iter().find(|v| v.uid.to_string() == f.decision) else { continue };
        if !is_checkable(v.commitment) {
            continue;
        }
        let Some(text) = src.text(f.node) else { continue };
        if fingerprint(&text) != f.text_fp {
            continue; // the passage changed since it was checked
        }
        out.push(CanonFinding {
            kind: CanonFindingKind::Contradicted,
            decision: v.uid,
            gist: v.gist.clone(),
            node: Some(f.node),
            locator: Some(f.locator.clone()),
            related: None,
            message: format!("“{}” is contradicted here — {}", v.gist, f.explanation),
            weight: v.commitment.map(|c| c as u32).unwrap_or(0),
        });
    }
    Ok(out)
}

/// Everything the canon reader has to say: the deterministic report plus the
/// contradictions still standing from the last deep pass. Free — no model call.
pub fn check_all(
    ledger: &CanonLedger,
    src: &dyn SourceText,
    language: &crate::prose::ProseLanguage,
    layout: &ProjectLayout,
) -> Result<Vec<CanonFinding>> {
    let mut all = super::read::check(ledger, src, language)?;
    all.extend(live_findings(ledger, src, &DeepSidecar::load(layout))?);
    Ok(sort_findings(all))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canon::NarrativeKind;
    use std::collections::HashMap;

    fn case(led: &CanonLedger, gist: &str, passages: &[(u128, &str)]) -> DeepCase {
        let uid = led.record_decision(NarrativeKind::WorldFact, gist, Uuid::from_u128(999), "src", &[]).unwrap();
        DeepCase {
            decision: uid,
            gist: gist.into(),
            kind: "world-fact".into(),
            commitment: Commitment::Canonical,
            passages: passages
                .iter()
                .map(|(n, t)| DeepPassage { node: Uuid::from_u128(*n), locator: format!("ch/{n}"), text: (*t).into() })
                .collect(),
        }
    }

    fn ledger() -> (tempfile::TempDir, CanonLedger) {
        let dir = tempfile::tempdir().unwrap();
        let led = CanonLedger::new(dir.path().join("canon.cbor").to_str().unwrap());
        (dir, led)
    }

    #[test]
    fn the_prompt_numbers_passages_globally_and_fits_a_budget() {
        let (_d, led) = ledger();
        let cases = vec![
            case(&led, "the harbour freezes each winter", &[(1, "The harbour lay open all winter."), (2, "Snow fell.")]),
            case(&led, "Tomas cannot swim", &[(3, "Tomas swam the channel twice.")]),
        ];
        let (prompt, n) = build_prompt(&cases, 100_000);
        assert_eq!(n, 2);
        assert!(prompt.contains("D1 (world-fact, canonical): the harbour freezes each winter"));
        assert!(prompt.contains("P1 [ch/1]") && prompt.contains("P2 [ch/2]") && prompt.contains("P3 [ch/3]"));
        assert!(prompt.contains("D2 ("));
        // A budget too small for both still sends the first.
        let (small, n) = build_prompt(&cases, 1);
        assert_eq!(n, 1);
        assert!(small.contains("D1") && !small.contains("D2"));
        // Nothing to check → nothing to send.
        assert_eq!(build_prompt(&[], 1000), (String::new(), 0));
    }

    #[test]
    fn replies_become_findings_and_invented_citations_are_dropped() {
        let (_d, led) = ledger();
        let cases = vec![
            case(&led, "the harbour freezes each winter", &[(1, "The harbour lay open all winter."), (2, "Snow fell.")]),
            case(&led, "Tomas cannot swim", &[(3, "Tomas swam the channel twice.")]),
        ];
        let replies: Vec<String> = [
            "D1 P1: the harbour is described as open in winter.",
            "d2, p3 — he swims.",
            "D1 P3: wrong owner (P3 belongs to D2).",
            "D9 P1: no such decision.",
            "D1 P7: no such passage.",
            "no labels at all",
            "D1 P1: a duplicate of the first.",
        ]
        .map(String::from)
        .to_vec();
        let got = parse_reply(&replies, &cases, 2);
        assert_eq!(got.len(), 2);
        assert_eq!((got[0].node, got[0].explanation.as_str()), (Uuid::from_u128(1), "the harbour is described as open in winter."));
        assert_eq!((got[1].node, got[1].explanation.as_str()), (Uuid::from_u128(3), "he swims."));
        assert_eq!(got[0].text_fp, fingerprint("The harbour lay open all winter."));
        // A case that did not fit the prompt cannot be cited.
        assert_eq!(parse_reply(&replies, &cases, 1).len(), 1);
    }

    #[test]
    fn a_stored_contradiction_expires_when_its_passage_or_its_decision_changes() {
        let (_d, led) = ledger();
        let node = Uuid::from_u128(1);
        let c = case(&led, "the harbour freezes each winter", &[(1, "The harbour lay open all winter.")]);
        led.commit(c.decision, Commitment::Canonical, "author").unwrap();
        let sidecar = DeepSidecar {
            findings: parse_reply(&["D1 P1: it is open.".to_string()], std::slice::from_ref(&c), 1),
            checked: 1,
            of: 1,
        };
        let mut book: HashMap<Uuid, String> = HashMap::new();
        book.insert(node, "The harbour lay open all winter.".into());

        let live = live_findings(&led, &book, &sidecar).unwrap();
        assert_eq!(live.len(), 1);
        assert_eq!(live[0].kind, CanonFindingKind::Contradicted);
        assert_eq!(live[0].node, Some(node));
        assert!(live[0].message.contains("contradicted here") && live[0].message.contains("it is open."));

        // The author rewrites the passage → the accusation no longer stands.
        book.insert(node, "The harbour froze solid that winter.".into());
        assert!(live_findings(&led, &book, &sidecar).unwrap().is_empty());
        book.insert(node, "The harbour lay open all winter.".into());

        // The author softens the decision → it is no longer one we check.
        led.commit(c.decision, Commitment::Floated, "author").unwrap();
        assert!(live_findings(&led, &book, &sidecar).unwrap().is_empty());

        // The passage is deleted → gone.
        led.commit(c.decision, Commitment::Canonical, "author").unwrap();
        book.remove(&node);
        assert!(live_findings(&led, &book, &sidecar).unwrap().is_empty());
    }

    #[test]
    fn the_sidecar_round_trips_and_a_corrupt_one_reads_as_empty() {
        let dir = tempfile::tempdir().unwrap();
        let layout = ProjectLayout::new(dir.path());
        assert_eq!(DeepSidecar::load(&layout), DeepSidecar::default());
        let s = DeepSidecar {
            findings: vec![DeepFinding {
                decision: "b3:x".into(),
                gist: "g".into(),
                node: Uuid::from_u128(4),
                locator: "ch/4".into(),
                explanation: "e".into(),
                text_fp: 7,
            }],
            checked: 3,
            of: 5,
        };
        s.save(&layout).unwrap();
        assert_eq!(DeepSidecar::load(&layout), s);
        std::fs::write(dir.path().join(".inkhaven").join("canon-contradictions.json"), "{ not json").unwrap();
        assert_eq!(DeepSidecar::load(&layout), DeepSidecar::default());
        assert!(is_checkable(Some(Commitment::Committed)) && !is_checkable(Some(Commitment::Drafted)) && !is_checkable(None));
        assert_ne!(fingerprint("a"), fingerprint("b"));
        assert_eq!(fingerprint("  a \n"), fingerprint("a"));
    }
}
