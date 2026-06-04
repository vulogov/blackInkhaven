# Tutorial 59 — Revision & continuity

*Inkhaven 1.2.19+*

First drafts are about getting the words down.  Revision
is about catching the things that slipped in while you
weren't looking: a word echoed three times in a
paragraph, a journey that's "three days" in one chapter
and "a week" in the next, a character whose eyes change
colour, a mystery you set up and forgot to pay off.

1.2.19 adds four detectors for exactly these, all
**multilingual**, all surfaced through
`inkhaven doctor --scan` (and the `Ctrl+B Shift+0` doctor
panel) as Info-severity, author-judgment findings — they
point, you decide.

## The four detectors

| Detector | Catches | How it works |
|----------|---------|--------------|
| `echo-repetition` | a distinctive word reused close together | stem + window |
| `numeric-contradiction` | reversed directions / mismatched durations | per-language quantity lexicon |
| `continuity-drift` | a character attribute that changes across chapters | AI extraction + stem comparison |
| `unresolved-tension` | a tension set up but never paid off | AI tagging + stem matching |

Three run in the default scan; `unresolved-tension` is
opt-in (see below).

## Echo detector

```bash
$ inkhaven doctor --scan --class echo-repetition
  [1]  info · echo-repetition · -
       echo: `lantern` appears 3× within ¶4–6 (chapter `The Lantern Room`)
```

Flags a *distinctive* word (rare in the manuscript's own
vocabulary, not a stop-word) reused
`editor.echo_min_repeats` times within
`editor.echo_window` paragraphs.  Inflected forms
collapse — "walked / walking / walked" counts as three
of `walk`.

The distinctiveness ceiling (`editor.echo_max_global`,
default 40) skips common vocabulary: a word you
legitimately use 200 times across the book isn't an
echo, even when it clusters.  Tune it up for long works,
down for short stories.

```hjson
{ editor: { echo_window: 5, echo_min_repeats: 3, echo_max_global: 40 } }
```

## Numeric / temporal / spatial contradictions

```bash
$ inkhaven doctor --scan --class numeric-contradiction
  [1]  info · numeric-contradiction · -
       direction reversal: `200 leagues north` vs `200 leagues south` — review whether these refer to the same thing
  [2]  info · numeric-contradiction · -
       duration mismatch: `three day` vs `a week` — review whether these refer to the same thing
```

Two deliberately-narrow checks:

* **Direction reversal** — a directed distance reversed
  at the same magnitude close together ("200 leagues
  north" … "200 leagues south").
* **Duration mismatch** — two different durations in the
  same or adjacent sentence ("the three-day journey,
  after a week of travel").

It extracts quantities — digits (`200`), number-words
(`three`, `twenty-five`, `two hundred`), units, and
directions — into a normalised form, then compares.
Directions are only read when attached to a magnitude,
so a character legitimately walking north then south
doesn't false-flag.

## Continuity bible

The headline: turn the Characters book into a living
continuity checker.  A two-step flow.

**Step 1 — extract** (uses the configured LLM):

```bash
$ inkhaven continuity extract
inkhaven continuity extract · language: English · model: gpt-… · 18 chapter(s)
  [1/18] Chapter 1 ... → 6 fact(s)
  …
continuity: extracted 94 fact(s) for 12 character(s) → .inkhaven/continuity.json
```

The AI records each character's established facts
(appearance, origin, relationships, possessions,
occupation) per chapter.  Inspect them:

```bash
$ inkhaven continuity list
Helena
  eye_color   green        [Chapter 1]
  eye_color   brown        [Chapter 9]   ← drift
  hometown    the Harbor   [Chapter 1]
```

**Step 2 — flag drift:**

```bash
$ inkhaven doctor --scan --class continuity-drift
  [1]  info · continuity-drift · -
       continuity drift: `Helena`'s `eye_color` changes across chapters — Chapter 1: green; Chapter 9: brown
```

An attribute that takes different values across chapters
is flagged.  Inflected restatements ("green eyes" vs
"green eye") don't false-flag — values are compared
through the project's stemmer.  An attribute *can*
legitimately change (an injury, dyed hair) — it's a
review prompt, not an error.

> Re-run `continuity extract` after major revisions to
> refresh the bible; it overwrites the sidecar.

## Unresolved tension (opt-in)

A tension introduced but never paid off — the gun on the
mantel that never fires.  Also two-step.

**Step 1 — tag** (uses the LLM):

```bash
$ inkhaven tension scan
  [1/18] Chapter 1 ... → 3 introduced, 1 resolved
  …
tension: tagged 41 tension(s) across 18 chapter(s)
```

**Step 2 — flag the open ones:**

```bash
$ inkhaven doctor --scan --class unresolved-tension
  [1]  info · unresolved-tension · -
       unresolved tension: `the hidden treasure` is introduced in `Ch1` but never paid off — review whether it's a deliberate open thread
```

`unresolved-tension` is **opt-in** — it does *not* run in
the plain `inkhaven doctor --scan`.  You must ask for it
with `--class unresolved-tension`.  Tension is a judgment
call (an open thread can be a deliberate series hook) and
the AI tagging is approximate, so it's kept out of the
routine scan.

## Multilingual coverage

Every detector works beyond English — by design, not as
an afterthought.  Coverage depends on the mechanism each
uses:

| Detector | Mechanism | Languages |
|----------|-----------|-----------|
| `echo-repetition` | Snowball stemmer | **all 18** Snowball languages (English, Russian, French, German, Spanish, Italian, Portuguese, Dutch, …); exact-form fallback for others (Japanese, Chinese) |
| `numeric-contradiction` | quantity lexicon | **English, French, Spanish** bundled; others skip cleanly (Russian + German + a `bootstrap-continuity` seed CLI land in a follow-up) |
| `continuity-drift` | AI + stemmer | extraction in any LLM-supported language; comparison across the 18 Snowball languages |
| `unresolved-tension` | AI + stemmer | tagging in any LLM-supported language; matching across the 18 Snowball languages |

The language comes from your project's `language` field
(or per-paragraph `whatlang` detection when
`editor.prompt_language_mode = "paragraph_detected"`).
A language without a bundled quantity lexicon doesn't
produce garbage — the numeric scan simply skips it.

A stemmer is not a lemmatiser: noun declensions collapse
reliably (ru `корабль`/`корабля`/`корабле`), but a few
irregular verb forms won't.  This is a documented limit
of Snowball stemming, not a bug.

## CI gating

All four are Info severity, so they don't trip the
default `inkhaven doctor --json` exit-2-on-Warning gate.
Opt into stricter gating by parsing the JSON:

```bash
$ inkhaven doctor --scan --class continuity-drift --json \
    | jq '[.findings[]] | length'
```

## See also

* [Tutorial 52 — Health monitor & doctor scan](52-health-and-doctor.md)
  — the doctor-scan surface these classes plug into.
* [Tutorial 60 — Submitting your manuscript](60-manuscript-format.md)
  — once the manuscript is clean, export it in standard
  submission format.
* `Documentation/PROPOSALS/1.2.19_PLAN.md` — the
  three-tier multilingual model.
* `Documentation/RELEASE_NOTES/1.2.19.md` — C.1–C.4
  implementation log.
