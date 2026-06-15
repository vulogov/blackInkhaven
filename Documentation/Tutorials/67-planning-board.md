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

**Map a beat to a chapter** by setting `mapped_chapter` to that chapter's
slug, and (optionally) listing the `threads` it advances. That's the whole
input the diagnosis needs.

## 2. Check the structure

```sh
inkhaven plan check                 # or --json / --drift 15 / --book-name "My Novel"
```

```
plan check · My Novel · Save the Cat · 24 chapter(s)

BEATS
  ✓ Catalyst          act 1  target  10%  → 03-the-letter (12%, +2%)
  ⚠ Midpoint          act 2  target  50%  → 14-the-reveal (64%, +14%)  ↪ inheritance
  ✗ All Is Lost       act 2  target  75%  (unmapped)

PACING (act word-share)
  Act 1   expected  20%   actual  30%   ⚠ long
  Act 2   expected  60%   actual  45%   ⚠ short

3 finding(s):
  ⚠ gap: `All Is Lost` is unmapped
  ⚠ drift: `Midpoint` lands at 64% (target 50%, +14%)
  ⚠ pacing: Act 1 is 30% of words (expected 20%, long)
```

Three diagnoses, none of them AI:

- **Coverage** — which beats have a home, which are gaps (`✗`).
- **Position drift** — a beat lands far from where the framework expects it
  (your Midpoint is two-thirds in, not half).
- **Pacing** — each act's *word share* vs. the framework's shape. "Act 1 is
  30% of your words" is the objective version of "the opening drags."

These are factual, not verdicts — prompts to think.

## 3. See it at a glance

In the editor, **`Ctrl+V Shift+K`** (K for sKeleton) opens the structure
outline: every beat as a position bar — `|` is where the framework wants
it, `●` where it actually lands — colour-coded on-target / drift / gap,
with the act pacing below. `↪N` marks beats that advance threads.

## 4. Ask the AI

For the qualitative read, over the book digest:

```sh
inkhaven plan analyze               # builds the digest if needed
```

It maps each beat to the best-fitting chapter and names the problems —
*"your 'bad guys close in' stretch is doing the work of a midpoint; the
real midpoint is missing."* From the outline view, **`a`** streams the same
analysis into the AI pane. Its prompt resolves through the usual three
tiers (a `plan-analyze` paragraph in your Prompts book → `prompts.hjson` →
built-in), so the editorial voice is tunable.

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
