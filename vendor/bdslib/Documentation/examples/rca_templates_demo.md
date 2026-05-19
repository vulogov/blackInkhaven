# rca_templates_demo.rs

**File:** `examples/rca_templates_demo.rs`

Demonstrates `RcaTemplatesResult`: root cause analysis on drain3 template observations using G-Forest co-occurrence clustering and causal ranking by lead time.

## What it demonstrates

| Method | Description |
|---|---|
| `RcaTemplatesResult::analyze(manager, duration, config)` | Cluster all template bodies stored in the window |
| `RcaTemplatesResult::analyze_failure(manager, body, duration, config)` | Cluster templates and rank probable causes of a named failure template |

## Dataset structure

The demo injects 15 template events directly via `tpl_add` (not drain3) to produce predictable clustering:

| Cluster | Templates | Pattern |
|---|---|---|
| Auth | `user <*> logged in from <*>`, `session opened for user <*> by service <*>` | Co-occur in 300 s buckets 0, 2, 4 (every 600 s) |
| Disk/crash | `disk <*> usage <*>% warning threshold reached`, `disk <*> write error ENOSPC`, `service <*> crashed with exit code <*>` | Co-occur in 300 s buckets 1, 3, 5 (alternating); sequential causal chain within each bucket |

The two clusters occupy non-overlapping time buckets so their inter-cluster Jaccard similarity is 0. Within the disk/crash cluster, `disk.warn` fires 120 s and `disk.error` fires 60 s before `service.crash` in every bucket.

## RcaTemplatesConfig

| Field | Value used | Description |
|---|---|---|
| `bucket_secs` | 300 | 5-minute co-occurrence window |
| `min_support` | 2 | Each body must appear in ≥ 2 distinct buckets |
| `jaccard_threshold` | 0.5 | Cluster threshold |
| `max_keys` | 100 | Body cap |

## Output sections

1. **Ingest** — 15 template events stored: 6 for the auth cluster, 9 for the disk/crash cluster.
2. **`analyze`** — Two clusters discovered. Auth cluster: cohesion=1.0, support=3 buckets. Disk/crash cluster: cohesion=1.0, support=3 buckets.
3. **`analyze_failure("service <*> crashed with exit code <*>")`** — `probable_causes` ranked by lead time: `disk.warn` (120 s lead), `disk.error` (60 s lead).
4. **Summary** — Top-ranked precursor with lead time, Jaccard, and co-occurrence count.

## Running

```bash
cargo run --example rca_templates_demo
```
