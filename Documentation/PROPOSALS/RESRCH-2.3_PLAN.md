# RESRCH-2.3 — Web search & fetch (RESRCH-2 / R2-C)

| | |
|---|---|
| **Track** | RESRCH-2 (Grounded Research) — R2-C |
| **Status** | Shipped 1.5.2 (WC-P1..WC-P4) |
| **Target** | 1.5.2 |
| **Builds on** | RESRCH-2.2 (document import: research-source retrieval + provenance) |
| **New runtime crates** | **1 direct** — `reqwest` (already transitive via `genai`); SearXNG HTML→text uses a crate-free tag-strip |
| **Decisions** | Provider **configurable (Tavily + SearXNG)**; default pipeline **LLM chat + factcheck-before-commit** (ingest-as-is via `--ingest`) |

Bring **live web sources** into the assistant, with the author's chosen safety posture: either ingest
the fetched pages directly as cited research sources, or route them through the LLM chat and **fact-check
each derived fact before it commits**. Network + a search provider are introduced deliberately, behind
the existing source-retrieval + provenance plumbing from R2-B.

## Grounding (verified)

- `reqwest 0.12` is already in the lockfile (transitive via `genai`) — a direct dep adds little.
- The async-task → `mpsc` → poll-drain pattern (`ai::stream::spawn_chat_stream`) is the model for the
  network call.
- `factcheck::truth_system` / `truth_user` already produce a single-statement accuracy verdict — reused
  for the pre-commit gate (via `ai::stream::collect_blocking`, one short call).
- Web pages become **research sources** through the exact R2-B path (`add_document` tagged
  `kind:research_source`, retrieved by `search_text` + cited), and `document`/`web` provenance reuses
  RESRCH-2.1.

## The optional pipeline (the core ask)

`/web <query>` runs the search + fetch, then branches by mode (default `chat`; `--ingest` / config
override):

- **`chat` (default — LLM + factcheck):** the fetched page text grounds an LLM chat answer (cited by
  URL). A `/fact` taken from that web-grounded answer is **fact-checked before it commits** — a
  single-fact truth check runs in the confirmation overlay; `ACCURATE` inserts, `DUBIOUS` / `INACCURATE`
  shows the verdict and requires a second confirm (like the dedup two-step). Provenance `origin=web`
  with the source URL(s).
- **`--ingest` (as-is):** the fetched pages are chunked + embedded as research sources (`origin=web`,
  cited by URL), exactly like `/import` but from the web — then available to RAG / `/diff` / `/fact`.

## Phases

| Phase | Content |
|---|---|
| WC-P1 | `research/web.rs`: `WebResult{title,url,text}`; `search(provider, key, endpoint, n, fetch, query)` dispatching **Tavily** (POST `/search`, content inline) and **SearXNG** (GET `?format=json` + per-URL fetch → crate-free `html_to_text` strip). `reqwest` dep (rustls). `research.web` config block (`enabled`, `provider`, `api_key`, `endpoint`, `max_results`, `fetch`, `pipeline`). Degrades cleanly when unconfigured / offline. |
| WC-P2 | `/web [--ingest] <query>` command: spawn the search (tokio task → `mpsc`, drained in the poll loop, `WebState`). On results — **chat:** build a web context block, spawn the grounded LLM answer (turn marked web-grounded, `sources` = URLs); **ingest:** embed chunks as `research_source` (origin web) + the imports sidecar. |
| WC-P3 | Factcheck-before-commit gate: in `confirm_insertion`, when the fact is web-grounded, run a single-fact truth check (`collect_blocking`); store the verdict on `ConfirmationState`; `ACCURATE` → insert, else show the verdict and require a second confirm. The verdict is noted in provenance `detail`. |
| WC-P4 | Provenance `origin=web` (+ URL) on web `/fact`s; `/sources` shows it; `Ctrl+B h` + hints + tutorial 103 + CONFIGURATION + KEYBINDING; tests (provider request shaping, html_to_text, verdict parsing, command parsing). |

## Notes & limits
- **API keys / network.** Tavily needs a key; SearXNG needs an instance URL. Unconfigured → `/web`
  reports it's unavailable (never crashes). Web fetches count toward the session cost display.
- **`html_to_text`** is a crude strip (drop script/style, strip tags, decode basic entities, collapse
  whitespace) — good enough for research extraction without an HTML-parser crate.
- Self-fact-checking is still the model grading itself, but on **web-sourced** claims it's a meaningful
  gate (the claim came from outside the model); the verdict + the source URL together let the author
  judge.

## Out of scope (later)
- Per-source credibility scoring; multi-query/agentic web research; result caching across sessions
  beyond the thread; image/PDF web results.
