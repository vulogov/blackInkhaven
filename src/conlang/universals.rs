//! LING-1 L-P3 — typological universals over a language's grammar block.
//!
//! Reads the WALS-aligned `{ grammar: { … } }` spec (see `conlang::grammar`) and
//! reports three things, all static + pure:
//!   * **head-directionality harmony** — whether the strong word-order correlates
//!     (adposition, genitive, relative clause) branch the same way as the verb
//!     phrase (Dryer's correlation-pair generalisation),
//!   * a handful of classic **implicational universals** (Greenberg 2/3/4, the
//!     OV↔GenN / OV↔RelN correlations), each judged satisfied / violated / n-a,
//!   * a one-line **word-order + morphotype survey**.
//!
//! This describes a *conlang against the typological baseline* — a violation is a
//! flag, not an error (natural languages break tendencies too); the point is to
//! show the conlanger where their language sits and what is unusual.

use serde::Serialize;

use crate::conlang::types::grammar::GrammarSpec;

/// Branching direction of a head–dependent pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Dir {
    /// Dependent precedes head (head-final): OV, postpositions, GenN, RelN.
    Final,
    /// Head precedes dependent (head-initial): VO, prepositions, NGen, NRel.
    Initial,
}

/// Overall branching classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Branch {
    HeadInitial,
    HeadFinal,
    Mixed,
    Unknown,
}

/// The verdict on one implicational universal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// The language matches the universal.
    Satisfied,
    /// The language runs against it (a typologically marked combination).
    Violated,
    /// The antecedent or the consequent feature isn't specified.
    NotApplicable,
}

/// One checked implicational universal.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct UniversalCheck {
    pub id: &'static str,
    pub statement: &'static str,
    pub verdict: Verdict,
    /// The observed feature values behind the verdict.
    pub detail: String,
}

/// The full typology report for one language.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct TypologyReport {
    /// Basic constituent order (`word_order`), if specified.
    pub word_order: Option<String>,
    /// Dominant morphological type (`morphological_type`), if specified.
    pub morphological_type: Option<String>,
    /// Head/dependent marking locus (`head_marking`), if specified.
    pub head_marking: Option<String>,
    /// Overall branching direction inferred from the directional features.
    pub branch: Branch,
    /// Correlated features that branch the same way as the majority.
    pub harmonic: usize,
    /// Correlated features that branch against it (the disharmonic outliers).
    pub disharmonic: Vec<String>,
    /// `harmonic / (harmonic + disharmonic)` (1.0 = perfectly harmonic).
    pub harmony_score: f64,
    /// The implicational-universal verdicts.
    pub checks: Vec<UniversalCheck>,
}

impl Default for Branch {
    fn default() -> Self {
        Branch::Unknown
    }
}

fn get<'a>(spec: &'a GrammarSpec, id: &str) -> Option<&'a str> {
    spec.grammar.get(id).map(|s| s.trim()).filter(|s| !s.is_empty())
}

/// `true` for object-before-verb orders, `false` for verb-before-object, `None`
/// for free / unspecified.
fn is_ov(word_order: &str) -> Option<bool> {
    match word_order.to_lowercase().as_str() {
        "sov" | "osv" | "ovs" => Some(true),
        "svo" | "vso" | "vos" => Some(false),
        _ => None, // free
    }
}

/// The branching direction of a single directional feature, if it has one.
fn feature_dir(id: &str, value: &str) -> Option<Dir> {
    let v = value.to_lowercase();
    match id {
        "word_order" => is_ov(&v).map(|ov| if ov { Dir::Final } else { Dir::Initial }),
        "adposition" => match v.as_str() {
            "postposition" => Some(Dir::Final),
            "preposition" => Some(Dir::Initial),
            _ => None, // none / case-marking
        },
        "genitive_order" => match v.as_str() {
            "possessor_first" => Some(Dir::Final), // GenN
            "possessed_first" => Some(Dir::Initial), // NGen
            _ => None,
        },
        "relative_clause" => match v.as_str() {
            "prenominal" => Some(Dir::Final), // RelN
            "postnominal" => Some(Dir::Initial), // NRel
            _ => None, // internally-headed / correlative aren't directional here
        },
        _ => None,
    }
}

/// The correlated features whose branching harmony we measure, against the verb
/// phrase set by `word_order`.
const CORRELATES: &[&str] = &["adposition", "genitive_order", "relative_clause"];

