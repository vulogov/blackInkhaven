# Tutorial 55 — Plot mining and worldbuilding consistency

*Inkhaven 1.2.16+*

The 1.2.16 cycle added four new author-judgment
detector classes to `inkhaven doctor --scan` /
`Ctrl+B Shift+0`:

| Class | What it catches | Source |
|-------|------------------|--------|
| `dropped-character` | character in first 30 % of chapters, absent from last 30 % | Characters book × manuscript prose |
| `pacing-collapse` | chapter > 3× or < 0.3× trailing 5-chapter mean | chapter word counts |
| `stalled-thread` | thread with newest waypoint > 30 days old (or empty) | Threads book |
| `naming-inconsistency` | near-miss spelling of a canonical multi-word name | Characters/Places/Artefacts × prose |

All four are **Info severity** — they're
author-judgment findings, not data-loss errors.
`--autofix` returns a clear "no autofix —
review the prose / outline / threads" message;
the doctor surfaces them, you decide what to do.

## Running the detectors

Same surface as every other doctor class:

```bash
$ inkhaven --project ~/Books/my-novel doctor --scan
Project scan
  findings : 3

  [1]    info · dropped-character     · -
         character `Lyra` first appears in chapter 3 (of
         18) but is absent from the last 30% (last seen
         chapter 6)
  [2]    info · pacing-collapse       · -
         chapter `The Wedding` (1843 words) is notably
         shorter than the trailing 5 chapters (mean
         6210, ratio 0.30×)
  [3]    info · naming-inconsistency · -
         near-miss `Aerin Stormbreaker` in prose vs.
         canonical `Aerin Stormbringer` (edit distance 3)
```

Single-class filters work too:

```bash
$ inkhaven doctor --scan --class dropped-character
$ inkhaven doctor --scan --class pacing-collapse
$ inkhaven doctor --scan --class stalled-thread
$ inkhaven doctor --scan --class naming-inconsistency
```

For CI gates, the `--json` output makes scripting
straightforward.  These classes are Info, so they
don't trip the default exit-2-on-Warning-or-above
behaviour — but you can opt into stricter gating
by parsing the output yourself:

```bash
$ inkhaven doctor --json | jq '[.findings[] | select(.class == "naming-inconsistency")] | length'
2
```

## How `dropped-character` works

Walks the Characters system book for entry titles.
For each character:

1. Finds the **first chapter** that mentions the
   character's name (case-insensitive substring
   match in chapter prose).
2. Finds the **last chapter** that mentions it.
3. Flags when `first < 30 % of total chapters`
   AND `last < 70 % of total chapters` — i.e.
   the character was introduced early but
   doesn't show up in the third-act resolution.

Skipped on projects with fewer than 5 chapters
(too few to apply the heuristic meaningfully).

**Recurring false positive:** a character whose
canonical name is a common English word
("Hope", "Page"), the prose-scanner will match
literal occurrences too.  Rename the system-
book entry to something more distinctive
(`Hope (the engineer)`) or accept the finding
and move on.

## How `pacing-collapse` works

Walks user-book chapters in order.  For each
chapter from index 5 onward, computes the
mean word count of the trailing 5 chapters.
Flags the current chapter when its word count
is:

* greater than 3× the trailing mean (a
  suspiciously long outlier), OR
* less than 0.3× the trailing mean (a
  suspiciously short one).

The trailing window is short on purpose — a
24-chapter manuscript's structural rhythm
shifts as you move through the book, and you
care more about "this chapter vs. its
neighbours" than "this chapter vs. the entire
manuscript".

**Common legitimate trigger:** an *epilogue*
chapter that's deliberately ⅓ the length of
the action chapters before it.  Read the
finding and dismiss it.

## How `stalled-thread` works

Walks the Threads system book.  For each
thread chapter, finds the newest waypoint
paragraph's file mtime.  Flags when:

* the newest mtime is > 30 days old, OR
* the thread has waypoints with unreadable
  mtimes (a sign of corruption — defer to the
  1.2.15 health monitor's tree-integrity
  check), OR
* the thread has zero waypoints at all (you
  declared the arc but never plotted it).

Mirrors the 1.2.14 `inkhaven thread doctor`
dormant detector — both surfaces report on the
same threshold so doctor scans, doctor panel,
and the manuscript intelligence dashboard
(`Ctrl+V Shift+J`) all agree.

## How `naming-inconsistency` works

The most carefully-tuned of the four
detectors — false positives in this class are
particularly noisy.

Steps:

1. Walks the Characters / Places / Artefacts
   system books.  Collects every entry title
   that has **2 or more words** (single-word
   names like "Aerin", "Aragorn" are skipped
   — too many natural variants).
2. For each canonical multi-word name (e.g.
   `Aerin Stormbringer`):
   * If the literal canonical name appears
     anywhere in prose, skip it.
   * Otherwise, scan prose for occurrences of
     the canonical's **first word** (`Aerin`)
     followed by any next word at a word-
     boundary (so `Aerinet` doesn't match
     the prefix).
   * Compute the **Levenshtein distance**
     between the candidate next word and the
     canonical's expected next word
     (`Stormbringer`).
   * Flag when distance > 0 AND distance ≤
     50 % of the longer string's length.

The 50 % cap lets the detector catch
`Stormbreaker` (3 edits over 12 chars = 25 %)
while rejecting `Stormblade` (closer to 50 %
but still a deliberate-looking alternate).

Trailing punctuation is stripped from the
candidate so `Stormbreaker,` matches the same
as bare `Stormbreaker`.  Same variant
appearing multiple times in prose only fires
one finding.

**When you see a finding:**

* If it's a typo, fix the prose.
* If it's a deliberate variant ("Aerin the
  Stormbringer" sometimes vs. "Aerin
  Stormbringer" most times), rename your
  Characters book entry to the **longer**
  canonical form so the shorter version
  doesn't generate matches.

## Worldbuilding glossary chip

While we're talking worldbuilding density: the
1.2.16 cycle also added a status-bar chip
showing `<N>C·<N>P·<N>A` — cumulative counts
of Characters / Places / Artefacts entries.

Visible to the right of the POV chip.  Auto-
hides on fresh projects (all three counts
zero).  Toggle via HJSON:

```hjson
{
  editor: {
    show_glossary_chip: false  // default true
  }
}
```

Useful at-a-glance check: 30 chapters in with
only 4 named characters tells you the cast is
thin; 50 named places across a 200-paragraph
manuscript tells you the reader has a thicket
to navigate.

## TUI integration

Same `Ctrl+B Shift+0` modal as the existing
1.2.15 doctor scan classes:

* The four new classes appear in the same
  list, sorted by severity-then-class.
* `↑↓` navigate · `Esc` close · `r` *attempts*
  repair (returns the "no autofix" message
  for these classes — it's expected; the
  modal still works fine).
* The status bar's `[health: …]` chip stays
  unaffected — these are doctor findings, not
  health-monitor findings.

## See also

* [Tutorial 52 — Health monitor and doctor scan](52-health-and-doctor.md)
  — the doctor scan + autofix surface these
  new classes plug into.
* [Tutorial 54 — Manuscript intelligence dashboard](54-manuscript-intelligence-dashboard.md)
  — the dashboard's Threads / Pacing sections
  align with the new detectors.
* `Documentation/RELEASE_NOTES/1.2.16.md` —
  Phase A.6 + A.5 implementation logs.
