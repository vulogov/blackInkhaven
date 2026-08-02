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
- **GM-P3 — the edge inbox (editor).** ✅ SHIPPED (`36a2564d`). `Modal::GraphEdgeInbox` +
  `Modal::GraphHub`; a `Ctrl+B z` graph hub (`n` neighbourhood / `i` inbox) — fixing a shipped
  P6 chord bug (the `Ctrl+V g` neighbourhood chord was shadowed by `ViewOpenProgress`). Inbox
  renders `store.pending_edges()` with `P`/`d` triage (reuse `promote_edge`/`dismiss_edge`).
  +1 regression test.
- **GM-P4 — the Graph scope (templated).** ✅ SHIPPED (`499f8e72`). `AiMode::Graph`
  (`inference.rs`), sticky like Facts. `graph_rag` module + `graph_rag_impl`: retrieve seed
  passages (reuse Book-RAG retrieval verbatim) → fold in each seed's one-hop graph relations →
  `compose_graph_context`; `graph-rag-system` prompt (×5 langs). Reuses `cited_ids` +
  `validate_citations` via `pending_book_rag_cited`; transparency pane extended to show
  passages + relations. Category `"graph_rag"`. +4 tests.
- **GM-P5 — the graph-tool loop.** ✅ SHIPPED (`804f9f66`). `graph_rag::ask` (pure, tested):
  `Action` + `parse_action` (balanced-brace JSON extraction) + `GraphOracle` trait + n#↔UUID
  handle registry + the bounded `ask` loop (malformed/unknown-handle fed back, forced final
  synthesis at the step cap). `inkhaven graph ask <question>` wires a `StoreOracle` over the
  real store + `collect_blocking`; ×5-lang system prompt. +6 tests.
- **GM-P6 — cost + config + multilingual.** ✅ SHIPPED. Multilingual prompts land in P4/P5;
  TUI Graph scope records under `"graph_rag"` (CLI `graph ask` matches the untracked
  `research --agentic` slow-track precedent — `usage::install` is TUI-only). Config: a `graph`
  section (`ask_max_steps`, `ask_search_width`) bounds the traversal cost — informs/caps, never
  blocks (permissive principle). +1 test.
- **GM-P7 — capstone.** ✅ SHIPPED. `GRAPH.md` "Chat with your graph" section (Graph scope +
  `graph ask` + config), the `ask` verb in the command table, corrected "Not yet wired";
  `KEYBINDING.md` F9 scope cycle refreshed (full cycle incl. Graph + sticky-scope note).

## Cross-cutting

AI-advisory (proposals Judged, chat streams to the pane, no prose edits); graph-is-source
(validator rejects fabricated citations); permissive cost sub-budgets; multilingual prompts;
warning-free / 1.2.15. Part A (P1–P3) fills the graph; Part B (P4–P5) interrogates it —
sequence A first.

**Status: the full arc GM-P0→P7 is SHIPPED on 2.0.1-dev (2026-08-01).** Part A fills the
graph (agentic contradictions · fact-link proposals · the editor edge inbox); Part B
interrogates it (the Graph AI scope · the `graph ask` traversal loop). Full suite green,
warning-free.

---

# GM-P8 — the streaming in-editor graph walk ✅ SHIPPED (2.0.1-dev)

