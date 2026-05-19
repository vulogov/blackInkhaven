# shardsmanager_tpl_frequency_demo.rs

**File:** `examples/shardsmanager_tpl_frequency_demo.rs`

Demonstrates drain3 log template discovery and the template FrequencyTracking query API on a `ShardsManager` with `drain_enabled = true`.

## What it demonstrates

| Method | Description |
|---|---|
| `ShardsManager::add_batch` | Bulk ingest of structured log entries; drain3 mines and stores templates automatically |
| `ShardsManager::templates_recent(duration)` | List all template documents whose FrequencyTracking timestamp falls within a lookback window |
| `ShardsManager::template_by_id(id)` | Fetch a single template document by its UUID v7 |
| `ShardsManager::templates_by_timestamp(start, end)` | List template documents within an explicit Unix-second range |

## Dataset structure

84 log entries across 7 structural template families, 12 variants each:

| Family | Key | Example log line |
|---|---|---|
| Auth login | `auth` | `"user alice logged in from 10.0.0.1"` |
| Network connection | `network` | `"connection to db-primary on port 5432 established"` |
| Service restart | `ops` | `"service api-gateway restarted after 3 seconds"` |
| HTTP request | `http` | `"HTTP GET /api/health returned 200 in 4 ms"` |
| Worker job | `worker` | `"worker 1 picked up job 1001 from queue ingest"` |
| Disk usage | `disk` | `"disk read usage 72% on volume /dev/sda1"` |
| Backup job | `backup` | `"backup job daily-001 for dataset postgres-main completed in 42s"` |

All 84 entries are placed in the past (base timestamp 95 minutes ago, spanning 84 minutes) so they all fall within the `templates_recent("2h")` window.

## Sections

1. **Setup** — Temporary hjson config with `drain_enabled: true`, `shard_duration: "1h"`, `drain_load_duration: "24h"`.
2. **Ingest** — 84 log entries ingested; UUID and timestamp range printed.
3. **`templates_recent("2h")`** — All freshly discovered templates listed with their UUID and drain3 body string.
4. **`template_by_id`** — First discovered template fetched by UUID; a random UUID verified to return `None`.
5. **`templates_by_timestamp`** — Ingestion window split at the midpoint; templates in the first and second halves compared. Full-range query also shown.
6. **FrequencyTracking summary** — Each template UUID paired with its drain3 body for reference.

## Running

```bash
cargo run --example shardsmanager_tpl_frequency_demo
```

Note: this example requires an EmbeddingEngine (fastembed). First run downloads the model weights and may take a minute.
