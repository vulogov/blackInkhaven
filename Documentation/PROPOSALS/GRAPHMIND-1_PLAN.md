# GRAPHMIND-1 — the graph reasons, and builds itself (RFC)

> Status: **draft RFC**, 2026-08-01. Target: **2.x** (the "AI partner" line, post-2.0 SEMNET).
> Builds entirely on the 2.0 knowledge graph (`Documentation/GRAPH.md`,
> `PROPOSALS/SEMNET-1_PLAN.md`). Companion impl plan: `GRAPHMIND-1_IMPL.md`.

## 1. Thesis

2.0 (SEMNET) built the knowledge graph as a **data layer**: nodes, typed edges, a CLI,
a read-only editor peek. But the graph is *inert* between manual `graph rebuild`s, and the
AI still chats over your *prose*, not your *structure*.

GRAPHMIND turns the graph into an **AI writing partner**, in two halves that feed each
other:

- **The graph fills itself** — when you add a fact, the assistant proposes typed edges to
  the facts it relates to (contradicts / agrees / qualifies), and deep research lands as a
  *connected subgraph*. An **edge inbox** lets you triage the proposals (the promote/dismiss
  you already have, batched).
- **You interrogate it** — a new **Graph AI scope** where a natural-language question
  becomes bounded graph *traversals* whose results ground the answer. The LLM narrates
  structure it can't fabricate — it can only report edges the graph actually holds.

The two halves are a loop: the more you research, the richer the graph; the richer the
graph, the more the assistant can tell you. And crucially — **this is almost entirely
wiring.** The `/relate` stance grader, `confront_stance_edges`, `clash_edge` (built in
SEMNET-P3, deliberately left unwired), the bounded graph query API, BOOK_RAG's
citation-contract + retrieval-transparency, and the Output-pane promote/dismiss are all
already here. GRAPHMIND points them inward at your own corpus.

## 2. Motivation

- **The graph is under-populated.** Structural edges come from `graph rebuild`; the rich
  *judged* relations only appear when you manually confront a paragraph. Nothing links your
  facts to each other as you write them. The graph is worth chatting with only once it's
  full — so **fill it at the source** (fact-add), not by ceremony.
- **`clash_edge` is built but unwired.** SEMNET-P3 shipped the fact↔fact contradiction
  mapping (`research::contradiction::clash_edge`) but noted "its persist seam is a follow-up
  — the `/contradict`/agentic flow flattens clashes to string tuples." The agentic loop's
  contradiction gate is exactly that seam. This RFC wires it.
- **AI chat is text-grounded, not structure-grounded.** BOOK_RAG (1.4) grounds chat in
  retrieved *passages* via vector similarity. It can't answer "why don't these two facts
  agree?" or "trace the citation chain" — those are *graph* questions. The graph now holds
  the structure; the assistant should be able to walk it.

## 3. Principles & non-goals

Binding constraints (inherited):

- **Reuse over reinvent.** Every phase is mostly plumbing existing components; net-new
  machinery is minimal (a Judged-edge query, a scope, a modal). If a phase needs a new
  substrate, it's mis-scoped.
- **AI is advisory, never authorial.** No prose is edited without per-change confirmation.
  Edge proposals are `Judged` (advisory) until *you* promote them; graph-chat answers stream
  into the AI pane; generation stays user-initiated. (The standing rule.)
