# Tutorial 67 — The Planning Board: diagnose your structure

*Inkhaven 1.3.2+*

Most planning tools help you *organize* — corkboards, index cards, outlines.
What none of them do well is help you **diagnose**: *"something's wrong with
my middle but I can't see it."* The Planning Board turns that vague unease
into a specific, located, objective finding.

Structure is a third axis alongside the **Timeline** (*when* things happen)
and **Threads** (*whether each arc pays off*): the acts, beats, and turning
points — the shape of the rise and fall. It works on a draft you already
have; you don't plan from a blank page.

## 1. Lay a framework over the book

Pick a story-structure framework and scaffold its beats into the
**Planning** system book:

```sh
inkhaven plan init --framework save_the_cat
# three_act (default) | save_the_cat | story_circle | hero_journey | seven_point
```

Each beat becomes an HJSON paragraph in the Planning book — open one in the
editor and it reads like a Thread:

```hjson
{
  framework:       "save_the_cat"
  beat:            "Midpoint"
  act:             2
  target_position: 0.50
  mapped_chapter:  null        // ← set this to a chapter slug
  threads:         []          // ← arc slugs this beat advances
  status:          "planned"
}
```

## 2. Find the slugs, then map a beat

A beat maps to a chapter by the chapter's **slug**, and to arcs by their
**thread slugs**. You don't have to guess them — **`inkhaven plan check`
prints both lists at the bottom of its report**, so the loop is *init →
check → map → check again*:

```
CHAPTER SLUGS (set `mapped_chapter:` to one of these)
  the-wharf            0%
  the-letter          12%
  the-reveal          48%
  the-long-night      71%
  …
THREAD SLUGS (add to a beat's `threads:` list)
  the-inheritance
  the-lighthouse-secret
```

These are the **bare** slugs — the title lowercased and hyphenated, with
**no** leading `NN-` number. (That `01-`/`02-` prefix you see on the folder
names under `books/…/` only orders the files on disk; it is *not* part of
the slug.) Threads come from the **Threads** book — add an arc with
`inkhaven thread add "The Inheritance"` and its slug is `the-inheritance`.

Now open the beat in the editor — it's a paragraph under **Planning** in
the tree (`Planning ▸ Midpoint`) — and fill the two fields, copying the
slugs from the lists above:

```hjson
{
  framework:       "save_the_cat"
  beat:            "Midpoint"
  act:             2
  target_position: 0.50
  mapped_chapter:  "the-reveal"          // ← was null
  threads:         ["the-inheritance"]   // ← was []
  status:          "drafted"
}
```

Save with `Ctrl+S`. One chapter can host several beats, and a beat can
carry several threads (`["the-inheritance", "the-lighthouse-secret"]`).
Re-run `plan check` and the beat resolves:

```
✓ Midpoint          act 2  target  50%  → the-reveal (48%, -2%)  ↪ the-inheritance
```

