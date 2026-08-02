//! GRAPHMIND GM-P5/P8 — the read-only [`GraphOracle`](super::ask::GraphOracle)
//! over a real `Store`. The graph-query surface the `ask` tool-loop walks,
//! shared by the `graph ask` CLI (GM-P5) and the in-editor graph walk (GM-P8) so
//! there is one implementation, not two. Every method degrades to a readable
//! "(none)" / error line rather than aborting — a single failed query shouldn't
//! kill the exploration.

use uuid::Uuid;

use crate::store::graph::{EdgeKind, EndpointRef};
use crate::store::hierarchy::Hierarchy;
use crate::store::Store;

use super::ask::GraphOracle;

/// A [`GraphOracle`] backed by a live store + hierarchy (both borrowed — the
/// oracle owns nothing and does only read-only queries).
pub struct StoreOracle<'a> {
    pub store: &'a Store,
    pub hierarchy: &'a Hierarchy,
}

impl StoreOracle<'_> {
    fn node_label(&self, id: Uuid) -> String {
        self.hierarchy
            .get(id)
            .map(|n| n.title.clone())
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| format!("node {}", &id.to_string()[..8]))
    }

    fn ep_label(&self, ep: &EndpointRef) -> String {
        match ep {
            EndpointRef::Node(u) => self.node_label(*u),
            EndpointRef::Extern(_) => {
                let (k, r) = ep.as_columns();
                format!("{k} {r}")
            }
        }
    }
}

impl GraphOracle for StoreOracle<'_> {
    fn search(&self, query: &str, limit: usize) -> Vec<(Uuid, String)> {
        let pool = (limit * 3 + 8).max(8);
        let raw = self.store.search_text(query, pool).unwrap_or_default();
        let mut out = Vec::new();
        for v in &raw {
            let Some(id) = v
                .get("id")
                .and_then(|x| x.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
            else {
                continue;
            };
            if self.hierarchy.get(id).is_none() {
                continue;
            }
            out.push((id, self.node_label(id)));
            if out.len() >= limit {
                break;
            }
        }
        out
    }

    fn neighbors(&self, node: Uuid) -> String {
        let edges = self.store.subgraph(node, 1, &[]).unwrap_or_default();
        crate::store::graph::render_neighbourhood(node, &edges, |ep| self.ep_label(ep))
    }

    fn contradicting(&self, node: Uuid) -> String {
        let edges = match self.store.contradicting(node) {
            Ok(e) => e,
            Err(e) => return format!("(graph error: {e})"),
        };
        if edges.is_empty() {
            return "(no contradictions recorded)".to_string();
        }
        let here = EndpointRef::Node(node);
        edges
            .iter()
            .map(|e| {
                let reason = e.reason.as_deref().filter(|r| !r.is_empty());
                let sep = if reason.is_some() { " — " } else { "" };
                format!(
                    "  [{}] {}{sep}{}",
                    e.kind.as_str(),
                    self.ep_label(e.other_endpoint(&here)),
                    reason.unwrap_or("")
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn loci(&self, node: Uuid) -> String {
        let edges = match self.store.edges_out(node, &[EdgeKind::CitesLocus]) {
            Ok(e) => e,
            Err(e) => return format!("(graph error: {e})"),
        };
        if edges.is_empty() {
            return "(cites no primary-source loci)".to_string();
        }
        edges
            .iter()
            .map(|e| {
                let (_k, r) = e.dst.as_columns();
                let key = e.attrs.get("key").and_then(|v| v.as_str()).unwrap_or("");
                format!("  @{key}  {r}")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn paths(&self, from: Uuid, to: Uuid) -> String {
        match self.store.paths(from, to, &[EdgeKind::Cites, EdgeKind::LinksTo], 8) {
            Ok(Some(path)) => {
                let hops = path.len().saturating_sub(1);
                let names: Vec<String> = path.iter().map(|id| self.node_label(*id)).collect();
                format!("  {hops} hop(s): {}", names.join(" → "))
            }
            Ok(None) => "(no path within 8 hops)".to_string(),
            Err(e) => format!("(graph error: {e})"),
        }
    }

    fn label(&self, node: Uuid) -> String {
        self.node_label(node)
    }
}
