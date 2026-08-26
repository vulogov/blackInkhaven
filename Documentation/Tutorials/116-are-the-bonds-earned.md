# Tutorial 116 — Are the Bonds Earned?

*Inkhaven 3.1 (BONDS)*

You write two characters as inseparable friends. Three hundred pages later a reader
shrugs at their big reunion — because, though you *told* us they were close, they were
never once on the page together. Or your antagonists soften into allies over a single
chapter break, with no scene to turn it. These are relationship plot-holes, and they're
invisible from inside the draft. BONDS is KEN's sibling: not *what a character knows*,
but *how two characters relate* — and whether the prose earns it.

## Declare the bond

BONDS never guesses how characters feel. You declare the relationships that matter with a
tag on any paragraph in the scene:

- `rel:ally:mara:kell` — Mara and Kell are allies here.
- `rel:enemy:mara:sella` — Mara and Sella are enemies.
- `rel:lover:kell:danel` — and so on. The `<kind>` is your own vocabulary.

The pair is canonical — `rel:ally:mara:kell` and `rel:ally:kell:mara` are one bond. Tag
the *same* pair with a different kind later and you've declared a **transition**: ally in
chapter 1, enemy in chapter 9.

And BONDS derives the other half — **who actually shares each scene** — for free: the
scene's POV character, anyone named in the prose, and the participants of any timeline
event linked into the scene.

## Run the check

```sh
inkhaven bonds
```

```
Bonds — 2 findings

  ⊗ unearned_shift   Mara & Kell shift ally → enemy (ch. 1 → ch. 9) with no shared scene to turn it
  ● unwritten_bond   Declared ally bond between Mara & Danel, but they share 0 scenes on the page
```

BONDS compares what you **declared** against what the page **dramatises**:

- **`unwritten_bond`** — you declared the bond, but the pair barely shares a scene.
  Asserted, not dramatised.
- **`unearned_shift`** — the bond changes state, but no scene between the two
  declarations puts the pair together to turn it. The hard break.
- **`dropped_bond`** — an established bond goes quiet for many chapters, then resurfaces.

It exits non-zero on an `unearned_shift`, so you can gate a draft in CI. BONDS is
**silent** where you've declared nothing — no `rel:` tags, no work — and the core costs
**nothing** (no model; it scales with your tags, not your page count).

## See the model

`inkhaven bonds` reports the *breaks*; `inkhaven bonds --ledger` reports the *model* it
checked against — every declared `rel:` bond grouped by character pair, in reading order,
transitions and all. It's the mirror of `inkhaven knowledge --ledger`: before you trust
the findings, read back what you actually declared. `--json` gives the same model as
structured output for a script.

## Jump to the slip

In the editor, **`Ctrl+V Shift+O`** opens the relationship dashboard: the findings
grouped by kind. `↑↓` scroll (`PgUp`/`PgDn` page, `Home`/`End` to the ends; blank
separators are skipped, so **Enter** always lands on a real row), **Enter** jumps
straight to the paragraph. The same
findings ride the `Ctrl+V Shift+R` Editorial Pass, where an `unearned_shift` becomes a
**Decision**: add the scene that earns the turn, or soften the declaration you couldn't
back up.

## Tune the thresholds

In `inkhaven.hjson`:

```hjson
bonds: {
  min_co_presence: 2     # shared scenes below which a declared pair is "unwritten"
  dormancy_window: 6     # chapters a bond may go quiet before a resurfacing is "dropped"
}
```

## When you want the subtle cases

The deterministic check catches faults against your *declarations*. A relationship that
quietly cools on the page — with no tag marking it — needs a reader's judgment. The
opt-in, cost-capped LLM pass supplies it:

```sh
inkhaven bonds --deep
```

It hands the model the declared-bond ledger plus the scenes and asks for implied,
undeclared shifts. Explicit and budgeted — never automatic.

## From a script

```
ink.bonds.check     ( -- dict )  { unwritten, unearned, dropped, clean }
ink.bonds.findings  ( -- list )  the breaks
ink.bonds.ties      ( -- list )  the declared bond ledger
```

`clean` is `true` when no hard break stands.

---

BONDS gives your reader's instinct a memory for relationships: it remembers who you said
was close, and holds the page to it. The full reference is [`BONDS.md`](../BONDS.md).
