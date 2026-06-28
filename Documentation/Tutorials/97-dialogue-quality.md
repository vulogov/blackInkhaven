# Tutorial 97 — Dialogue Quality & Attribution

*Inkhaven 1.4.14*

Dialogue is the most technically demanding prose mode: readers must always know
who is speaking, said-bookisms (`he ejaculated`) degrade the page, and an
unbroken run of talking heads loses the physical scene. Inkhaven's Inner
Socrates flags *unattributed* dialogue in passing; nothing measured dialogue as
a mode with its own properties. DIALOG-1 does — **deterministically, with no AI,
no parser, and no external dependency**, in five languages (EN/RU/DE/FR/ES).

## Scan a book

```sh
inkhaven dialogue scan
```

```
Dialogue scan — `The Sunken Throne` [en]
──────────────────────────────────────────────────────────────────
  dialogue spans detected   34
  zero attribution          3
  talking-head sequences    1
──────────────────────────────────────────────────────────────────
  [ch.12 · 7f1a…] unattributed speech — no tag or character name within range
  …
──────────────────────────────────────────────────────────────────
```

`scan` **exits non-zero** if any zero-attribution span is found, so it doubles
as a pre-submission gate:

```sh
inkhaven dialogue scan --findings zero-attribution || echo "fix attribution first"
```

Three findings, all deterministic:

- **Zero-attribution** — a speech span with no tag or nearby character name. An
  established two-speaker exchange is allowed to alternate untagged for a run
  (default 8 turns) before it fires — readers track that fine.
- **Said-bookism density** — a chapter whose non-neutral tag verbs (`whispered`,
  `growled`) run above the book's own baseline by more than a threshold.
- **Talking heads** — a run of dialogue-only paragraphs with no action beat (a
  ≥8-word line of narration) to ground the scene.

## Three quotation conventions

Detection is **not** uniform across languages — that's the heart of it:

- **EN, DE** — paired quotation marks (`"…"`, `„…"`, `»…«`).
- **FR, RU** — guillemets `«…»` and em-dash openers (`— …`); the French inline
  *incise* (`, dit-il,`) is stripped from the speech before it's measured.
- **ES** — all three, detected additively and de-duplicated.

## Per-character fingerprints

```sh
inkhaven dialogue profile
```

```
Character    Utts  Avg words  MATTR  Questions  Excl.  Hedging
Mara           47      11.3   0.74      0.31    0.08    0.019
Aldric         83      11.8   0.69      0.12    0.22    0.041
Seren          31       7.1   0.61      0.44    0.04    0.009
```

Each character gets a six-metric voice signature built from the lines confidently
attributed to them: are they terse or expansive, do they ask or declare, do they
hedge or assert? In the editor, **`Ctrl+V Shift+Q`** opens the fingerprint view
for the nearest character (one named in the paragraph you're in, else the
most-speaking) as ASCII bars with a compare line.

## In the editor

The dialogue check rides the **unified review pass** — **`Ctrl+B Shift+C`** runs
it alongside the fact / Socratic / timeline checkers, and the findings land in
the Output pane (navigable to the flagged paragraph). It's zero-AI and
hash-lazy, so only the chapter you just edited is re-detected.

There's also a new Inner Socrates persona, **`theatergoer`** (`Ctrl+B J → S` to
cycle to it): it reads a dialogue scene as if the narrator's private glosses
were invisible, and asks whether the subtext is legible from the words and
visible action alone.

## Tune it

The optional `dialogue:` block in `inkhaven.hjson` tunes the windows and
thresholds, and — importantly for SF/fantasy — lets you mark genre speech verbs
as neutral so they aren't counted as said-bookisms:

```hjson
dialogue: {
  talking_head_threshold: 6
  said_bookism_threshold: 0.15
  extra_neutral_verbs: ["transmitted", "sent"]   // telepathy isn't a bookism
}
```

## From Bund

`ink.dialogue.stats`, `ink.dialogue.fingerprint`, `ink.dialogue.violations`,
`ink.dialogue.spans`, `ink.dialogue.refresh` — all read-only (refresh writes
only the derived cache). Profiles live in `.inkhaven/dialogue.duckdb`.

It measures; it never prescribes.
