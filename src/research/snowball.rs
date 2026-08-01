//! RESRCH-6 (snowball) — citation-following. Given a seed paper (a title, DOI, or
//! topic), it resolves the seed on OpenAlex and follows its citations both ways:
//! **backward** to the works it references, and **forward** to the works that
//! cite it — the seed's citation neighborhood. The neighbourhood is reported (not
//! auto-ingested), each paper with its OpenAlex id, so the author chooses which to
//! bring in as a Source (`research --openalex "<id>"`) — keeping the research
//! corpus curated rather than flooded, the same discipline the agentic loop uses.
//!
//! Built on `scholarly.rs` (OpenAlex, keyless). Gated by `research.scholarly`.

use anyhow::{Result, anyhow};

use crate::config::Config;
use crate::storage::edge_store::{Edge, EdgeKind, EdgeOrigin, EndpointRef, ExternRef, Registry};
use crate::store::Store;

use super::scholarly::{self, Paper};

/// How many papers to follow per direction (backward references / forward citers).
const LIMIT: usize = 10;

/// Run a snowball pass from `seed_query`. `out` is the report path (stdout when
/// `None`). The citation neighbourhood is also persisted into the project graph
/// as `Cites` edges (`store`), so the latent citation graph accumulates instead
/// of being rendered and discarded.
pub(crate) fn run(cfg: &Config, store: &Store, seed_query: &str, out: Option<&str>) -> Result<()> {
    if !scholarly::available(&cfg.research.scholarly) {
        return Err(anyhow!(
            "scholarly sources are disabled — set `research.scholarly.enabled: true` in inkhaven.hjson"
        ));
    }
    let seed_query = seed_query.trim();
    if seed_query.is_empty() {
        return Err(anyhow!("give a seed paper to snowball from (a title, DOI, or topic)"));
    }
    let sc = cfg.research.scholarly.clone();

    eprintln!("» snowball: resolving seed \"{seed_query}\"…");
    let seed = block_on_async(scholarly::openalex(sc.clone(), seed_query.to_string()))
        .map_err(|e| anyhow!("could not resolve a seed paper for `{seed_query}`: {e}"))?;
    eprintln!("· seed: {} ({})", seed.title, seed.id);

    // Backward (works the seed references) + forward (works that cite the seed).
    let backward =
        block_on_async(scholarly::works_by_ids(sc.clone(), seed.referenced_works.clone(), LIMIT))
            .unwrap_or_default();
    let forward =
        block_on_async(scholarly::cited_by(sc.clone(), seed.id.clone(), LIMIT)).unwrap_or_default();

    // SEMNET — persist the citation neighbourhood as durable `Cites` edges (the
    // paths() citation-chain query reads these). Best-effort — never fails the
    // report.
    match store.persist_cites(&cites_edges(&seed, &backward, &forward)) {
        Ok(n) if n > 0 => eprintln!("· graph: +{n} cites edge(s)"),
        Ok(_) => {}
        Err(e) => eprintln!("note: could not persist cites edges to the graph: {e}"),
    }

    let report = render(&seed, &backward, &forward);
    match out {
        Some(p) => {
            std::fs::write(p, &report).map_err(|e| anyhow!("write {p}: {e}"))?;
            eprintln!("report → {p}");
        }
        None => print!("{report}"),
    }
    eprintln!(
        "✓ snowball complete — {} reference(s), {} citer(s). Bring any into your Sources \
         with `/openalex <query>` in the research TUI.",
        backward.len(),
        forward.len()
    );
    Ok(())
}

/// Drive one owned, `Send + 'static` future to completion from this (in-runtime,
/// multi-thread) sync context: spawn it and block on the result — the same
/// mechanism `ai::stream::collect_blocking` relies on.
fn block_on_async<F, T>(fut: F) -> T
where
    F: std::future::Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let _ = tx.send(fut.await);
    });
    rx.blocking_recv().expect("snowball async task was dropped")
}

