# 44 — Prompts editor TUI

`inkhaven prompts-editor -p <dir>` launches a
standalone four-pane workbench for editing
`<project>/prompts.hjson` — the prompt library the
main TUI reads when you press F7 (grammar check),
F12 (critique), or any AI flow that references a
named prompt.

The four panes:

```
┌───────────────┬────────────────────────────────┬──────────────────────────┐
│ Prompts       │ Editor — `tighten`             │ AI · gemini · 1.4s       │
│               │                                │                          │
│ ▶ critique-   │ Tighten the following prose.   │ ▸ template under review  │
│   edit        │ Preserve voice and meaning;    │   `tighten`              │
│   critique-   │ remove redundancy and weak     │   Tighten the following  │
│   changes     │ constructions.                 │   prose...               │
│   show-dont-  │                                │   {{selection}}          │
│   tell        │ {{selection}}                  │                          │
│   tighten ◀   │                                │ ▸ analysis request       │
│               │                                │   is this template clear?│
│               │                                │                          │
│  9 prompts    │ L4 C18                         │ ▸ assistant              │
│               │                                │   The template is clear  │
│               │                                │   and tightly scoped …   │
├───────────────┴────────────────────────────────┴──────────────────────────┤
│ Test prompt: is this template clear?│                                     │
└────────────────────────────────────────────────────────────────────────────┘
```

`Tab` cycles focus across the three editable panes:
`Prompts list → Editor → AI prompt → Prompts list`.
The AI response pane is display-only.

## The mental model

This is a **prompt-engineering reviewer**, not a
prompt executor.  You write a template in the centre
pane; you ask the LLM (via the bottom pane) to
critique it; the right pane shows the critique.

The LLM does **not** try to execute the template.
Placeholders like `{{selection}}` and `{{context}}`
are NOT substituted — they're shown to the reviewer
as literal text so it can comment on whether
they're used appropriately.

## The send pipeline

When you press `Enter` in the AI prompt input:

  1. The editor body is snapshotted as the *template
     under review*.
  2. Your typed text becomes the *analysis request*
     (or an embedded default kicks in if empty).
  3. inkhaven builds a fenced user message:

     ```
     --- PROMPT TEMPLATE UNDER REVIEW ---
     <editor body verbatim>
     --- END TEMPLATE ---

     Analysis request:
     <your typed instruction or default>
     ```

  4. It pairs this with a fixed system prompt that
     tells the LLM "you are a reviewer — analyze,
     don't execute" and explains the placeholder
     conventions.
  5. The response streams into the AI pane.

