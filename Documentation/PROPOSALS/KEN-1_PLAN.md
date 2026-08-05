# KEN-1 — Who Knows What, When (a 2.6.0 flagship RFC)

*A character's **ken** is the range of what they know. KEN tracks each character's
knowledge across the timeline and flags the epistemic breaks a generic reader — human
or AI — can't reliably catch across a whole book: a character acting on something they
haven't learned yet, a secret that already leaked, dramatic irony that quietly broke.*

*(Codename open, as REDLINE's was: KEN / GNOSIS / the descriptive `knowledge`. "KEN"
below; the CLI verb is `inkhaven knowledge`.)*

---

## 1. The problem — the one continuity axis nothing watches

Inkhaven watches several kinds of continuity, and misses the one that breaks plots:

- **Physical / temporal** — where and when (SENTINEL co-location, the timeline).
- **Existence** — an entity named before its first scene (SENTINEL `introduce`).
- **Factual** — does the world stay self-consistent (Facts).
- **Emotional** — a character's arc and agency (CHAR-1).
- **Narrative access** — whose head we're in (CHORUS POV / head-hops).

**None of them track what a character *knows*.** Yet the epistemic break is the one a
mystery, a thriller, or any story with a secret lives or dies on: Bob mentions the
murder in chapter 4, but Bob wasn't there and no one told him until chapter 6. A reader
feels the wrongness and can't place it; the author, 300 pages deep, cannot hold every
character's information state in their head; and a generic AITHOUT the structured world
cannot reconstruct it from prose. It is the most common invisible plot-hole, and
nothing in the toolchain — inkhaven's or anyone's — catches it.

## 2. The thesis

> **KEN is SENTINEL's "referenced-before-introduced" invariant, extended to knowledge.**
> SENTINEL flags an entity *named before it exists*. KEN flags a character *acting on a
> fact before they could know it* — same forward-walk + mention-detection machinery, a
> new axis. It answers, per character and per moment: **could they know this yet?**

## 3. Why it is sound, native, and cheap (the case you asked for)

Three properties, and each is the answer to a question you have pressed on:

**Sound — the "when did they learn it?" boundary is derivable, not guessed.**
Knowledge isn't declared prose, so the naive version ("have an AI judge what everyone
knows") is exactly the expensive, unreliable trap KEN avoids. Instead the *grant* — the
moment a character could first know something — comes from structure inkhaven already
holds:
- **Event presence** (`TlEvent.characters` + timeline ticks): a character present when
  something happens knows it from that tick. Stored, deterministic, free.
- **Author reveal-tags** (the tension-tag pattern inkhaven already uses): the author
  marks `secret:the-betrayal` and `know:the-betrayal @ Mara` — a declared grant.
And the *use* — a character referencing the topic — is caught deterministically by
reusing **SENTINEL's Unicode-aware mention matching** over **DIALOG-1's attributed
dialogue** (who is speaking) and **CHORUS's scene POV** (whose head we're in). A named
reference before the earliest grant is the break.

**Native — a finding a generic AI structurally cannot produce.**
Reconstructing who-knows-what across 300k words requires the **timeline**, the
**event-participant lists**, the **character bible**, and **scene-by-scene POV** — the
structured world only inkhaven maintains. Paste the manuscript into ChatGPT and it
cannot reliably tell you Bob wasn't at the murder, because it has no event graph. This
is the strongest moat inkhaven has: the finding *is* the payoff of SEMNET + the timeline
+ SENTINEL + DIALOG-1, working together.

**Cheap — deterministic core, invariant by design.**
The whole core is a forward walk over scenes (a spine LECTOR already walks), a
per-character known-topic set seeded from events + tags, and named-mention matching.
**No model, ≈ $0, independent of book length** — cost scales with declared topics and
scenes, not an LLM reading pages. The subtle cases (irony without a named reference)
ride an *optional, explicit, cost-capped* LLM pass — same shape as SENTINEL's coherence
and LECTOR's synthetic read, never automatic, never whole-book.

## 4. What it catches

| Finding | The break |
| ------- | --------- |
| `premature_knowledge` | a character references a topic before their earliest grant (present-at / told / declared) — "Bob names the murder in ch. 4; he learns of it in ch. 6" |
| `leaked_secret` | a topic the author marked `secret:` is referenced by a character never granted it |
| `dropped_reveal` | a declared grant (`know: @ X`) that never lands — X is told, but it never surfaces again (dangling knowledge, the epistemic `unpaid_setup`) |
| `implied_irony` *(opt-in LLM)* | a character *acts* informed/ignorant without naming the topic — the subtle case the deterministic layer can't see |

```
Knowledge check — 3 findings

  ⊗ premature_knowledge  ch. 4  Bob refers to "the murder" — not present (ch. 6) and untold
  ⚠ leaked_secret        ch. 3  Sella mentions "the heir's true name" — secret, never granted to her
  ● dropped_reveal       ch. 7  Mara is told "the betrayal" — it never surfaces again
```

Confirmed findings promote into the `Ctrl+V Shift+R` worklist (a `knowledge` source,
routed **Decision**: *"is this a leak to fix, or did you mean to reveal it earlier?"*).

## 5. Cost — a design invariant

KEN **never sends the manuscript to a model** for its core. Grants come from stored
event-participant lists + author tags; uses come from deterministic mention-matching
over attributed dialogue and POV; the check is set membership on a forward walk. Core
cost ≈ **$0 at any book size.** The only LLM touchpoint is the opt-in `--deep`
`implied_irony` pass, cost-capped under the daily cap, per-scene, user-triggered. There
is deliberately no whole-book "judge what everyone knows" pass.

## 6. The honest limitation

- The deterministic core catches **explicit / named** epistemic breaks (a character
  speaks a secret's name, references an off-page event they weren't at). Genuinely
  *subtle* dramatic irony — a knowing smile, an implication with no named topic — needs
  either an author tag or the opt-in LLM pass.
- Its value **skews to plot-driven genres** — mystery, thriller, betrayal, anything with
  secrets and reveals. A quiet literary novel with no information asymmetry gets little
  from it, and KEN says so (it stays silent rather than inventing breaks).
- Grant precision improves with author tags. Event-presence gives free grants; the
  `secret:` / `know:` tags make the check sharp. KEN is useful un-tagged and precise
  tagged — the same declared-then-checked contract SENTINEL and the tension ledger use.

## 7. Scope discipline (what KEN is not)

- Not an all-knowing oracle — it reasons only over what it can ground (events, tags,
  named mentions); it stays silent where it can't, rather than guessing.
- Not a rewriter — it flags; REDLINE (confirmed-diff) owns any edit.
- Not a fact-checker — Facts asks "is the world consistent"; KEN asks "could this
  character know this yet." Different question, different axis.
- Not LLM-first — the core is deterministic and free; the subtle pass is explicit.

The phase-by-phase, file-grounded build is in `KEN-1_IMPL.md` (KEN-P0→P8, value core
P1+P2+P3).
