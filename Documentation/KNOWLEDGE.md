# Who Knows What, When (KEN)

*(2.6, RFC KEN-1 — see [`PROPOSALS/KEN-1_PLAN.md`](PROPOSALS/KEN-1_PLAN.md) and
[`PROPOSALS/KEN-1_IMPL.md`](PROPOSALS/KEN-1_IMPL.md))*

Inkhaven watches several kinds of continuity — where and when (SENTINEL), whether an
entity exists yet (the `introduce` invariant), whether the world stays consistent
(Facts), the emotional arc (CHAR-1), whose head we're in (CHORUS POV). **None of them
watch the one that breaks plots: what a character *knows*.**

> **KEN is SENTINEL's "referenced-before-introduced" invariant, extended to
> knowledge.** SENTINEL flags an entity *named before it exists*; KEN flags a
> character *acting on a fact before they could know it* — a mystery's cardinal sin,
> and the most common invisible plot-hole.

It is **deterministic and free** at the core (a character's *ken* — the range of what
they know — is derived from structure, not guessed), **advisory** (it flags, it never
edits prose), and produces a finding a generic AI **cannot**: reconstructing
who-knows-what across a whole book needs the timeline, the event-participant lists, the
character bible, and scene POV that only inkhaven maintains.

---

## The grant — when could they know it?

KEN never guesses what a character knows. The *grant* — the earliest point a character
could know a topic — is derived two deterministic ways:

- **Presence** — a character in a timeline **event's participant list** knows that
  event from the moment it happens. Free, from the structure you already keep.
- **Declared** — you mark it with a tag:
  - `secret:<topic>` — declares `<topic>` a secret (a reference by someone ungranted
    is a leak).
  - `know:<topic>` — grants the scene's POV character; `know:<topic>@<name>` grants a
    named character.
  - `reveals:<topic>` — on an event's paragraph, binds a terse event title to the
    matchable topic it reveals.

A *use* — a character referencing the topic — is caught deterministically by matching
it (Unicode-aware, like character names) in the character's **attributed dialogue** or
in their **POV scene's narration**.

---

## What it catches

| Finding | The break |
| ------- | --------- |
| `premature_knowledge` | a character references a topic before their earliest grant — "Bob names the murder in ch. 4; he learns of it in ch. 6" |
| `leaked_secret` | a `secret:` topic referenced by a character never granted it |
| `dropped_reveal` | a declared `know:` reveal whose topic never surfaces again (dangling knowledge — the epistemic `unpaid_setup`) |
| `implied_irony` *(opt-in `--deep`)* | a character *acts* informed/ignorant without naming the topic — the subtle case the deterministic layer can't see |

```
Knowledge check — 2 findings

  ⊗ premature_knowledge  Bob speaks of "the murder" in ch. 4 — before learning it in ch. 6
  ⊗ leaked_secret        Sella references "the heir's true name" in ch. 3 — never established to know it
```

---

## The command line

```
inkhaven knowledge                 # the deterministic check (non-zero exit on a break)
inkhaven knowledge --json          # the findings as JSON
inkhaven knowledge --deep          # + the opt-in, cost-capped implied_irony pass
```

`knowledge` exits non-zero on any hard break (`premature_knowledge` / `leaked_secret`)
— a CI gate, like `continuity check`. It is **self-gating**: no `secret:`/`know:` tags
and no events → nothing to check, no cost.

---

## In the editor

- **The dashboard** — **`Ctrl+B Shift+Z`** opens the knowledge findings grouped by
  kind; `↑↓` scroll, **Enter** jumps to the offending paragraph, `Esc` closes.
- **The revision worklist** — knowledge findings ride the `Ctrl+V Shift+R` Editorial
  Pass (a `knowledge` source, routed **Decision**: *fix the leak, move the reveal, or
  add a grant?*) and `inkhaven revise`.

---

## Cost — a design invariant

The whole core is a forward walk + set membership + Unicode mention matching — **no
model, ≈ $0, independent of book length**. Cost scales with declared topics and scenes,
not pages. The **only** LLM touchpoint is the opt-in `--deep` `implied_irony` pass,
cost-capped under the daily cap on the world fact-checker's rail — never automatic,
never whole-book. There is deliberately no "have the AI judge what everyone knows"
pass.

---

## From a script

```
ink.knowledge.grants   ( -- list )  the who-could-know-what ledger {character, topic, chapter, source}
ink.knowledge.findings ( -- list )  the deterministic breaks {kind, severity, chapter, character, topic, message}
ink.knowledge.check    ( -- dict )  {premature, leaked, dropped, clean}
```

Read-only. `check.clean` is `true` when no hard break stands — a pre-submit gate. The
`--deep` pass is not exposed to Bund (it costs).

---

## Multilingual

KEN inherits SENTINEL's and DIALOG-1's language coverage: the mention matcher is
Unicode-aware (Cyrillic, accents, and Latin match alike), and dialogue attribution
ships EN/RU/DE/FR/ES conventions. Topics and character names are matched in the
project's language.

---

## What it is not

- Not an all-knowing oracle — it reasons only over what it can ground (events, tags,
  named mentions) and stays silent where it can't, rather than inventing breaks.
- Not a fact-checker — Facts asks *is the world consistent*; KEN asks *could this
  character know this yet*. Different axis.
- Not LLM-first — the core is deterministic and free; the subtle pass is explicit.
- Not a rewriter — it flags; the `Ctrl+V Shift+R` Editorial Pass owns any edit.