**Built as specced below.** `graph_rag::ask::AskSession` (the resumable core, extracted from
the blocking `ask()` with no behaviour change — CLI + its tests unchanged, +2 session tests);
`graph_rag::oracle::StoreOracle` (the P5 oracle body, now shared by CLI + TUI); the frame driver
in `src/tui/app/graph_walk_impl.rs` (`GraphWalk` state + `start_graph_walk` / `advance_graph_walk`
/ `cancel_graph_walk`, hooked at `pump_inference`'s finalize point); the live render
(`draw_graph_walk` — step transcript + streamed answer, never raw JSON); hub `Ctrl+B z → w` to
start, `Esc` to abort. Exploration turns use the JSON tool contract; the terminal turn streams
the P4 prose-grounding contract and commits as one `(question → answer)` chat turn. Warning-free.

---

# GM-P8 — the streaming in-editor graph walk (spec)

**Goal.** Bring GM-P5's multi-turn graph *traversal* into the GM-P4 editor Graph scope —
streamed and non-blocking — so the author can watch the model walk their graph live, instead
of getting P4's single one-hop retrieval or leaving the editor for the `graph ask` CLI.

**The one hard constraint that dictates the design.** The TUI render loop must never block, but
`graph_rag::ask::ask()` is a *blocking* loop (`collect_blocking` → wait → query → repeat). We do
**not** move it to a background thread: that thread would need the graph, and a second live
`Store` handle onto the same DuckDB files is exactly the concurrency hazard the stability bar
(1.2.15) guards against. Instead we keep **all graph access on the UI thread** and turn the
blocking loop into a **frame-driven resumable state machine** layered over the *existing*
single-shot streaming machinery (`spawn_chat_stream` → `Inference` → `pump_inference`). Local
DuckDB queries are single-digit-ms, so running them inline between streamed turns is safe.

**Turn protocol (settles the "streaming raw JSON is ugly" problem).** Two turn kinds:
- **Exploration turns** keep P5's JSON-action protocol (`{"search":…}` / `{"neighbors":…}` / …).
  Their tokens are *buffered* in `Inference.response` as usual but **never rendered raw** — on
  completion we `parse_action`, run the query on the UI thread, and render a compact **step
  line** (`🔍 search "the harbour" → n1 003·Quiet hour, n2 …`) into a live walk transcript.
- **The terminal turn** is a single **prose synthesis**, streamed live token-by-token (the P4
  experience): once the model emits `{"answer":…}` — or the step cap is hit (the existing
  *forced synthesis* path) — we issue one final grounded-answer inference over all observations,
  citation-validated. So the *exploration* streams as progress; the *answer* streams as prose.

## Sub-phases

- **P8a — resumable core (pure, no behaviour change).** Extract the loop *state* out of the
  blocking `ask()` into an `AskSession` in `graph_rag::ask`: it owns `Handles`, `observations`,
  `steps`, `step`/`max_steps`, `search_width`. Methods: `AskSession::new(question, max_steps,
  width)`; `next_prompt(&self) -> String` (the current `build_prompt`); `on_reply(&mut self,
  reply: &str, oracle: &dyn GraphOracle) -> AskStep` where `AskStep = Continue | Answer(String)
  | Exhausted` (the last two both route to synthesis). The existing blocking `ask()` becomes a
  thin driver over `AskSession` (`while let Continue = session.on_reply(llm(session.next_prompt())?, oracle) {}`),
  so **the CLI + its 6 tests keep passing unchanged**. New unit tests drive `AskSession` directly
  (a scripted reply sequence + the `FakeGraph` oracle already in the test module).
- **P8b — `App` as a `GraphOracle`.** The oracle is `store` + `hierarchy`, both already on `App`.
  Factor the CLI `StoreOracle` body (search=`search_text`; neighbors=`subgraph`+`render_neighbourhood`;
  contradicting=`store.contradicting`; loci=`edges_out(CitesLocus)`; paths=`store.paths`) into a
  shared `graph_rag::oracle::StoreOracleParts(&Store, &Hierarchy)` used by BOTH the CLI and a
  thin `App` impl — no logic duplication. (App can't `impl GraphOracle for App` cleanly because
  `on_reply` needs `&mut session` + `&self` oracle at once; see the borrow note below.)
- **P8c — the frame driver.** New `App` state `graph_walk: Option<GraphWalk>` (the `AskSession` +
  turn bookkeeping + the live transcript `Vec<String>`). Start from the **graph hub** — add
  `w` to `Ctrl+B z` (`graph_hub_handle_key`) → `start_graph_walk()`, which takes the current AI
  prompt as the question, builds the session, and kicks turn 1. The driver hooks the single
  finalize point — `pump_inference`'s `just_finished` block (app.rs:4380) — *before* the normal
  chat-history commit: if `graph_walk.is_some()` and this inference is the walk's, **intercept**.
  `Option::take` the walk (so `session` is owned locally and the `&self` oracle borrow is
  independent — resolves the split-borrow), call `on_reply`; on `Continue` append the step line
  and kick the next exploration turn (do **not** commit an intermediate assistant turn); on
  `Answer/Exhausted` fire the streamed synthesis turn, then let the *normal* finalize commit it
  as the single assistant turn paired with the original question.
- **P8d — live rendering.** Render `graph_walk.transcript` as a progress block above the
  streaming answer (reuse the `book_rag_transparency_lines` prepend seam in `draw_chat_history`).
  Status bar shows `graph walk · turn k/N`.
- **P8e — cancel, cost, docs, tests.** `Esc` in the AI pane while `graph_walk.is_some()` aborts
  the whole session (`graph_walk = None`, `inference = None`, status "walk cancelled") — not just
  one turn. Each turn records usage `"graph_rag"`; the walk is bounded by `cfg.graph.ask_max_steps`
  and the status surfaces the turn count (cost *informs*). Flip GRAPH.md's "Not yet wired" note;
  KEYBINDING gets the hub `w`. Tests: a headless driver test (fake oracle + scripted replies →
  assert the transcript + single committed turn + citation-validated answer) and an `Esc`-mid-walk
  cancel test.

## Decisions & risks

- **Opt-in, not the Graph-scope default.** A walk is *N* billable calls; P4's one-hop stays the
  light default. The walk is an explicit action (hub `Ctrl+B z → w`), so cost is chosen — the
  permissive principle (cost informs, the author opts into depth).
- **Split-borrow** (`&mut session` + `&self` oracle) is handled by `take`-ing the walk out of
  `self` for the duration of `on_reply`; the oracle reads `store`/`hierarchy` immutably meanwhile.
- **No background thread / no second Store handle** — the whole point of the frame-driven design;
  keeps the 1.2.15 concurrency guarantees intact.
- **Orthogonal to P4's retrieval cache** (`graph_rag_last_retrieval`): the walk doesn't use it;
  a sticky Graph-scope chat and a walk can coexist without stepping on each other.
- **Reuse ledger:** `parse_action`, `Handles`, `build_prompt`, `system_prompt`, the `GraphOracle`
  trait, `validate_citations`, the `Inference`/`pump_inference` stream loop, the graph-hub modal —
  all already exist. Net-new is the `AskSession` wrapper (P8a), the shared oracle (P8b), the frame
  driver + transcript (P8c/d). Estimate: the smallest of the Part-B phases by new surface, but the
  only one touching the TUI inference *lifecycle*, so it earns its own phase + test pass.