Empty AI prompt input still works — Enter yields a
baseline critique ("Identify strengths and
weaknesses, comment on placeholder use, suggest
improvements").

## Chord set

### Prompts list pane

| Chord       | Action                                |
|-------------|---------------------------------------|
| `↑↓`        | Navigate; cursor auto-loads into editor |
| `Enter`     | Load focused prompt + jump to editor   |
| `a`         | Add new prompt (name prompt → empty)   |
| `d`         | Delete focused prompt (confirm modal)  |
| `PgUp/PgDn` | Page by 10                             |
| `Home/End`  | First / last entry                     |

### Editor pane

| Chord                  | Action                          |
|------------------------|---------------------------------|
| Arrow keys, Home/End   | Move cursor                     |
| PgUp/PgDn              | Page scroll                     |
| Shift+arrows           | Extend selection                |
| Ctrl+A / Ctrl+E        | Start / end of line             |
| Ctrl+B / Ctrl+F        | Left / right (readline)          |
| Ctrl+N / Ctrl+P        | Down / up (readline)             |
| Ctrl+K                 | Kill to end of line             |
| Ctrl+W                 | Delete previous word            |
| Ctrl+U / Ctrl+Y        | Undo / redo                     |
| **`Ctrl+G`**           | **Get** AI response into editor at cursor |
| Type to insert         |                                 |

### AI prompt input pane

| Chord       | Action                                |
|-------------|---------------------------------------|
| Type        | Append to buffer                       |
| Backspace/Del | Delete                              |
| Left/Right + Home/End + Ctrl+A/Ctrl+E | Cursor movement |
| `Up/Down`   | History walk (in-session, deduped)      |
| `Enter`     | Send — analyse focused template        |
| `Ctrl+L`    | Clear input                            |
| `Ctrl+K`    | Clear input + clear history            |

### Global (any focus)

| Chord       | Action                                |
|-------------|---------------------------------------|
| `Ctrl+S`    | Save library (confirm modal lists ✚/✱/✗ buckets) |
| `Ctrl+R`    | Rollback picker — list `.prompts-backups/` |
| `Ctrl+H` / `?` | Focus-aware help pane               |
| `Tab` / `Shift+Tab` | Cycle pane focus              |
| `Esc` / `Ctrl+Q` | Quit (confirm if unsaved)        |

## Save semantics

`Ctrl+S` writes the entire library back to
`<project>/prompts.hjson` via atomic write (`.tmp` +
rename) and snapshots a copy to
`<project>/.prompts-backups/prompts_YYYYMMDD_HHMMSS.hjson`.

The confirmation modal partitions pending changes:

  * `✚ added` — newly-created entries.
  * `✱ modified` — existing entries whose body
    differs from disk.
  * `✗ removed` — entries staged for deletion (still
    in the list, struck-through, until save).

Empty AI prompt input is fine.  Empty library is
fine — `Ctrl+S` writes an empty library and lands a
backup, so you can always roll forward.

## Rollback

`Ctrl+R` lists every `prompts_*.hjson` file in
`.prompts-backups/`, newest-first.  Each row shows
the absolute timestamp + a relative-time chip
("just now", "12 minutes ago", "yesterday", "4 days
ago", or absolute beyond 14 days) + file size.

Inside the picker:

  * `↑↓` navigate, `PgUp/PgDn` / `Home/End`
    scroll
  * `Enter` *stages* the backup into the working
    library (no disk write yet — Ctrl+S commits).
    Every leaf diff vs the live file becomes
    `✱ modified`; entries in the backup but not
    in the live file become `✚ added`; entries
    in the live file but not in the backup become
    `✗ staged for deletion`.
  * `v` preview the file's contents (scrollable
    full-pane view; Esc back to the picker)
  * `d` delete with confirm
  * `Esc` back to the main view

The first Ctrl+S after a rollback writes a fresh
backup of the pre-rollback state on the way through,
so the safety chain stays intact.

## Auto-populate on first launch

When `prompts.hjson` doesn't exist, the editor
auto-loads inkhaven's embedded default prompts
(`tighten`, `darker`, `critique-edit`, …) into
memory and lights a yellow `from embedded defaults`
chip on the top bar.  The first `Ctrl+S` writes the
file from those defaults.

This matches the main TUI's
`reseed_prompt_examples` policy — you can `d` to
drop any defaults you don't want before saving.

## Iteration loop

The typical workflow:

  1. Open the editor: `inkhaven prompts-editor`.
  2. Pick a prompt with `↑↓` (or `a` to start a new
     one).
  3. Tweak the template in the centre pane.
  4. Tab to the AI prompt input.  Type a question
     like "is this clear?" or "rewrite to be more
     concise" — or just press Enter for a default
     critique.
  5. Read the reviewer's response in the right
     pane.
  6. Tab back to the editor, position the cursor
     where you want to use the suggested rewrite,
     press **`Ctrl+G`** — the response drops in
     verbatim.
  7. Edit to taste, repeat.
  8. `Ctrl+S` to commit.

## See also

  * `Documentation/PROPOSALS/PROMPTS_EDITOR_TUI.md`
    — the design doc this implementation follows.
  * `Documentation/Tutorials/36-config-editor.md` —
    sibling standalone editor for `inkhaven.hjson`
    (`inkhaven config`).
  * `Documentation/PROMPTS.md` — the canonical
    reference for prompt template variables +
    expected shape.
