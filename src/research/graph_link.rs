//! GRAPHMIND GM-P2 — fact-add edge proposals. Propose typed stance edges
//! (`Judged`, advisory) from a fact to the facts it relates to — the confront
//! machinery pointed at your own corpus — for you to triage (`graph pending`, or
//! the confront `P`/`d` keys). Explicit + on-demand (`inkhaven graph link
//! <node>`), so it never slows a fact insert or spends tokens by surprise.

use std::collections::HashMap;

use uuid::Uuid;

use crate::config::Config;
use crate::storage::edge_store::{Edge, EdgeOrigin, EndpointRef};
use crate::store::hierarchy::Hierarchy;
use crate::store::Store;

use super::contradiction::{self, Evidence, Relation};

/// Map graded relations (from the `/relate` judge, each keyed by its evidence
/// label) to `Judged` stance edges from the new fact to each related fact node.
/// `Silent` relations, self-links, and labels that don't resolve to a node are
/// skipped. Unlike confront's `Extern::Evidence` edges, the far endpoint here is
/// a real fact **node**. Pure.
pub(super) fn propose_stance_edges(
    new_fact: Uuid,
    relations: &[Relation],
    neighbor_by_label: &HashMap<String, Uuid>,
) -> Vec<Edge> {
    let mut edges = Vec::new();
    for r in relations {
        let Some(kind) = contradiction::stance_to_edge_kind(r.stance) else {
            continue;
        };
        let Some(&dst) = neighbor_by_label.get(&r.label) else {
            continue;
        };
        if dst == new_fact {
            continue;
        }
        let mut e = Edge::new(
            EndpointRef::Node(new_fact),
            kind,
            EndpointRef::Node(dst),
            EdgeOrigin::Judged,
        );
        if !r.reason.is_empty() {
            e = e.with_reason(r.reason.clone());
        }
        edges.push(e);
    }
    edges
}

/// Propose + persist stance edges from a fact to its nearest related facts:
/// retrieve the neighbours (book-RAG over the Facts book), grade each relation
/// with the `/relate` judge, and persist the non-`Silent` ones as `Judged`
/// edges. Returns the number proposed. Best-effort — an LLM/IO failure yields 0.
pub(crate) fn link_fact(
    store: &Store,
    hierarchy: &Hierarchy,
    cfg: &Config,
    ai: &crate::ai::AiClient,
    model: &str,
    facts_book_id: Uuid,
    new_id: Uuid,
    body: &str,
    lang: &str,
) -> usize {
    let Ok(passages) =
        crate::book_rag::retrieval::retrieve(store, hierarchy, &cfg.book_rag, facts_book_id, body)
    else {
        return 0;
    };
    let mut evidence: Vec<Evidence> = Vec::new();
    let mut by_label: HashMap<String, Uuid> = HashMap::new();
    for p in passages.into_iter().filter(|p| p.id != new_id).take(6) {
        by_label.insert(p.breadcrumb.clone(), p.id);
        evidence.push(Evidence { label: p.breadcrumb, body: p.body });
    }
    if evidence.is_empty() {
        return 0;
    }
    let system = contradiction::relate_system(lang);
    let user = contradiction::relate_user(body, &evidence);
    let Ok(reply) = crate::ai::stream::collect_blocking(
        ai.client.clone(),
        model.to_string(),
        Some(system),
        user,
    ) else {
        return 0;
    };
    let relations = contradiction::parse_relations(&reply, &evidence);
    let edges = propose_stance_edges(new_id, &relations, &by_label);
    if edges.is_empty() {
        return 0;
    }
    let _ = store.add_edges(&edges);
    edges.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::research::contradiction::Stance;
    use crate::storage::edge_store::{EdgeKind, ExternRef};

    fn rel(stance: Stance, label: &str, reason: &str) -> Relation {
        Relation { label: label.into(), stance, reason: reason.into() }
    }

    #[test]
    fn proposes_judged_node_to_node_stance_edges() {
        let (new_fact, a, b) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
        let mut by_label = HashMap::new();
        by_label.insert("fact: A".to_string(), a);
        by_label.insert("fact: B".to_string(), b);
        let relations = vec![
            rel(Stance::Contradicts, "fact: A", "opposes it"),
            rel(Stance::Silent, "fact: B", "nothing relevant"), // skipped
            rel(Stance::Agrees, "fact: unknown", "backs it"),   // no node → skipped
        ];
        let edges = propose_stance_edges(new_fact, &relations, &by_label);
        assert_eq!(edges.len(), 1, "Silent + unresolved-label relations are dropped");
        let e = &edges[0];
        assert_eq!(e.kind, EdgeKind::Contradicts);
        assert_eq!(e.src, EndpointRef::Node(new_fact));
        assert_eq!(e.dst, EndpointRef::Node(a), "far endpoint is a real fact node");
        assert_eq!(e.origin, EdgeOrigin::Judged);
        assert_eq!(e.reason.as_deref(), Some("opposes it"));
        assert!(!matches!(e.dst, EndpointRef::Extern(ExternRef::Evidence { .. })));
    }
}
