//! 2.0 hardening — `inkhaven _soak` (hidden). A sustained-load endurance test
//! for the store + SEMNET knowledge-graph layer: opens a throwaway
//! `DocumentStorage` and, for N seconds, hammers node CRUD + edge
//! insert/query/cascade-GC + periodic embedding (which exercises the background
//! vector-sync lock the readiness gate names) + checkpoints, running an
//! `integrity_check` on all three stores every heartbeat. Corruption shows up as
//! a non-`ok` integrity result; a memory leak shows up as unbounded process RSS
//! (sampled externally by the shell wrapper). Self-contained temp store, no
//! project. Prints a parseable `SOAK_DONE …` summary.

use std::collections::{HashSet, VecDeque};
use std::time::{Duration, Instant};

use uuid::Uuid;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::storage::edge_store::{Edge, EdgeKind, EdgeOrigin, EndpointRef, ExternRef};
use crate::storage::DocumentStorage;

/// Live-node window — retiring the oldest node past this keeps disk + edge counts
/// bounded, so a growing process RSS means a real leak, not legitimate growth.
const WINDOW: usize = 200;

pub fn run(seconds: u64) -> Result<()> {
    let dir = std::env::temp_dir().join(format!("inkhaven-soak-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| Error::Config(format!("soak tmp dir: {e}")))?;
    let root = dir.to_str().ok_or_else(|| Error::Config("soak path is not utf-8".into()))?;
    let cfg = Config::default();
    let engine = crate::store::build_embedding_engine(&cfg.embeddings.model)
        .map_err(|e| Error::Config(format!("soak embedding engine: {e}")))?;
    let store = DocumentStorage::with_embedding(root, engine, 4)
        .map_err(|e| Error::Config(format!("soak store: {e}")))?;

    let start = Instant::now();
    let deadline = start + Duration::from_secs(seconds.max(1));
    let mut iter: u64 = 0;
    let mut live: VecDeque<Uuid> = VecDeque::new();
    let mut integrity_ok = true;
    let mut heartbeat = Instant::now();

    println!("soak: start — {seconds}s, pid {}", std::process::id());
    while Instant::now() < deadline {
        iter += 1;

        // A node: embed 1-in-50 (exercises the vector engine + bg sync), else the
        // no-embed path. Both persist metadata + blob.
        let meta = serde_json::json!({ "kind": "paragraph", "title": format!("p{iter}") });
        let body = format!("soak body {iter} — the tide returns at dusk").into_bytes();
        let id = if iter % 50 == 0 {
            let id = store.add_document(meta, &body).map_err(soak_err)?;
            store.sync_in_background();
            id
        } else {
            store.add_document_no_embed(meta, &body).map_err(soak_err)?
        };
        live.push_back(id);

        // Edges: a fan of structural links + a judged stance edge to an extern.
        let mut edges = Vec::with_capacity(10);
        for _ in 0..8 {
            edges.push(Edge::new(
                EndpointRef::Node(id),
                EdgeKind::LinksTo,
                EndpointRef::Node(Uuid::now_v7()),
                EdgeOrigin::Structural,
            ));
        }
        edges.push(Edge::new(
            EndpointRef::Node(id),
            EdgeKind::Contradicts,
            EndpointRef::Extern(ExternRef::Evidence { label: format!("ev{iter}") }),
            EdgeOrigin::Judged,
        ));
        store.add_edges(&edges).map_err(soak_err)?;

        // Reverse-index queries, both directions.
        let _ = store.edges_out(id, &[]).map_err(soak_err)?;
        let _ = store.edges_around(id, &[EdgeKind::LinksTo]).map_err(soak_err)?;

        // Retire the oldest node — delete it + cascade-GC its edges.
        if live.len() > WINDOW {
            if let Some(old) = live.pop_front() {
                let _ = store.delete_document(old);
                let mut set = HashSet::new();
                set.insert(old);
                let _ = store.gc_edges_for_nodes(&set).map_err(soak_err)?;
            }
        }

        // Heartbeat every 10 s: checkpoint + integrity on all three stores.
        if heartbeat.elapsed() >= Duration::from_secs(10) {
            store.checkpoint().map_err(soak_err)?;
            let (m, b) = store.integrity_check().map_err(soak_err)?;
            let e = store.edges_integrity_check().map_err(soak_err)?;
            let ok = m == "ok" && b == "ok" && e == "ok";
            integrity_ok &= ok;
            println!(
                "soak: t={}s iter={iter} live={} edges={} integrity meta={m} blob={b} edges={e}",
                start.elapsed().as_secs(),
                live.len(),
                store.edge_count().map_err(soak_err)?,
            );
            heartbeat = Instant::now();
        }
    }

    store.checkpoint().map_err(soak_err)?;
    let (m, b) = store.integrity_check().map_err(soak_err)?;
    let e = store.edges_integrity_check().map_err(soak_err)?;
    let final_ok = integrity_ok && m == "ok" && b == "ok" && e == "ok";
    let _ = std::fs::remove_dir_all(&dir);
    println!(
        "SOAK_DONE iters={iter} seconds={} integrity_ok={final_ok} final=(meta={m},blob={b},edges={e})",
        start.elapsed().as_secs(),
    );
    Ok(())
}

fn soak_err(e: anyhow::Error) -> Error {
    Error::Config(format!("soak op failed: {e}"))
}
