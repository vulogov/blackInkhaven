# Reasoning-Rigor Reader (RIGOR)

*(1.6.20+ — the argument-side member of the Inner family, alongside the Inner
Socrates, Inner Editor, Inner Theologian, and Inner Poet)*

The Inner Theologian made a work's claims *grounded* and its citations *validated*;
the Inner Socrates' Dialectician asks the hard question by hand. RIGOR is the piece
that asks, deterministically and at book scale, whether the **arguments themselves
hold**:

> **RIGOR scans manuscript prose for argument-rigor signals — false dichotomy,
> question-begging, straw man, overgeneralization, non-sequitur (and, with the
> Glossary, equivocation) — via language-keyed cue markers, and surfaces each as an
> advisory finding (glyph `⊬`).**

It is **deterministic and free** — no LLM, no network, no persistence — **advisory**
(a cue is a candidate weakness for the author to weigh, never a verdict),
**multilingual**, and adds **no new runtime dependencies**. It is stateless:
findings are recomputed each pass and emitted straight to the Output pane (or
stdout).

---

## What it reads

Every prose paragraph under a user book, in reading order by chapter. Each
paragraph's Typst is stripped to plain text, lowercased, and matched against the
prose language's cue tables. Jinja (`content_type: jinja`) paragraphs are skipped.
At most **one finding per category per paragraph** (the first matched cue), so a
dense paragraph never floods the pane. There is no state on disk — the reader reads
the manuscript files and reports.

---

## What it detects

Six categories, each keyed to a conservative table of strong, rarely-innocent cues
(a false positive costs a glance; noise erodes trust, so the lists stay lean):

| Category | Signal | Cue shape |
| -------- | ------ | --------- |
| `false-dichotomy` | a forced exhaustive binary | correlative *"either … or"* pair, or a forced-binary phrase (*"the only alternative"*, *"one or the other"*) |
| `question-begging` | a claim asserted as self-evident instead of argued | assertion boosters (*"obviously"*, *"of course"*, *"needless to say"*) |
| `straw-man` | a view characterized dismissively | dismissive framings (*"so-called"*, *"would have us believe"*, *"simplistic"*) |
| `overgeneralization` | an unqualified universal | strong absolutes (*"always"*, *"never"*, *"without exception"*) — deliberately **not** the innocent *"all"* / *"every"* |
| `non-sequitur` | a conclusion drawn with no visible warrant | a conclusion connective (*"therefore"*, *"thus"*, *"hence"*) present **while no warrant marker** (*"because"*, *"since"*) appears anywhere in the paragraph |
| `equivocation` | a term silently shifting between its senses | a Glossary term declared with **≥2 senses** and `watch_equivocation`, used **≥2 times** in one paragraph without pinning a sense |

Each finding carries a localized advisory sentence that names the matched cue and
poses the question the author should answer — e.g. *"A universal claim (\"never\") —
would a single counterexample break it? If so, qualify it."*

The `equivocation` category is the one that needs more than prose: it projects from
the scholarly Glossary, counting the watched multi-sense terms' surface forms
(canonical + synonyms). With no such Glossary entries it is simply inert.

---

## The command line

```
inkhaven rigor scan [--book NAME] [--signal CODE] [--json] [--strict]
```

Runs the reader across one user book (default: the resolved user book) and prints
each signal as `⊬ [ch.N · label] advisory…`. `--signal` filters to a single
category code (`false-dichotomy` | `question-begging` | `straw-man` |
`overgeneralization` | `non-sequitur` | `equivocation`) or `all` (the default);
`--json` emits an array of `{signal, chapter, para_id, description}`.

By default the command **exits 0** — it is advisory. `--strict` makes **any** signal
a non-zero exit, so it drops into CI as a pre-submission gate.

---

## Configuration

The `rigor:` block (all fields optional; the shown values are the defaults):

```hjson
rigor: {
  enabled: true            // master switch
  fast_track: false        // join the review pass / deep-refresh ambient surface
  language: null           // marker-language override (en/ru/de/fr/es); null → project language → English
  false_dichotomy: true    // per-category toggles
  question_begging: true
  straw_man: true
  overgeneralization: true
  non_sequitur: true
  equivocation: true       // needs Glossary entries with ≥2 senses + watch_equivocation
}
```

Turning a category off silences it everywhere; turning `enabled` off gates the whole
reader.

---

## Multilingual

First-class in **English, Russian, German, French, and Spanish** — each language
provides its own cue tables *and* its own advisory sentences (the reader answers
*"does it work in Russian?"* with localized Cyrillic findings that name the matched
Cyrillic cue). Any other project language falls back to the English tables. Matching
is Unicode-aware and whole-word: single tokens match on word boundaries (so *"art"*
never fires inside *"start"*), multi-word and hyphenated cues match as bounded
substrings. Spanish declares no *"either … or"* correlative pair (it has no clean
one), so false-dichotomy there rests on the forced-binary phrases alone — the reader
never claims coverage it does not have.

---

## What it is not

- Not an LLM reader — the whole thing is deterministic cue-matching; it costs
  nothing and needs no provider.
- Not a verdict — every finding is a candidate weakness the author weighs, in the
  tradition of the Inner Socrates made deterministic.
- Not a corrector — it flags, it never rewrites your prose.
- Not stateful — nothing is persisted; each scan recomputes from the manuscript.
