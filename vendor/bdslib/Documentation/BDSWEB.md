# bdsweb — bdsnode Web Interface

`bdsweb` is a dark-themed web UI for `bdsnode`. It connects to a running
`bdsnode` instance via its JSON-RPC 2.0 API and exposes seven pages covering
system status, semantic search over telemetry and logs, document retrieval,
trend analysis, and an interactive BUND scripting workbench.

---

## Table of Contents

1. [Starting bdsweb](#1-starting-bdsweb)
2. [Global Options](#2-global-options)
3. [Environment Variables](#3-environment-variables)
4. [Navigation](#4-navigation)
5. [Dashboard](#5-dashboard)
6. [Telemetry Search](#6-telemetry-search)
7. [Log Search](#7-log-search)
8. [Document Search](#8-document-search)
9. [Aggregated Search](#9-aggregated-search)
10. [Trends](#10-trends)
11. [Bund Workbench](#11-bund-workbench)
12. [Common UI Patterns](#12-common-ui-patterns)

---

## 1. Starting bdsweb

```bash
bdsweb [OPTIONS]
```

`bdsweb` must be able to reach a running `bdsnode` process. Start `bdsnode`
first, then launch `bdsweb`.

**Minimal start (all defaults):**
```bash
bdsweb
# Binds to http://127.0.0.1:8080, connects to bdsnode at http://127.0.0.1:9000
```

**Custom host/port and remote node:**
```bash
bdsweb --host 0.0.0.0 --port 8888 --node http://prod-node.internal:9000
```

---

## 2. Global Options

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--host <ADDR>` | | `127.0.0.1` | Address to bind the HTTP server |
| `--port <PORT>` | `-p` | `8080` | TCP port to listen on |
| `--node <URL>` | `-n` | `http://127.0.0.1:9000` | bdsnode JSON-RPC endpoint |
| `--verbose <LEVEL>` | | `1` | Log verbosity: 0 = warn, 1 = info, 2 = debug |

---

## 3. Environment Variables

| Variable | Equivalent flag | Example |
|----------|-----------------|---------|
| `BDSNODE_URL` | `--node` | `http://prod-node.internal:9000` |

When `BDSNODE_URL` is set, `--node` is not required.

---

## 4. Navigation

The sticky navigation bar at the top of every page contains seven items:

| Label | Path | Purpose |
|-------|------|---------|
| Dashboard | `/` | System health and shard overview |
| Telemetry | `/telemetry` | Semantic search over telemetry records |
| Logs | `/logs` | Semantic search over log entries + LDA topics |
| Documents | `/docs` | Knowledge-base document retrieval |
| Agg. Search | `/search` | Combined telemetry + document search |
| Trends | `/trends` | Statistical analysis and time-series charts |
| Bund | `/bund` | Interactive BUND scripting workbench |

The active page is highlighted in blue.

---

## 5. Dashboard

**Path:** `GET /`

A read-only health page. No inputs required.

### Displayed Information

**Status row (4 cards):**
- Node ID — the unique identifier of the connected bdsnode instance
- Hostname — OS hostname of the node
- Uptime — time since the node process started
- Total Records — aggregate count across all shards

**Timeline & Queues (2-column):**
- Data Timeline — timestamps of the oldest and newest stored events
- Ingest Queues — current depths of the log, JSON-file, and syslog-file
  ingestion queues; shown in yellow when non-zero, green when empty

**Shard chart:**
- Bar chart (Chart.js) showing telemetry record count per shard
- Shard start timestamp on the X-axis

**Shard table:**
- One row per shard: start timestamp, primary record count, secondary
  record count

### JSON-RPC calls made

| Method | Purpose |
|--------|---------|
| `v2/status` | Node ID, hostname, uptime, queue depths |
| `v2/count` | Total record count |
| `v2/timeline` | Oldest / newest event timestamps |
| `v2/shards` | Per-shard record counts |

---

## 6. Telemetry Search

**Path:** `GET /telemetry`

Semantic (vector) search over stored telemetry records.

### Query Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `q` | string | `""` | Natural-language search query |
| `duration` | string | `1h` | Look-back window (see Duration Values) |

### Duration Values

`15min`, `30min`, `1h`, `2h`, `4h`, `6h`, `12h`, `24h`, `7days`

### Interactions

- **Typing in the query field** triggers a search automatically after a
  450 ms debounce.
- **Changing the duration** reloads the key cloud immediately.
- **Submit button** runs the search explicitly.

### Displayed Information

**Key cloud** — clickable tag buttons for every known key in the selected
duration. Clicking a key sets it as the search query.

**Results table** — one row per matching record:
- Timestamp
- Key (metric or event name)
- Data (truncated to 120 characters)
- Score (cosine similarity, 3 decimal places)

### JSON-RPC calls made

| Method | Trigger |
|--------|---------|
| `v2/keys.all` | Page load; duration change |
| `v2/search.get` | Query submit / debounced input |

---

## 7. Log Search

**Path:** `GET /logs`

Semantic search over stored log entries, with an LDA topic sidebar and a
floating results panel.

### Query Parameters

Same as Telemetry: `q` and `duration`.

### Interactions

- **Typing** in the query field (450 ms debounce) opens the results panel.
- **Changing duration** reloads both the key cloud and the topic cloud.
- **Esc key** or the **✕ button** closes the results panel.
- **Clicking a topic keyword** sets it as the search query.

### Displayed Information

**Left sidebar — Known Keys:** clickable tag cloud of log source keys for
the selected duration.

**Right sidebar — Topic Keywords:** LDA-derived topics, each with several
representative keywords. Keywords are clickable and link to a search query.

**Floating results panel** (slides in from the right, 660 px wide):
- Result count and current query displayed in the panel header
- Table: Timestamp, Key, Message, Score
- Long JSON messages wrap within the panel (word-break enforced)
- Panel persists until explicitly closed

### JSON-RPC calls made

| Method | Trigger |
|--------|---------|
| `v2/keys.all` | Page load; duration change |
| `v2/topics.all` | Page load; duration change |
| `v2/search.get` | Query submit / debounced input |

---

## 8. Document Search

**Path:** `GET /docs`

Semantic search over stored documents (runbooks, tickets, post-mortems,
knowledge-base articles).

### Query Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `q` | string | `""` | Natural-language search query |
| `limit` | integer | `10` | Maximum results: 5, 10, 20, 50 |

### Interactions

- **Typing** in the query field (500 ms debounce) runs the search.
- **Changing the limit** re-runs the current query immediately.

### Displayed Information

One **card per document**:
- **Name** — from `metadata.name` or `metadata.document_name`
- **Category badge** — colour-coded: runbook (blue), ticket (yellow),
  postmortem (red), kb (purple), change (green)
- **Document ID** (UUID)
- **Preview** — first 280 characters of content
- **Score** — similarity score in green
- **Expandable metadata** — full metadata JSON in a `<details>` block

### JSON-RPC calls made

| Method | Trigger |
|--------|---------|
| `v2/doc.search` | Query submit / debounced input |

---

## 9. Aggregated Search

**Path:** `GET /search`

Runs a single query simultaneously against both telemetry records and
documents, displaying results side by side.

### Query Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `q` | string | `""` | Natural-language search query |
| `duration` | string | `1h` | Look-back window for telemetry hits |

### Displayed Information

Two-column layout (stacks on narrow viewports):

**Observability (left):** matching telemetry records — Timestamp, Key,
Data, Score — with a hit-count badge.

**Documents (right):** matching documents — Name, Category, Score,
Preview — with a hit-count badge.

### JSON-RPC calls made

| Method | Trigger |
|--------|---------|
| `v2/aggregationsearch` | Query submit |

---

## 10. Trends

**Path:** `GET /trends`

Statistical analysis and time-series visualisation for a single metric key.

### Query Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `key` | string | `""` | Exact metric key (e.g. `cpu.usage`, `http.latency_ms`) |
| `duration` | string | `1h` | Look-back window |

### Interactions

- Enter a metric key and click **Analyse** (or press Enter).
- Changing the duration re-runs the analysis.

### Displayed Information

**Statistics grid (4 cards):** sample count (n), mean, standard deviation,
variability coefficient.

**Statistics row (3 cards):** minimum, median, maximum.

**Alert badges** (when applicable): anomaly count, breakout count.

**Time-series chart** (uPlot):
- X-axis: wall-clock time
- Y-axis: metric value (auto-scaled)
- Blue line with light-blue fill
- Data points shown as dots when ≤ 200 samples

### JSON-RPC calls made

| Method | Purpose |
|--------|---------|
| `v2/trends` | Statistical summary and anomaly detection |
| `v2/primaries.get.telemetry` | Raw time-series data points for the chart |

---

## 11. Bund Workbench

**Path:** `GET /bund`

An interactive workbench for the BUND stack-based scripting language.
Scripts are evaluated against a named VM context that persists state between
runs.

### Using the Editor

1. Type a BUND script in the CodeMirror editor.
2. Optionally change the **Context** name (default: `default`). The same
   context name reuses accumulated VM state across runs; enter a new name or
   click **↺** to start with a fresh context.
3. Press **Run** (or **⌘↵** / **Ctrl+↵**) to evaluate.
4. The result is the last value pushed to the workbench (`vm.stack.workbench`)
   by the script.

### Syntax Highlighting

The editor provides full BUND language colouring:

| Colour | Token type | Examples |
|--------|-----------|---------|
| Blue, bold | Keywords | `if`, `while`, `for`, `map`, `register`, `alias` |
| Red | Builtins | `dup`, `drop`, `push`, `type`, `string.upper`, `float.sqrt` |
| Green | Double-quoted strings | `"hello world"` |
| Emerald | Single-quoted literals | `'symbol'` |
| Pink | Atoms | `:ok`, `:error` |
| Violet | Pointers | `` `myword `` |
| Cyan | Named stacks | `@context` |
| Orange | Numbers | `42`, `3.14`, `-1e5` |
| Amber | Brackets | `{`, `}`, `[`, `]` |
| Teal | Operators | `+`, `-`, `*`, `/`, `>=`, `==` |
| Grey | Comments | `// comment` |

### Output

| Condition | Display |
|-----------|---------|
| Script pushed a value with `.` | Pretty-printed JSON of the last workbench value |
| Script ran without pushing to workbench | "Script ran — workbench is empty." |
| RPC or evaluation error | Red error box with the error message |

### Context Management

| Action | Effect |
|--------|--------|
| Same context name across runs | VM state (defined words, stack) accumulates |
| Different context name | Fresh VM with only stdlib loaded |
| Click **↺** | Generates a random context name (`ctx-XXXXXXXX`) |

Contexts are evicted server-side after a configurable idle timeout
(default 300 s, set via `bund_ttl_secs` in the bdsnode config).

### Example Scripts

```bund
// Arithmetic — push result to workbench
2 2 + .

// String operation
"hello" string.upper .

// List processing
[ 1 2 3 4 5 ] dup len swap
```

### JSON-RPC calls made

| Method | Trigger |
|--------|---------|
| `v2/eval` | Run button / ⌘↵ |

---

## 12. Common UI Patterns

### Debounced Search

Telemetry, Logs, and Document search fields fire automatically after the
user stops typing (450–500 ms delay), avoiding excessive requests during
fast input.

### HTMX Partial Updates

All search results and dynamic sections use HTMX to replace only the
relevant DOM fragment. The rest of the page is not reloaded. An inline
spinner (e.g. "Searching…") appears during in-flight requests.

### Duration Selector

All search pages share the same look-back window options:
`15min` · `30min` · `1h` · `2h` · `4h` · `6h` · `12h` · `24h` · `7days`

The selected value is preserved in the URL query string so pages can be
bookmarked or shared.

### Error Display

RPC errors are rendered as a red bordered box with the error message inline
(no full page reload). Hard failures (network unreachable, template panic)
produce a full-page error response with a link back to the dashboard.

### Frontend Libraries

| Library | Version | Use |
|---------|---------|-----|
| Tailwind CSS | CDN | Layout and styling (dark theme) |
| HTMX | 2.0.2 | Partial page updates |
| Chart.js | 4.4.4 | Dashboard shard bar chart |
| uPlot | 1.6.31 | Trends time-series chart |
| CodeMirror | 5.65.16 | Bund editor with syntax highlighting |
