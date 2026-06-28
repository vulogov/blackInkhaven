# Tutorial 95 — Narrative Voice

*Inkhaven 1.4.12*

You're forty chapters into a novel. Does chapter 40 still sound like the person
who wrote chapter 1? Inkhaven's prose companions (Inner Editor, Inner Socrates)
read one paragraph at a time; none of them characterise your **voice as a
statistical property of the whole book**. NARR-1 does — **deterministically, with
no AI and no cost.**

## Profile your book

```sh
inkhaven prose profile
```

```
Prose voice profile — "The Drowned City" (38 chapters, 142,800 words) [en]
────────────────────────────────────────────────────────────────
Sentence CV          0.49
Burstiness B         +0.09
MATTR (w=100)        0.72
Modal density        0.041
Interiority ratio    0.16
────────────────────────────────────────────────────────────────
Tier-2 not computed. Run with --deep for sensory balance + active/passive ratio.
```

CV near 0.5 means rhythmically varied sentences; MATTR 0.72 is healthy lexical
diversity; modal density is how much the narrator hedges. Add **`--deep`** for
sensory balance (which senses your prose leans on) and the active/passive ratio.

## See where the voice drifted

```sh
inkhaven prose drift
```

This compares every chapter against the baseline (chapter 1 by default) and lists
the metrics that moved past their threshold:

```
ch.34 vs ch.1     ΔCV  -0.180   ΔMATTR  -0.060   ΔModal  +0.025   ΔInterior  +0.110

Threshold crossings (info — descriptive, not prescriptive):
  ch.34  sent_len_cv          -0.180  (baseline 0.490 → 0.310)
  ch.34  interiority_ratio    +0.110  (baseline 0.160 → 0.270)
```

Chapter 34's sentences got more uniform and its interiority climbed — maybe a
deliberate shift into a tense, inward sequence, maybe drift. The numbers tell you
*where* to look; you decide.

## From the editor — `Ctrl+V V`

Press **`Ctrl+V V`** (Voice) while writing. It runs the same check in the
background — content-hash lazy, so only the chapter you just edited recomputes —
and drops any threshold crossings into the **Output pane** (`Ctrl+B Tab`) as
informational findings you can jump to. No model call, no budget.

Want it automatic? **`Ctrl+V Shift+V`** toggles ambient mode: after an editing
pause it re-runs itself (off by default, with a cooldown floor since it's a
whole-book scan).

## Works in your language

Russian, German, French, and Spanish are first-class — modal hedging,
interiority, sensory vocabulary, and passive detection all have curated lists per
language (German even tracks *erlebte Rede* particles). A book in an unsupported
language still gets the full rhythm tier (CV, burstiness, MATTR), with a note
about which metrics couldn't be computed. Add your own hedging collocations via
`prose.extra_modal_tokens` in `inkhaven.hjson`.

## From a script

```
ink.prose.refresh      ( -- count )   recompute, return profile count
ink.prose.profile      ( -- list )    stored per-scope profiles
ink.prose.drift        ( -- list )    per-chapter deltas vs baseline
ink.prose.violations   ( -- list )    threshold crossings
```

---

**See also:** [PROSE_VOICE.md](../PROSE_VOICE.md) ·
[KEYBINDING.md → `Ctrl+V V`](../KEYBINDING.md) · `inkhaven prose suggest`.
