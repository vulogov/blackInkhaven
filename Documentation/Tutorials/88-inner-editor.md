# Tutorial 88 — The Inner Editor

*Inkhaven 1.4.2–1.4.3*

Inner Socrates asks questions about your structure. The **Inner Editor** — the
second member of the Inner family — does something different: it reads your
prose for *craft* and tells you what it notices. Not what to fix; what it
*sees*. A row of repeated words, a sentence whose rhythm earns its weight, a
register that shifts halfway through, a paragraph that insists on a feeling its
texture doesn't quite support. It is the thoughtful editor reading over your
shoulder while you write — present, specific, and never bossy.

It is LLM-only and works at the paragraph scope. It never rewrites your prose.

## 1. Engage it

Put the cursor in a paragraph and press **`Ctrl+V O`** (`O` for *Observe*) to
open the Inner Editor overview, then **`E`** to engage the open paragraph:

```
┌─ Inner Editor ✎ ───────────────────────────────────────┐
│ Inner Editor — enabled                                 │
│   tone balanced · verbosity concise · praise moderate  │
│   genre-aware true (literary_realism) · belief true    │
│ Ambient auto-engage: off (A toggles)                   │
│ Today: 3/200 engagement call(s) (cap is informative)   │
│ …                                                      │
│ ↑↓ · E engage ¶ · C converse · A ambient · F findings  │
└────────────────────────────────────────────────────────┘
```

A few seconds later the observations land in the **Output pane**, each marked
with a **✎** and graded Praise / Note / Concern:

```
✎ inner-editor   Note     ch01-p007  Tautology
   "The sentence restates the same idea four ways — 'quiet hush', 'quiet and
    hushed', 'noiseless and without sound' all declare the absence of noise.
    If the chant is intentional it works; if not, you might thin it."

✎ inner-editor   Note     ch01-p007  Belief
   "The prose insists so strenuously on the silence that the texture protests
    too much. If that's ironic, it lands; if played straight, is it intended?"
```

Every observation grounds in actual textual evidence (`o` expands a row to show
it). The Editor speaks in observations and qualified suggestions — *"I notice"*,
*"you might consider"*, *"if intentional"* — never *"should"* or *"must"*. The
choice always stays yours.

**Praise is a first-class output**, but it has to be *earned* — generic
encouragement is forbidden by design. So Praise is **hidden by default** (the
visible threshold is `note`); a paragraph that genuinely does something well
earns a specific note you can reveal:

```sh
inkhaven inner-editor findings list --severity praise
```

```
✎ Praise [Craft] The alliterative consonance — 'silent stones of the silent keep
                 kept their silence' — builds an incantatory rhythm that carries
                 the motif.
```

## 2. Let it read as you write (ambient)

Press **`A`** in the overview to turn on the **ambient auto-engage**. Now a pause
on a paragraph runs one engagement in the background — the Editor reads as you
write, without you asking. It's off by default (it spends LLM calls), it has a
same-paragraph cooldown, and an edit re-arms the timer, so it never interrupts
you mid-sentence. Tune `inner_editor.engagement.idle_threshold_seconds` and
`cooldown_seconds` to taste.

## 3. Talk it through (conversation)

When an observation deserves more than a glance, press **`C`** in the overview
(or cycle the AI pane scope with **F9** to **Editor**). The AI pane becomes the
Editor, opening with what it noticed and ready to discuss — in the same
non-prescriptive voice:

```
┌─ AI · scope=Editor ────────────────────────────────────┐
│ Editor: A couple of things I noticed. The register      │
│ shifts from formal to colloquial in the third sentence. │
│ If that mirrors the character waking up, it's well       │
│ placed; if not, you may want to look at it.             │
│ > _                                                     │
└─────────────────────────────────────────────────────────┘
```

It helps you *think*; it won't draft the scene for you.

## 4. Declare a choice deliberate

The Editor will sometimes notice a pattern you put there on purpose — a motif of
repetition, a deliberately unstable voice, an unreliable narrator whose prose
shouldn't believe him. Tell it so once, and it stops raising that category.
Select the finding in Output and press **`i`** (record intent), or from the
terminal:

```sh
inkhaven inner-editor intent dictionary_richness --description "the silence motif is intentional"
inkhaven inner-editor intent style_instability --chapter ch04 --description "Ch.4 alternates POV registers"
```

This writes into the **shared intent ledger** — the same one Inner Socrates
consults — so the declaration is a first-class part of your examined-authorship
record. Future Editor findings of that category (project-wide, or scoped to the
chapter) are suppressed.

## 5. Tune the persona

One persona, several knobs — in `inkhaven.hjson` under `inner_editor.persona`:

- **`tone`** — `critical` / `balanced` / `encouraging` (how it weighs critique
  against praise).
- **`verbosity`** — `concise` / `standard` / `detailed`.
- **`praise_frequency`** — `rare` / `moderate` / `frequent`.
- **`genre_aware`** + the project **`genre`** — judges richness within your
  genre's conventions (fantasy's invented terms aren't "vocabulary problems").
- **`belief_stance_enabled`** — the subtle "does the prose believe its own
  message?" category.
- **`categories`** — turn any of the eight off individually.

See [CONFIGURATION.md → `inner_editor`](../CONFIGURATION.md#inner_editor-142--the-literarystylistic-companion)
for the full block.

## 6. Cost

Each engagement is one LLM call, tagged under the **`inner_editor`** budget in
`inkhaven cost` / **`Ctrl+B $`** (and `inkhaven inner-editor usage`). The caps
**inform, never block** — past the daily budget the Editor warns and continues;
you decide. Praise hidden by default, the cooldown, and the opt-in ambient all
keep the feedback (and the spend) calm during composition.

## Coexisting with Inner Socrates

Both Inner-family members run on the same paragraph and render distinctly in
Output — **◇ purple** Socratic questions, **✎ warm-earth** Editor observations.
Filter Output to one feature (`f` cycles the source), disable either
independently, or run both. Socrates examines your *choices*; the Editor
examines your *craft*. Together they are the texture half of Inkhaven's
examined-authorship system.

---

**See also:** [Tutorial 78 — Inner Socrates](78-inner-socrates.md)
· [Tutorial 12 — Configuring AI providers](12-configuring-ai-providers.md)
· [CONFIGURATION.md → `inner_editor`](../CONFIGURATION.md#inner_editor-142--the-literarystylistic-companion)
