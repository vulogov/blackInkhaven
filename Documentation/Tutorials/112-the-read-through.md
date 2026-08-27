# Tutorial 112 — The Read-Through

*Inkhaven 2.3 (LECTOR)*

Every reader Inkhaven has given you works small — a paragraph, a break, the voice.
LECTOR reads the whole book the way the one reader who matters most does: the
**first reader**, forward, once, not knowing the ending. It reports both the
*shape* of the read and the *experience* of the read. Deterministic and free at the
core; the one LLM pass is explicit and cost-capped.

## Read your book

```sh
inkhaven readthrough
```

```
Read-through — 12 chapter(s) · Hero's Journey
  measured   ▂▃▄▂▁▁▃▅▄▆█▃
  expected   ▁▂▃▄▅▅▆▇▇██▂
  ch  1  ▂ ·   The Village
  ch  2  ▃ ▶   The Summons
  …
⚠ [shape_sag] the Hero's Journey shape wants rising tension around ch. 5 (~55%) but the prose reads flat (~12%).
⚠ [info_dump] ch. 2 introduces 5 new names at once (Mara, Joren, Aldous, Sella, Cael) — hard for a reader to hold.
● [attention_dip] ch. 5 reads low-energy with nothing new — the reader's attention may drift.
```

Two things at once:

- **`measured`** is the book's dramatic intensity, read straight from the prose —
  dialogue density, a stakes/conflict vocabulary, sentence-rhythm acceleration, a
  chapter-ending turn. No tagging required.
- **`expected`** is the intended shape — the framework's tension curve. When a
  chapter should rise but the prose reads flat, that's a **`shape_sag`** (the
  empirical saggy middle).

The framework comes from `lector.framework`, else it's **suggested from your
`genre`** (fantasy → Hero's Journey, thriller → Save the Cat, mystery →
Seven-Point, slice-of-life → **Kishōtenketsu**), else Three-Act.

## What a first reader trips on

LECTOR walks the book forward, carrying the reader's state — who they've met, what's
still open — and flags the problems a first reader hits, **forward-only** (a later
payoff never cancels an earlier dip):

| Finding | What it catches |
| ------- | --------------- |
| `confusion` | an entity used before it's introduced |
| `info_dump` | too many new names in one chapter |
| `attention_dip` | a flat, eventless chapter |
| `put_down_risk` | a run of flat chapters — a likely put-down point |
| `unpaid_setup` | a setup raised but never paid off |

It also reads the **scene ⇄ sequel** rhythm (`▶` scene / `◉` sequel / `·` mixed) and
flags arrhythmia — an all-scene stretch reads breathless, an all-sequel stretch sags.

## When you want a reader's eyes

The deterministic walk can't judge whether the stakes actually *land*. The
synthetic first-read can — an LLM reacts to each chapter **as a first reader who
does not know the ending** (it only ever sees a recap of what came before). Explicit
and cost-capped:

```sh
inkhaven readthrough --deep
```

or press **`k`** in the read-through dashboard. Its findings land in Output as
`source: reader`; the cost is previewed against your daily cap (cost *informs*, it
never blocks).

## In the editor

- **The dashboard** — **`Ctrl+B Shift+A`** opens the read-through: the curve, the
  beats, the findings. `↑↓` to scroll (`PgUp`/`PgDn` page, `Home`/`End` to the
  ends; blank separators are skipped, so **Enter** always lands on a real row),
  **Enter** jumps to the chapter, **`k`** runs the synthetic first-read, `Esc`
  closes.
- **The review pass** — `Ctrl+B Shift+C` includes a `read-through` line (Output
  `readthrough` category), so the reader problems ride the same sweep as everything
  else.

## Configure it

```hjson
lector: {
  enabled: true      // the read-through line in the review pass
  framework: null    // three_act | save_the_cat | story_circle | hero_journey |
                     // seven_point | kishotenketsu; null = suggest from `genre`
}
```

## From a script

```
ink.readthrough.check    ( -- dict )  { chapters, findings, concerns, notices, info, by_kind }
ink.readthrough.report   ( -- list )  the ranked findings
ink.readthrough.curve    ( -- list )  per chapter { chapter, title, position, measured, expected, kind }
```

Read-only — the synthetic first-read is not exposed to Bund.

---

LECTOR doesn't replace your first reader; it catches what you'd wince at before you
hand them the pages. The full reference is [`LECTOR.md`](../LECTOR.md).