/// Compute the typology report for `spec`.
pub fn survey(spec: &GrammarSpec) -> TypologyReport {
    let mut r = TypologyReport {
        word_order: get(spec, "word_order").map(str::to_string),
        morphological_type: get(spec, "morphological_type").map(str::to_string),
        head_marking: get(spec, "head_marking").map(str::to_string),
        ..Default::default()
    };

    // Collect the directions of every directional feature present.
    let mut dirs: Vec<(&str, Dir)> = Vec::new();
    if let Some(v) = get(spec, "word_order") {
        if let Some(d) = feature_dir("word_order", v) {
            dirs.push(("word_order", d));
        }
    }
    for &id in CORRELATES {
        if let Some(v) = get(spec, id) {
            if let Some(d) = feature_dir(id, v) {
                dirs.push((id, d));
            }
        }
    }

    // Overall branch, from every directional feature.
    let finals = dirs.iter().filter(|(_, d)| *d == Dir::Final).count();
    let initials = dirs.len() - finals;
    r.branch = match (finals, initials) {
        (0, 0) => Branch::Unknown,
        (f, 0) if f > 0 => Branch::HeadFinal,
        (0, i) if i > 0 => Branch::HeadInitial,
        _ => Branch::Mixed,
    };

    // Harmony: the correlates should branch the same way as the verb phrase
    // (Dryer). The reference is `word_order`'s direction; with a free/unset word
    // order, fall back to the correlates' own majority.
    let reference = feature_dir("word_order", get(spec, "word_order").unwrap_or(""))
        .unwrap_or(if finals >= initials { Dir::Final } else { Dir::Initial });
    for &id in CORRELATES {
        if let Some(d) = get(spec, id).and_then(|v| feature_dir(id, v)) {
            if d == reference {
                r.harmonic += 1;
            } else {
                r.disharmonic.push(id.to_string());
            }
        }
    }
    let measured = r.harmonic + r.disharmonic.len();
    r.harmony_score = if measured > 0 { r.harmonic as f64 / measured as f64 } else { 0.0 };

    r.checks = run_checks(spec);
    r
}

fn check(id: &'static str, statement: &'static str, verdict: Verdict, detail: String) -> UniversalCheck {
    UniversalCheck { id, statement, verdict, detail }
}

