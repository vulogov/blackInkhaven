# The Output Pane

*(1.3.24+, RFC PANE-1)*

Inkhaven's right-hand region holds one of two **panes**:

- the **AI pane** — a *conversation*: you ask, the model answers, the
  history scrolls;
- the **Output pane** — a *notice board*: structured, one-way messages
  that subsystems post for you to read, act on, and dismiss.

They are different UI patterns, so they get different surfaces. The AI
pane is a dialogue you drive. The Output pane is everything Inkhaven
needs to *tell* you — a translation result, a lexicon proposal, a Bund
script's `print`, a finished background job — without expecting a reply.

> **Not to be confused with the floating Bund pane.** The `Ctrl+Z`
> floating pane (`ink.pane.*`, see
> [`Tutorials/18-bund-pane-and-script-picker.md`](Tutorials/18-bund-pane-and-script-picker.md))
> is a transient scratch surface a script paints by hand. The Output
> pane is a *persistent, structured* channel with a typed message
> envelope and a database behind it. A Bund script writes to the Output
> pane with `ink.io.*`, not `ink.pane.*`.

---

## Switching panes

| Chord | Effect |
| ----- | ------ |
| `Ctrl+B Tab` | Cycle the right region forward (AI ↔ Output) and focus it. |
| `Ctrl+B Shift+Tab` | Cycle backward. |

The chord works from **any** focus — editor, tree, an overlay — because
the `Ctrl+B` prefix absorbs the keystroke before the editor sees `Tab`.
Your editor's plain `Tab` (insert) is unchanged.

On a project's **first** launch the right region shows the **Output
pane**. After that, whichever pane you leave active is remembered in
`<project>/.session.json` and restored next start, just like which
paragraph was open — so returning to a project where you last used the
AI pane brings you back to the AI pane.

---

## Reading the pane

Each message shows two lines — a header (severity icon + kind) and the
message text — with the selected message marked `▌` and bolded:

```
 ● translation_result
   the warrior raises his sword  →  kira nami pata
 ⚠ translation_uncovered_word_report
   2 word(s) uncovered: sword, sun
```

Press `o` (or `Space`) on a message to **expand** it: a translation
shows its per-word trace and alternatives, a lexicon proposal lists every
candidate word, a variety rendering lists each base→variety pair.

### Severities

| Icon | Severity | Meaning |
| ---- | -------- | ------- |
| `●` | `info` | Neutral result — a translation, a Bund `print`, a finished task. |
| `⚠` | `warning` | Something needs a look — uncovered words, a soft inconsistency. |
| `⊗` | `contradiction` | A hard conflict against a recorded fact or invariant. |
| `↻` | `progress` | A long task reporting that it is still running. |

### Lifetimes

Every message carries a **lifetime** set by whoever emitted it; the pane
expires messages automatically so it never becomes a junk drawer:

| Lifetime | Expires when… |
| -------- | ------------- |
| `Session(N)` | …it falls outside the most recent `N` of its kind. |
| `Hours(h)` | …`h` hours have passed since it was posted. |
| `UntilActedOn` | …you act on it (e.g. dismiss, or resolve its subject). |
| `UntilParagraphEdited(id)` | …the referenced paragraph changes (the warning was likely addressed). |
| `Never` | …only when you dismiss it by hand. |

Pinned messages (`p`) are never auto-expired and sort to the top.

---

## Acting on a message

With the Output pane focused:

| Key | Action | Notes |
| --- | ------ | ----- |
| `↑` / `↓` or `k` / `j` | Move the selection | |
| `g` / `G` | Jump to top / bottom | |
| `o` / `Space` | Expand / collapse detail | trace, proposals, renderings |
| `Enter` | **Primary** action | *per kind* — see below |
| `r` | Remember | translation results → translation memory |
| `e` | Edit + remember | insert the target **and** remember the pair |
| `a` | Ask AI about this | takes the message's full detail into the AI pane |
| `d` | Dismiss | |
| `p` | Pin / unpin | pinned messages stay until dismissed |

The footer hint row is **context-aware**: it shows only the actions that
apply to the selected message.

### `Enter` is per-kind

`Primary` (the `Enter` key) means different things depending on what you
have selected:

| Selected kind | `Enter` does… |
| ------------- | ------------- |
| `translation_result` | Insert the target text at the editor cursor. |
| `lexicon_proposal` | Promote — commit the proposed words into the Dictionary. |
| `ai_task_complete` | Open the task's target paragraph. |

### Ask AI (`a`) — the bridge

`a` carries a message's *full structured detail* into the AI conversation
**by reference, not by value**: the detail is armed as the next prompt's
hidden context (exactly like the `Ctrl+B P` / `Ctrl+B C` RAG flows), a
short human-readable quote is placed in the AI input, and focus moves to
the AI prompt. You just type your question. The model sees the rich
context; the conversation pane shows only the quote and your question.

### Remember (`r`) and Edit + Remember (`e`)

On a `translation_result`:

