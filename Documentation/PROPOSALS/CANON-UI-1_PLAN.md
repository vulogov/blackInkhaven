# CANON-UI-1 — "Canon at Hand" (3.14.0)

*Status: PLAN. The build of the A2 + B + A1 options from
[`CANON-UI-1_PROPOSAL.md`](CANON-UI-1_PROPOSAL.md). On `3.14.0-dev`.*

## Why

The Canon Ledger (3.11–3.13) is rich but lives *off to the side* of writing — a
dashboard you open, a CLI, a Bund surface. This flagship brings it **into the
writing surface**, so the decisions a scene commits, and the answers grounded in
them, are present while you draft — not a modal away. All of it surfaces the corpus
3.11–3.13 already built; no new store, no new dependency, deterministic-core except
B's LLM call (which reuses the existing cost-capped AI path).

## Phases (value-ordered; A1 cheapest first, B mostly-built, A2 the centerpiece)

- **CU1-P1 — A1: the decision-source glyph + cursor summary.** Mark paragraphs that
  established a canon decision with a glyph in the Tree (and Outline), via the
  existing per-node glyph mechanism (`structural_glyph` / `compute_tree_badges`,
  throttled cache). Compute the decision-source node set once (from the ledger's
  live decisions' `source` nodes), not per-node. When the cursor sits on a marked
  paragraph, a free status line: *"canon: sources N decision(s) · M rest on them."*
  Cheap, high-visibility, no new pane. Advisory/read-only.

- **CU1-P2 — B: ground "chat with your book" on the canon corpus.** Wire the
  already-built `CanonLedger::canon_context_for_query` (CL-P6 — fits relevant
  decisions + grounds/rebuttals to a token budget, closure-complete) into the AI
  pane's Book-scope assembly (`tui/app/book_rag_impl.rs`). Surface it as a toggle
  on Book scope (or, if cleaner, a dedicated `F9` **Canon** scope): the model
  answers grounded in the story's *decisions*, not just its prose. Reuses the
  existing retrieval + packer + cost-capped `collect_blocking`; the work is the
  wiring + the toggle + a transparency line ("grounded on N canon decisions").
  Ships early because the substrate exists.

- **CU1-P3 — A2: the paragraph-aware Canon right-pane (the centerpiece).** A new
  `RightPane::Canon` cycled by `Ctrl+B Tab` beside Output / AI / Thoughts. It
  tracks the **open paragraph** and shows, live as the cursor moves between
  paragraphs: the decisions this paragraph *sources* (kind · `«commitment»`), each
  with its `impact` count (what rests on it) and a line of its `why` (what it rests
  on). Read-first; a small key set to act in place (jump to a ground, open the full
  dashboard). Built on `units_for_node` + `impact`/`why`, throttled like the other
  panes' caches. (Design note: a new *cycled* pane mode is distinct from the
  rejected *resizable* panes; it still warrants a deliberate check that a 4th mode
  earns its place vs. rendering the summary into Thoughts on demand.)

- **CU1-P4 — docs + cut.** CANON.md ("In the editor" gains the glyph / pane / AI
  grounding), KEYBINDING (the pane cycle + any keys), RELEASE_NOTES/3.14.0.md,
  Manual ch19 + Know-Your-Book ch8 touch. Then cut 3.14.0 (README regen per rule).

**Value core = CU1-P1 + CU1-P2** (canon becomes visible while writing, and the AI
is grounded in it); CU1-P3 is the deepest but largest piece.

## The ~1-user gate

Would the author use it? The glyph + pane make canon impossible to forget while
drafting the very scenes it's about — the moment "what breaks if I cut this?"
arises; the AI grounding makes the flagship "chat with your book" aware of the
canon the author curates. Accretion: A1/status small; B is wiring over built
substrate; A2 is one new pane mode (the real cost, deliberately last). No new
store/dep.

## Constraints kept

- **Advisory.** Canon informs the editor; it never rewrites prose. The pane and
  glyph are read surfaces; B's grounding feeds the prompt, and prose changes stay
  on the confirmed-diff path.
- **No resizable panes** (durable feedback) — A2 is a cycled *mode*, not a split.
- **Deterministic + free** for A1/A2; B reuses the opt-in, cost-capped AI path.
- **Multilingual** — the canon-context prompt already writes in the project
  language (CL-P6/P7).

## Non-goals

Auto-editing from canon; a new persistence axis; replacing the dashboard (it stays
the whole-ledger view). Impact-on-edit (proposal C) and commitment-in-outline
(proposal D) stay follow-ons, not in this flagship.
