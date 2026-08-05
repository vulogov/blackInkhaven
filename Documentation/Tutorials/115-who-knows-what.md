# Tutorial 115 — Who Knows What, When

*Inkhaven 2.6 (KEN)*

The most common invisible plot-hole is a character acting on something they haven't
learned yet — Bob mentions the murder before anyone tells him. A reader feels the
wrongness and can't place it; you, three hundred pages deep, can't hold every
character's knowledge in your head. KEN can. It's SENTINEL's continuity watch extended
to a new axis: not *where* a character is, but *what they know*.

## Tell KEN what's secret

KEN never guesses. You declare the knowledge that matters with tags (`Ctrl+B ]` in the
editor, or your tag workflow):

- `secret:the betrayal` — marks "the betrayal" a secret.
- `know:the betrayal@Mara` — Mara learns it here. (`know:the betrayal` alone grants the
  scene's `pov:` character.)
- `reveals:the heir's true name` — on an event's paragraph, binds the event to the fact
  it reveals.

And KEN gets grants for free from your **timeline**: anyone in an event's participant
list knows that event from the moment it happens.

## Run the check

```sh
inkhaven knowledge
```

```
Knowledge check — 1 finding

  ⊗ premature_knowledge  Bob speaks of "the betrayal" in ch. 4 — before learning it in ch. 7
```

KEN walks the book forward, and wherever a character **references** a topic — in their
own dialogue, or in their POV scene — it checks: *could they know this yet?* If the
reference comes before their earliest grant, that's the break. A `secret:` referenced
by someone never told it is a `leaked_secret`; a declared reveal that never surfaces
again is a `dropped_reveal`. It exits non-zero on a hard break, so you can gate a draft
in CI.

KEN is **silent** where it can't ground a break — it never invents one — and it costs
**nothing** (no model; it scales with your tags, not your page count).

## Jump to the slip

In the editor, **`Ctrl+B Shift+Z`** opens the knowledge dashboard: the findings grouped
by kind. `↑↓` scroll, **Enter** jumps straight to the paragraph where the character
knew too much. The same findings ride the `Ctrl+V Shift+R` Editorial Pass, where you
**decide** the fix: cut the reference, move the reveal earlier, or add the grant you
forgot.

## When you want the subtle cases

The deterministic check catches *named* breaks. A character who *acts* knowing —
"Mara smiled, certain of what he'd done" — without naming it, needs a reader's judgment.
The opt-in, cost-capped LLM pass supplies it:

```sh
inkhaven knowledge --deep
```

It hands each scene, plus a ledger of who-knows-what, to the model and asks for implied
breaks. Explicit and budgeted — never automatic.

## From a script

```
ink.knowledge.check     ( -- dict )  { premature, leaked, dropped, clean }
ink.knowledge.findings  ( -- list )  the breaks
ink.knowledge.grants    ( -- list )  the who-could-know-what ledger
```

`clean` is `true` when no hard break stands.

---

KEN gives your reader's instinct a memory: it remembers who could know what, so a
secret stays secret until you choose to spend it. The full reference is
[`KNOWLEDGE.md`](../KNOWLEDGE.md).
