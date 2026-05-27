# 43 — Show-don't-tell flag

Two complementary layers for catching prose that
*tells* the reader an emotion — `she was angry` —
instead of *showing* it through behaviour, sensory
detail, dialogue, or action — `her knuckles whitened
around the glass`.

## Layer 1 — inline overlay (always-on)

Hooks under the same `Ctrl+B Shift+F` toggle as
filter-words and repeated-phrases.  Underlines (in
soft teal) three categories of telling:

### 1. Copula + emotion adjective

A 2-gram `(linking_verb)(emotion_adjective)`:

```
She was angry.            ← `was angry` both flagged
He seemed nervous.        ← `seemed nervous` both flagged
They appeared exhausted.  ← `appeared exhausted` both flagged
The room felt empty.      ← `felt empty` both flagged
```

Linking-verb stems (English defaults): `be` / `is` /
`am` / `are` / `was` / `were` / `seem` / `feel` /
`appear` / `look` / `become` / `remain` / `grow` /
`sound`.  All Snowball-stemmed at detector init so
inflections collapse — `seemed`/`seems`/`seeming`
all key on `seem`.

Emotion adjectives: ~70 curated lemmas spanning the
anger / sadness / fear / joy / fatigue / confusion /
surprise / shame / pride / boredom families.

### 2. Manner-of-emotion adverbs

Single `-ly` adverbs that label emotion outright:

```
"Get out," she said angrily.    ← `angrily` flagged
He waited nervously.            ← `nervously` flagged
She turned wearily.             ← `wearily` flagged
```

About 25 adverbs ship in the English default.

### 3. Cognition verbs

Single verbs that tell the reader what's happening
inside the character:

```
She realised the room was empty.    ← `realised` flagged
He knew it was over.                ← `knew` flagged
They wondered why he had come.      ← `wondered` flagged
He decided to leave.                ← `decided` flagged
```

About 15 verbs in the English default —
`realised` / `realized`, `understood`, `knew`,
`thought`, `wondered`, `wished`, `hoped`,
`believed`, `supposed`, `decided`, `concluded`,
`discovered`, `recognised` / `recognized`,
`remembered`, `considered`, `assumed`, `expected`.

### False-positive control

The 2-gram rule keeps the overlay quiet on non-
telling prose:

```
She was running.            ← NOT flagged (no emotion adj)
He looks at the door.       ← NOT flagged
It became cold.             ← NOT flagged (not in emotion list)
```

Both halves of the 2-gram must hit known lists.

### Theme

The teal underline colour is configurable:

```hjson
theme: {
  style_warning_show_dont_tell_fg: "#94e2d5"
}
```

Distinct from filter-word amber (`#f9c44e`) and
repeated-phrase magenta (`#eb6f92`) so the three
overlays stay visually separate when adjacent.

### Per-language scaffold

English defaults populated.  Russian / French /
German / Spanish lists start empty so users can fill
them in for their corpus — emotion vocabulary varies
enough per genre and language register that shipping
defaults would mislead more than help.

```hjson
editor: {
  style_warnings: {
    enabled: true
    show_dont_tell: {
      enabled: true
      use_stemming: true
      // English defaults loaded automatically.
      // Provide non-empty lists to REPLACE the default
      // for that language.
      russian_emotion_adjectives: [
        "сердитый", "грустный", "счастливый", "испуганный"
      ]
      russian_linking_verbs: ["быть", "казаться", "чувствовать"]
    }
  }
}
```

## Layer 2 — AI scan (`Ctrl+B Shift+T`)

Sends the open paragraph to the configured LLM with
a system prompt asking for telling passages and
suggested rewrites.  The response streams into the
AI pane.

The prompt asks the model to:

1. Quote every telling phrase exactly.
2. Name what's being told (the emotion / state).
3. Propose one concrete show-rewrite — a body-
   language beat, a sensory detail, an action, or
   a fragment of dialogue — matched in length to
   the original line, not a paragraph.

Skips deliberate cases (transition lines, summary,
established interiority in first-person POV) so the
output is triage-able rather than exhaustive.

Reuses existing AI plumbing — same `Inference`
machinery as F12 critique + Ctrl+G grammar.

### Overriding the prompt

The prompt name is `show-dont-tell`.  Override it
the same way you'd override the critique prompt:

- **Per-project**: paragraph titled `show-dont-tell`
  (or `show dont tell`) under your project's
  `prompts` system book.
- **Global**: a `show-dont-tell` entry in your
  `~/.inkhaven/prompts.hjson`.
- **Embedded fallback**: the prompt baked into
  inkhaven (`show_dont_tell_default_prompt()` in
  `src/tui/app.rs`).

## When to use which layer

| Situation                              | Reach for…                |
|----------------------------------------|---------------------------|
| Drafting a paragraph                   | Layer 1 — instant feedback while typing |
| Revision pass on a finished paragraph  | Layer 2 — deeper, with rewrites    |
| No API key configured                  | Layer 1 only — Layer 2 needs LLM access |
| Auditing a chapter for telling-heavy   | Layer 1 first, Layer 2 on the worst paragraphs |

The regex catches the obvious 2-grams (`was angry`,
`realised`).  The AI scan catches the subtler
cases — declarative narration that explains rather
than dramatises, adverb-modified verbs the regex
misses — and proposes specific alternatives.

## Mnemonic

`Shift+T` for *tell*.

## See also

- [`03-the-editor.md`](03-the-editor.md) — every
  style-warning overlay + the toggle chord.
- [`28-ai-critique-and-memory.md`](28-ai-critique-and-memory.md)
  — F12 critique, prompt-template override
  machinery (shared with the show-don't-tell AI
  scan).
