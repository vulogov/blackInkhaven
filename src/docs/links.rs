//! TDOC-2 — link integrity. Two checks over the manuscript:
//!
//! * **Internal** (deterministic, always run): a node's `linked_paragraphs`
//!   references a paragraph that no longer exists — a cross-reference to a
//!   renamed / deleted node.
//! * **External** (opt-in, network): the `http(s)` URLs embedded in prose, checked
//!   for link-rot with the same conservative classifier the research `/deadsources`
//!   sweep uses (only 404/410 and hard failures count as dead).

use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};

/// One broken internal cross-reference found by the sweep.
#[derive(Debug, Clone)]
pub struct LinkFinding {
    /// The paragraph's slug-path.
    pub loc: String,
    /// The broken target (the linked paragraph id).
    pub target: String,
    /// Why it is considered broken.
    pub reason: String,
}

/// Whether a node sits under a *system* book (so it is not manuscript prose).
fn under_system_book(h: &Hierarchy, node: &Node) -> bool {
    let mut cur = node;
    loop {
        if cur.kind == NodeKind::Book {
            return cur.system_tag.is_some();
        }
        let Some(pid) = cur.parent_id else { return false };
        match h.get(pid) {
            Some(p) => cur = p,
            None => return false,
        }
    }
}

/// Internal cross-reference check: any `linked_paragraphs` target that no longer
/// resolves in the hierarchy. Pure and deterministic.
pub fn check_internal(h: &Hierarchy) -> Vec<LinkFinding> {
    let mut out = Vec::new();
    for node in h.iter() {
        if node.kind != NodeKind::Paragraph || under_system_book(h, node) {
            continue;
        }
        for target in &node.linked_paragraphs {
            if h.get(*target).is_none() {
                out.push(LinkFinding {
                    loc: h.slug_path(node),
                    target: target.to_string(),
                    reason: "linked paragraph no longer exists".into(),
                });
            }
        }
    }
    out
}

/// Extract `http(s)` URLs from a paragraph body (both `#link("…")` and bare),
/// trimming trailing sentence punctuation. Deduplicated.
pub fn extract_urls(body: &str) -> Vec<String> {
    let mut urls: Vec<String> = Vec::new();
    for token in body.split(|c: char| {
        c.is_whitespace() || matches!(c, '"' | '(' | ')' | '[' | ']' | '<' | '>' | '`' | '\\')
    }) {
        let pos = token.find("http://").or_else(|| token.find("https://"));
        if let Some(pos) = pos {
            let url = token[pos..].trim_end_matches(|c| matches!(c, '.' | ',' | ';' | ':' | '!' | '?'));
            if url.len() > "https://".len() {
                urls.push(url.to_string());
            }
        }
    }
    urls.sort();
    urls.dedup();
    urls
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_urls_finds_link_calls_and_bare_urls() {
        let body = "See #link(\"https://example.com/a\")[the docs] and http://plain.example.org/b, \
                    also (https://example.com/a) again. No url here.";
        let urls = extract_urls(body);
        assert_eq!(
            urls,
            vec![
                "http://plain.example.org/b".to_string(),
                "https://example.com/a".to_string(), // deduped
            ]
        );
    }

    #[test]
    fn extract_urls_trims_trailing_punctuation() {
        assert_eq!(extract_urls("read https://x.example/y."), vec!["https://x.example/y"]);
    }
}
