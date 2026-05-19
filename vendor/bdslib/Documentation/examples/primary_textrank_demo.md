# primary_textrank_demo.rs

**File:** `examples/primary_textrank_demo.rs`

Demonstrates `ShardsManager::summary_for_recent` and `ShardsManager::summary_for_query` — extractive TextRank summarisation over primary observability records.

## What it demonstrates

| Method | Description |
|---|---|
| `summary_for_recent(txn_id, lookback, &TextRankConfig)` | Summary built from the bodies of every primary observed in `[now − lookback, now)` |
| `summary_for_query(txn_id, query, &TextRankConfig)` | Summary built from the bodies of primaries matching a vector query |

## Body-extraction rule

Both methods share the same per-record extractor:

| `data` shape | Action |
|---|---|
| `12.5` (bare number) | skipped — numeric measurement |
| `{ "value": 12.5 }` (number under `value`) | skipped — numeric measurement |
| `{ "value": "text…" }` | extracted as the body |
| `{ "raw": "text…" }` (when `value` missing/non-string) | extracted as the body |
| anything else | skipped |

This lets the same record stream feed both telemetry trends (numeric) and text-summary tools (textual), with each picking up only the records meant for it.

## Sections

| # | Topic | Behaviour shown |
|---|---|---|
| 0 | Setup | One-shot `ShardsManager` over a tempdir |
| 1 | Ingestion | 13 records — 4 numeric, 9 text — across `cpu.usage`, `mem.used_pct`, `log.web`, `log.auth`, `log.cron`, `log.misc` |
| 2 | `summary_for_recent` | Default auto-sizing, then capped to 2 sentences; verifies numeric values do not leak into the summary |
| 3 | `summary_for_query` | Vector queries `"nginx upstream timeout"` and `"user logged in"` produce focused summaries |
| 4 | Edge cases | Empty window → `""`; off-topic query → falls back to highest-ranked text records |

## Run

```bash
cargo run --example primary_textrank_demo
```