fn render(seed: &Paper, backward: &[Paper], forward: &[Paper]) -> String {
    let line = |p: &Paper| {
        let who = if p.authors.is_empty() {
            String::new()
        } else {
            let mut a = p.authors.first().cloned().unwrap_or_default();
            if p.authors.len() > 1 {
                a.push_str(" et al.");
            }
            format!(" — {a}")
        };
        let yr = if p.year.is_empty() { String::new() } else { format!(", {}", p.year) };
        format!("- {}{who}{yr}  `openalex:{}`\n", p.title, p.id)
    };

    let mut s = String::from("# Snowball — citation neighborhood\n\n");
    s.push_str(&format!("**Seed:** {} `openalex:{}`\n\n", seed.title, seed.id));

    s.push_str(&format!("## References (backward) — {} works the seed cites\n\n", backward.len()));
    if backward.is_empty() {
        s.push_str("_none found (OpenAlex lists no references for this work)._\n\n");
    } else {
        for p in backward {
            s.push_str(&line(p));
        }
        s.push('\n');
    }

    s.push_str(&format!("## Citations (forward) — {} works that cite the seed\n\n", forward.len()));
    if forward.is_empty() {
        s.push_str("_none found (the seed has no indexed citers yet)._\n\n");
    } else {
        for p in forward {
            s.push_str(&line(p));
        }
        s.push('\n');
    }

    s.push_str("_Bring any of these into your Sources with `/openalex <title or DOI>` in the research TUI._\n");
    s
}

// ── SEMNET — snowball citation neighbourhood → graph `Cites` edges ────

fn registry_for_source(source: &str) -> Registry {
    match source {
        "openalex" => Registry::OpenAlex,
        "arxiv" => Registry::Arxiv,
        _ => Registry::Other,
    }
}

/// A paper as a `Work` graph endpoint.
fn work_ep(p: &Paper) -> EndpointRef {
    EndpointRef::Extern(ExternRef::Work {
        registry: registry_for_source(p.source),
        id: p.id.clone(),
    })
}

/// The citation edges of a snowball pass: the seed **cites** each backward
/// reference, and each forward citer **cites** the seed. `Work → Work`,
/// `origin = Imported` (external reference data — `graph rebuild` preserves it).
/// Papers with no id are skipped. Pure.
fn cites_edges(seed: &Paper, backward: &[Paper], forward: &[Paper]) -> Vec<Edge> {
    let seed_ep = work_ep(seed);
    let mut edges = Vec::new();
    if seed.id.is_empty() {
        return edges;
    }
    for b in backward.iter().filter(|p| !p.id.is_empty()) {
        edges.push(
            Edge::new(seed_ep.clone(), EdgeKind::Cites, work_ep(b), EdgeOrigin::Imported)
                .with_attrs(serde_json::json!({ "title": b.title })),
        );
    }
    for f in forward.iter().filter(|p| !p.id.is_empty()) {
        edges.push(
            Edge::new(work_ep(f), EdgeKind::Cites, seed_ep.clone(), EdgeOrigin::Imported)
                .with_attrs(serde_json::json!({ "title": f.title })),
        );
    }
    edges
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paper(source: &'static str, id: &str, title: &str) -> Paper {
        Paper {
            source,
            id: id.into(),
            doi: String::new(),
            title: title.into(),
            authors: Vec::new(),
            year: String::new(),
            abstract_: String::new(),
            url: String::new(),
            referenced_works: Vec::new(),
        }
    }

    #[test]
    fn cites_edges_link_seed_to_references_and_citers() {
        let seed = paper("openalex", "W1", "Seed");
        let backward = vec![paper("openalex", "W2", "Ref"), paper("openalex", "", "no-id")];
        let forward = vec![paper("arxiv", "2401.1", "Citer")];
        let edges = cites_edges(&seed, &backward, &forward);
        assert_eq!(edges.len(), 2, "the id-less reference is skipped");

        // Seed cites its backward reference (Work→Work, Imported, directed).
        let to_ref = edges.iter().find(|e| e.dst == work_ep(&backward[0])).unwrap();
        assert_eq!(to_ref.kind, EdgeKind::Cites);
        assert_eq!(to_ref.src, work_ep(&seed));
        assert!(to_ref.directed);
        assert_eq!(to_ref.origin, EdgeOrigin::Imported);

        // The forward citer (arXiv) cites the seed.
        let from_citer = edges.iter().find(|e| e.dst == work_ep(&seed)).unwrap();
        assert_eq!(
            from_citer.src,
            EndpointRef::Extern(ExternRef::Work { registry: Registry::Arxiv, id: "2401.1".into() })
        );
    }
}
