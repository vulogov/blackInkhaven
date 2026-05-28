# 46 — AI sentence-rhythm rewrite

`Ctrl+B Shift+M` asks the configured LLM to rewrite
the open paragraph so its sentence rhythm breaks out
of a monotonous pattern.  Pairs with the
[`Ctrl+B Shift+H` rhythm gauge](42-sentence-rhythm-gauge.md):
the gauge tells you *whether* you have a rhythm
problem; the rewrite chord tells you *what to do
about it*.

The chord ships in 1.2.11.

## The natural workflow

The minimal sequence:

1. Open the suspect paragraph.
2. Press `Ctrl+B Shift+H` — the rhythm gauge pops up.
3. Read the verdict.  If it says MONOTONE, press
   `Ctrl+B Shift+M` *from inside the gauge modal*.
4. The gauge dismisses; the AI pane lights up with
   the streaming rewrite.
5. When streaming completes, an AI diff modal opens
   automatically — before / after side-by-side, the
   original on the left, the rewrite on the right.
6. Read the rewrite.  Press `a` (or Enter) to
   accept; `r` to reject.

The chord works from the editor pane too; you don't
have to open the gauge first.  But the diagnose-
then-rewrite path is the natural ergonomic shape,
and the chord is wired so it fires whether you're in
the editor or staring at a MONOTONE verdict.

## What the AI receives

The default prompt asks the model to:

- Mix short, punchy sentences with longer ones so
  the reader's ear has variety to follow.
- **Preserve** voice, tone, meaning, named
  characters, quoted dialogue, em-dashes, and
  paragraph breaks.
- **Not translate** — keep the prose in the
  project's working language.
- **Not summarise** — rewrite line for line.
- Return only the rewritten paragraph — no preface,
  no commentary, no markdown headings.

The prompt resolves through the standard inkhaven
precedence (covered in
[tutorial 47 — multilingual prompts](47-multilingual-prompts.md)).
For projects with no custom prompt: the binary ships
a hand-written variant in each of the five
supported languages (English / Russian / French /
German / Spanish), so the rewrite lands in the
project's language without you having to configure
anything.

To override globally, add a `sentence-rhythm-rewrite`
entry to your `prompts.hjson` (with a `language:` tag
if you want it to win against the embedded floor).
To override per-project, add a paragraph titled
`sentence-rhythm-rewrite` to the Prompts system book
(optionally tag it `lang:<code>`).  See
[tutorial 44 — prompts editor](44-prompts-editor.md)
for the editor surface.

## The diff modal

Side-by-side render of the original (left) and the
rewrite (right).  Long lines wrap with continuation
indent so a paragraph-length sentence still reads as
a single visual block.

- `a` or `Enter` — accept; the rewrite replaces the
  paragraph buffer, *and a snapshot is created
  first* with annotation `Sentence rhythm rewrite`.
  That snapshot shows up in F6 with a `✎` indent so
  you can roll back the rewrite later from history.
- `r` — reject; the modal closes, the paragraph
  buffer is unchanged.
- `↑↓` / PgUp / PgDn / Home / End — scroll the
  diff.

The snapshot-on-accept is the safety net.  If the
rewrite is technically valid but loses something
the AI didn't preserve — a turn of phrase you
remember caring about — `F6` → pick the
`Sentence rhythm rewrite` snapshot → Enter restores
it.

## Worked example

Starting paragraph (verdict: MONOTONE, CV 0.18):

> Bob walked to the door. He opened it slowly. He
> looked into the hallway. The hallway was empty.
> He felt relieved. He stepped through. He closed
> the door behind him.

After `Ctrl+B Shift+H` → MONOTONE → `Ctrl+B Shift+M`
→ accept:

> Bob walked to the door — slowly, as if the next
> creak might wake whatever waited beyond.  Empty.
> He stepped through, closed the door behind him,
> and only then let himself breathe.

Same beats; different cadence.  Re-running the
gauge on the result lands STEADY or VARIED.

## When to run it manually

Most paragraphs that test MONOTONE benefit from a
rewrite — short identical-shape sentences usually
mean the author was on autopilot.  But a few cases
where you should *not* run the rewrite:

- **Dialogue-heavy passages** where the rhythm
  comes from speaker beats rather than narration.
  The gauge sees them as monotone but they read
  fine.
- **Deliberate hypnotic / liturgical
  passages** — short identical sentences as a
  rhetorical device.  The MONOTONE verdict is
  technically correct but a rewrite would dilute
  the effect.
- **First-draft scaffolding** where you know the
  paragraph is a placeholder.  Rewriting before
  the structure is solid wastes the model's work
  on a draft you'll cut.

The chord is a tool, not a directive.

## Configuration

- `editor.diff_review_on_apply = true` (the
  default) gates the AI diff modal.  Set false to
  drop straight into the apply step (the snapshot
  still lands).  Almost no one wants this off; the
  diff is the whole point.
- `llm.default` picks the model.  Larger models
  (Claude Opus 4.7, GPT-5, Gemini 2.5 Pro) produce
  better rewrites; the chord works against any
  configured provider.

## Why a snapshot?

The author's reflex when a rewrite lands and feels
*almost* right is to start tweaking it in place.
That's fine — but if the tweak goes sideways, you
want a one-keystroke path back to the AI's version
*and* a one-keystroke path back to the original.
The annotated snapshot is what gives you the
original.  The AI's version is already in the
buffer; the annotated snapshot is the
pre-rewrite state.

## See also

- [42 — sentence-rhythm gauge](42-sentence-rhythm-gauge.md) — the diagnostic chord this rewrite pairs with.
- [28 — AI critique and memory](28-ai-critique-and-memory.md) — the broader AI-diff-modal pattern.
- [29 — snapshot annotations](29-snapshot-annotations.md) — what the `Sentence rhythm rewrite` annotation looks like in F6.
- [47 — multilingual prompts](47-multilingual-prompts.md) — how the rewrite prompt picks its language.
