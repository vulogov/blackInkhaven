# bdsweb — User Manual

This is the **user-facing manual** for `bdsweb`, the web UI shipped with
bdslib. It walks through every page in the order you'll typically use
them, explains what each control does in practical terms, and shows the
common task flows (debug an incident, find a recurring pattern, run a
scheduled script, ask the AI assistant a question).

If you're looking for the **operator reference** — startup flags, route
paths, RPC calls behind each page — see [BDSWEB.md](BDSWEB.md). Both
documents describe the same UI; this one is task-oriented, that one is
component-oriented.

---

## Contents

1. [First steps](#1-first-steps)
2. [The navigation bar](#2-the-navigation-bar)
3. [Dashboard — system pulse](#3-dashboard--system-pulse)
4. [Telemetry → Metrics — semantic search over numeric records](#4-telemetry--metrics--semantic-search-over-numeric-records)
5. [Telemetry → Logs — semantic search + topic cloud](#5-telemetry--logs--semantic-search--topic-cloud)
6. [Telemetry → Templates — drain3 template browser](#6-telemetry--templates--drain3-template-browser)
7. [Analysis → Agg. Search — combined search across stores](#7-analysis--agg-search--combined-search-across-stores)
8. [Analysis → Trends — statistics, anomalies, breakouts](#8-analysis--trends--statistics-anomalies-breakouts)
9. [Analysis → Templates Summary — TextRank over templates](#9-analysis--templates-summary--textrank-over-templates)
10. [Analysis → Primary Summary / Query Summary — TextRank over records](#10-analysis--primary-summary--query-summary--textrank-over-records)
11. [Analysis → Primary LSA Summary / LSA Query Summary — LSA equivalents](#11-analysis--primary-lsa-summary--lsa-query-summary--lsa-equivalents)
12. [Analysis → Detect anomalies — n-gram phrase-rarity outliers](#12-analysis--detect-anomalies--n-gram-phrase-rarity-outliers)
13. [Analysis → Denoise primaries — n-gram noise removal](#13-analysis--denoise-primaries--n-gram-noise-removal)
14. [Analysis → k-NN analysis — TF-IDF clustering + isolation](#14-analysis--k-nn-analysis--tf-idf-clustering--isolation)
15. [RCA → Telemetry RCA — co-occurrence + causal ranking](#15-rca--telemetry-rca--co-occurrence--causal-ranking)
16. [RCA → Template RCA — same algorithm on drain3 templates](#16-rca--template-rca--same-algorithm-on-drain3-templates)
17. [Documents — semantic knowledge-base search](#17-documents--semantic-knowledge-base-search)
18. [Scripts — store, run, and schedule BUND scripts](#18-scripts--store-run-and-schedule-bund-scripts)
19. [Signals — emit and search named events](#19-signals--emit-and-search-named-events)
20. [Bund — interactive scripting workbench](#20-bund--interactive-scripting-workbench)
21. [Chat — Ollama-powered RAG assistant](#21-chat--ollama-powered-rag-assistant)
22. [Common interaction patterns](#22-common-interaction-patterns)
23. [Cookbook — typical workflows end to end](#23-cookbook--typical-workflows-end-to-end)
24. [Troubleshooting](#24-troubleshooting)

---

## 1. First steps

### Start the server

`bdsweb` needs a running `bdsnode` to talk to. Start `bdsnode` first,
then launch `bdsweb`:

```bash
bdsnode --config /etc/bdslib/bds.hjson      # in one terminal
bdsweb --node http://127.0.0.1:9000          # in another
```

Open `http://127.0.0.1:8080/` in a browser. You'll land on the
**Dashboard** — the system-pulse page that auto-refreshes every 30
seconds (configurable via `dashboard_refresh_secs` in `bds.hjson`).

If the dashboard shows a "Wait…" spinner that never resolves, bdsweb
can't reach bdsnode. Check `--node` and confirm bdsnode is listening
with `bdscmd status`.

### About the look and feel

`bdsweb` is dark-themed, keyboard-friendly, and entirely server-rendered
— there's no SPA, no JavaScript framework, no build step. Pages use
HTMX to swap fragments in place, so search results appear inline
without a full page reload.

Every page works without JavaScript except for two: the **Bund**
workbench and the **Scripts** editor (both rely on CodeMirror for
syntax highlighting).

---

## 2. The navigation bar

The sticky bar at the top of every page contains nine items, in this
order:

```
Dashboard │ Telemetry ▾ │ Analysis ▾ │ RCA ▾ │ Documents │ Scripts │ Signals │ Bund │ Chat
```

Three items are **dropdown groups** — clicking opens a menu of
sub-pages:

| Group | Sub-pages |
|---|---|
| **Telemetry ▾** | Metrics · Logs · Templates |
| **Analysis ▾**  | Agg. Search · Trends · Templates Summary · Primary Summary · Primary Query Summary · Primary LSA Summary · Primary LSA Query Summary |
| **RCA ▾**       | Telemetry RCA · Template RCA |

Conventions:

- **Active page** is highlighted in blue. When the active page is
  inside a dropdown, the parent button (e.g. *Analysis*) is also
  highlighted blue, so you can see at a glance which group you're in.
- Clicking a dropdown button **toggles** the menu. Clicking outside
  closes it. Pressing `Escape` closes it.
- The bar always shows at the top while you scroll; long pages won't
  hide it.

The footer of every page reports three version strings side by side:

- **bdsweb** — version of the bdsweb binary you're talking to.
- **bdsnode** — version of the bdsnode it's connected to (fetched
  live from `v2/status`).
- **bundcore** — version of the BUND VM crate compiled into bdsweb;
  bdsnode loads the same crate so they normally match.

If bdsweb and bdsnode versions disagree, pin the older one until you
upgrade both — newer-format JSON-RPC responses are mostly
forward-compatible but newer query parameters won't be understood by
an older bdsnode.

---

## 3. Dashboard — system pulse

**URL:** `/`

The first thing you see. Read-only, no inputs. Auto-refreshes on a
configurable interval; click **Reload** to force an immediate refresh.

### What's on the page

**Status row — five cards across the top:**
- **Node ID** — the unique identifier of the connected bdsnode.
  Click-and-drag to copy.
- **Hostname** — OS hostname of the machine running bdsnode.
- **Uptime** — seconds since the bdsnode process started. If this
  resets unexpectedly while the Node ID stays the same, the process
  was restarted.
- **Total Records** — count of every primary record across all shards.
- **Embedding model** — name of the fastembed model bdsnode is using
  for vector indexing (e.g. `AllMiniLML6V2`, `BGESmallENV15`). Set via
  `embedding_model` in `bds.hjson` and locked to the dbpath after
  first vector insert — see [`EMBEDDINGENGINE.md`](EMBEDDINGENGINE.md#dimension-lock-in)
  for the dimension-lock-in note. A dash (`—`) means bdsnode hasn't
  reported one (e.g. older bdsnode build).

**JSON Cache row** — a horizontal bar showing how full bdsnode's
in-memory record cache is. Green ≤ 70%, yellow ≤ 90%, red above. A
persistently red bar means cache eviction is hot; consider raising
`jsoncache_capacity` in `bds.hjson`.

**Timeline & Queues** — two cards:
- **Data Timeline** — Unix timestamps of the oldest and newest
  records in the system.
- **Ingest Queues** — current depths of three queues: log records,
  JSON files, syslog files. Yellow when non-zero (work pending),
  green when empty (system caught up).

**BUND Runtime** — four mini-tiles plus two side-by-side tables.
- **Result queues** — number of distinct ids tracked in
  `v2/results.*`. Each `v2/eval.queued` submission adds one queue.
- **Bund contexts** — number of named v2/eval VMs currently held in
  memory.
- **Recent submissions** — count (max 5) of the last queued scripts.
- **Running** — number of BundWorker threads currently executing a
  script. Turns green when > 0.
- **Recent submissions table** — the last 5 jobs accepted by the
  worker pool, newest first, with submission time and age.
- **Currently running table** — one row per active worker, showing
  worker index and job id. Hover the truncated id to see the full
  UUID.

**Telemetry Records — 5 Most Recent Shards** — a stacked bar chart
(blue = primary, grey = secondary) showing record volume per shard.
Below it, a **Shard Details** table lists shard start time, primary
count, secondary count, and total. Older shards beyond the most
recent five are summarised in the header.

### When to look at the dashboard

- **Ingestion seems stuck.** Check the ingest queues. A persistently
  growing depth means writes outpace storage; consider reducing
  ingestion rate or scaling shards.
- **Memory pressure suspected.** Watch the JSON cache utilisation
  bar; persistent red is a tuning signal.
- **Verifying a deployment.** Note the node ID and uptime; a
  mismatch with what you expect means the wrong bdsnode is running.
- **A scheduled script just fired.** Switch to the Scripts page
  after seeing it briefly appear in the BUND Runtime → Running
  table.

---

## 4. Telemetry → Metrics — semantic search over numeric records

**URL:** `/telemetry`

This is the page for "find me records relevant to this search query
within a time window". It works on any primary records, but is most
useful for **numeric telemetry** (`cpu.usage`, `mem.free`,
`http.latency_ms`, …).

### Controls

- **Search box** — free-text query. Examples: *"cpu spike"*, *"memory
  pressure"*, *"http latency"*. The query is embedded with the same
  vector model bdsnode uses, so paraphrases work.
- **Duration** — lookback window. `15min`, `30min`, `1h`, `2h`, `4h`,
  `6h`, `12h`, `24h`, `7days`.
- **Limit** — how many results to return.
- **Search** button — kicks the query.

### Reading results

Each result shows:
- **Key** — the metric/key name.
- **Timestamp** — when the record was observed.
- **Score** — cosine similarity to your query (0 to 1). Higher is
  more relevant.
- **Data** — the JSON payload, wrapped if long.

Click a column header to **sort** by that column. Long `data` cells
wrap onto multiple lines so JSON payloads stay readable.

### Tips

- Run the same query at successively wider durations (15min → 1h →
  24h → 7days) to see when a pattern first appeared.
- The score column lets you tell a confident match (>0.7) from a
  weak one (<0.3). Weak top results suggest the query doesn't
  actually match what's stored.

---

## 5. Telemetry → Logs — semantic search + topic cloud

**URL:** `/logs`

Same shape as Metrics, but tuned for log lines. The difference is
two extra panels:

- **Topic cloud** — a sidebar listing the top LDA-derived topics for
  every key in the window. Click a topic word to seed a new query
  with it.
- **Secondary records side panel** — clicking any primary log line
  in the results opens a slide-out showing every secondary record
  attached to that primary. This is the "this template fired N
  times in this window" drill-down.

The topic cloud is generated by the LDA topic-modelling algorithm
(see [`Algorithm/LDA.md`](Algorithm/LDA.md)) — it's particularly
useful when you don't yet know what to search for; click a recurring
topic word to find the matching records.

### Tips

- After clicking a topic, refine your query in the search box. The
  topic word seeds the search but you can extend it freely.
- The slide-out panel is keyboard-dismissible: click outside or
  press `Esc`.

---

## 6. Telemetry → Templates — drain3 template browser

**URL:** `/templates`

Browse the drain3-mined log templates. Each row is one template body
discovered by the streaming parser, with its observation count and
first/last seen timestamps.

### Controls

- **Search box** — semantic vector search over template bodies.
  Same shape as the other search boxes.
- **Duration** — filter to templates whose firing observations fall
  in the lookback window.

### When to use it

- "Are there templates I haven't seen before?" — sort by *Created*
  to surface the newest.
- "Which templates fired most this hour?" — sort by *Observations*.
- "Find me the template behind this log line" — paste a fragment
  into the search box; the closest template will rank first.

---

## 7. Analysis → Agg. Search — combined search across stores

**URL:** `/search`

A single query across both telemetry and the document knowledge
base, returned as two side-by-side panels.

### Controls

- **Search box** — your free-text query.
- **Duration** — lookback window for the telemetry side; the
  document side is always searched in full.

### Reading results

Two panels render side by side:

- **Observability** — telemetry records ranked by semantic score,
  newest first.
- **Documents** — knowledge-base documents ranked by score.

Use this when you don't know whether what you need is a *log line*
or a *runbook*. Most operational workflows benefit from both:
"the system is doing X" (telemetry) plus "what to do about X"
(documents).

---

## 8. Analysis → Trends — statistics, anomalies, breakouts

**URL:** `/trends`

A statistical analysis page for a **single numeric key**. Outputs:
mean / median / std-dev / min / max, an anomaly list (S-H-ESD),
breakout points (E-Divisive), and a time-series chart.

### Controls

- **Key** — the metric key to analyse. Example: `cpu.usage`.
- **Duration** — lookback window.
- **Analyze** button.

### Reading the output

- **Statistics** — mean, median, std-dev, min, max, sample count.
  The sample count tells you how confident the stats are.
- **Anomalies** — table of (timestamp, value) pairs flagged by the
  Seasonal Hybrid ESD algorithm.
- **Breakouts** — timestamps where the time series shifts to a new
  level.
- **Chart** — a Chart.js line plot of the raw values, with anomaly
  markers in red and breakout markers as vertical lines.

### When to use it

- Capacity planning ("did this metric drift higher this week?").
- Post-incident review ("when did the spike start?").
- Watching the change after a deploy ("is latency steady now?").

---

## 9. Analysis → Templates Summary — TextRank over templates

**URL:** `/templates_summary`

Run the TextRank summariser over every drain3 template observed in
a window. Output is a **single concatenated summary string**.

### Controls

- **Duration** — lookback window for template observations.
- **Max sentences** — hard cap on how many template bodies the
  summary contains. `auto (~30%)` lets the algorithm pick.
- **Min word len** — drop short tokens before scoring (default 2).

### When to use it

- *"Quick: what was happening in the last hour?"* — a
  three-template summary often captures the operational gist
  faster than reading the dashboard.
- Generating a one-line context for an alert that fires off a chat
  bot or email.

The full algorithm is documented in
[`Algorithm/TEXTRANK.md`](Algorithm/TEXTRANK.md).

---

## 10. Analysis → Primary Summary / Query Summary — TextRank over records

**URLs:** `/primary_summary` and `/primary_query_summary`

Two related pages:

- **Primary Summary** — TextRank over every text-bearing primary
  record in a time window.
- **Primary Query Summary** — same, but the records are pre-filtered
  by a vector search query.

### Controls (both)

- **Duration** (Primary Summary only) / **Query** (Primary Query
  Summary) — what to summarise.
- **Max sentences** — hard cap on summary length.
- **Min word len** — short-token filter.

### Numeric-record exclusion

These pages skip records whose `data` is a bare number or whose
`data["value"]` is a number — those are telemetry measurements and
have nothing to summarise. See the explainer banner under the form
on each page.

---

## 11. Analysis → Primary LSA Summary / LSA Query Summary — LSA equivalents

**URLs:** `/primary_lsa_summary` and `/primary_lsa_query_summary`

Identical UI to §10 but with **Latent Semantic Analysis** as the
ranking algorithm instead of TextRank. Same `Duration` / `Query`
inputs, same numeric-exclusion behaviour, same output shape.

Extra control: **Concepts** — number of latent concepts to extract
(1, 2, 3 default, 5, 8). Higher values capture more subtle themes
when the corpus is heterogeneous.

When to prefer LSA over TextRank: multi-theme corpora where you
want explicit control over how many "axes" of the corpus to
surface. See [`Algorithm/LSA.md`](Algorithm/LSA.md) for the
algorithmic details.

---

## 12. Analysis → Detect anomalies — n-gram phrase-rarity outliers

**URL:** `/anomaly_recent`

Surface log lines whose **phrase structure** is unusual relative to the
rest of the lookback window. bdsnode fetches every primary record in
the chosen window, fingerprints each (the record's `key` plus a
flattened `json_fingerprint(data)`), and runs n-gram anomaly detection
over the resulting strings. The page renders a stat row plus a table
of the most rare-phrase fingerprints.

This is **not** a vocabulary outlier ("what words are rare?"); it's a
phrase outlier ("what *combinations* of words are rare?"). A line built
entirely of common words can still be flagged when the words are
arranged in an unusual sequence — the algorithm's whole point.

### Controls

- **Duration** — lookback window (`15min` … `7days`).
- **N-gram** — `1` (unigram, ≈ rare-word detection), `2` (bigram, the
  default), `3` (trigram, catches trailing-token differences bigrams
  smooth over).
- **Min word len** — drop short tokens before n-gram construction
  (default 2).
- **Threshold** — mean rarity above which a fingerprint is flagged as
  anomalous, range `[0, 1]`. Default 0.7. Lower (~0.5) on small or
  homogeneous corpora; raise to 0.9+ to surface only the most striking
  outliers.
- **Max anomalies** — cap on the number of rows shown. The true total
  is always reported in the stat tile.
- **Detect** button — runs the analysis.

### Reading the output

**Stat tiles:**
- *Records scanned* — total primary records in the window.
- *Unique n-grams* — size of the corpus n-gram vocabulary.
- *Anomalies* — true count above threshold (turns red when > 0).
- *Mean rarity* — informational; the corpus-wide average.

**Anomalies table:** one row per flagged fingerprint, sorted by rarity
descending. Each row shows:
- *#* — display position.
- *Idx* — index back into the per-call fingerprint vector.
- *Rarity* — score in `[0, 1]`.
- *Novel n-grams* — the rarest distinct phrases in this fingerprint
  (chips, sorted by ascending document frequency). This is the
  per-line explanation of *why* the line was flagged.
- *Fingerprint* — the actual fingerprint string scored. Note this is
  the flattened `key + json_fingerprint(data)` form, not the original
  record JSON. To resolve back to the record, take the `key` portion
  and run a `bdscmd primaries-get` for it within the same window.

### When to use it

- *"Show me the weirdest lines from the last hour."* — quick triage.
- *"Has anything new started firing?"* — drift detection. The first
  few firings of a new template surface as anomalies; once the
  template becomes baseline, anomalies for it disappear automatically.
- Pre-LLM context cleanup — remove the obviously-anomalous edge cases
  before passing a corpus to a chat assistant.

For the algorithm internals see
[`Algorithm/NGRAM_ANOMALY.md`](Algorithm/NGRAM_ANOMALY.md). The companion
**Denoise primaries** page (next section) is the dual: same fingerprint
pipeline, scored on the opposite axis to *remove* repetition rather
than flag uniqueness.

---

## 13. Analysis → Denoise primaries — n-gram noise removal

**URL:** `/denoise_recent`

Strip the boring repetitive lines from a recent batch of primary
records, leaving only those that actually carry distinct information.
The page splits the corpus into two tables: **kept** (signal) and
**removed** (noise), classified by mean n-gram **commonness** of each
fingerprint.

This is the dual of *Detect anomalies*: same fingerprinting, same
n-gram pipeline, scored on the opposite axis. A line classified as
noise here will *survive* an anomaly scan; a line flagged as anomalous
there will *survive* this denoise cut.

### Controls

- **Duration** — lookback window.
- **N-gram** — `1`, `2` (default), or `3`.
- **Min word len** — short-token filter (default 2).
- **Threshold** — mean commonness at-or-above which a fingerprint is
  classified as noise. Default 0.85 is **intentionally strict** —
  for typical operational streams (heartbeat-heavy traffic where the
  noise floor is 30–60% of the corpus) a value in the `0.3–0.6` range
  produces visible denoising.
- **Max kept** / **Max removed** — caps on the response arrays. The
  stat tiles always show the true totals.
- **Denoise** button — runs the analysis.

### Reading the output

**Stat tiles:**
- *Records scanned* — total primary records in the window.
- *Unique n-grams* — size of the corpus n-gram vocabulary.
- *Kept (signal)* — true count below threshold (green).
- *Removed (noise)* — true count at-or-above threshold (red).

**Kept table** (signal, upper):
- Rendered in **input order**, so it reads sequentially as the
  denoised corpus.
- *Commonness* values are low (the lower, the more distinctive).

**Removed table** (noise, lower):
- Sorted by **commonness descending** — the most-noise-like first.
- Use this to verify the denoiser isn't aggressively removing
  meaningful traffic; if real signal is appearing here, raise the
  threshold.

### When to use it

- *"Pre-process the last hour before summarising it."* — pipe the kept
  array into TextRank or LSA to summarise the actual signal, not the
  heartbeat floor.
- *"What's actually new in this hour vs. the typical pattern?"* — the
  kept table is exactly the answer.
- *"Reduce LLM context cost."* — denoise before passing a corpus to a
  chat assistant; the same answer at a fraction of the token count.

For the algorithm internals see
[`Algorithm/NGRAM_NOISE.md`](Algorithm/NGRAM_NOISE.md).

### Tip on threshold tuning

If "Removed" is empty on first run, lower the threshold. A useful
calibration loop:

1. Start at `0.85` (default). Observe the commonness range in the
   *kept* table.
2. Pick a threshold just below the highest commonness you'd consider
   noise (e.g. if your repetitive heartbeat sits at 0.6 commonness,
   set threshold 0.5).
3. Re-run. The *Removed* table should now contain the heartbeat-style
   lines; the *Kept* table the genuine signal.

---

## 14. Analysis → k-NN analysis — TF-IDF clustering + isolation

**URL:** `/knn`

The companion view to *Detect anomalies* and *Denoise primaries*. Same
fingerprinting pipeline as those two pages — every primary record in the
window is rendered as `"<key with . _ - → spaces>  <json_fingerprint(data)>"`
— but here the analysis switches from n-gram commonness to **TF-IDF +
cosine similarity** on a bag-of-words view.

Two things come out:

1. **Clusters** — connected components of a k-NN graph (each fingerprint
   keeps its `k` nearest neighbours by cosine similarity). Each cluster
   is summarised by a **representative**: the member with the highest
   mean similarity to its in-cluster neighbours.
2. **Anomalies** — fingerprints whose **maximum** similarity to any
   neighbour is at or below the configured threshold. These are records
   with no close phrase-structural match anywhere in the lookback window
   — phrases that stand on their own.

Where *Detect anomalies* asks "which records use rare phrases?" k-NN
asks "which records have no structural twin?" — the two views often
disagree on edge cases, which is the point.

### Controls

- **Duration** — lookback window.
- **k** — neighbours per node in the k-NN graph (default 5). Bigger `k`
  ⇒ broader, looser clusters.
- **Min word len** — short-token filter (default 2).
- **Anomaly threshold** — max nearest-neighbour cosine similarity at-or-below
  which a fingerprint is flagged as anomalous. Default `0.2`. Lower ⇒
  stricter (fewer, more isolated anomalies); higher ⇒ broader.
- **Members cap** — limit on members listed per cluster (the true total
  is always reported as `size`).
- **Anomalies cap** — limit on the response anomalies array.
- **Analyze** button — runs the analysis.

### Reading the output

**Stat tiles:**
- *Records scanned* — total primary records in the window.
- *k* — neighbours-per-node actually used (capped at `n_logs - 1`).
- *Clusters* — number of connected components found (green when > 0).
- *Anomalies* — number of isolated records (red when > 0).

**Clusters card** — one collapsible `<details>` per cluster, sorted by
size descending. Each header shows a `cluster #N` badge, the size, the
representative's density, and its index. Expanding a cluster reveals:
- the representative fingerprint (full text), and
- a table of members sorted by density descending (most representative
  first), capped at *Members cap*.

**Anomalies table** — sorted by max-similarity ascending so the
**most-isolated** records appear at the top. Each row shows the index,
the maximum similarity to any neighbour (a value at or below the
threshold), and the fingerprint text.

### When to use it

- *"Group the last hour into a handful of patterns and show me one
  example of each."* — read the cluster representatives.
- *"What's structurally one-of-a-kind in this window?"* — read the
  anomalies table.
- *"Cross-check the n-gram anomaly view."* — a record flagged here that
  *Detect anomalies* missed is a strong signal: structurally isolated
  even though its individual phrases are common.
- *"Estimate corpus diversity at a glance."* — a high cluster count
  with small sizes means a heterogeneous window; one giant cluster plus
  a long anomaly tail means heavy boilerplate with sporadic novelty.

### Tip on threshold tuning

The default `0.2` is a reasonable starting point for typical operational
streams. If the *Anomalies* tile stays empty, raise the threshold (try
`0.3` or `0.4`). If it floods with too many records to be useful,
tighten it (`0.15` or `0.1`). You can also raise `k` to make the graph
denser, which suppresses spurious singletons.

For the algorithm internals see
[`Algorithm/KNN.md`](Algorithm/KNN.md).

---

## 15. RCA → Telemetry RCA — co-occurrence + causal ranking

**URL:** `/rca`

The root-cause-analysis page. Either:

- Cluster all events in a window by co-occurrence, **or**
- Name a specific failure key and rank probable precursors.

### Controls

- **Failure key** (optional) — leave blank to just cluster; provide
  a key (e.g. `service.api.down`) to additionally rank causes.
- **Duration** — lookback window.
- **Bucket secs** — co-occurrence window width. Default 300 (5
  minutes). Smaller values demand tighter temporal proximity.
- **Min support** — minimum bucket count before a key enters the
  analysis (prunes one-offs).
- **Jaccard threshold** — cluster-membership cutoff (`[0, 1]`).
  Lower ⇒ larger, looser clusters.

### Reading the output

- **Clusters** table — every co-occurrence cluster with member keys,
  support, cohesion (mean pairwise Jaccard).
- **Probable causes** table — only when a failure key was given.
  Sorted by `avg_lead_secs` descending — positive values are
  candidates that fired *before* the failure (precursors), negative
  values are consequences.

The full algorithm is documented in
[`Algorithm/RCA_JACCARD.md`](Algorithm/RCA_JACCARD.md).

### Tips

- Start without a failure key to see "what was firing together at
  all". Then drill into a specific failure.
- If the clusters look noisy, raise the Jaccard threshold.
- If a single cluster contains everything, lower the threshold or
  shrink `bucket_secs`.

---

## 16. RCA → Template RCA — same algorithm on drain3 templates

**URL:** `/rca/templates`

Identical UI to §12 but the keys being clustered are **drain3
template UUIDs** rather than event keys. Use this when the meaningful
unit of "thing that fires" is a log template (one structural pattern
parameterised by varying values), not a metric key.

The output renders template bodies in the cluster/cause tables so
you can read them at a glance instead of cross-referencing UUIDs.

---

## 17. Documents — semantic knowledge-base search

**URL:** `/docs`

Search the knowledge-base document store. This is where runbooks,
postmortems, design docs, and any other narrative content live.
Documents are added via the JSON-RPC API (`v2/doc.add`,
`v2/doc.add.file`); the UI is read-only.

### Controls

- **Search box** — semantic query. Example: *"circuit breaker
  payment service runbook"*.
- **Limit** — how many documents to return.

### Reading results

Each result is a card showing the document metadata, score, and
content. Long content is truncated with a "Show more" button.

### Tips

- Run the same query on `/docs` and on `/search` (Agg. Search) to
  see the document hits in isolation versus alongside telemetry.
- The score is cosine similarity, same scale as on the other search
  pages.

---

## 18. Scripts — store, run, and schedule BUND scripts

**URL:** `/scripts`

The interactive **scripts manager**. Three-pane layout:

```
┌─────────────────┬──────────────────────────────────┬──────────────────┐
│ Scripts list    │ Editor                           │ Run output       │
│  (left)         │  (centre)                        │  (right)         │
│                 │                                  │                  │
│  hello          │  [name] [schedule]               │  No output yet.  │
│   */5 * * * *   │  ┌────────────────────────────┐  │                  │
│  daily_report   │  │  // BUND source code with  │  │                  │
│   0 9 * * *     │  │  syntax highlighting       │  │                  │
│  cleanup        │  └────────────────────────────┘  │                  │
│   0 0 * * 0     │  [Delete]      [Run]   [Save]    │                  │
└─────────────────┴──────────────────────────────────┴──────────────────┘
```

### Left pane — script list

Every stored script appears with its name and crontab schedule.
Click an entry to load it into the editor. Click **+ New** at the
top to start a fresh script.

### Centre pane — editor

Three controls:

- **Name** field — required. Human-readable label.
- **Schedule** field — required. Crontab-style expression. The
  scheduler runs every minute and fires any script whose next
  occurrence falls inside the current minute.
- **CodeMirror editor** — BUND source with syntax highlighting,
  line numbers, bracket matching. Same colour scheme as the
  Bund page (§20).

Three buttons at the bottom:

- **Delete** (only when editing an existing script) — confirm-then-
  remove. Idempotent in the underlying API.
- **Run** — submits the current script body (whatever is in the
  editor right now, even unsaved changes) to the persistent
  BundWorkerPool via `v2/eval.queued`. Output appears in the right
  pane.
- **Save** — creates a new script (when fields are empty) or
  updates the existing one. The list refreshes automatically.

### Right pane — run output

When you click Run, the right pane:

1. Shows a "Running… polling for results" indicator.
2. The bdsweb server submits to `v2/eval.queued`, then polls
   `v2/results.empty` every 250 ms (up to 30 s) until the queue
   is non-empty.
3. Once results arrive, the server pulls all values from the queue
   and renders them as numbered cards (one per workbench value).

The header line shows the result-queue id (which equals the
script's storage UUID — every run accumulates into the same
queue) and the wall-clock elapsed time.

If the script produces no workbench values, you'll see "Script
completed but produced no workbench values." If it doesn't finish
within the 30-second budget, you'll see a timeout banner with the
queue id so you can pull results later via `bdscmd results-pull`.

### Tips

- Use Run during development to verify your script before saving.
  Saving doesn't run; only Run runs.
- Schedule values like `* * * * *` (every minute) make the script
  fire on every scheduler tick. Test this with a small script that
  pushes the current time, then watch the dashboard's "Recent
  submissions" tile.
- The cron parser is `croner` — full standard expressions supported
  (`*/5 * * * *`, `0 9 * * 1-5`, `@hourly` is **not** supported,
  use `0 * * * *` instead).

---

## 19. Signals — emit and search named events

**URL:** `/signals`

The **signals** page lists named-severity events emitted via
`v2/signal.emit`. Use signals when you want to record discrete
incidents ("payment timeout 503", "deploy started", "alert
firing") with attached metadata, separate from the bulk telemetry
stream.

### Controls

- **Search box** — semantic query over signal names + metadata.
- **Duration** — lookback window.
- **Limit** — how many signals to return.

### Reading results

Each signal card shows:

- **Name** — what the signal is.
- **Severity** — info / warn / error / critical (your convention).
- **Timestamp** — when it was emitted.
- **Metadata** — arbitrary JSON object emitted with the signal.

Severities are colour-coded: green / yellow / orange / red.

### When to use signals vs. telemetry

| Use signals for | Use telemetry for |
|---|---|
| Discrete events with metadata | Continuous numeric measurements |
| Alerts and incident markers | Per-second / per-minute samples |
| "This happened once at 12:34" | "This rate has been fluctuating" |

---

## 20. Bund — interactive scripting workbench

**URL:** `/bund`

The **live scripting workbench**. Type BUND code, click Run, see the
workbench stack output. Fast feedback loop, no save required.

### Controls

- **Context** — name of the named VM to use. Defaults to `default`.
  Different context names give different VMs with isolated state.
- **↺ button** — generates a fresh random context name. Use this to
  start with a clean VM whose stdlib is loaded but with no
  user-defined words.
- **CodeMirror editor** — BUND source with syntax highlighting.
- **Run** button (or `⌘↵` / `Ctrl↵`) — submits the script for
  evaluation in the named context.

### Why named contexts?

Each context accumulates VM state across runs. If you define a word
`:add { + } register` in context `playground`, then in the same
context type `5 3 add .`, the result is 8 — the word stayed
registered between submissions.

Switch contexts to start fresh, or use the ↺ button to mint a random
one. Idle contexts are evicted after the configured TTL (default
300 s).

### Reading the output

The **Workbench output** card below the editor shows everything left
on the workbench (`.`) after the script finishes, pretty-printed
JSON.

If the script throws an error, the error message appears in red.

### Tips

- Use the context name as a "session" mechanism — different
  experiments in different contexts.
- The Bund workbench is for **interactive exploration**. Once a
  script settles, move it to the Scripts page for storage and
  scheduling.

---

## 21. Chat — Ollama-powered RAG assistant

**URL:** `/chat`

A retrieval-augmented chat interface powered by a local Ollama
model. Ask operational questions in natural language; bdsnode
retrieves relevant telemetry and document context, then sends the
question + context to Ollama for an answer.

### Controls

- **Duration** dropdown — telemetry lookback window for retrieval.
- **Question textarea** — multi-line input.
- **Send** button.
- **New session** — starts a fresh conversation. Sessions are
  stateful (Ollama remembers context across messages within a
  session), so use New session whenever you switch topics.

### How it works

1. Your question is sent to bdsnode (`v2/chat.ollama`).
2. bdsnode runs an aggregation search over the lookback window:
   matching telemetry records + matching documents.
3. The combined context is prepended to your question and sent to
   the configured Ollama model.
4. The response streams back into the chat pane.

### Tips

- The quality of answers depends entirely on what's stored. If the
  system has never seen logs about the topic, the assistant will
  say so or hallucinate.
- For deterministic answers, prefer specific questions ("Which
  services had >5% error rate in the last hour?") over open-ended
  ones ("What's wrong?").
- New session resets the conversation; the underlying telemetry /
  document context is re-fetched on the next question.

---

## 22. Common interaction patterns

A few UI conventions repeat across pages:

### Sortable tables

Every table with `sortable` headers can be sorted by clicking a
column. Click again to flip direction. Numeric columns sort
numerically; text columns sort alphabetically.

### HTMX live indicators

When a search is in flight, a small spinner appears next to the
button (e.g. *"Searching…"*, *"Summarising…"*, *"Running… polling
for results"*). The button is disabled during the request.

### Empty states

When a query returns no results, the result panel shows a friendly
empty-state message — never a blank page.

### Skeleton placeholders

Heavy pages (Dashboard, Bund Runtime tile) render a skeleton with
"…" placeholders immediately, then swap in real content via HTMX
once data arrives. Layout doesn't jump.

### Keyboard shortcuts

| Shortcut | Where | Effect |
|---|---|---|
| `⌘↵` / `Ctrl↵` | Bund editor | Run script |
| `Esc` | Logs side panel, RCA modal, dropdown menus | Close |
| Tab navigation | Every form | Standard browser tabbing works |

### Auto-refresh

The Dashboard auto-refreshes every `dashboard_refresh_secs`
seconds (default 30). Other pages do **not** auto-refresh —
re-submit the form to refresh.

---

## 23. Cookbook — typical workflows end to end

### Triage an unfamiliar alert

1. **Dashboard** — confirm the system is alive. Check ingest queues
   and uptime.
2. **Telemetry → Logs** — search for the alert text. Use the topic
   cloud sidebar to spot related themes.
3. **Analysis → Primary Summary** — get a one-paragraph summary of
   what's been happening in the last hour.
4. **RCA → Telemetry RCA** — provide the alert key as the failure
   key. Look at *probable causes* sorted by lead time.
5. **Chat** — ask the assistant: *"What runbook covers <topic>?"*
   to surface related documents.

### Recurring scheduled report

1. **Bund** — write the report logic interactively until it works:
   query telemetry, format the result, push it to the workbench.
2. **Scripts → + New** — paste the working script into the editor.
3. Set the **schedule** (e.g. `0 9 * * *` for 9 AM daily).
4. Click **Save**. The cron scheduler picks it up automatically;
   no restart needed.
5. Watch the Dashboard's "Recent submissions" tile to confirm
   the next firing.

### Find the dominant pattern in a noisy hour

1. **Telemetry → Templates Summary** — duration 1h, max 5
   sentences. The five most representative templates are returned
   as one summary string.
2. **Telemetry → Templates** — search for the surfaced template
   text to drill into individual occurrences.
3. **Analysis → Trends** — if the pattern matches a numeric metric
   key, plot it over the same window.

### Investigate a specific record

1. **Telemetry → Logs** — find the record with the search box.
2. Click the record row. The slide-out panel opens, showing every
   secondary record attached to that primary (i.e., every other
   variation of the same template).
3. Use the timestamp pattern in the slide-out to spot bursts.

### Build a knowledge base for the chat assistant

1. Add documents via `bdscmd doc-add` or `bdscmd doc-add-file`
   (covered in [BDSCMD.md](BDSCMD.md)).
2. **Documents** page — verify the documents are searchable.
3. **Chat** — ask questions; the assistant retrieves from both
   the document store and the telemetry stream.

### Triage with n-gram tooling

A two-page workflow that mirrors the algorithmic dual:

1. **Analysis → Detect anomalies** — duration 1h, default threshold.
   Skim the table for any rarity-flagged fingerprints; click into the
   underlying records via *Telemetry → Logs* if anything looks worth
   investigating.
2. **Analysis → Denoise primaries** — same window, threshold lowered
   to ~0.4 (or whatever fits your noise floor — see the per-page tip).
   The *Kept* table gives you the same hour with the routine traffic
   stripped out, ready to read top-to-bottom.

The two pages answer complementary questions: anomalies tells you
"what stood out?", denoise tells you "what was *not* boring?". On a
healthy hour the anomaly table is empty and the kept-from-denoise
table is short — both are quick negative checks.

---

## 24. Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| Dashboard stuck on "Loading…" | bdsweb cannot reach bdsnode | Check `--node` URL; run `bdscmd status` |
| Search returns nothing | Empty store, or duration too narrow | Increase Duration; verify with `bdscmd count` |
| Topic cloud is empty | LDA had no scorable corpus | Increase Duration; check that the chosen key has stored records |
| Run output never arrives | Script took longer than 30 s | Use `bdscmd results-pull --id <queue-id>` to fetch when ready |
| Scheduler never fires my script | Cron expression invalid | Verify with `croner` semantics; `@hourly` is not supported, use `0 * * * *` |
| Trends chart is empty | Wrong key name or all-numeric records absent | Check key spelling (`bdscmd keys -d 1h`); confirm the records carry numeric `data["value"]` |
| Chat says "no relevant context found" | The retrieval window has no matching records | Widen Duration; ensure documents exist on the Documents page |
| BUND editor lost my script when I clicked another script | Save first | Pending changes are not auto-preserved when switching scripts; click Save before navigating |
| Worker shows "panicked on shutdown" log lines | Normal during graceful shutdown | Ignore unless it persists during steady-state runs |

For protocol-level issues (RPC errors, malformed payloads), check
[BDSWEB.md](BDSWEB.md)'s component reference or
[jsonrpc_api/README.md](jsonrpc_api/README.md) for the RPC method
that backs the failing page.

---

## See also

- [BDSWEB.md](BDSWEB.md) — operator reference: route paths,
  startup flags, the JSON-RPC method behind every page.
- [BDSCLI.md](BDSCLI.md) — local CLI for offline analysis.
- [BDSCMD.md](BDSCMD.md) — JSON-RPC command-line client; covers
  every API the UI uses.
- [DATABASE.md](DATABASE.md) — what's actually stored under the
  hood.
- [Algorithm/](Algorithm/README.md) — deep dives into the
  algorithms behind every search, summary, RCA, and topic page.
- [Bund/README.md](Bund/README.md) — BUND VM language reference;
  read before writing scripts on the Scripts page.
