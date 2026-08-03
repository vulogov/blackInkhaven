//! Inner Stylist fast track (CH-P7) — the deterministic synthesiser. Turns the
//! CHORUS pillar outputs into Praise / Note / Concern findings. Pure, offline,
//! zero AI: it observes what the measurements say, never rewrites the prose.

use crate::chorus::distinct::DistinctMatrix;
use crate::chorus::pov::SceneHeadHops;
use crate::chorus::register::RegisterReport;
use crate::chorus::tense::TenseSummary;
use crate::prose::violations::Violation;

use super::{Finding, Severity};

/// Synthesise the pillar outputs into voice-at-scale findings.
pub(crate) fn synthesize(
    distinct: &DistinctMatrix,
    drifts: &[(String, Vec<Violation>)],
    head_hops: &[SceneHeadHops],
    tense: &TenseSummary,
    register: &RegisterReport,
) -> Vec<Finding> {
    let mut out = Vec::new();

    // Character distinctiveness — praise a fully-distinct cast, flag look-alikes.
    if distinct.names.len() >= 2 {
        if distinct.indistinguishable.is_empty() {
            out.push(Finding {
                severity: Severity::Praise,
                kind: "distinctiveness",
                key: "distinct:ok".into(),
                message: format!(
                    "{} comparable voices, all distinct — nobody reads like anybody else.",
                    distinct.names.len()
                ),
            });
        } else {
            for p in &distinct.indistinguishable {
                let (a, b) = ordered(&p.a, &p.b);
                out.push(Finding {
                    severity: Severity::Concern,
                    kind: "distinctiveness",
                    key: format!("distinct:{}|{}", a.to_lowercase(), b.to_lowercase()),
                    message: format!(
                        "{a} and {b} read alike (voice distance {:.2}) — consider sharpening one.",
                        p.distance
                    ),
                });
            }
        }
    }

    // Per-character voice drift across the arc.
    for (name, vs) in drifts {
        if vs.is_empty() {
            continue;
        }
        let metrics: Vec<&str> = vs.iter().map(|v| v.metric).collect();
        out.push(Finding {
            severity: Severity::Note,
            kind: "drift",
            key: format!("drift:{}", name.to_lowercase()),
            message: format!("{name}'s voice drifts across their arc ({}).", metrics.join(", ")),
        });
    }

    // POV / head-hop.
    for s in head_hops {
        for hh in &s.hops {
            out.push(Finding {
                severity: Severity::Concern,
                kind: "pov",
                key: format!("pov:{}:{}:{}", s.chapter_ord, s.scene_index, hh.experiencer.to_lowercase()),
                message: format!(
                    "ch.{} scene {}: {}'s interiority leaks — not the scene's {}.",
                    s.chapter_ord,
                    s.scene_index,
                    hh.experiencer,
                    s.pov.describe()
                ),
            });
        }
    }

    // Tense slips (English-gated; a Russian project simply yields no findings).
    if let TenseSummary::Scanned(scenes) = tense {
        for s in scenes {
            out.push(Finding {
                severity: Severity::Concern,
                kind: "tense",
                key: format!("tense:{}:{}", s.chapter_ord, s.scene_index),
                message: format!(
                    "ch.{} scene {}: narration slips out of {} tense.",
                    s.chapter_ord,
                    s.scene_index,
                    s.dominant.label()
                ),
            });
        }
    }

    // Register drift.
    for d in &register.drifts {
        let dir = if d.delta >= 0.0 { "rose" } else { "fell" };
        out.push(Finding {
            severity: Severity::Note,
            kind: "register",
            key: format!("register:{}:{}", d.metric, d.chapter_ord),
            message: format!(
                "ch.{}: register drifts — {} {dir} to {:.3} (ch.1 {:.3}).",
                d.chapter_ord, d.metric, d.value, d.baseline
            ),
        });
    }

    out
}

fn ordered<'a>(a: &'a str, b: &'a str) -> (&'a str, &'a str) {
    if a.to_lowercase() <= b.to_lowercase() { (a, b) } else { (b, a) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chorus::distinct::{DistinctMatrix, VoicePair};
    use crate::chorus::register::RegisterReport;
    use crate::chorus::tense::TenseSummary;

    fn empty_distinct(names: Vec<&str>) -> DistinctMatrix {
        DistinctMatrix {
            names: names.into_iter().map(String::from).collect(),
            pairs: Vec::new(),
            indistinguishable: Vec::new(),
        }
    }

    #[test]
    fn praises_a_fully_distinct_cast() {
        let d = empty_distinct(vec!["Mara", "Joren", "Sela"]);
        let out = synthesize(
            &d,
            &[],
            &[],
            &TenseSummary::Scanned(Vec::new()),
            &RegisterReport { chapters: Vec::new(), drifts: Vec::new() },
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].severity, Severity::Praise);
        assert_eq!(out[0].kind, "distinctiveness");
    }

    #[test]
    fn flags_an_indistinguishable_pair_with_a_stable_key() {
        let mut d = empty_distinct(vec!["Mara", "Joren"]);
        d.indistinguishable.push(VoicePair { a: "Joren".into(), b: "Mara".into(), distance: 0.1 });
        let out = synthesize(
            &d,
            &[],
            &[],
            &TenseSummary::Scanned(Vec::new()),
            &RegisterReport { chapters: Vec::new(), drifts: Vec::new() },
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].severity, Severity::Concern);
        // Key is order-normalised regardless of pair order.
        assert_eq!(out[0].key, "distinct:joren|mara");
    }

    #[test]
    fn russian_tense_yields_no_tense_findings() {
        let d = empty_distinct(vec![]);
        let out = synthesize(
            &d,
            &[],
            &[],
            &TenseSummary::Unsupported("aspect"),
            &RegisterReport { chapters: Vec::new(), drifts: Vec::new() },
        );
        assert!(out.iter().all(|f| f.kind != "tense"));
    }
}
