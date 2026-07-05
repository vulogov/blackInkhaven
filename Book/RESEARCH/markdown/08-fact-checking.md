# 8 — Auditing Everything You Kept

So far you have checked claims one at a time, as you kept them. But a knowledge base has a property no single fact has: **internal consistency**. Two facts, each plausible on its own, can quietly contradict each other — a date in Chapter 3 that cannot be reconciled with a journey in Chapter 9, a population that does not match a later figure. As a corpus grows past what you can hold in your head, you need a way to check not just each fact, but **all of them together**. That is `/factcheck`.

## Two questions at once

`/factcheck` audits your Facts book along two axes.

The first is **truth**: taking each fact on its own and asking whether it holds up — the same kind of accuracy check the gate runs, now swept across everything you have kept, so a fact that slipped through months ago gets a fresh look.

The second is **consistency**: taking your facts in related groups and asking whether they **agree with each other** — surfacing the internal contradictions that no single-fact check could ever see, because the problem is not in either fact but in the **pair**.

**Audit** — An **audit** is a pass over your whole Facts book at once, rather than a check of a single claim as you write it. It catches two things a one-at-a-time workflow misses: old facts that were never re-examined, and **contradictions between** facts that are each fine alone.

You run it when a draft is far enough along that consistency matters — before you hand a manuscript to a reader, an editor, or a reviewer. It is the research equivalent of a spell-check pass: not something you do on every sentence, but something you would not ship without.

## Reading the verdicts

An audit is only useful if you can **see** its results without reading a report. After `/factcheck` runs, each fact in your tree wears a small verdict mark, so the state of your whole corpus is legible at a glance:

- **✓** — the fact held up. Nothing to do.
- **?** — the fact is **dubious**; the audit had doubts worth your attention.
- **✗** — the fact did not pass; something is wrong or contradicted.

**Verdict** — A **verdict** is the audit's judgement on one fact, shown as a glyph beside it in the tree — passed (✓), dubious (?), or failed (✗). The glyph is a standing reminder: it persists after the audit, so you can work through the doubtful and failed facts at your own pace, and see at a glance which parts of your corpus are solid.

The glyphs turn a wall of text into a map. Green-check branches are settled; question-marks and crosses are your worklist. You are never handed a verdict you cannot act on, and you are never forced to act on one immediately — the marks wait for you.

## `/whatswrong`: why a fact failed

A ✗ tells you **that** a fact failed; it does not, by itself, tell you **why**. For that, select the flagged fact and ask:

```
/whatswrong
```

The Assistant explains — specifically and concretely — what it believes is inaccurate or contradicted about that fact, and what the correct information appears to be. Now the cross is actionable: you can fix the wording, re-ground the fact on a better source, or — if you know the check is mistaken — leave it, having looked. As always, the tool explains and recommends; the decision to change a fact is yours, and it never edits your prose to make it.

**For fiction —** Run an audit before a beta read. The ✗ and ? marks catch the continuity slips that pull a reader out of the story — the mismatched date, the town that moved sixty miles between chapters — while they are still cheap to fix.

**For non-fiction —** Run an audit before submission or review. The consistency pass is exactly what a hostile reader will do to your argument; better that you find the contradicted figure than a referee does. `/whatswrong` turns each failure into a specific correction.

> **The audit informs; you decide:** Like every check in this book, `/factcheck` never changes a fact on its own. It marks, it explains, it recommends. A ✗ you disagree with can stay — sometimes the audit is wrong, or the "contradiction" is a deliberate feature of your world. The glyph simply guarantees you **saw** it.

## From finding faults to fixing tiers

An audit tells you which facts are shaky. Often the fix is not to delete a fact but to **strengthen** it — to take a claim that is only a model's guess and re-ground it on a real source, moving it up the ladder. The Assistant can do that for you, almost automatically, and it can also warn you when a fact has simply grown old. Strengthening and maintaining what you have kept is the subject of the next chapter — along with the mirror-image of triangulation promised earlier: a check that tries its hardest to **disprove** a claim before you trust it.

**Recap**

- `/factcheck` **audits the whole Facts book** along two axes — per-fact **truth** and cross-fact **consistency** — catching contradictions no single-fact check can see.
- Run it at draft milestones (before a beta read, an edit, a submission), like a spell-check pass for your facts.
- Each fact then wears a **verdict** glyph — ✓ passed, ? dubious, ✗ failed — turning your tree into a legible worklist that waits for you.
- `/whatswrong` explains **why** a flagged fact failed and what the correct information is, making each ✗ actionable.
- The audit marks and explains but never edits — and often the right fix is to **strengthen** a shaky fact, which is the next chapter.
