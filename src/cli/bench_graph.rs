//! 2.0 perf harness — `inkhaven _bench-graph` (hidden). Builds an isolated edge
//! store in a temp dir, inserts N edges fanning out from a small hub set, then
//! times neighbour queries against the hubs. Self-contained (no project /
//! fixture, no network), so it slots into the gated bench set like the others.
//! Drives the criterion `graph` bench.

use std::time::Instant;

use uuid::Uuid;

use crate::error::{Error, Result};
use crate::storage::edge_store::EdgeStore;
use crate::store::graph::{Edge, EdgeKind, EdgeOrigin, EndpointRef};

const HUBS: usize = 16;
const QUERY_ITERS: usize = 1000;

pub fn run(edges: usize) -> Result<()> {
    let dir = std::env::temp_dir().join(format!("inkhaven-bench-graph-{}", std::process::id()));
    std::fs::create_dir_all(&dir)
        .map_err(|e| Error::Config(format!("bench-graph tmp dir: {e}")))?;
    let path = dir.join("edges.db");
    let store = EdgeStore::new(&path, 2).map_err(|e| Error::Config(format!("bench-graph store: {e}")))?;

    let hubs: Vec<Uuid> = (0..HUBS).map(|_| Uuid::now_v7()).collect();
    let mut batch = Vec::with_capacity(edges);
    for i in 0..edges {
        let src = EndpointRef::Node(hubs[i % hubs.len()]);
        let dst = EndpointRef::Node(Uuid::now_v7());
        batch.push(Edge::new(src, EdgeKind::LinksTo, dst, EdgeOrigin::Structural));
    }

    let t_ins = Instant::now();
    store
        .insert_batch(&batch)
        .map_err(|e| Error::Config(format!("bench-graph insert: {e}")))?;
    let insert = t_ins.elapsed();

    // Neighbour queries against the hubs — exercises the reverse index.
    let t_q = Instant::now();
    let mut acc = 0usize;
    for i in 0..QUERY_ITERS {
        let ep = EndpointRef::Node(hubs[i % hubs.len()]);
        acc += store
            .outgoing(&ep, &[])
            .map_err(|e| Error::Config(format!("bench-graph query: {e}")))?
            .len();
    }
    let query = t_q.elapsed();
    std::hint::black_box(acc);

    // Best-effort cleanup of the throwaway store.
    let _ = std::fs::remove_dir_all(&dir);

    println!("edge_count: {edges}");
    println!("edge_insert_total_us: {}", insert.as_micros());
    println!("edge_query_iters: {QUERY_ITERS}");
    println!("edge_query_total_us: {}", query.as_micros());
    println!(
        "edge_query_avg_us: {}",
        query.as_micros() / (QUERY_ITERS.max(1) as u128)
    );
    Ok(())
}