- **The graph is the source; the LLM narrates.** Graph-chat answers must cite real
  nodes/edges from the retrieved subgraph — a post-hoc validator rejects any citation not in
  the retrieval set (reuse BOOK_RAG's contract). No fabricated relationships.
- **Read-only, bounded traversal.** The graph-chat tool surface is exactly the bounded,
  read-only query API SEMNET already ships (hop-capped `paths`, edge-capped `subgraph`).
  The LLM plans a small number of calls; it can't mutate or run unbounded walks.
- **Permissive.** Every new AI pass gets a cost sub-budget that *informs, never blocks*
  (the `cost:` / `ai::usage` pattern). Proposals are opt-in-frequency (see §6).
- **Multilingual.** Entity resolution, the relate grader, and narration key off project
  language (the `feedback_multilingual` rule).
- **1.2.15 bar / warning-free.** Standard.

**Non-goals:** not auto-editing prose from graph inferences; not a general NL→query engine
(the traversal surface is the handful of graph questions the product needs); not replacing
BOOK_RAG (Graph scope is a *sibling* to Book/Facts scope, for structural questions); not a
multi-writer graph.

## 4. Part A — the graph fills itself

### 4.1 Fact-add edge proposals

When a fact is inserted (the research `/fact` flow / the Facts system), *after* it commits:

1. **Find related facts** — semantic-search the Facts book for the top-N most-similar
   existing facts (reuse the Facts-search retrieval).
2. **Grade the relation** — run the existing `/relate` stance grader
   (`contradiction::relate_system` / `relate_user` / `parse_relations`) on the new fact vs.
   each candidate — the confront machinery pointed at your own corpus.
3. **Mint edges** — via the existing `confront_stance_edges` (Contradicts / InTension /
   Qualifies / Agrees, `origin=Judged`), plus a `SourcedFrom` from the recorded provenance.
4. **Surface** — post the proposals to the Output pane (the `confront` message kind already
   carries `edge_id` + `P`/`d`), and collect them in the edge inbox (§4.2).

Frequency is a config knob (every fact / on-demand `/link` / off) so it never floods.

### 4.2 The edge inbox

A single triage surface for every `origin=Judged` edge (from confront, fact-add, research),
grouped by the fact/paragraph it touches:

- **CLI** — `inkhaven graph pending` lists Judged edges; `graph promote`/`dismiss` (exist).
- **Editor** — an edge-inbox modal (clone the read-only `Modal::GraphNeighbourhood` I built
  in SEMNET-P6): `↑↓` navigate, `P` promote, `d` dismiss, batch-triage the advisory layer.

This is the concrete answer to SEMNET-RFC §12's "judged-edge noise" risk: instead of
`Judged` edges piling up invisibly, they're a workqueue. Needs one new query —
`EdgeStore::by_origin(Judged)` (a one-liner beside `by_kind`).

### 4.3 Agentic research, graph-aware

`research --agentic` (deep research → Facts book) already runs a contradiction gate
(`parse_clashes` in `agentic.rs`) over the facts it emits. Wire those clashes to
**`clash_edge`** so the deep-research output lands as a *connected subgraph* of `Contradicts`
edges, not a flat list — completing the SEMNET-P3 follow-up.

## 5. Part B — chat with your graph

### 5.1 The Graph scope

A new AI scope (sibling to Book / Facts) — or a `Ctrl+B G` graph-chat modal modeled on the
Facts-search modal — where a question becomes traversal. Two paths, shipped in order:

- **Templated (fast-path)** — classify the question into a small intent set
  (`what-contradicts`, `who-appears-with`, `what-sources`, `trace-citation`,
  `what's-unresolved`, `related-to`), resolve the entity to a node via semantic search, run
  the matching bounded query. Deterministic, cheap, no wasted tokens.
- **Graph-tool loop (the "wow")** — give the LLM a handful of read-only, already-bounded
  graph *tools* and let it plan ≤K calls to answer. inkhaven already runs agentic loops; the
  graph API is *made* of safe bounded primitives.

### 5.2 The traversal grammar (the tools)

Each is an existing or one-line-over-existing bounded, read-only primitive:

| Tool | Backed by |
|---|---|
| `find(text) → [node]` | `Store::search_text` / Facts-search retrieval |
| `neighbors(node, kinds?)` | `Store::neighbors` |
| `contradicting(node)` | `Store::contradicting` |
| `sourced_from` / `graded_as` / `loci`(node) | `edges_out(node, kind)` |
| `paths(a, b, kinds, max)` | `Store::paths` (hop-bounded) |
| `involves(char)` / `co_appear(a,b)` | `EventInvolves` + `grounding_signals` |
| `senses(node)` / `translate(sense)` | the lexical bridge |
| `unresolved()` / `stats()` | `edges_by_kind` + contradiction / dead-source counts |

### 5.3 Grounding envelope (reuse BOOK_RAG)

The retrieved subgraph (facts + their edges, as structured context) fills the system-prompt
envelope. Reuse BOOK_RAG's two proven contracts verbatim:

- **Transparency** — a "what I looked at" panel showing the retrieved subgraph (like
  BOOK_RAG's "Retrieved passages").
- **Citation contract + validator** — the answer must cite node ids/breadcrumbs; a post-hoc
  validator flags any citation not in the retrieved set. No fabricated edges.

Cost rides a `graph_chat` (and `graph_link` for proposals) `ai::usage` category with a
permissive sub-budget.

## 6. Phase map

Sequence Part A first (it makes the graph worth chatting with). Each phase folds into a 2.x
release, cut with tests.

- **GM-P0 — plumbing.** `EdgeStore::by_origin` + `Store` wrapper; `inkhaven graph pending`
  (list Judged edges). Cheap foundation for the inbox. Tests: by-origin query.
- **GM-P1 — agentic graph-aware.** Wire `clash_edge` into `agentic.rs`'s contradiction gate
  → deep research persists `Contradicts` edges. Completes the SEMNET-P3 follow-up. Small,
  high-reuse. Tests: clash → edge (already have `clash_edge` test; add the wiring path).
- **GM-P2 — fact-add edge proposals.** On `/fact` insert: find related facts → relate-grade
  → `confront_stance_edges` (Judged) + `SourcedFrom` → Output-pane surfacing. Config
  frequency knob. Pure proposal-derivation testable; the LLM/IO path is the wrapper.
- **GM-P3 — the edge inbox.** In-editor `Modal::GraphEdgeInbox` (clone `GraphNeighbourhood`):
  list Judged edges grouped, `P`/`d` triage. Reuses the P/d handler + the modal pattern.
- **GM-P4 — the Graph scope (templated).** New AI scope; question → intent → bounded
  traversal → grounded answer with the BOOK_RAG transparency + citation contract.
- **GM-P5 — the graph-tool loop.** LLM plans bounded graph-tool calls (the flagship "wow");
  builds on P4's envelope + validator.
- **GM-P6 — cost + config + multilingual + CLI/Bund.** `graph_chat`/`graph_link` usage
  categories + `cost:` sub-budgets; per-language prompts; any CLI/Bund surface.
- **GM-P7 — capstone.** Companion-book chapter + `GRAPHMIND.md` + `GRAPH.md`/KEYBINDING
  updates.

## 7. Risks & open questions

- **Proposal noise.** Every fact proposing edges could flood the inbox. Mitigation: a
  frequency config (every-fact / on-demand `/link` / off), a similarity threshold, and the
  inbox as a batched workqueue rather than per-fact interruptions.
- **Templated vs. tool-loop for chat.** Templated is cheap + deterministic but rigid;
  tool-loop is powerful but costs tokens + needs guardrails. Ship templated (P4) first, add
  the loop (P5) once the envelope + validator are proven.
- **Entity resolution.** "the war ended in spring" → which fact node? Semantic search + a
  disambiguation step (ask when ambiguous). Reuse Facts-search ranking.
- **Cost.** Fact-add grading is an LLM call per fact-add (× N candidates). The frequency
  knob + a small N + batching keep it permissive. Graph-chat is one call per question (the
  tool-loop, a few).
- **Freshness.** Graph-chat over structural edges needs a recent `graph rebuild`; proposals
  (Judged) are written live so they're always fresh. A "graph may be stale — rebuild?" nudge
  when structural edges are old.

## 8. What it unlocks

The assistant stops chatting over your prose and starts reasoning over your *world*: it
answers "what contradicts this?", "trace this citation", "what's unresolved?" from real
edges; your facts wire themselves together as you write; deep research arrives connected;
and the advisory graph becomes a workqueue you actually clear. The graph 2.0 built as a data
layer becomes a partner you can talk to — and one that helps build itself.
