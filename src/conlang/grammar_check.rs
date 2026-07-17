//! LING-1 Wave-2 — validation of the typed grammar blocks.
//!
//! Checks a `GrammarSpec`'s `ug_parameters` and `verb_classes` blocks: that
//! every parameter and value is recognised, that verb valences are valid and
//! names unique, and — the useful part — that the generative parameters do not
//! *contradict* the language's WALS feature answers (a `head_final: true`
//! language whose `word_order` is SVO is telling two different stories). Pure +
//! deterministic; a finding is a flag, not a hard error.

use serde::Serialize;

use crate::conlang::grammar::{self};
use crate::conlang::types::grammar::GrammarSpec;

/// The kind of a validation finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueKind {
    UnknownParameter,
    InvalidValue,
    InvalidValence,
    DuplicateVerbClass,
    /// A typed block disagrees with the flat WALS feature answers.
    Inconsistency,
}

/// One validation finding.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Issue {
    pub kind: IssueKind,
    pub message: String,
}

/// The result of validating the typed grammar blocks.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct GrammarCheck {
    pub parameter_count: usize,
    pub verb_class_count: usize,
    pub issues: Vec<Issue>,
}

impl GrammarCheck {
    pub fn ok(&self) -> bool {
        self.issues.is_empty()
    }
}

fn get<'a>(map: &'a std::collections::BTreeMap<String, String>, k: &str) -> Option<&'a str> {
    map.get(k).map(|s| s.trim()).filter(|s| !s.is_empty())
}

/// `true` for object-before-verb orders, `false` for verb-before-object.
fn word_order_is_final(word_order: &str) -> Option<bool> {
    match word_order.to_lowercase().as_str() {
        "sov" | "osv" | "ovs" => Some(true),
        "svo" | "vso" | "vos" => Some(false),
        _ => None,
    }
}

/// Validate the typed blocks in `spec`.
pub fn check(spec: &GrammarSpec) -> GrammarCheck {
    let mut c = GrammarCheck {
        parameter_count: spec.ug_parameters.len(),
        verb_class_count: spec.verb_classes.len(),
        ..Default::default()
    };

    // ── UG parameters: recognised id + valid value ──
    for (id, value) in &spec.ug_parameters {
        match grammar::ug_parameter(id) {
            None => c.issues.push(Issue {
                kind: IssueKind::UnknownParameter,
                message: format!("unknown parameter `{id}`"),
            }),
            Some(p) if !p.is_valid(value) => c.issues.push(Issue {
                kind: IssueKind::InvalidValue,
                message: format!("`{id}` = `{value}` — expected one of {}", p.values.join(" | ")),
            }),
            Some(_) => {}
        }
    }

    // ── Consistency: head_final vs word order + adposition ──
    if let Some(hf) = get(&spec.ug_parameters, "head_final") {
        let head_final = hf.eq_ignore_ascii_case("true");
        if let Some(wo) = get(&spec.grammar, "word_order") {
            if let Some(order_final) = word_order_is_final(wo) {
                if head_final != order_final {
                    c.issues.push(Issue {
                        kind: IssueKind::Inconsistency,
                        message: format!(
                            "head_final = {head_final} but word_order `{wo}` is head-{}",
                            if order_final { "final" } else { "initial" }
                        ),
                    });
                }
            }
        }
        if let Some(adp) = get(&spec.grammar, "adposition") {
            let adp_final = adp.eq_ignore_ascii_case("postposition");
            let adp_initial = adp.eq_ignore_ascii_case("preposition");
            if (head_final && adp_initial) || (!head_final && adp_final) {
                c.issues.push(Issue {
                    kind: IssueKind::Inconsistency,
                    message: format!(
                        "head_final = {head_final} but adposition `{adp}` points the other way"
                    ),
                });
            }
        }
    }

    // ── Verb classes: valid valence + unique names ──
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for vc in &spec.verb_classes {
        let name = vc.name.trim();
        if name.is_empty() {
            c.issues.push(Issue {
                kind: IssueKind::InvalidValence,
                message: "a verb class has no name".to_string(),
            });
            continue;
        }
        if !seen.insert(name.to_lowercase()) {
            c.issues.push(Issue {
                kind: IssueKind::DuplicateVerbClass,
                message: format!("duplicate verb class `{name}`"),
            });
        }
        let val = vc.valence.trim();
        if val.is_empty() || !grammar::VERB_VALENCES.iter().any(|v| v.eq_ignore_ascii_case(val)) {
            c.issues.push(Issue {
                kind: IssueKind::InvalidValence,
                message: format!(
                    "verb class `{name}` valence `{val}` — expected one of {}",
                    grammar::VERB_VALENCES.join(" | ")
                ),
            });
        }
    }

    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conlang::types::grammar::VerbClass;

    fn spec(pairs: &[(&str, &str)], params: &[(&str, &str)], verbs: &[(&str, &str)]) -> GrammarSpec {
        GrammarSpec {
            grammar: pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            ug_parameters: params.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            verb_classes: verbs
                .iter()
                .map(|(n, v)| VerbClass { name: n.to_string(), valence: v.to_string(), note: String::new() })
                .collect(),
        }
    }

    #[test]
    fn a_consistent_spec_has_no_issues() {
        let s = spec(
            &[("word_order", "sov"), ("adposition", "postposition")],
            &[("head_final", "true"), ("pro_drop", "true")],
            &[("motion", "intransitive"), ("handling", "transitive")],
        );
        let c = check(&s);
        assert!(c.ok(), "issues: {:?}", c.issues);
        assert_eq!(c.parameter_count, 2);
        assert_eq!(c.verb_class_count, 2);
    }

    #[test]
    fn head_final_contradicting_word_order_is_flagged() {
        let s = spec(&[("word_order", "svo")], &[("head_final", "true")], &[]);
        let c = check(&s);
        assert!(c.issues.iter().any(|i| i.kind == IssueKind::Inconsistency));
    }

    #[test]
    fn unknown_parameter_and_bad_value_are_flagged() {
        let s = spec(&[], &[("teleportation", "true"), ("pro_drop", "maybe")], &[]);
        let c = check(&s);
        assert!(c.issues.iter().any(|i| i.kind == IssueKind::UnknownParameter));
        assert!(c.issues.iter().any(|i| i.kind == IssueKind::InvalidValue));
    }

    #[test]
    fn bad_valence_and_duplicate_verb_class_are_flagged() {
        let s = spec(&[], &[], &[("a", "transitive"), ("A", "transitive"), ("b", "quadrivalent")]);
        let c = check(&s);
        assert!(c.issues.iter().any(|i| i.kind == IssueKind::DuplicateVerbClass));
        assert!(c.issues.iter().any(|i| i.kind == IssueKind::InvalidValence));
    }
}
