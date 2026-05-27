# 42 — Sentence-rhythm gauge

`Ctrl+B Shift+H` opens a modal that quantifies the
rhythm of the open paragraph — useful for noticing
when your sentences have drifted into a monotone
drone and need a short one to break the pattern.

The principle behind the gauge, from Gary Provost:

> "This sentence has five words.  Here are five more
> words.  Five-word sentences are fine.  But several
> together become monotonous.  Listen to what is
> happening.  The writing is getting boring.  The
> sound of it drones.  It's like a stuck record.
> The ear demands some variety."

The gauge measures exactly that monotony, in numbers
you can act on.

## What it shows

```
┌─ Sentence rhythm — open paragraph ────────────────────┐
│ verdict: VARIED  (strong variation · good prose)      │
│ 17 sentences · mean 11.3 · stdev 6.8 · CV 0.60 ·      │
│                              min 3 · max 24           │
│  #     bar                              words   preview
│   1   ████████                            8     The morning…
│   2   ████                                4     Bob shivered.
│ ▶ 3   ████████████████████████          24     She had not slept
│   4   ███                                 3     Then silence.
│   …                                                   │
│                                                       │
│ shortest:                                             │
│   l4    3w   Then silence.                            │
│   l7    3w   He coughed.                              │
│   l11   4w   Bob shivered.                            │
│ longest:                                              │
│   l3   24w   She had not slept in seventy hours…     │
│   l9   21w   The morning fog clung to the cobble…    │
│   l14  19w   And then the wind shifted and brought…  │
│                                                       │
│ ↑↓ / PgUp / PgDn / Home / End scroll · any key closes │
└───────────────────────────────────────────────────────┘
```

## How to read the verdict

The verdict is computed from the **coefficient of
variation** (CV = stdev / mean), with thresholds
chosen against Provost's parable:

| CV range       | Verdict     | Colour | Note                                             |
|----------------|-------------|--------|--------------------------------------------------|
| `< 0.25`       | MONOTONE    | red    | drones · break it with a short one               |
| `0.25 – 0.45`  | STEADY      | yellow | modest variation · workable but can sing louder  |
| `0.45 – 0.80`  | VARIED      | green  | strong variation · good prose rhythm             |
| `≥ 0.80`       | CHOPPY      | cyan   | extreme variation — fragments + long mixed       |
| `< 3 sentences` | TOO SHORT  | grey   | need at least 3 sentences to judge               |

CV normalises stdev against the mean, so a 10-word-
mean passage with stdev 5 is correctly rated
identically to a 20-word-mean passage with stdev 10
— same rhythm at twice the tempo.  Raw stdev would
mislead toward the long-sentence passage being "more
varied" when it's actually the same shape at a
different pace.

## How the sentence splitter works

The gauge ships a hand-rolled walker (deliberately
not a parser — the goal is a rhythm gauge, not a
linguistic engine):

- Splits on `.` / `!` / `?` followed by whitespace
  or end-of-text.
- Consumes trailing closing quotes + parens
  greedily, so `said "Hello."` is one sentence.
- Treats `...` and longer dot runs as a mid-
  sentence pause, not a terminator.
- Suppresses splits inside common abbreviations:
  Mr., Mrs., Dr., Sr., Jr., Prof., e.g., i.e.,
  Ph.D., M.D., etc., Inc., Ltd., …  Full list in
  `src/tui/sentence_rhythm.rs`.

## Inside the modal

| Chord                    | Action                          |
|--------------------------|---------------------------------|
| `↑` `↓`                  | Scroll one sentence              |
| `PgUp` `PgDn`            | Scroll ten sentences             |
| `Home` / `End`           | Jump to first / last sentence    |
| Any other key            | Close                            |

## Mnemonic

`Shift+H` for *heartbeat* — the felt rhythm of the
prose.

## Use cases

- **Drone diagnosis**.  If the verdict is MONOTONE,
  scan the bar chart for the longest flat run and
  break it with a short sentence.
- **Outlier sanity check**.  The shortest + longest
  callouts let you ask "is this 47-word sentence on
  purpose?" without re-reading the paragraph.
- **End-of-revision sanity pass**.  Open the gauge
  on each chapter's first paragraph — if every one
  starts VARIED, your openings are doing rhythmic
  work.

## See also

- [`03-the-editor.md`](03-the-editor.md) —
  filter-word + repeated-phrase + show-don't-tell
  overlays (always-on style audit).
- [`40-concordance.md`](40-concordance.md) — word-
  level audit; the rhythm gauge is its sentence-
  level counterpart.
