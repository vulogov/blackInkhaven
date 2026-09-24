# CANON-UI-1 — Bringing the smysl corpus into the writing surface

*Proposal (not scheduled). A UI review + integration options for the Canon Ledger
(the `smysl`-backed decision corpus). For a 3.14+ track if chosen.*

## Where the corpus surfaces today

The Canon Ledger is a rich, live artifact — decisions, grounds edges, commitment
levels, development history — but almost all of it lives **off to the side** of the
writing surface:

- **Reader hub → Canon** (`Ctrl+B *`): a dashboard (list + the new `t` grounds-DAG
  view) with `Enter` jump, `g` ground, `h` history. A separate modal you open.
- **CLI** (`inkhaven canon …`) and **Bund** (`ink.canon.*`): outside the editor.
- **Editorial Pass**: commitment forks surface as Briefs (only after a merge).
- **Pre-cut guard**: the one place the ledger reaches into an editing action —
  deleting a decision-source paragraph warns with its blast radius.

The gap: **while you are actually writing a scene, the editor has no idea the
paragraph in front of you is load-bearing.** The decisions and the prose that
established them are only connected in a modal you have to go open. The ledger
already stores the link (`source` = the paragraph node; `units_for_node` is the
reverse lookup), so the connection is there to surface — it just isn't, at the
point of writing.

## The integrations, ranked

### A — Canon as *ambient context* at the point of writing (the core idea)

**A1. A decision-source glyph in the Tree / Outline.** Reuse the existing per-node
glyph mechanism (`structural_glyph` / `compute_tree_badges`) to mark paragraphs
that established a canon decision (e.g. `◈`). Load-bearing prose becomes visible at
a glance; a chapter with many canonical decisions reads as "settled." Cheap,
high-visibility, no new surface. Cursor on a marked paragraph → the status line
already-free summary: *"sources 2 decisions · 3 rest on them."*

**A2. A Canon right-pane** (`RightPane::Canon`, cycled by `Ctrl+B Tab` beside
Output / AI / Thoughts). It tracks the **open paragraph** and shows, live as you
move: the decisions this paragraph sources, each with its `«commitment»`, and — one
key — its `impact` (what rests on it) and `why` (what it rests on). So while
drafting the sea-gate scene you *see* "this establishes: [world-fact] the city
floats on a leviathan «canonical» — 2 decisions rest on it." Acts in place: commit
a level, jump to a ground. This is the piece that closes the ledger↔prose gap.
*(Trade-off / alternative: rather than a 4th pane mode, the same paragraph-aware
summary could render into the existing Thoughts pane on demand — lighter, avoids
pane proliferation. The durable "no resizable panes" feedback is about sizing, not
a new cycled mode, but a new mode is still worth a deliberate yes.)*

### B — Ground "chat with your book" on the canon corpus *(mostly already built)*

The Book-scope AI ("chat with your book", `book_rag`) retrieves relevant **prose**.
But `CanonLedger::canon_context_for_query` already exists (CL-P6): it fits the
relevant **decisions + their grounds/rebuttals** to a token budget, closure-
complete, and renders them for a prompt. It was built and **never wired into the
AI pane.** Wiring it — as a toggle on Book scope, or a new `F9` **Canon** scope —
means the model answers grounded in the story's *decisions*, not just its
sentences: *"is the escape consistent?"* sees the leviathan/sea-gate decisions and
what they rest on. High value, and the retrieval + packer are done; the work is
surfacing it in `book_rag_impl` + a scope/toggle.

### C — Impact-on-edit (extend the pre-cut guard)

The delete guard warns before cutting a decision-source paragraph. A lighter,
advisory version on **edit**: opening a canonical-decision-source paragraph for
editing sets a one-line status ("this establishes a canonical world-fact 3
decisions rest on — a change may ripple"). Never blocks; just situational
awareness. Small, reuses `units_for_nodes` + `impact`.

### D — Commitment at a glance in the Outline

Aggregate commitment per chapter in the outline (e.g. "3 canonical · 1 floated"),
so you can see where the canon is firm vs. still tentative across the book. A
reporting nicety on top of the data already there.

## Recommendation

**A2 (Canon paragraph-pane) + B (AI grounded on canon)** are the two that turn the
ledger from a report you visit into a living part of the writing surface: one shows
what the current scene commits and what leans on it; the other lets you interrogate
the book through its own decisions. **A1 (glyph)** is a cheap, high-signal
complement worth bundling. C and D are good follow-ons, not headline.

If a single starter is wanted: **B**, because its substrate (`canon_context_for_query`)
is already built and merely unsurfaced — the highest value for the least new code —
followed by **A1 + A2**.

## The ~1-user gate

Would the author use it? A2/A1 make canon impossible to forget while drafting the
scenes it's about — the exact moment the "what breaks if I cut this?" question
arises. B makes the flagship "chat with your book" aware of the canon the author
has been curating. All deterministic-core except B's LLM call, which reuses the
existing cost-capped AI path. Accretion is moderate (A2 is a new pane mode; A1/C/D
are small; B is wiring + a scope). Nothing here needs a new store or dependency —
it's all surfacing the corpus that 3.11–3.13 already built.

## What this is not

Not a rewrite of the dashboard (it stays the "whole ledger" view). Not auto-editing
prose from canon (the advisory rule holds — canon informs, never rewrites). Not a
new persistence axis.
