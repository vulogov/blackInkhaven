# GRAPHMIND-1 — Implementation plan

> Companion to `GRAPHMIND-1_PLAN.md` (the RFC). Grounds every phase against the code
> (file:line). Drafted 2026-08-01 on `2.0.1-dev`. Phases fold into 2.x releases.

## Reality check — the machinery already exists

An anchor audit (2026-08-01) confirms the RFC is buildable almost entirely by wiring:

1. **The AI scope + chat spine is a clean seam.** Scopes are `AiMode` (`src/tui/inference.rs:76`,
   `label()` :94, `next()` :107); F9 cycles via `cycle_ai_mode` (`app.rs:22635`); per-query
   context is `build_ai_mode_context` (`app.rs:22678`, `Book`/`Facts` return `Ok(None)`); the
   single dispatch is `start_inference` (`src/tui/app/ai_impl.rs:413`) — scope-branch at :482,
   system-prompt envelope at :574, usage category at :599, `spawn_chat_stream`
   (`src/ai/stream.rs:53`) at :604. A `Graph` scope adds one enum variant + one branch here.
2. **BOOK_RAG's citation contract is reusable verbatim.** `retrieve` (`book_rag/retrieval.rs:21`),
   `compose_context_prefix` (`book_rag/mod.rs:48`), `cited_ids` (`mod.rs:73`), and — crucially —
   `validate_citations(response, valid: &HashSet<String>)` (`mod.rs:85`) is **generic over a
   token set** — graph-chat reuses it as-is. Transparency: `book_rag_transparency_lines`
   (`render/panes.rs:2313`), validator runs at finalize (`app.rs:4458`).
3. **The Facts-search modal is the graph-chat UX template.** `open_facts_search`
   (`app.rs:11551`) → `search_facts` (`app.rs:11572`, `store.search_text` + Facts-subtree
   filter) → `facts_search_send` (`app.rs:11655`, seeds a grounded chat).
4. **The fact-add hook has everything in scope.** `confirm_insertion` (`research/app.rs:4959`);
   after `Provenance::record` (~:5087) `new_id`/`body`/`title`/`store`/`hierarchy` are all live,
   and `find_near_duplicate` (:5117) already calls `book_rag::retrieval::retrieve` against the
   Facts book — the exact related-fact search a proposal pass reuses.
5. **`clash_edge` is built + tested + has its Store seam waiting.** `contradiction::clash_edge`
   (`contradiction.rs:444`, `#[allow(dead_code)]`) maps a `Clash` (real node ids) → a `Judged`
   `Contradicts` edge. The agentic gate `detect_contradictions` (`agentic.rs:211`) already holds
   `store: &Store` + the `SourcedFact` node ids where `parse_clashes` runs (:251) — the exact
   Store-access point P3 said to wait for.
6. **The edge inbox needs no new query.** `EdgeStore::by_origin` (`edge_store.rs`, added GM-P0);
   Output confront messages already carry `edge_id` (stamped `app.rs:7300`, read by
   `confront_edge_id` :7314) and the `P`/`d` keys (`handle_output_key`, promote :8774 / dismiss
   :8761) triage for free.
7. **Cost categories are free-form `&'static str`.** `ai::usage::record` (`usage.rs:53`) — no
   enum; pass `"graph_chat"` / `"graph_link"`. `CostConfig` (`config.rs:3411`) for informative
   sub-budget caps (the WORLD-4 `slow_preflight` / `DAILY_CALL_CAP` pattern).

### Pinned decisions

- Graph scope mirrors **Book** (deferred grounding, retrieved in the send path), not Facts
  (seeded prologue): `build_ai_mode_context` returns `Ok(None)`; a `graph_context(query)`
  composes the evidence block at `ai_impl.rs:482`.
- Graph-chat **citation tokens are node breadcrumbs** (path-shaped), so `validate_citations`'
  `contains('/')` heuristic (`mod.rs:108`) applies unchanged.
- Proposals are `origin=Judged` (advisory); nothing auto-promotes; the inbox is the workqueue.

## Phase map

- **GM-P0 — plumbing.** ✅ SHIPPED (`2e0bb0d6`). `EdgeStore::by_origin` + `Store::pending_edges`
  + `inkhaven graph pending`. +1 test.
- **GM-P1 — agentic graph-aware.** Wire `clash_edge` into `detect_contradictions`
  (`agentic.rs:251`): `store.add_edge(&clash_edge(&c))` best-effort in the `parse_clashes` loop;
  drop clash_edge's `#[allow(dead_code)]`. Deep research now persists `Contradicts` edges —
  the SEMNET-P3 follow-up. Test: the wiring is exercised; `clash_edge` mapping already tested.
- **GM-P2 — fact-add edge proposals.** New `research::graph_link` module: pure
  `propose_stance_edges(new_id, neighbors, relations) -> Vec<Edge>` (reuse `confront_stance_edges`
  shape, `origin=Judged`) + a gather that reuses `find_near_duplicate`'s `retrieve` call + the
  `/relate` grader (`relate_system`/`relate_user`/`parse_relations`). Hook after
  `Provenance::record` (`research/app.rs:~5087`), gated by a `research.link_facts` frequency knob.
  Post proposals to the Output pane (`confront` kind + `edge_id`). Tests: pure proposal derivation.
- **GM-P3 — the edge inbox (editor).** `Modal::GraphEdgeInbox` cloning `Modal::GraphNeighbourhood`
  (SEMNET-P6): render `store.pending_edges()` grouped; a `Ctrl+V` chord to open; `P`/`d` triage
  (reuse the confront handlers via `edge_id`, or a dedicated inbox key handler). Tests: render.
- **GM-P4 — the Graph scope (templated).** `AiMode::Graph` (`inference.rs`); `graph_context(query)`
  = resolve entity (`search_text`) → intent-classify → bounded traversal → `compose_context_prefix`
  -style evidence + `cited_ids`; `graph-system` prompt (citation contract, ×5 langs); reuse
  `validate_citations` at finalize; a `graph_transparency_lines` clone. Category `"graph_chat"`.
- **GM-P5 — the graph-tool loop.** Replace/augment P4's intent-classifier with an LLM tool-plan
  over the bounded read-only graph tools (§RFC 5.2); ≤K calls; same envelope + validator.
- **GM-P6 — cost + config + multilingual + CLI/Bund.** `graph_chat`/`graph_link` usage +
  `cost.graph_link_daily_call_cap`; per-language prompts; `research --link` CLI.
- **GM-P7 — capstone.** `GRAPHMIND.md` + companion chapter + GRAPH.md/KEYBINDING updates.

## Cross-cutting

AI-advisory (proposals Judged, chat streams to the pane, no prose edits); graph-is-source
(validator rejects fabricated citations); permissive cost sub-budgets; multilingual prompts;
warning-free / 1.2.15. Part A (P1–P3) fills the graph; Part B (P4–P5) interrogates it —
sequence A first.
