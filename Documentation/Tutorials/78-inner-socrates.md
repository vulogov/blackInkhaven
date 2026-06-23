# Tutorial 78 — Examined authorship with Inner Socrates

*Inkhaven 1.3.28+ (RFC INNER_SOCRATES-1)*

Revision is interrogative work: stepping back from the prose, seeing what is
actually there, distinguishing what you intended from what is merely on the page.
Beta readers and editors do this — they notice what you've stopped seeing and ask
the questions that surface unexamined assumptions. **Inner Socrates** makes a
structured form of that reading available continuously, in your terminal, against
your actual prose. It produces **questions**, never corrections. You examine your
prose; it never edits it.

Reference: [`../INNER_SOCRATES.md`](../INNER_SOCRATES.md).

## A first question

No setup is needed — Inner Socrates runs on any project. Ask it about a line:

```
$ inkhaven inner-socrates check --text "The regent had to declare war; the council left him no choice."
◇ Inquiry [Asserted Necessity] This passage treats an outcome as inevitable ("had to"). What alternatives did you decide to leave out?

1 question(s) · persona: Inner Socrates
```

That is the whole spirit: a question a careful reader would ask, in the persona's
voice, about a choice the prose made. There is no "fix" — only the question.

## As you write

In the editor, the **Fast track** can run ambiently. Open the hub with **`Ctrl+B
J`**, then **`A`** to toggle the auto-check: now pausing a few seconds on a
paragraph surfaces its questions in the **Output pane** (`Ctrl+B Tab` to look),
without stealing your focus. Or check the open paragraph on demand with **`Ctrl+B J
→ F`**.

The Fast track is deterministic and instant — seven categories (asserted
necessity, hedging, repeated structure, unattributed dialogue, long sentences,
tense shifts, ambiguous pronouns), in five languages. It needs no AI provider.

## The deeper read — the Slow track

For the questions patterns can't reach — a hidden assumption, an internal tension,
a framing, what a scene *does* for the book — add `--slow` (this calls your
configured LLM, and is cost-capped):

```
$ inkhaven inner-socrates check --paragraph <id> --slow
slow track · model: … · ~900 tokens · 1/150 calls today · reading…
◇ Inquiry [Framing] The battle is described entirely from the regent's vantage. What does that framing emphasize, and what does it leave unseen?
```

If your project has a **timeline**, `inkhaven inner-socrates timeline` reads the
prose against it — asking about events the timeline declares but no paragraph
dramatizes.

## Reader Personas

The same paragraph reads differently to different readers. Five personas ship:

```
$ inkhaven inner-socrates persona list
→ inner-socrates     Every question opens what the prose has closed.
  careful-editor     Notice what the prose is doing — to itself and to the reader.
  skeptical-reader   What's not being said is often louder than what is.
  first-time-reader  Pretend you've read nothing of this book before this scene.
  slow-reader        The rhythm of prose is doing something. What?

$ inkhaven inner-socrates persona activate skeptical-reader
```

In the TUI, **`Ctrl+B J → S`** cycles the active persona. Author your own with
`persona new <id>` (scaffolds an editable HJSON file) or **`Ctrl+B J → N`** for an
AI-guided wizard — adjust its voice and per-category emphasis weights, and it reads
your prose with that attention.

## Declaring intent

Some of what Inner Socrates flags is deliberate — an ambiguity you're holding, a
framing you chose, an echo you're building. Tell it, and it stops asking. This is
the **intent ledger** (the prose sibling of the world simulator's magic ledger): a
declared choice **suppresses** the matching finding with a note instead of nagging.

You don't have to write the ledger by hand. When you dismiss the same kind of
question five times, the **promotion mechanism** offers to declare it:

```
$ inkhaven inner-socrates suggestions list
  framing_interrogation (5×, chapter ch12) → propose `framing_choice`

$ inkhaven inner-socrates suggestions promote framing_interrogation --chapter ch12
declared intent … covering framing_interrogation — the interrogator will respect it
```

Carry a series' intentions into the next book with `inner-socrates bundle export`.

## Talking it through

When a finding deserves more than a glance, **`Ctrl+B J → C`** opens a
**conversation**: the AI pane is seeded with the active persona's voice and the
paragraph's questions, and you discuss. The persona thinks *with* you — it never
drafts your prose.

## What you learned

- `inner-socrates check [--slow]` asks questions about a line, a paragraph, a book;
  the Fast track runs ambiently (`Ctrl+B J → A`) into the Output pane.
- Five **Reader Personas** read the same prose differently; `Ctrl+B J → S` cycles
  them, and you can author your own.
- The **intent ledger** suppresses what you've chosen deliberately; the promotion
  mechanism helps you build it from your own dismissals.
- **`Ctrl+B J → C`** opens a conversation with the persona; it discusses, never
  rewrites.
- Every finding is a question. Inner Socrates helps you see what you wrote — it does
  not write it for you.

See also: the world simulator's [Tutorial 77](77-world-maps-and-fact-checking.md)
(its fact-checker is the consistency sibling of this interrogator).
