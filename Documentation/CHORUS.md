# Voice & Style at Book Scale (CHORUS)

*(2.1, RFC CHORUS-1 — see [`PROPOSALS/CHORUS-1_PLAN.md`](PROPOSALS/CHORUS-1_PLAN.md)
and [`PROPOSALS/CHORUS-1_IMPL.md`](PROPOSALS/CHORUS-1_IMPL.md))*

Inkhaven already measures the **narrator's** voice book-wide — sentence rhythm,
lexical diversity, hedging, interiority (NARR-1, `inkhaven prose`, see
[`PROSE_VOICE.md`](PROSE_VOICE.md)). CHORUS measures the rest of what makes a
book's voice hold together:

> **NARR-1 profiles the narrator. CHORUS profiles the cast, enforces the rules of
> the game, and coaches.**

Three measurement pillars, synthesised by a new inner-family reader — all
**advisory**: CHORUS measures and reports, it never edits your prose.

---

## Pillar A — Character voice

Every character's dialogue is profiled with the *same* metric engine that
profiles the narrator, so a character's voice is measured on the same axes.

- **Voice fingerprints** — each character's attributed dialogue (from DIALOG-1)
  run through the NARR-1 metrics: rhythm, lexical diversity, hedging, interiority.
- **The distinctiveness matrix** — do any two characters read *identically*? Each
  voice is z-scored across the cast (genre-relative — the baseline is your own
  book's spread) and compared; pairs that read alike are flagged. The headline
  finding: *"Mara and Joren read identically."*
- **Per-character drift** — does a character sound like themselves across their
  arc? Measured against their *first* chapter.

Sparse speakers (a handful of lines) are profiled but never *flagged* — CHORUS
reports a confidence and refuses to judge a voice it can't measure.

```
inkhaven chorus voices [--book NAME] [--character NAME] [--json]
```

A signature card per character (rhythm / diversity / hedging / interiority, a
confidence badge, and a Δ-from-cast-mean), followed by the distinctiveness
summary and any voice-drift rows. Deliberate look-alikes (twins, a uniform
chorus) are silenced with `chorus.distinct_ignore_pairs`.

---

## Pillar B — POV & tense discipline

The two classic discipline errors that survive line edits because they're
structural.

- **Head-hopping** — a named character other than the scene's POV shown accessing
  their own inner life (`Joren wondered…` in a Mara-POV scene). A scene's POV is
  **declared** with a `pov:<name>` paragraph tag, or inferred as the
  most-mentioned character. Heuristic by necessity (there is no parser in the
  tree): it catches interiority attributed to a *named* subject, not pronoun
  antecedents (`she thought` where "she" isn't the POV needs antecedent
  resolution CHORUS deliberately doesn't attempt).
- **Tense slips** — a manuscript that lapses out of its established tense. Each
  narration sentence is classified past/present from copula/auxiliary anchors;
  the scene's dominant tense is the majority; sentences that break it are flagged.

### Declaring a scene's POV

Tag any paragraph in the scene:

| Tag | Meaning |
| --- | ------- |
| `pov:Mara` | single POV — Mara. Anyone else's interiority is a leak. |
| `pov:first` | first person — any *named* character's interiority is a leak. |
| `pov:omniscient` (`pov:multi`) | deliberately multi-POV — head-hop off. |

Undeclared scenes infer the POV from mention counts.

### The tense gate — EN/DE/FR/ES, Russian excluded

Tense-slip detection covers **English, German, French, and Spanish** — languages
that share the "keep one narrative tense" convention, each with its own
copula/auxiliary anchors and past-suffix markers. **Russian is excluded by
design**: its narrative tense is governed by *aspect* — the historical present
and perfective/imperfective interleaving are legitimate devices, not slips — and
nothing in the tree models aspect, so a past→present heuristic would be *wrong*
for Russian. `chorus scan` says so plainly rather than false-flagging. (Character
voice and head-hop **do** work in Russian.)

```
inkhaven chorus scan [--book NAME] [--json]
```

Per-scene POV / head-hop findings and per-scene tense slips (or the Russian
"not analysed" notice), plus the register drift below.

---

## Pillar C — Register & diction

The narrator's **register** — formal or plain, contracted or measured, plain or
archaic — tracked per chapter so *drift* becomes visible ("the prose gets casual
in Act III"). A word-list bundle: contraction rate, archaism density, a
formality balance, and an English latinate-diction proxy. Chapters that drift
from the opening beyond `chorus.register_drift_threshold` are flagged (part of
`chorus scan`). Solid for EN/RU; sparser languages degrade to what their lists
cover rather than guessing.

---

## The Inner Stylist — the coach

The seventh inner-family reader (alongside the Editor, Socrates, Theologian,
Poet, Rigor, Grounding). It doesn't measure — it **synthesises**: it reads all
three pillars and turns the numbers into a few grounded **Praise / Note /
Concern** observations, and — on the slow track — LLM coaching in the
inner-family voice (*"I notice…"*, never a rewrite).

- **In the editor** — the review pass **`Ctrl+B Shift+C`** includes the Inner
  Stylist (its observations land in the Output pane), and the family hub
  **`Ctrl+B J → Y`** opens its overview: **`F`** synthesises to Output, **`E`**
  engages the AI coach into the Thoughts pane, **`R`** opens the report dashboard.
- **On the CLI**:

```
inkhaven chorus stylist [--book NAME] [--json]         # Praise/Note/Concern
inkhaven chorus stylist --coach                         # grounded LLM coaching
inkhaven chorus stylist --suppress <key>                # silence a finding
inkhaven chorus report                                  # the full dashboard
```

Each finding carries a stable **key**; `--suppress <key>` silences it for good
(persisted in `inner_stylist.db`, this reader's own store). `chorus report` is
the one-screen dashboard: the narrator profile, the cast voices +
distinctiveness, and the Stylist's synthesis together.

---

## Multilingual

CHORUS keys off the project language and is honest about coverage. Character
voice + distinctiveness + drift work in every language (rhythm/diversity) with
the language-sensitive axes filling in for EN/RU/DE/FR/ES. Head-hop reuses the
per-language interiority markers (**including Russian**). Register is solid for
EN/RU. The **tense** check covers EN/DE/FR/ES (Russian excluded), and it says so. The Inner
Stylist coaches in the book's language.

---

## Configuration

```hjson
chorus: {
  distinct_threshold: 0.5           // RMS z-distance below which two voices read alike
  distinct_ignore_pairs: []         // ["Mara|Joren"] — deliberate look-alikes
  register_drift_threshold: 0.08    // register change vs ch.1 that flags a drift
}
stylist: {
  enabled: true
  session_budget: 0.15              // informative LLM budget (never blocks)
  language: null                    // coaching-language override (default: project)
}
```

All knobs inform and cap — per Inkhaven's permissive principle, they never block.

---

## What CHORUS is *not*

- Not a style *corrector* — it flags, it never rewrites.
- Not a grammar checker — there's no parser; the tense check is an honest,
  EN/DE/FR/ES heuristic (Russian excluded).
- Not a "good writing" score — it measures *consistency and distinctiveness*, not
  quality. Statistical voice ≠ literary voice: every surface states its limits.

---

## Not yet wired

A Bund `ink.chorus.*` scripting surface is the natural remaining step (the CLI +
the in-editor overview/report/engage cover the interactive workflows). Tense
covers EN/DE/FR/ES; other languages beyond Russian are simply not built yet.