- `r` commits the `source → target` pair to the language's **translation
  memory** (LANG-3), so the next identical or near-identical sentence is
  recalled instead of re-derived.
- `e` does both halves of the editing loop in one gesture: it **inserts**
  the target at your cursor (so you can edit it in place) *and* remembers
  the pair.

Both refuse a target that still contains uncovered words (`«word»`) — add
those words to the Dictionary first.

---

## Where messages come from

| Producer | Kind(s) | How |
| -------- | ------- | --- |
| **Editor translation** — `Ctrl+B D` | `translation_result`, `translation_uncovered_word_report` | Deterministic (rule-based) LANG-3 translation of the open paragraph. The AI translation `Ctrl+B Q` still streams into the AI pane. |
| **CLI / Bund translation** | `translation_result`, … | `inkhaven language translate / reverse / cross`, and the `lang.translate` / `reverse` / `cross` Bund words. |
| **Lexicon generation** | `lexicon_proposal` | `inkhaven language generate-lexicon` (dry run) and `ink.lang.generate_lexicon`. Promote with `Enter`. |
| **Variety rendering** | `variety_rendering` | `inkhaven language lect` and `ink.lang.lect`. |
| **Bund scripts** | `bund_print`, `bund_log`, *any* | `ink.io.print` / `log` / `notify`. A bare `print` / `println` is also mirrored here (one message per line) so script output is visible in the pane, not only in the transient floating Bund pane. |
| **Background / long AI tasks** | `ai_task_complete` | A single completion notice when a long task finishes — the deep world refresh (`Ctrl+V Shift+F`) and a full-chapter fact-check (`Ctrl+B Shift+X`) post one — so you can switch panes during it and still be told. The fact-check's notice jumps to the checked paragraph via `Enter`. |

> Emission is **active-aware**: a producer posts to Output only when an
> Output store is installed in the process. That is always true inside
> the TUI; on the plain command line `inkhaven language translate …`
> just prints, with no pane to post to.

---

## Scripting the pane — `ink.io.*`

A Bund script writes to the Output pane through the `ink.io.*` stdlib.
Read words (`print` / `log` / `message.list` / `message.count`) need the
`store_read` policy category (allowed by default); write words (`notify`
/ `message.dismiss` / `pin` / `unpin`) need `fs_write` (enable it in
`inkhaven.hjson` under `scripting.enabled_categories`).

| Word | Stack effect | Category | Effect |
| ---- | ------------ | -------- | ------ |
| `ink.io.print` | `( text -- )` | `store_read` | Post a `bund_print` info message. |
| `ink.io.log` | `( text level -- )` | `store_read` | Post a `bund_log` message; severity from `level` (`info`/`warn`/`error`). |
| `ink.io.notify` | `( kind metadata -- id )` | `fs_write` | Post a message of an arbitrary `kind` with a metadata dict; returns its id. |
| `ink.io.message.list` | `( kind -- list )` | `store_read` | List active messages of `kind` (`""` = all) as `{id,kind,severity,text}` dicts. |
| `ink.io.message.count` | `( kind -- n )` | `store_read` | Count active messages of `kind`. |
| `ink.io.message.dismiss` | `( id -- )` | `fs_write` | Dismiss a message by id. |
| `ink.io.message.pin` | `( id -- )` | `fs_write` | Pin a message. |
| `ink.io.message.unpin` | `( id -- )` | `fs_write` | Unpin a message. |

```bund
# Post a couple of notices, then read them back.
"build finished" ink.io.print
"3 chapters lint-clean" "info" ink.io.log

# A structured notice of your own kind, with a jump target in metadata.
"lint_report" [ "text" "2 issues in chapter 4" "issues" 2 ] ink.lang.dict
  ink.io.notify
drop   # discard the returned id

# How many notices are live right now?
"" ink.io.message.count .   # prints the total
```

See [`Bund/README.md`](Bund/README.md) for the full stdlib and policy
model.

---

## The `output` CLI

For testing, scripting, and headless inspection:

| Command | Effect |
| ------- | ------ |
| `inkhaven output show [--kind K] [--severity S] [--limit N] [--json]` | List active messages. |
| `inkhaven output emit <kind> [--metadata JSON] [--severity S]` | Post a message. |
| `inkhaven output dismiss <id>` | Dismiss one. |
| `inkhaven output clear` | Dismiss all. |

Messages persist per project in `<project>/output.db` (a DuckDB file
beside `progress.db`), so the CLI and the TUI see the same board, and it
survives a restart. Expired and snoozed messages are cleaned up
automatically whenever the store is opened.

---

## See also

- [`Tutorials/75-the-output-pane.md`](Tutorials/75-the-output-pane.md) —
  a hands-on walkthrough.
- [`CONLANG.md`](CONLANG.md) — the translation engine whose results land
  here.
- [`Bund/README.md`](Bund/README.md) — the `ink.io.*` scripting surface.
- [`PROPOSALS/PANE-1_PLAN.md`](PROPOSALS/PANE-1_PLAN.md) — the design
  rationale.
