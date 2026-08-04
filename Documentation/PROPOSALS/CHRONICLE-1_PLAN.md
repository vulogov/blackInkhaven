# CHRONICLE-1 — "Did it get better?" (the 2.5.0 flagship RFC)

*The draft-history intelligence: persist every reader's metric per draft milestone,
trend it over time, and — the signature move — tell the writer which of REDLINE's
findings their revision **cleared** and which **new** ones it **introduced**.*

---

## 1. The problem

Revision is only as good as your ability to *see it working*. Inkhaven has spent
the whole 2.x arc teaching the book to understand itself — SEMNET gave it a mind,
SENTINEL let it watch itself, LECTOR let it read itself, CHORUS let it hear its
voices, and REDLINE (2.4) helped the writer *act* on all of it. But nothing
**remembers what the book measured last draft.** Every metric — LECTOR's intensity
curve and saggy middle, CHORUS's voice-distinctiveness matrix, SENTINEL's
continuity breaks, the reader-experience findings — is computed on the *current*
state and thrown away. So the one question a reviser most wants answered has no
answer:

> *I spent a week revising. Is the book actually better, or did I just move the
> furniture — and did I break anything that was fine before?*

CHRONICLE answers it.

## 2. The thesis

> **CHRONICLE snapshots the readers' collective verdict at each draft milestone and
> trends it. It measures whether revision is working — did the sag lift, did the
> voices separate, did the continuity breaks close, did confusion drop — and, most
> usefully, which specific findings your edits *cleared* versus which *new* ones
> they *introduced*.**

It is **deterministic and free** at the core (it persists and diffs numbers the
readers already compute — no LLM needed for the trend), **advisory** (it *measures*;
it has **zero** prose-write surface), **multilingual by inheritance** (the
underlying metrics already are), and **self-contained** (it reuses every metric
engine 2.x already shipped — no new external deps).

## 3. Why this is the right 2.5.0 (the arc capstone)

The 2.x arc has two movements: **understand** (SEMNET → GRAPHMIND → CHORUS →
SENTINEL → LECTOR) and **act** (REDLINE). CHRONICLE is the beat that was always
missing: *feedback*. You built readers that **diagnose** and a partner that
**fixes**; CHRONICLE is the instrument that says whether the fixes **worked**. It
makes every prior flagship's intelligence **cumulative** rather than momentary — the
distinctiveness matrix, the shape curve, the continuity ledger all gain a time axis.
And it closes REDLINE's loop directly: REDLINE turns a finding into a change;
CHRONICLE confirms the change landed and flags the collateral.

## 4. The core insight — one source, one fingerprint

Two facts make CHRONICLE small and honest:

1. **`crate::cli::editorial::collect` already aggregates every reader** into one
   headless `EditorialReport` — doctor classes, Facts/drift, plan structure, the
   prose-style detectors, SENTINEL continuity, LECTOR read-through, CHORUS voice,
   the Inner Editor. One call captures the whole diagnostic state. CHRONICLE does
   **not** re-run each reader; it snapshots what `collect` returns.
2. **Every `EditorialFinding` already has a stable `fingerprint()`** (`category ⟂
   message`) — the same identity REDLINE's defer uses. So "which findings cleared /
   were introduced" is just a **set difference** between two milestones' fingerprint
   sets. No new matching machinery.

From that one source CHRONICLE derives a **metric vector** per milestone:

```
milestone "draft-3"  (2026-08-10, book "the-tower")
  total findings         27
  by severity            error 2 · warn 11 · info 14
  by response            rewrite 9 · decision 6 · brief 12
  by category            echo 4 · shape_sag 3 · co_location 2 · confusion 1 · …
  by source              continuity 5 · read-through 7 · stylist 4 · doctor 8 · …
  deferred 3 · stale false
  fingerprints           { echo⟂"about ×5" , shape_sag⟂"ch.5 …" , … }   (the set)
```

Optionally enriched (P2+) with two raw LECTOR numbers `collect` doesn't surface —
**mean measured intensity** and **sag count** from a `walk::read_forward` — for a
true shape trend. Everything else is finding-count deltas.

## 5. What the writer sees

**The trend** (`inkhaven chronicle` — current live state vs the last mark):

```
Chronicle — since "draft-2" (2026-08-03) → now

  findings         31 → 27    ▼  4 fewer
  errors            4 →  2    ▼  cleared 2
  shape sag         3 →  1    ▼  improved
  voice pairs <.5   4 →  2    ▼  the voices separated
  continuity break  2 →  0    ▼  cleared
  confusion (ch.7)  0 →  1    ▲  NEW — introduced by your edits

  ✓ 6 findings cleared    ▲ 1 introduced    · 20 unchanged
  Enter on an introduced finding → jump to the paragraph
```

The **cleared / introduced / unchanged** split (the REDLINE hook, §4.2) is the
emotional payload: proof the work landed, and an early warning on collateral damage
before it ships.

## 6. Surfaces

- **`inkhaven chronicle`** — the trend since the last mark (live capture vs latest
  milestone). `--json` for tooling.
- **`inkhaven chronicle mark <label> [--ref <git-ref>]`** — capture a milestone now
  (metric vector + fingerprint set). The optional `--ref` records a git ref string
  *verbatim* for the writer's own bookkeeping (inkhaven never resolves or enumerates
  git tags — git stays best-effort external, matching `docs review --since`).
- **`inkhaven chronicle list`** — the milestones with their headline numbers.
- **`inkhaven chronicle diff <a> <b>`** — two named milestones head-to-head.
- **The dashboard** — `Ctrl+B Shift+U` opens the trend + cleared/introduced; `Enter`
  jumps to an introduced finding's paragraph. (`Shift+U` and `Shift+Z` are the only
  free `Ctrl+B Shift+<letter>` chords; a `resolve_in` guard test locks it — the
  shadow lesson from SENTINEL/LECTOR.)
- **`ink.chronicle.{marks,trend,check}`** — read-only Bund. `check` is a gate:
  *clean* when your latest edits introduced **no new error-severity** finding since
  the last mark (a CI/pre-submit guard, mirroring `revise.check`).

## 7. Principles it honors

- **Advisory** ([[feedback_ai_advisory]] made trivial): CHRONICLE has no prose-write
  path at all — it is pure measurement. It cannot touch the manuscript.
- **Deterministic + free** at the core; any LLM "what changed in plain English"
  synthesis is an explicit, cost-capped *option*, never the trend itself.
- **Permissive**: it informs, never blocks (even `check`'s gate is opt-in for CI).
- **Multilingual**: inherited — the metrics and finding messages already key off the
  project language; the trend is language-agnostic counts.
- **Self-contained**: reuses `collect` + `read_forward`; mirrors the `progress`
  DuckDB store; no new runtime crates.

## 8. Scope discipline (what CHRONICLE is *not*)

- Not a new reader — it snapshots the readers you have.
- Not a git tool — it never creates or lists tags; a "milestone" is an explicit
  `chronicle mark`, the same shape as a paragraph snapshot.
- Not an auto-capture daemon — marks are deliberate (a draft is a decision).
- Not a corrector — the introduced-findings list *points*; REDLINE (`Ctrl+V Shift+R`)
  is where you act.

The phase-by-phase, file-grounded build is in `CHRONICLE-1_IMPL.md` (CH-P0→P6, value
core P1+P2+P3).
