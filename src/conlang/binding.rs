//! LING-1 L-P6b (Wave 3) — binding theory.
//!
//! Decides whether one argument of a clause can refer to another, from the
//! structure `xbar` builds. The engine is *c-command* — a node c-commands its
//! sibling and everything the sibling dominates — plus the three binding
//! principles:
//!   * A — an anaphor (reflexive) must be bound (c-commanded by a coindexed
//!     antecedent) in its clause;
//!   * B — a pronoun must be *free* (not so bound) in its clause;
//!   * C — a name must be free everywhere.
//!
//! So a reflexive object *may* corefer with the subject that c-commands it, a
//! pronoun object may *not*, and neither may a name. Deterministic.

use serde::Serialize;

use crate::conlang::xbar::{self, SyntaxNode};

/// The binding verdict for one antecedent–anaphor pair.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BindingReport {
    pub antecedent: String,
    pub anaphor: String,
    /// `reflexive` | `pronoun` | `name`.
    pub anaphor_type: String,
    /// Does the antecedent c-command the anaphor?
    pub c_commands: bool,
    /// The binding principle applied (`A` | `B` | `C`).
    pub principle: &'static str,
    /// `required` | `permitted` | `forbidden`.
    pub coreference: &'static str,
    pub note: String,
}

fn word_of(role: &str, subject: &str, object: Option<&str>, indirect: Option<&str>) -> Option<String> {
    match role.to_lowercase().as_str() {
        "subject" | "subj" => Some(subject.to_string()),
        "object" | "obj" => object.map(str::to_string),
        "indirect" | "iobj" | "recipient" => indirect.map(str::to_string),
        _ => None,
    }
}

/// Analyse whether `anaphor_role` (of `anaphor_type`) can corefer with
/// `antecedent_role` in the clause. `None` if a role isn't filled.
pub fn analyze(
    word_order: &str,
    verb: &str,
    subject: &str,
    object: Option<&str>,
    indirect: Option<&str>,
    antecedent_role: &str,
    anaphor_role: &str,
    anaphor_type: &str,
) -> Option<BindingReport> {
    let antecedent = word_of(antecedent_role, subject, object, indirect)?;
    let anaphor = word_of(anaphor_role, subject, object, indirect)?;
    let tree = xbar::build(word_order, verb, subject, object, indirect);
    let cc = c_commands(&tree, &antecedent, &anaphor);

    let (principle, coreference, note): (&'static str, &'static str, String) =
        match anaphor_type.to_lowercase().as_str() {
            "reflexive" | "anaphor" => {
                if cc {
                    ("A", "required", format!("a reflexive must be bound; `{antecedent}` c-commands it and binds it"))
                } else {
                    ("A", "forbidden", format!("a reflexive must be bound, but `{antecedent}` does not c-command it — unbound (ungrammatical)"))
                }
            }
            "pronoun" => {
                if cc {
                    ("B", "forbidden", format!("a pronoun must be free in its clause, but `{antecedent}` c-commands it"))
                } else {
                    ("B", "permitted", format!("`{antecedent}` does not c-command it, so the pronoun is free to corefer"))
                }
            }
            _ => {
                // name / R-expression → Principle C.
                if cc {
                    ("C", "forbidden", format!("a name must be free; `{antecedent}` c-commands it (a Principle C violation)"))
                } else {
                    ("C", "permitted", format!("`{antecedent}` does not c-command it, so coreference is free"))
                }
            }
        };

    Some(BindingReport {
        antecedent,
        anaphor,
        anaphor_type: anaphor_type.to_lowercase(),
        c_commands: cc,
        principle,
        coreference,
        note,
    })
}

/// Does the NP headed by `a` c-command the NP headed by `b`? True iff `b` lies in
/// the subtree of `a`'s parent but not in `a`'s own subtree.
fn c_commands(root: &SyntaxNode, a: &str, b: &str) -> bool {
    match parent_and_np(root, a) {
        Some((parent, np)) => contains_head(parent, b) && !contains_head(np, b),
        None => false,
    }
}

/// The parent node and the NP node headed by `word`.
fn parent_and_np<'a>(node: &'a SyntaxNode, word: &str) -> Option<(&'a SyntaxNode, &'a SyntaxNode)> {
    for child in &node.children {
        if child.label == "NP" && child.children.first().and_then(|n| n.word.as_deref()) == Some(word) {
            return Some((node, child));
        }
        if let Some(found) = parent_and_np(child, word) {
            return Some(found);
        }
    }
    None
}

/// Whether any N terminal in `node`'s subtree has head `word`.
fn contains_head(node: &SyntaxNode, word: &str) -> bool {
    if node.label == "N" && node.word.as_deref() == Some(word) {
        return true;
    }
    node.children.iter().any(|c| contains_head(c, word))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(anaphor_type: &str) -> BindingReport {
        // "she Vs OBJ" — subject `she`, object `x`; test coreference of the object.
        analyze("svo", "sees", "she", Some("x"), None, "subject", "object", anaphor_type).unwrap()
    }

    #[test]
    fn subject_c_commands_the_object() {
        let r = run("reflexive");
        assert!(r.c_commands, "subject should c-command object");
    }

    #[test]
    fn a_reflexive_object_may_bind_to_the_subject() {
        let r = run("reflexive");
        assert_eq!(r.principle, "A");
        assert_eq!(r.coreference, "required");
    }

    #[test]
    fn a_pronoun_object_may_not_corefer_with_the_subject() {
        let r = run("pronoun");
        assert_eq!(r.principle, "B");
        assert_eq!(r.coreference, "forbidden");
    }

    #[test]
    fn a_name_object_is_a_principle_c_violation() {
        let r = run("name");
        assert_eq!(r.principle, "C");
        assert_eq!(r.coreference, "forbidden");
    }

    #[test]
    fn the_object_does_not_c_command_the_subject() {
        // Reverse the roles: can the subject be an anaphor bound by the object?
        let r = analyze("svo", "sees", "she", Some("x"), None, "object", "subject", "pronoun").unwrap();
        assert!(!r.c_commands, "object should not c-command subject");
        // A pronoun subject is then free to corefer.
        assert_eq!(r.coreference, "permitted");
    }
}
