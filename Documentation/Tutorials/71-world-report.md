# Tutorial 71 — The world report, and drift that reads pronouns

*Inkhaven 1.3.11+*

Tutorials [69](69-world-consistency.md) and [70](70-semantic-drift.md) added
the world-consistency *checks* — facts, anachronisms, drift. This chapter
adds the things that tie them together: a deeper drift retriever, and one
**snapshot** that shows the whole world layer at a glance.

## Drift now reads pronouns

The 1.3.10 drift retriever kept only paragraphs that *named* the entity — so
it missed the most common description shape:

```
Mara crossed the yard.            ← names Mara
She was taller than he remembered. ← describes Mara, but only "She"
```

1.3.11 adds **coreference-lite**: walking each chapter, it tracks the most
recently, *unambiguously* named entity of each kind; a name-less paragraph
with a matching pronoun is attributed to it. Two same-kind names in a
paragraph clear the anchor (no guessing), and attribution never crosses a
chapter — it favours precision, with the AI judge as the backstop.

It's **multilingual**: built-in pronoun sets ship for English, Russian,
French, German, and Spanish, keyed by your project `language` (so "Она была
выше" attributes in a Russian manuscript exactly as "She was taller" does in
English). Articles and high-frequency homographs are excluded to keep it
precise; pro-drop languages (Spanish) lean on possessives, so recall is
naturally a little lower there.

Nothing to configure — re-run `inkhaven drift scan` and the recovered
descriptions are simply in the mix.

## `inkhaven world` — the consistency snapshot

One command folds the whole world layer into a single dashboard:

```sh
inkhaven world            # the snapshot
inkhaven world --json     # for a CI gate
inkhaven world --deep     # refresh facts-check / facts-scan / drift / continuity first
```

```
World: 2 issue(s) — 1 fact conflict · 1 drift

Facts
  established: 41
  internal conflicts: 1
    ⚠ winters are mild  ⟷  the harbor freezes each January  — a mild winter can't freeze a harbor
  prose-vs-fact contradictions: 0

Drift
  1 description contradiction(s):
    ⚠ The Drunken Goose (place) — [ch.2] “cramped and smoky”  ⟷  [ch.20] “airy and bright”

Continuity
  12 tracked attribute(s)

Anachronisms
  0 flagged term(s)

Coverage
  9 character(s) · 4 place(s) · 2 artefact(s)
  1 defined but never named in the prose:
    · Joss
```

It **complements** `inkhaven edit`: `edit` is a walkable worklist of
*everything* (style, structure, world); `world` is a focused *snapshot* of the
world layer, grouped by fact and entity, gateable in CI. The same one-line
health summary now also banners the top of the **`Ctrl+V Shift+L`** story
bible.

### Coverage: the dangling cast

The **Coverage** section flags entities defined in the Characters / Places /
Artefacts books but **never named in the prose** — a character you sketched
but never wrote in, a place that exists only in the bible. They also surface
in `inkhaven edit` as an `info`-level `coverage` finding, jumping to the
entity's bible definition. (Silent on an unwritten draft — no prose, no
flags.)

### Anachronisms, named

With a setting `year`, the **Anachronisms** section lists each flagged term
and its chapter, not just a count.

## Zooming in: `--entity`

To focus on one entity across all of this:

```sh
inkhaven world --entity Mara     # Mara's drift, description trail, attributes, prose-status
inkhaven drift list --entity Mara  # just the retrieved descriptions
```

## Where to go next

- The checks themselves: [Tutorial 69](69-world-consistency.md) (hard) ·
  [Tutorial 70](70-semantic-drift.md) (soft).
- The worklist these feed: [Tutorial 68](68-editorial-pass.md).
- The design: [WORLD-3 plan](../PROPOSALS/WORLD-3_PLAN.md).
