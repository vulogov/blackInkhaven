# Are the Bonds Earned? (BONDS)

*(3.1, RFC BONDS-1 — see [`PROPOSALS/BONDS-1_PLAN.md`](PROPOSALS/BONDS-1_PLAN.md) and
[`PROPOSALS/BONDS-1_IMPL.md`](PROPOSALS/BONDS-1_IMPL.md))*

Inkhaven watches several kinds of continuity — where and when (SENTINEL), whether a
character *knows* something yet (KEN), whose head we're in (CHORUS POV), the emotional
arc of one character (CHAR-1). **None of them watch the thing readers feel most: how two
characters *relate*, and whether the page earns it.**

> **BONDS is KEN's sibling.** Where KEN checks *knowledge* (who could know what, when),
> BONDS checks *bonds* (how two characters relate, and whether the prose backs it up).
> A declared friendship that never shares a scene, an ally who becomes an enemy with no
> turn on the page, a marriage that vanishes for ten chapters — these are the invisible
> relationship plot-holes.

It is **deterministic and free** at the core (co-presence is *derived* from structure,
not guessed), **advisory** (it flags, it never edits prose), and produces a finding a
generic AI **cannot**: judging whether a relationship is earned across a whole book needs
the timeline, the event-participant lists, the character bible, and scene POV that only
inkhaven maintains.

---

## The bond — declared, then checked

BONDS never guesses how characters feel. You **declare** a bond; inkhaven **derives**
whether the page earns it. The two halves:

- **Declared** — you tag a paragraph with `rel:<kind>:<A>:<B>` — e.g.
  `rel:ally:mara:kell`, `rel:enemy:mara:sella`, `rel:lover:kell:danel`. `<kind>` is your
  own vocabulary (ally, enemy, kin, lover, rival, mentor…); the pair is canonical, so
  `rel:ally:mara:kell` and `rel:ally:kell:mara` are one bond. A later, differently-kinded
  tag for the same pair is a **transition** (ally → enemy).
- **Derived co-presence** — inkhaven works out, for free, which characters share each
  scene: the union of the scene's **POV** character, any roster character **named** in the
  prose (Unicode-aware, like KEN), and the **participants of any timeline event** linked
  into the scene. A generous union — a false "present" only makes BONDS *quieter*, the
  safe bias for an advisory reader.

The **mismatch between the two is the finding.**

---

## What it catches

| Finding | Severity | The break |
| ------- | -------- | --------- |
| `unwritten_bond` | Notice | a declared pair barely (or never) shares a scene — "you declare Mara & Kell allies, but they're never on the page together": asserted, not dramatised |
| `unearned_shift` | **Break** | a declared bond changes state with no shared scene to turn it — "allies in ch. 1, enemies in ch. 9, and they never meet between": the relationship plot-hole |
| `dropped_bond` | Notice | an established bond goes dormant for a long stretch, then resurfaces — "inseparable through ch. 3, gone until ch. 12" |
| `implied_cooling` *(opt-in `--deep`)* | advisory | a relationship that warms or cools on the page with **no** `rel:` tag marking it — the subtle, undeclared drift the deterministic layer can't see |

```
Bonds — 2 findings

  ⊗ unearned_shift   Mara & Kell shift ally → enemy (ch. 1 → ch. 9) with no shared scene to turn it
  ● unwritten_bond   Declared ally bond between Mara & Danel, but they share 0 scenes on the page
```

---

## The command line

```
inkhaven bonds                 # the deterministic check (non-zero exit on a break)
inkhaven bonds --json          # the findings as JSON
inkhaven bonds --deep          # + the opt-in, cost-capped implied_cooling pass
```

`bonds` exits non-zero on any hard break (`unearned_shift`) — a CI gate, like
`knowledge` / `continuity check`. It is **self-gating**: no `rel:` tags → nothing to
check, no cost.

---

## In the editor

- **The dashboard** — **`Ctrl+V Shift+O`** opens the relationship findings grouped by
  kind; `↑↓` scroll, **Enter** jumps to the offending paragraph, `Esc` closes.
- **The revision worklist** — bond findings ride the `Ctrl+V Shift+R` Editorial Pass (a
  `bonds` source): `unearned_shift` routes to a guided **Decision** (*add the scene, or
  soften the declaration?*); `unwritten_bond` / `dropped_bond` stay advisory **Briefs**.
  BONDS never rewrites prose.

---

## Tuning it

The `bonds` config block (see [`CONFIGURATION.md`](CONFIGURATION.md)):

```hjson
bonds: {
  enabled: true          # master switch for the review-pass line
  min_co_presence: 2     # shared scenes below which a declared pair is "unwritten"
  dormancy_window: 6     # chapters a bond may go quiet before a resurfacing is "dropped"
}
```

`enabled` governs only the review-pass line (`inkhaven edit` / the Editorial Pass); the
standalone `inkhaven bonds` and the dashboard always run.

---

## Cost — a design invariant

The whole core is a forward walk + set membership + Unicode mention matching — **no
model, ≈ $0, independent of book length**. Cost scales with declared bonds and scenes,
not pages. The **only** LLM touchpoint is the opt-in `--deep` `implied_cooling` pass,
cost-capped under the daily cap on the world fact-checker's rail — never automatic,
never whole-book. There is deliberately no "have the AI judge every relationship" pass.

---

## From a script

```
ink.bonds.ties     ( -- list )  the declared bond ledger {a, b, kind, chapter}
ink.bonds.findings ( -- list )  the deterministic breaks {kind, severity, chapter, a, b, message}
ink.bonds.check    ( -- dict )  {unwritten, unearned, dropped, clean}
```

Read-only. `check.clean` is `true` when no hard break (`unearned_shift`) stands — a
pre-submit gate. The `--deep` pass is not exposed to Bund (it costs).

---

## Multilingual

BONDS inherits SENTINEL's and KEN's language coverage: the mention matcher is
Unicode-aware (Cyrillic, accents, and Latin match alike), and co-presence draws on
timeline participants and POV, which are language-agnostic. Character names are matched
in the project's language.

---

## What it is not

- Not an all-knowing oracle — it reasons only over what it can ground (`rel:` tags, POV,
  named mentions, event participants) and stays silent where it can't, rather than
  inventing bonds.
- Not KEN — KEN asks *could this character know this yet*; BONDS asks *is this
  relationship earned on the page*. Different axis.
- Not a sentiment analyzer — the core never reads emotion from prose; it measures
  declared-vs-derived. The opt-in `--deep` pass is the only place a model reads the page.
- Not a rewriter — it flags; the `Ctrl+V Shift+R` Editorial Pass owns any edit.
