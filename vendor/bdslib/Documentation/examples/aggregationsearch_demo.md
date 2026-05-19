# aggregationsearch_demo.rs

**File:** `examples/aggregationsearch_demo.rs`

Demonstrates `ShardsManager::aggregationsearch()` — a single call that fires a time-scoped vector search over the shard store and a semantic search over the embedded document store in parallel via `rayon::join`, merging both results into one JSON object.

The scenario is a Kubernetes platform operations assistant. Cluster telemetry (pod restarts, OOM kills, node pressure events, HPA scaling, network errors) arrives continuously through the shard layer. Runbooks and post-mortems live in the embedded DocumentStorage. A single `aggregationsearch` call on an alert query surfaces both the live incident telemetry and the relevant operational runbook in one round trip.

## What it demonstrates

| Capability | How shown |
|---|---|
| Parallel execution | Both searches start at once; the call returns when both complete |
| Time-scoped observability | Telemetry hits respect the `duration` lookback window; wider windows return more shards |
| Global document store | Document hits are invariant to the window — the document store has no time dimension |
| Semantic relevance on both sides | Each query returns the highest-scoring telemetry records *and* the most relevant runbook chunks |
| Mixed document types | Small whole-doc runbooks and large chunked documents compete in the same result set |
| Result field inventory | Section 6 enumerates every field present in observability and document hits |

## Sections

| Section | Description |
|---|---|
| 1. Construction | Write hjson config; `ShardsManager::with_embedding`; show docstore path and shared embedding model |
| 2. Document corpus | `doc_add` for 3 short runbooks; `doc_add_from_file` for 2 large docs (20 and 15 chunks); total indexed record count |
| 3. Telemetry corpus | `add_batch` for 4 phases × ~30 records (baseline → pressure → incident → recovery); each phase lands in its own 1-hour shard |
| 4. aggregationsearch | 4 queries covering distinct failure modes; `show_aggregation` prints top-N hits from both sides with scores and content previews |
| 5. Duration scoping | Same query at 1h / 2h / 4h / 6h windows; observability hit count grows with window; document hit count stays constant |
| 6. Result structure | Enumerate fields on `observability[0]` and on chunked vs whole-doc `documents[N]` hits |

## Data set

### Telemetry — 131 records across 4 shards

| Phase | Records | Key signals |
|---|---|---|
| baseline | 28 | Normal metrics, etcd latency warn, autoscaler add-node |
| pressure | 32 | Node-01 memory pressure, pod evictions, HPA scale-out |
| incident | 38 | OOM kills on node-02/03, CrashLoopBackOff, circuit breaker open, DNS failure, connection refused |
| recovery | 33 | Spark job cancelled, nodes Ready, circuit breaker closed, DNS/network restored |

### Document store — 5 source documents, 40 indexed records

| Document | Storage | Chunks |
|---|---|---|
| Pod CrashLoopBackOff Runbook | `doc_add` | 1 |
| Node Memory Pressure Response | `doc_add` | 1 |
| Pod Network Connectivity Debugging | `doc_add` | 1 |
| Kubernetes Cluster Major Incident Runbook | `doc_add_from_file` (slice=240, overlap=18%) | 20 |
| Container OOM Post-Mortem | `doc_add_from_file` (slice=230, overlap=20%) | 15 |

## Key observations from the output

**Query A — CrashLoopBackOff:** Observability top hits are the exact `log.error` records for the crashing pods (score ≈ 0.73). Document top hit is the whole-doc CrashLoopBackOff runbook (score ≈ 0.77), followed by the specific chunk of the cluster incident runbook that covers the CrashLoopBackOff epidemic scenario.

**Query B — OOM / memory pressure:** Observability surfaces the kubelet eviction and OOM kill log lines first. Documents return the OOM-specific chunk of the cluster runbook, the Node Memory Pressure whole-doc runbook, and the OOM post-mortem chunk describing the eviction cascade.

**Query C — Network / DNS:** Observability correctly ranks the network-specific log lines (timeout, DNS failure, connection refused, TCP retransmit) above unrelated metric records. Documents return the network debugging runbook and the two NetworkPolicy-related chunks of the cluster runbook.

**Query D — Circuit breaker recovery:** The recovery-phase `log.info` for circuit breaker close scores 0.82 — highest of any hit across all queries — because the record text is nearly identical to the query string. Documents return post-mortem chunks that mention the circuit breaker event.

**Duration scoping (Section 5):** At 1h only the recovery shard is in range (17 hits); at 4h and 6h all four shards are covered (61 hits). Documents return 10 hits at every window — the document store has no time dimension and `DEFAULT_DOC_LIMIT` caps the result at 10.

## Result JSON shape

```json
{
  "observability": [
    {
      "id": "019dcee6-…",
      "timestamp": 1777292700,
      "key": "log.error",
      "data": "pod userdata/user-api-6bfr9 CrashLoopBackOff exit code 137",
      "_score": 0.7322,
      "secondaries": []
    }
  ],
  "documents": [
    {
      "id": "019dcf6d-…",
      "score": 0.766,
      "metadata": {
        "name": "Pod CrashLoopBackOff Runbook",
        "category": "runbook",
        "domain": "kubernetes",
        "severity": "P2"
      },
      "document": "Pod CrashLoopBackOff Runbook\n\nA pod in CrashLoopBackOff…"
    },
    {
      "id": "019dcf6d-…",
      "score": 0.608,
      "metadata": {
        "document_name": "Kubernetes Cluster Major Incident Runbook",
        "document_id": "019dcf6d-…",
        "chunk_index": 12,
        "n_chunks": 20
      },
      "document": "CrashLoopBackOff epidemic scenario: an epidemic — many pods…"
    }
  ]
}
```

## Running

```bash
cargo run --example aggregationsearch_demo
```

Requires network access on first run to download `AllMiniLML6V2` (~23 MB). Subsequent runs use the cached model from `~/.cache/huggingface/hub`.
