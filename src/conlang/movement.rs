//! LING-1 L-P6b (Wave 3) — syntactic movement.
//!
//! Fronts a constituent — the operation behind wh-questions and topicalisation —
//! over the X-bar tree that `xbar` builds: the moved phrase lands in the
//! specifier of CP and leaves a coindexed *trace* in the position it vacated.
//! This makes the derivation visible (what moved, from where, to where), which
//! is what binding and the syntax Oracle then reason over. Deterministic.

use serde::Serialize;

use crate::conlang::xbar::{self, SyntaxNode};

/// The result of moving a constituent.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MovementReport {
    /// The fronted word.
    pub moved: String,
    /// The grammatical role that moved (subject / object / indirect).
    pub role: String,
    /// The landing site.
    pub landing: &'static str,
    /// The derived tree, with the moved phrase at Spec-CP and a trace behind.
    pub tree: SyntaxNode,
}

/// Front the argument in `role` (`subject` | `object` | `indirect`) of the clause.
/// `None` if the role isn't filled.
pub fn front(
    word_order: &str,
    verb: &str,
    subject: &str,
    object: Option<&str>,
    indirect: Option<&str>,
    role: &str,
) -> Option<MovementReport> {
    let target = match role.to_lowercase().as_str() {
        "subject" | "subj" => Some(subject),
        "object" | "obj" => object,
        "indirect" | "iobj" | "recipient" => indirect,
        _ => None,
    }?;

    let mut tree = xbar::build(word_order, verb, subject, object, indirect);
    let mut captured: Option<SyntaxNode> = None;
    if !replace_with_trace(&mut tree, target, &mut captured) {
        return None;
    }
    let mut moved_np = captured?;
    // Coindex the moved phrase with its trace.
    moved_np.label = "NP\u{2081}".to_string(); // NP₁
    // Land it in Spec-CP (the first child, before C).
    if tree.label == "CP" {
        tree.children.insert(0, moved_np);
    }

    Some(MovementReport {
        moved: target.to_string(),
        role: role.to_lowercase(),
        landing: "Spec-CP",
        tree,
    })
}

/// Replace the first NP headed by `target` with a coindexed trace `t₁`, capturing
/// the removed NP. Returns whether a replacement happened.
fn replace_with_trace(node: &mut SyntaxNode, target: &str, captured: &mut Option<SyntaxNode>) -> bool {
    for i in 0..node.children.len() {
        let child = &node.children[i];
        let is_target_np = child.label == "NP"
            && child.children.first().and_then(|n| n.word.as_deref()) == Some(target);
        if is_target_np {
            *captured = Some(node.children[i].clone());
            node.children[i] = SyntaxNode {
                label: "t\u{2081}".to_string(), // t₁
                word: None,
                children: Vec::new(),
            };
            return true;
        }
        if replace_with_trace(&mut node.children[i], target, captured) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find<'a>(n: &'a SyntaxNode, label: &str) -> Option<&'a SyntaxNode> {
        if n.label == label {
            return Some(n);
        }
        n.children.iter().find_map(|c| find(c, label))
    }

    #[test]
    fn fronting_the_object_leaves_a_trace() {
        // Move the object of an SVO clause to Spec-CP.
        let r = front("svo", "sees", "she", Some("bird"), None, "object").unwrap();
        assert_eq!(r.moved, "bird");
        assert_eq!(r.landing, "Spec-CP");
        // The moved NP₁ is the first child of CP.
        assert_eq!(r.tree.children[0].label, "NP\u{2081}");
        assert_eq!(r.tree.children[0].children[0].word.as_deref(), Some("bird"));
        // A trace t₁ sits where the object was (inside the VP), and "bird" no
        // longer appears as an in-situ N there.
        assert!(find(&r.tree, "t\u{2081}").is_some());
        let vp = find(&r.tree, "VP").unwrap();
        assert!(find(vp, "t\u{2081}").is_some());
    }

    #[test]
    fn fronting_the_subject_works_too() {
        let r = front("svo", "sees", "she", Some("bird"), None, "subject").unwrap();
        assert_eq!(r.moved, "she");
        assert_eq!(r.tree.children[0].label, "NP\u{2081}");
    }

    #[test]
    fn an_unfilled_role_yields_no_movement() {
        // No object to move.
        assert!(front("svo", "sleeps", "she", None, None, "object").is_none());
        // Unknown role.
        assert!(front("svo", "sees", "she", Some("bird"), None, "wombat").is_none());
    }
}
