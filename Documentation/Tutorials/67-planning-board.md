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
- **`t`** links threads to the beat (Space toggles each on/off).
- **`s`** cycles the beat's status (planned → drafted → done).
- **`Enter`** opens the beat's mapped chapter in the editor.
- **`a`** streams the AI analysis into the AI pane (§5).
- **`v`** flips to the scene board (§7).

So the tighten-the-structure loop — *see the drift, map the beat, watch it
snap to target* — never leaves this one view.

### The tension curve

Beneath the position bars the outline draws two block-ramp sparklines
(`▁`..`█`), aligned to the same axis so a beat's `●` sits over its cell:

- **expected** — the framework's intended *intensity* (calm open, peak in
  the back half — the canonical rise and fall).
- **actual** — how much narrative tension your draft actually carries at
  each point, measured deterministically as **open-obligation density**:
  every question you raise but haven't paid off keeps the line high.

Where expected is high but actual is low, the beat is flagged **flat** — the
objective version of "the midpoint sags." `plan check` prints the same
read as a TENSION section:

```
TENSION (expected vs actual intensity)
  Midpoint            expected  65%  actual  33% ⚠ flat
  Climax              expected 100%  actual 100%
```

The actual line needs the open/resolved data: run **`inkhaven tension
scan`** (the 1.2.19 AI pass that tags each chapter's introduced/resolved
tensions) and/or link your **Threads** so the Board can see which arcs are
open where. Without either, you still get the expected shape, with a hint.

**A second opinion.** The actual line is deterministic — it counts *open
obligations*. For an independent reading, run **`inkhaven plan tension
rate`**: an AI pass that rates each chapter's felt intensity 0–100 and
caches it. `plan check` then adds an `ai` column and the overlay a third
sparkline, so you compare three readings:

```
TENSION (expected vs actual vs ai intensity)
  Midpoint            expected  65%  actual  33%  ai  35% ⚠ flat
  Climax              expected 100%  actual 100%  ai  95%
```

When *both* actual and ai land low where the framework wants a peak — as at
the Midpoint above — that's two independent signals agreeing the middle
sags. The deterministic curve stays the default; the `ai` line is opt-in and
absent until you rate.

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

## 7. Scene cards (a finer grain than beats)

Beats are chapter-scale. **Scene cards** work one level down — each scene's
**goal → conflict → disaster** (Swain's model: the POV character wants
something, something opposes it, and the scene *turns* on a disaster that
ends it worse or changed):

```sh
inkhaven plan scene add "Mara confronts the harbourmaster" \
    --chapter the-wharf \
    --goal "get the manifest" \
    --conflict "he stonewalls" \
    --disaster "he names her father as the debtor"

inkhaven plan scene list      # grouped by chapter, with G/C/D marks
```

The deterministic check is the **turn**: a scene that states a goal but has
no disaster *doesn't turn* — it's flagged `⚠ no turn`, in `plan scene list`
and folded into `plan check`. `plan scene set <title> --disaster "…"` fills
it in; `plan scene remove <title>` drops the card. Cards live under a
**Scenes** chapter in the Planning book.

In the outline (`Ctrl+V Shift+K`), press **`v`** for the scene board: every
card grouped by chapter with its G/C/D marks and turn flag, the selected one
expanding its full goal/conflict/disaster spine. And `plan analyze` reads
the cards too — it'll name the scenes that don't turn and suggest the turn
they're missing.

**Don't type it from scratch.** The prose already implies the
goal/conflict/disaster, so let the AI propose them:

```sh
inkhaven plan scene scaffold --chapter the-wharf   # one chapter
inkhaven plan scene scaffold --all                 # every cardless chapter
```

In the scene board, **`g`** regenerates the selected card from its chapter's
prose — the proposal streams into the AI pane, and **`L`** files it back
into the card (the same lift you use for the structural analysis).

### Sequels: the reactive half

Swain's structure alternates **scenes** (proactive: goal → conflict →
disaster) with **sequels** (reactive: **reaction → dilemma → decision**).
The disaster lands; the character reels, weighs bad options, and *decides* —
which sets the next scene's goal. Model the reactive beats too:

```sh
inkhaven plan sequel add "Mara reels" --chapter the-wharf \
    --reaction "she's gutted" \
    --dilemma "pay the debt or expose her father" \
    --decision "pay it quietly and hunt the truth"

inkhaven plan sequel list      # or `plan scene list` for both, kind-tagged
```

The weak-card check flips for sequels: one that reaches a **dilemma** but
never **decides** stalls the story (`⚠ no decision`). And once you're using
sequels, two scenes back-to-back flag the first's **unprocessed disaster** —
a skipped sequel. The `v` board shows both kinds, tagged.

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