(Prefer the editor, but the same edit works from any text editor on the
beat's `.typ` file under `books/planning/` — inkhaven reloads it.)

**Or skip the hand-edit entirely.** `plan map` does the write-back for you,
straight from the slugs `plan check` just printed:

```sh
inkhaven plan map Midpoint --chapter the-reveal --threads the-inheritance
inkhaven plan unmap "All Is Lost"     # clears mapped_chapter back to null
```

The beat is matched by name, slug, or beat-number; the mapping lands in its
Planning-book HJSON exactly as the hand-edit would. This is the same
primitive the interactive outline (§4) uses.

## 3. Check the structure

```sh
inkhaven plan check                 # or --json / --drift 15 / --book-name "My Novel"
```

```
plan check · My Novel · Save the Cat · 24 chapter(s)

BEATS
  ✓ Catalyst          act 1  target  10%  → the-letter (12%, +2%)
  ⚠ Midpoint          act 2  target  50%  → the-reveal (64%, +14%)  ↪ the-inheritance
  ✗ All Is Lost       act 2  target  75%  (unmapped)

PACING (act word-share)
  Act 1   expected  20%   actual  30%   ⚠ long
  Act 2   expected  60%   actual  45%   ⚠ short

3 finding(s):
  ⚠ gap: `All Is Lost` is unmapped
  ⚠ drift: `Midpoint` lands at 64% (target 50%, +14%)
  ⚠ pacing: Act 1 is 30% of words (expected 20%, long)

CHAPTER SLUGS (set `mapped_chapter:` to one of these)
  the-wharf            0%
  the-letter          12%
  …
THREAD SLUGS (add to a beat's `threads:` list)
  the-inheritance
```

Three diagnoses, none of them AI:

- **Coverage** — which beats have a home, which are gaps (`✗`).
- **Position drift** — a beat lands far from where the framework expects it
  (your Midpoint is two-thirds in, not half).
- **Pacing** — each act's *word share* vs. the framework's shape. "Act 1 is
  30% of your words" is the objective version of "the opening drags."

These are factual, not verdicts — prompts to think.

## 4. See it at a glance

In the editor, **`Ctrl+V Shift+K`** (K for sKeleton) opens the structure
outline: every beat as a position bar — `|` is where the framework wants
it, `●` where it actually lands — colour-coded on-target / drift / gap,
with the act pacing below. `↪N` marks beats that advance threads.

The outline is **interactive** — you map without leaving it:

- `↑↓` browse beats; the selected beat's intention shows under the bar.
- **`m`** opens a chapter picker and maps the selected beat to your choice
  (the write-back from §2, no HJSON editing).
- **`s`** cycles the beat's status (planned → drafted → revised → done).
- **`a`** streams the AI analysis into the AI pane (§5).

So the tighten-the-structure loop — *see the drift, map the beat, watch it
snap to target* — never leaves this one view.

## 5. Ask the AI

For the qualitative read, over the book digest:

```sh
inkhaven plan analyze               # builds the digest if needed
```

It maps each beat to the best-fitting chapter and names the problems —
*"your 'bad guys close in' stretch is doing the work of a midpoint; the
real midpoint is missing."* From the outline view, **`a`** streams the same
analysis into the AI pane. Its prompt resolves through the usual three
tiers (a `plan-analyze` paragraph in your Prompts book → `prompts.hjson` →
built-in), so the editorial voice is tunable. To keep an analysis you like,
press **`L`** in the AI pane — it files the response as a *Structural
Analysis* paragraph in the Planning book, next to your beats.

## 6. Start from nothing (plan-first)

The sections above diagnose an existing draft. You can also run the Board
the other way — skeleton first, prose later. Give a framework a one-line
premise and let it write the beat sheet:

```sh
inkhaven plan init --framework save_the_cat
inkhaven plan scaffold --premise "A lighthouse keeper's daughter inherits a
  debt that can only be paid by the secret her father drowned to keep."
```

`scaffold` writes a concrete **intention** into every beat — what actually
happens there in *this* story, not the generic beat description — which the
outline then shows under each bar. Add `--chapters` to materialize a
**chapter shell** per beat, named from the beat and pre-linked via
`mapped_chapter`:

```sh
inkhaven plan scaffold --premise "…" --chapters
```

That's opt-in and guarded — it refuses to run once the book has chapters,
so it can't clobber a draft. The result is a mapped beat sheet and a
chapter scaffold you can start writing into, with `plan check` already
green because every beat has a home.

## Threads do narrative work

Listing a beat's `threads` connects structure to arcs: `plan check` shows
them (`↪`), flags a reference to a thread that doesn't exist, and — once
you're using thread-links — nudges you about mapped beats that advance no
tracked thread. A beat that moves the plot forward but pays off no arc is
worth a second look.

## Where to go next

- The submission path once the structure holds:
  [Tutorial 66](66-submission-package.md).
- Every chord: [`../KEYBINDING.md`](../KEYBINDING.md).
- The design: [PLANNING-1 plan](../PROPOSALS/PLANNING-1_PLAN.md).