/// The classic implicational universals we judge the language against.
fn run_checks(spec: &GrammarSpec) -> Vec<UniversalCheck> {
    let adp = get(spec, "adposition");
    let genitive = get(spec, "genitive_order");
    let rel = get(spec, "relative_clause");
    let ov = get(spec, "word_order").and_then(is_ov);
    // `is(feature, value)` — case-insensitive equality, false when unspecified.
    let is = |feat: &str, val: &str| get(spec, feat).is_some_and(|s| s.eq_ignore_ascii_case(val));

    use Verdict::*;
    let mut out = Vec::new();

    // Greenberg #3 — VSO languages are prepositional.
    let (v, d) = if !is("word_order", "vso") || adp.is_none() {
        (NotApplicable, "not VSO, or adposition unspecified")
    } else if is("adposition", "preposition") {
        (Satisfied, "VSO + prepositions")
    } else {
        (Violated, "VSO + postpositions")
    };
    out.push(check("greenberg-3", "VSO languages are prepositional", v, d.into()));

    // Greenberg #4 — SOV languages are postpositional (more than chance frequency).
    let (v, d) = if !is("word_order", "sov") || adp.is_none() {
        (NotApplicable, "not SOV, or adposition unspecified")
    } else if is("adposition", "postposition") {
        (Satisfied, "SOV + postpositions")
    } else if is("adposition", "preposition") {
        (Violated, "SOV + prepositions")
    } else {
        (NotApplicable, "adposition is case-marking / none")
    };
    out.push(check("greenberg-4", "SOV languages are postpositional", v, d.into()));

    // Greenberg #2 — prepositional ⇒ NGen; postpositional ⇒ GenN.
    let (v, d) = if genitive.is_none() || !(is("adposition", "preposition") || is("adposition", "postposition")) {
        (NotApplicable, "adposition or genitive unspecified")
    } else if is("adposition", "preposition") {
        if is("genitive_order", "possessed_first") {
            (Satisfied, "prepositions + NGen")
        } else {
            (Violated, "prepositions + GenN")
        }
    } else if is("genitive_order", "possessor_first") {
        (Satisfied, "postpositions + GenN")
    } else {
        (Violated, "postpositions + NGen")
    };
    out.push(check("greenberg-2", "adposition order predicts genitive order", v, d.into()));

    // Dryer — OV ⇒ GenN, VO ⇒ NGen.
    let (v, d) = match (ov, genitive.is_some()) {
        (Some(true), true) => {
            if is("genitive_order", "possessor_first") {
                (Satisfied, "OV + GenN")
            } else {
                (Violated, "OV + NGen")
            }
        }
        (Some(false), true) => {
            if is("genitive_order", "possessed_first") {
                (Satisfied, "VO + NGen")
            } else {
                (Violated, "VO + GenN")
            }
        }
        _ => (NotApplicable, "word order free/unset, or genitive unspecified"),
    };
    out.push(check("dryer-gen", "OV correlates with GenN, VO with NGen", v, d.into()));

    // Dryer — OV ⇒ RelN (prenominal), VO ⇒ NRel (postnominal).
    let rel_dir = rel.and_then(|r| feature_dir("relative_clause", r));
    let (v, d) = match (ov, rel_dir) {
        (Some(true), Some(Dir::Final)) => (Satisfied, "OV + RelN"),
        (Some(true), Some(Dir::Initial)) => (Violated, "OV + NRel"),
        (Some(false), Some(Dir::Initial)) => (Satisfied, "VO + NRel"),
        (Some(false), Some(Dir::Final)) => (Violated, "VO + RelN"),
        _ => (NotApplicable, "word order free/unset, or relative strategy non-directional"),
    };
    out.push(check("dryer-rel", "OV correlates with prenominal relatives, VO with postnominal", v, d.into()));

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(pairs: &[(&str, &str)]) -> GrammarSpec {
        let mut g = std::collections::BTreeMap::new();
        for (k, v) in pairs {
            g.insert(k.to_string(), v.to_string());
        }
        GrammarSpec { grammar: g, ..Default::default() }
    }

    fn verdict(r: &TypologyReport, id: &str) -> Verdict {
        r.checks.iter().find(|c| c.id == id).unwrap().verdict
    }

    #[test]
    fn a_harmonic_head_final_language_satisfies_the_universals() {
        // Japanese-like: SOV, postpositions, GenN, prenominal relatives.
        let r = survey(&spec(&[
            ("word_order", "sov"),
            ("adposition", "postposition"),
            ("genitive_order", "possessor_first"),
            ("relative_clause", "prenominal"),
        ]));
        assert_eq!(r.branch, Branch::HeadFinal);
        assert_eq!(r.harmony_score, 1.0);
        assert!(r.disharmonic.is_empty());
        assert_eq!(verdict(&r, "greenberg-4"), Verdict::Satisfied);
        assert_eq!(verdict(&r, "greenberg-2"), Verdict::Satisfied);
        assert_eq!(verdict(&r, "dryer-gen"), Verdict::Satisfied);
        assert_eq!(verdict(&r, "dryer-rel"), Verdict::Satisfied);
    }

    #[test]
    fn a_harmonic_head_initial_language_satisfies_the_universals() {
        // Welsh-like: VSO, prepositions, NGen, postnominal relatives.
        let r = survey(&spec(&[
            ("word_order", "vso"),
            ("adposition", "preposition"),
            ("genitive_order", "possessed_first"),
            ("relative_clause", "postnominal"),
        ]));
        assert_eq!(r.branch, Branch::HeadInitial);
        assert_eq!(r.harmony_score, 1.0);
        assert_eq!(verdict(&r, "greenberg-3"), Verdict::Satisfied);
        assert_eq!(verdict(&r, "greenberg-2"), Verdict::Satisfied);
    }

    #[test]
    fn a_disharmonic_language_is_flagged() {
        // SOV but prepositional + NGen — a marked mix (like Persian's tendencies).
        let r = survey(&spec(&[
            ("word_order", "sov"),
            ("adposition", "preposition"),
            ("genitive_order", "possessed_first"),
        ]));
        assert_eq!(r.branch, Branch::Mixed);
        assert!(r.harmony_score < 1.0);
        assert!(!r.disharmonic.is_empty());
        assert_eq!(verdict(&r, "greenberg-4"), Verdict::Violated);
        assert_eq!(verdict(&r, "dryer-gen"), Verdict::Violated);
    }

    #[test]
    fn unspecified_features_are_not_applicable_not_violations() {
        let r = survey(&spec(&[("morphological_type", "agglutinative")]));
        assert_eq!(r.branch, Branch::Unknown);
        assert_eq!(r.morphological_type.as_deref(), Some("agglutinative"));
        assert!(r.checks.iter().all(|c| c.verdict == Verdict::NotApplicable));
    }
}
