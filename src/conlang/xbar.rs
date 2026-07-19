//! LING-1 L-P6b (Wave 3) — X-bar phrase structure.
//!
//! Builds the X-bar tree of a clause from its components — subject, verb, and
//! objects — placing heads and complements according to the language's word
//! order (head-initial vs head-final). This is the deterministic *builder*: given
//! a described clause it constructs the CP → TP → VP projection that generative
//! syntax assumes. (Parsing an arbitrary surface string *into* a tree is the
//! harder inverse, and comes later; movement and binding read the tree this
//! produces.)
//!
//! Specifiers sit left (the near-universal position); the head–complement order
//! is the one parameter, set by `word_order`.

use serde::Serialize;

/// A node in the X-bar tree: a phrasal projection (with children) or a terminal
/// (with a word).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SyntaxNode {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub word: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<SyntaxNode>,
}

impl SyntaxNode {
    fn leaf(label: &str, word: &str) -> SyntaxNode {
        SyntaxNode { label: label.to_string(), word: Some(word.to_string()), children: Vec::new() }
    }
    fn phrase(label: &str, children: Vec<SyntaxNode>) -> SyntaxNode {
        SyntaxNode { label: label.to_string(), word: None, children }
    }
    /// A minimal NP over a noun (`NP → N`).
    fn np(word: &str) -> SyntaxNode {
        SyntaxNode::phrase("NP", vec![SyntaxNode::leaf("N", word)])
    }

    fn label_line(&self) -> String {
        match &self.word {
            Some(w) => format!("{} {}", self.label, w),
            None => self.label.clone(),
        }
    }

    /// Render as an indented tree.
    pub fn render(&self) -> String {
        let mut s = String::new();
        s.push_str(&self.label_line());
        s.push('\n');
        self.render_children(&mut s, "");
        s
    }

    fn render_children(&self, out: &mut String, prefix: &str) {
        let n = self.children.len();
        for (i, c) in self.children.iter().enumerate() {
            let last = i + 1 == n;
            let branch = if last { "└─ " } else { "├─ " };
            out.push_str(&format!("{prefix}{branch}{}\n", c.label_line()));
            let child_prefix = format!("{prefix}{}", if last { "   " } else { "│  " });
            c.render_children(out, &child_prefix);
        }
    }

    /// Bracketed notation, e.g. `[TP [NP she] [T' [T ∅] [VP [V' [V sees] [NP bird]]]]]`.
    pub fn bracketed(&self) -> String {
        match &self.word {
            Some(w) => format!("[{} {}]", self.label, w),
            None => {
                let kids: Vec<String> = self.children.iter().map(|c| c.bracketed()).collect();
                format!("[{} {}]", self.label, kids.join(" "))
            }
        }
    }
}

/// `true` for object-before-verb (head-final) orders.
fn is_head_final(word_order: &str) -> bool {
    matches!(word_order.to_lowercase().as_str(), "sov" | "osv" | "ovs")
}

/// Build the X-bar tree of a clause. `object` / `indirect` are optional
/// complements; head–complement order follows `word_order`.
pub fn build(
    word_order: &str,
    verb: &str,
    subject: &str,
    object: Option<&str>,
    indirect: Option<&str>,
) -> SyntaxNode {
    let hf = is_head_final(word_order);

    // VP → V' → (complements + head, ordered by head-direction).
    let v = SyntaxNode::leaf("V", verb);
    let comps: Vec<SyntaxNode> =
        [object, indirect].into_iter().flatten().map(SyntaxNode::np).collect();
    let mut vbar_children = Vec::new();
    if hf {
        vbar_children.extend(comps);
        vbar_children.push(v);
    } else {
        vbar_children.push(v);
        vbar_children.extend(comps);
    }
    let vp = SyntaxNode::phrase("VP", vec![SyntaxNode::phrase("V'", vbar_children)]);

    // T' → (T + VP), ordered by head-direction.
    let t = SyntaxNode::leaf("T", "∅");
    let tbar = SyntaxNode::phrase("T'", if hf { vec![vp, t] } else { vec![t, vp] });

    // TP → NP-subject (specifier, left) + T'.
    let tp = SyntaxNode::phrase("TP", vec![SyntaxNode::np(subject), tbar]);

    // CP → (C + TP), ordered by head-direction.
    let c = SyntaxNode::leaf("C", "∅");
    SyntaxNode::phrase("CP", if hf { vec![tp, c] } else { vec![c, tp] })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_head_initial_transitive_clause_puts_the_verb_before_its_object() {
        // SVO: within V', V precedes the object NP.
        let t = build("svo", "sees", "she", Some("bird"), None);
        let br = t.bracketed();
        assert!(br.contains("[V' [V sees] [NP [N bird]]]"), "{br}");
        // C precedes TP at the top.
        assert!(t.children[0].label == "C" && t.children[1].label == "TP");
    }

    #[test]
    fn a_head_final_clause_puts_the_object_before_the_verb() {
        // SOV: within V', the object NP precedes V; T follows VP.
        let t = build("sov", "sees", "she", Some("bird"), None);
        let br = t.bracketed();
        assert!(br.contains("[V' [NP [N bird]] [V sees]]"), "{br}");
        // The T' has VP before T (head-final).
        let tp = &t.children[0];
        let tbar = &tp.children[1];
        assert_eq!(tbar.children[0].label, "VP");
        assert_eq!(tbar.children[1].label, "T");
    }

    #[test]
    fn a_ditransitive_clause_has_two_complements() {
        let t = build("svo", "gives", "she", Some("book"), Some("him"));
        let br = t.bracketed();
        assert!(br.contains("book") && br.contains("him"));
        // VP's V' holds V + two NP complements.
        let vbar = find(&t, "V'").unwrap();
        assert_eq!(vbar.children.len(), 3);
    }

    #[test]
    fn the_subject_is_the_specifier_of_tp() {
        let t = build("svo", "sees", "she", Some("bird"), None);
        let tp = find(&t, "TP").unwrap();
        assert_eq!(tp.children[0].label, "NP");
        assert_eq!(tp.children[0].children[0].word.as_deref(), Some("she"));
    }

    fn find<'a>(n: &'a SyntaxNode, label: &str) -> Option<&'a SyntaxNode> {
        if n.label == label {
            return Some(n);
        }
        n.children.iter().find_map(|c| find(c, label))
    }
}
