# 75 — The Output pane

Inkhaven's right-hand region can show one of two panes. You already know
the **AI pane** — a conversation. This tutorial introduces its sibling,
the **Output pane** — a notice board where subsystems post structured,
one-way messages: translation results, lexicon proposals, Bund script
output, finished background jobs.

```
┌─ Output · 3 ─────────────────────────────────────────┐
│▌● translation_result                                  │
│    the warrior raises his sword  →  kira nami pata    │
│      warrior → kira (lexicon, 0.90)                   │
│      raise   → nami (lexicon, 0.85)                   │
│      sword   → pata (lexicon, 0.88)                   │
│ ⚠ translation_uncovered_word_report                   │
│    1 word(s) uncovered: sun                           │
│ ● lexicon_proposal                                    │
│    4 proposed word(s) · sea: mara (ocean), …          │
│                                                       │
│ ↑↓ · ⏎ insert · e edit+remember · r remember · a … │
└───────────────────────────────────────────────────────┘
```

## Switch to it

From anywhere — editor, tree, an overlay — press **`Ctrl+B Tab`**. The
right region flips from AI to Output and takes focus. `Ctrl+B Tab` again
flips back; `Ctrl+B Shift+Tab` cycles the other way. (Your editor's plain
`Tab` still inserts a tab — the `Ctrl+B` prefix grabs the keystroke
first.)

Whichever pane you leave active is remembered across restarts, alongside
which paragraph was open.

## Put something on the board

The quickest way to see a message appear: open a paragraph of prose in a
project that has a constructed language, and press **`Ctrl+B D`** —
"**D**eterministic" translation. Inkhaven runs the rule-based LANG-3
engine over the paragraph and posts the result to Output, then flips you
there automatically.

> `Ctrl+B D` is the *deterministic* sibling of `Ctrl+B Q`. `Ctrl+B Q`
> asks the **AI** to translate and streams prose into the **AI pane**;
> `Ctrl+B D` runs the **rule engine** and posts a structured result —
> with a per-word trace — to the **Output pane**. Same language
> resolution (one language → direct; several → the first, named on the
> status bar).

## Read it

With the pane focused:

- `↑` / `↓` (or `k` / `j`) move the selection; `g` / `G` jump to
  top / bottom.
- **`o`** (or `Space`) expands the selected message. A translation shows
  its per-word trace; a lexicon proposal lists every candidate word; a
  variety rendering lists each base→variety pair.

The footer hint row only ever shows the actions that apply to whatever
you have selected — so you never guess which keys do something.

New messages arrive **newest-first** at the top. Your selection is
anchored by message id, not by row position, so a message landing while
you read no longer bumps your cursor onto a different message — it stays
on the one you're on. Two conveniences follow from that:

- While parked on the **top** row, the pane **follows the newest**
  message as it arrives — a `⟳follow` cue shows in the pane title.
  Moving down (`↓` / `j`) turns follow off so the newest arrival can't
  yank the board out from under you; returning to the top turns it back
  on.
- Dismissing (`d`) keeps the cursor on the row that shifts up into the
  gap, so you can clear several in a row without re-aiming.

## Act on it

The `Enter` key is the message's **primary** action, and it adapts:

- On a **`translation_result`**, `Enter` **inserts** the target at your
  editor cursor.
- On a **`lexicon_proposal`**, `Enter` **promotes** — commits the
  proposed words into the language's Dictionary, the same way
  `generate-lexicon --yes` would, with no second model call.
- On an **`ai_task_complete`**, `Enter` **opens** the task's target
  paragraph.

More broadly, `Enter` on **any** message that records a source
paragraph — a fact-check note, a continuity flag, a Socratic prompt, and
the rest — **opens that paragraph in the editor**, so the board doubles
as an actionable worklist: read a finding, press Enter, land on the
sentence it's about. Kind-specific primary actions (promote a lexicon
proposal, insert a translation, jump to a timeline event) are unchanged —
this is simply the fallback for the many findings whose natural next step
is "take me there".

Two more, on translation results:

- **`r`** — *Remember*: commit the `source → target` pair to translation
  memory, so the next identical (or semantically near) sentence is
  recalled instead of re-derived.
- **`e`** — *Edit + Remember*: insert the target at your cursor **and**
  remember the pair, in one keypress. Use it when the translation is good
  enough to drop into the manuscript and keep.

(Both refuse a target that still has uncovered `«words»` — add those to
the Dictionary first.)

## Ask the AI about a message

Press **`a`**. The message's full structured detail is carried into the
AI conversation as hidden context, a short quote is pre-filled in the AI
prompt, and focus moves there. Type your question — *"why did it choose
this word order?"* — and press Enter. The model sees the whole message;
your chat history stays clean, showing only the quote and the question.

## Tidy up

- **`d`** dismisses the selected message.
- **`p`** pins it — pinned messages sort to the top and never auto-expire.

Most messages expire on their own (a translation result lingers for the
last few of its kind; a completion notice fades after hours), so the
board stays current without housekeeping.

## From a script, or the command line

A Bund script posts to the board with `ink.io.print` / `log` / `notify`,
and reads it back with `ink.io.message.list` / `count`:

```bund
"render complete — 142 pages" ink.io.print
"" ink.io.message.count .
```

And the `output` CLI inspects the same board headlessly — handy in a
shell or a test:

```
inkhaven output show
inkhaven output show --kind translation_result --json
inkhaven output clear
```

The board lives in `<project>/output.db`, so the TUI, your scripts, and
the CLI all see the same messages.

## See also

- [`../OUTPUT_PANE.md`](../OUTPUT_PANE.md) — the full reference: every
  kind, severity, lifetime, action, and the `ink.io.*` surface.
- [`05-ai-writing-assistant.md`](05-ai-writing-assistant.md) — the AI
  pane, the Output pane's conversational sibling.
- [`18-bund-pane-and-script-picker.md`](18-bund-pane-and-script-picker.md)
  — the *floating* Bund pane (`ink.pane.*`), a different surface for
  scratch script output.
