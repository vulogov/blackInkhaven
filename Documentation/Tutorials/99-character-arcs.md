# Tutorial 99 — Character Arc Tracking

*Inkhaven 1.4.16*

A character arc is a promise: *this person starts here, and the book earns them
to there.* Inkhaven already tracks **facts** about characters (the continuity
bible) and their **speech** (DIALOG-1 fingerprints). CHAR-1 adds the third axis
— **change over time**: a chapter-ordered chain of each character's observable
state, a deterministic *agency* score, and completeness checks against the arc
you declared.

It reads only your manuscript and your declarations, and — like every advisory
feature — it **never edits your prose**. It measures; you decide.

## Declare the arc

In the **Characters** system book, give a character an arc by writing a
`character_arc` block in their paragraph (HJSON, alongside any other metadata):

```hjson
character_arc: {
  arc_type: positive_change      // or: flat, corruption, fall, disillusionment
  desired_state_start: "Defers to her family in every decision."
  desired_midpoint_state: "Begins to act without asking permission."   // optional
  desired_state_end: "Chooses for herself, against her family's wishes."
}
```

`arc_type` is one of the five structural arcs (aliases like `redemption`,
`growth`, `steadfast` are accepted; any other string falls back to a generic
probe). The midpoint is optional. Only the **start** and **end** are required to
get checks.

## The two passes

**Agency** is deterministic and zero-AI. For each chapter a character appears
in, Inkhaven scans their sentences and scores `active / (active + passive)`
presence: a name *before* an action verb (and no other character between) reads
as active; a name *after* the verb, or the subject of a passive construction,
reads as passive. A character whose agency collapses mid-book is being acted
*upon* — useful to see, deliberate or not. It runs in all five languages
(EN/RU/DE/FR/ES), keyed off your project language.

**State extraction** is the LLM pass. Chapter by chapter, with the previous
chapter's summary fed forward, it summarises what the character's behaviour,
speech, and reactions *demonstrate* — observable state only, never invented
psychology. It's content-hash lazy: editing one chapter re-extracts from there
forward, nothing else. The chain is enriched with DIALOG-1 utterance/hedge and
NARR-1 interiority signals when those stores exist.

## Run it

```sh
inkhaven character refresh          # agency (instant) + state extraction (LLM)
inkhaven character check            # arc-completeness checks → exit 1 / 2
inkhaven character plan             # Planning-Board coverage gaps (instant)
inkhaven character arc <name>       # the full report for one character
```

`check` runs four LLM checks per declared arc — start / midpoint / end
alignment and *arc earned* (is the ending prepared, or asserted?) — plus a
deterministic **stall** check (a long run of unchanged chapters). It exits **2**
if the ending or the earned-arc check fails, **1** on any other gap or stall, so
it gates a pre-submission CI run. `plan` exits **1** if a declared arc has no
scene card, sits only in the first half, or never reaches the final act.

## In the TUI

- **`Ctrl+V Shift+N`** opens the **character arc view** for the nearest
  character (one named in the open paragraph, else the first tracked one): the
  declaration, the chapter state chain (✦ marks a change, with each chapter's
  agency), the arc checks, and any planning gaps. Read-only; `↑↓` scroll, `Esc`.
- The **`Ctrl+B Shift+C`** review pass folds in the deterministic layers
  (agency + planning gaps + stalls) and surfaces the cached arc-check problems
  under the Output `character` category — zero-cost, no provider needed. The LLM
  checks stay explicit on the CLI.

## Scripting

Bund exposes read-only words over `char.duckdb`:
`ink.char.arc`, `ink.char.stalls`, `ink.char.checks`, `ink.char.plan`,
`ink.char.refresh` (the last two recompute only the deterministic cache).

## Tuning

The `char:` config block (all optional) sets the stall threshold, the agency
windows, the minimum chapters before LLM checks run, the enrichment toggles, the
language override, and `extra_action_verbs` for genre verbs the agency scorer
should treat as deliberate action. See [`../CONFIGURATION.md`](../CONFIGURATION.md).

It tracks the bend in the line. Where the arc should go is always your call.
