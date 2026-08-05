# READERS-1 — The Reader Reckoning (a 2.6.0 flagship RFC)

*Real readers meet the book's instruments. READERS imports human beta-reader
feedback, anchors each note to its paragraph, and — the point — **reconciles** it
against every AI reader inkhaven has, turning scattered reception into a ranked,
corroborated, actionable worklist.*

*(Codename open, as REDLINE's was: READERS / WITNESS / RECKONING. "READERS" below.)*

---

## 1. The problem

Everything inkhaven calls a "reader" is a **simulation**. The Inner family (Editor,
Socrates, Theologian, Stylist, Poet), LECTOR, SENTINEL — all of them *predict* how a
reader will respond. None of them is a reader. And inkhaven has **no way to hold the
real thing**: a writer's beta-reader notes live in email, margins, and Google Docs,
disconnected from the paragraph objects, never cross-checked against the AI verdicts,
never actionable through the revision machinery. The one signal that could tell the
writer whether any of the simulation was *right* — actual reception — is the one
signal the tool cannot see.

## 2. The value — the case, made honestly

The skeptic's version: *"I have beta readers. I read their notes. It's a 20-minute
job. Why involve a tool?"* Four things inkhaven does that hand-reading cannot — and
each one is possible **only because inkhaven already has the whole reader family to
check against.** That is the payoff of the entire 2.x arc.

**a. Convergence you can't hold in your head.**
Five readers × ~40 notes = 200 sticky notes. The ones that matter are where readers
*converge* — three of them stumble at the same paragraph. A human cannot cheaply
cross-tabulate 200 free-text notes by location. inkhaven anchors each note to its
paragraph and counts how many *distinct readers* hit it. Drowning-in-feedback → a
ranked worklist, most-agreed-upon first.

**b. Corroboration against the instruments.**
For each converged note, inkhaven already holds *every AI reader's* verdict — that is
exactly what `collect` (the revision worklist) is. So it can say: *"3 readers stumbled
at ch. 5 — and SENTINEL flagged a continuity break there, and LECTOR measured an
attention dip."* No manual process produces that, because no human holds all the AI
findings against all the human notes at once. **This is the native core** — the one
feature that puts CHORUS, SENTINEL, LECTOR, and the Inner family *all to work at once*
against reality.

**c. Anchoring to the object, not the margin.**
"Around where she enters the tower" → anchored to the actual paragraph (via the
existing semantic retrieval), landed as a comment *on that paragraph*, jumpable,
surviving edits, sitting on the writer's real editing surface instead of a
disconnected doc.

**d. One worklist, one machinery.**
A confirmed reader finding becomes an `EditorialFinding` in the `Ctrl+V Shift+R`
queue — so "reader confusion, ch. 5" gets the same Decision / Brief treatment as an
AI finding, through the same confirmed-diff contract. No context-switch between "AI
findings I fix in inkhaven" and "human notes I track in a doc."

**The compounding payoff — calibration.**
Reconciliation, run across drafts, tells the writer *which AI readers actually predict
real reception*. If `put_down_risk` keeps landing where readers really quit, trust it.
If the Inner Editor flags things no human ever notices, weight it down. READERS is
where the book's **simulated** readers finally meet its **real** ones — and the writer
learns which simulations were right. It is the honest climax of an arc that taught a
book to read, watch, and measure itself: now it checks its self-image against the
world.

**The honest limitation — stated, not buried.**
Unlike CHRONICLE, READERS is **not self-contained**: its value is gated on the writer
*having* beta readers and their notes being in an ingestible form. A reader-less
writer gets nothing from it. And anchoring free text to paragraphs is fuzzy — it will
sometimes miss, so every auto-anchor is a *proposal the writer confirms or moves*,
never a silent commitment. We design for graceful degradation, not a rigid format.

*This is why READERS earns its keep despite the dependency: the import is light (reader
notes are just comments — see §4), and the part that isn't light — the reconciliation
— is the most native intelligence inkhaven could build, because only inkhaven has the
whole family to reconcile against.*

## 3. The thesis

> **READERS turns a pile of beta-reader notes into a corroborated revision worklist.**
> It anchors each note to its paragraph, counts where readers agree, cross-references
> every AI reader's verdict at that spot, and promotes the confirmed findings into
> REDLINE — so real reception becomes as located, ranked, and fixable as everything
> else inkhaven already finds.

Observe-and-reconcile, never generate: READERS ingests and cross-checks; it never
writes prose. It reuses inkhaven's own machinery end to end and adds **no new runtime
crates**.

## 4. The architecture — reuse, not reinvention

Three steps, each built almost entirely on substrate that already exists:

**Import → notes become comments.**
A reader note *is* a comment. `readers import <file> --reader "Sam"` parses a feedback
file (a flat `ch N: …` list, a markdown notes doc, or plain paragraphs), and for each
note **anchors it to a paragraph** — explicit `ch N` via the deterministic
chapter-resolver, else free text via `book_rag::retrieve` (top-scored hit → paragraph
Uuid) — then lands it as a `Comment` (author = the reader) in that paragraph's
existing sidecar. Zero new storage: the `Ctrl+V Shift+C` comment panel, `ink.review.*`,
resolve/export all light up for free. Fuzzy anchors are surfaced for confirmation.

**Reconcile → the native core.**
`readers reconcile` reads the reader-authored comments and runs `collect` (every AI
reader's findings). It groups reader notes by anchored paragraph, counts **distinct-
reader convergence**, and cross-references the **AI findings at that paragraph /
chapter**. Every cluster is classified:
- **confirmed** — readers converge *and/or* an AI reader corroborates → high signal;
- **felt** — readers flag it, no AI echo → the taste/impact miss the simulations can't
  reach → a *new* signal;
- **(and, as a side view) unwitnessed** — an AI finding no reader ever mentioned → a
  candidate false positive, to deprioritize.

**Promote → into REDLINE.**
The confirmed (and, opt-in, the felt) reader clusters convert to `EditorialFinding`s
(`source: "reader"`) and join `collect`'s worklist. Because a reader note has no honest
single-locus fix, its category routes to **Brief** (developmental advice) or, when it
maps to a known judgement kind like `confusion`, to **Decision** — both already wired
in REDLINE. It is now a row in `Ctrl+V Shift+R`, jumpable and actionable.

## 5. What the writer sees

```
inkhaven readers import beta-sam.md --reader Sam     # notes → anchored comments
inkhaven readers reconcile                            # the reckoning

Reader reckoning — 3 readers · 47 notes

  CONFIRMED (readers agree, instruments corroborate)
    ch. 5 ¶ tower-arrival   3 readers "dragged / lost" · ⚑ LECTOR attention_dip · SENTINEL break
    ch. 9 ¶ the-reveal      2 readers "saw it coming"  · ⚑ LECTOR unpaid_setup

  FELT (readers flag it; the instruments are silent)
    ch. 2 ¶ mara-intro      2 readers "didn't like her" — no AI echo (a taste signal)

  UNWITNESSED (instruments flag it; no reader mentioned it)
    ch. 7 ¶ …               Inner Editor: filter words — no reader noticed (deprioritize?)

  → 4 confirmed findings promoted to the revision worklist (Ctrl+V Shift+R)
```

## 6. Cost — a design invariant, near-zero at any book size

READERS **never sends the manuscript to a model.** The reading is done by humans; the
AI's role is bookkeeping. This is a stated invariant, not an accident:

- **Anchoring** a note to a paragraph uses `book_rag::retrieve` — a **local** fastembed
  embedding of the note (one sentence) plus a local HNSW vector search over paragraphs
  that were already embedded once, locally, at index time. No API call.
- **Reconciliation** is pure deterministic set-matching (group notes by paragraph, count
  distinct readers, co-locate against `collect`'s findings). No model. `collect` itself
  is deterministic — it reads computed findings + cached sidecars and makes **no live
  LLM calls** (verified: it runs on projects with no LLM provider configured at all).
- **Promotion** to the worklist is a struct conversion.

So the whole core (import + reconcile + promote) costs **≈ $0 in API, independent of
book length** — for a 1000-page volume the same as for a short story, because cost
scales with the number of *notes* (hundreds), not pages, and every note-anchor is a
local vector search. The book's one-time embedding is local, too.

The **only** LLM touchpoint is opt-in and bounded: pressing `f` on a confirmed reader
finding to get a REDLINE **brief** — one paragraph (~300 words) + the note, one call,
user-triggered, under the daily cap. There is deliberately **no whole-book "let the AI
judge reception" pass** (that would be prohibitive at scale — and it is exactly what
READERS exists to avoid, since it has real readers). Reconciliation stays deterministic
co-location; any future semantic-corroboration refinement must stay per-pair and
cost-capped, never book-wide.

## 7. Scope discipline (what READERS is not)

- Not a new AI reader — it's the *conduit* for the real ones, and the reconciler that
  makes the AI ones earn out.
- Not a survey tool — it ingests notes the writer already has; it doesn't collect them.
- Not a prose editor — it lands comments and promotes findings; it never writes prose
  (REDLINE, gated by the confirmed-diff contract, still owns every edit).
- Not magic anchoring — every fuzzy anchor is a proposal the writer confirms.

The phase-by-phase, file-grounded build is in `READERS-1_IMPL.md` (RE-P0→P6, value
core P1+P2+P3).
