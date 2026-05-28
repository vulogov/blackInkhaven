# Proposal — Prompts editor TUI (`inkhaven prompts-editor`)

Status: **research / approved — ready for
implementation**.  Target cycle: 1.2.11 (or late
1.2.10 if scheduling allows).  All four flagged
open questions resolved 2026-05-27 (§10).

## 1. Summary

A standalone TUI workbench for editing
`prompts.hjson` — the project's prompt library — with
live LLM testing in the same screen.  Launched as
`inkhaven prompts-editor --project-directory <dir>`.

Four-pane layout: prompts list (left), prompt editor
(centre), AI response (right), AI prompt input
(bottom).  Pressing Enter in the AI prompt input
sends the *editor's current prompt* as the system
prompt and the *typed text* as the user message to
the configured LLM; the streaming response lands in
the AI pane.  Same iteration loop a writer would do
manually with curl + a scratch file, compressed into
one screen.

Reuses the existing AI plumbing (`AiClient`,
`spawn_chat_stream`, `Inference`) so this isn't a
new LLM client — it's a new editor surface that
borrows the main TUI's infrastructure.

## 2. Motivation

Today's prompt-iteration workflow:

  1. Open `prompts.hjson` in `Ctrl+B 0` (or an
     external editor).
  2. Edit a prompt.
  3. Close the editor.
  4. Open the main TUI.
  5. Find a paragraph that exercises the prompt.
  6. Trigger the AI workflow (F7 / F12 / etc.).
  7. Read the response.
  8. Repeat from step 1.

That's a six-context-switch loop for what should be
*write, test, refine*.  The Prompts editor TUI
collapses it to: write in the centre pane, type a
test input in the bottom pane, read the response in
the right pane, refine.  No paragraph involved; no
mode switches.

## 3. User-facing surface

### 3.1 Invocation

```
$ inkhaven prompts-editor --project-directory ~/Books/aerin
$ inkhaven prompts-editor -p ~/Books/aerin             # short form
$ inkhaven prompts-editor                              # cwd default
```

Standalone TUI — exits to shell on `Esc` from the
list pane or `Ctrl+Q`.

### 3.2 Layout

```
┌───────────────┬────────────────────────────────┬──────────────────────────┐
│ Prompts       │ Editor — `critique-edit`       │ AI · gemini-2.5-pro      │
│               │                                │                          │
│ ▶ critique-   │ Read the paragraph below as a  │ ▸ user                   │
│   edit        │ draft.  Point out the          │   Test this prompt with  │
│   critique-   │ weakest two or three           │   a sample paragraph     │
│   changes     │ elements: vague verbs,         │   from a war novel.      │
│   grammar     │ abstract nouns where the       │                          │
│   show-dont-  │ concrete would land harder…    │ ▸ assistant              │
│   tell        │                                │   The prompt is well-    │
│   …           │                                │   suited for paragraph-  │
│               │                                │   level critique.        │
│               │                                │   Three concerns…        │
│               │                                │                          │
│  10 prompts   │  L4 C18 · saved                │  1.2s · 412 tokens       │
├───────────────┴────────────────────────────────┴──────────────────────────┤
│ Test prompt: Test this prompt against a war-novel paragraph│              │
└────────────────────────────────────────────────────────────────────────────┘
```

### 3.3 Pane semantics

  * **Prompts list (left)** — one row per prompt
    name from the loaded `prompts.hjson`.
    `↑↓`/`Home`/`End` navigate; `Enter` (or
    auto-on-focus-change) loads the selected
    prompt into the centre editor.  `a` adds a new
    prompt (name prompt → empty template); `d`
    deletes with confirm.

  * **Editor (centre)** — `tui-textarea`-backed,
    same chord set as the main inkhaven editor.
    All the standard mechanics: arrows, Ctrl+Left /
    Right for word motion, Home/End, Shift+arrows
    for selection, Ctrl+C / Ctrl+K / Ctrl+P
    clipboard, Ctrl+U undo, Ctrl+Y redo, etc.
    Save is `Ctrl+S` (writes the whole prompt
    library, not just this entry — same atomic
    + backup pipeline as the Config TUI).

  * **AI pane (right)** — read-only scrollable
    transcript.  Currently a single
    user/assistant turn at a time; multi-turn
    continuation is in §10 Q5.  `↑↓` / `PgUp` /
    `PgDn` scroll while focused.

  * **AI prompt input (bottom)** — single-line
    `TextInput` (same widget the main app's AI
    prompt uses).  `Up` / `Down` walks the prompt
    history.  `Enter` sends.

### 3.4 Focus + Tab cycling

`Tab` cycles focus through the **three editable
panes** in this order:

```
Prompts list  →  Editor  →  AI prompt  →  Prompts list
```

`Shift+Tab` reverses.  The currently focused pane
draws a coloured border + a chip in the status line.

The **AI response pane** is display-only and not in
the Tab cycle — it always shows the most recent
send.  Scroll it via `Ctrl+↑` / `Ctrl+↓` /
`Ctrl+PgUp` / `Ctrl+PgDn` (no focus required) or
the mouse wheel when the cursor is over it.  Long
responses auto-scroll to the bottom as the stream
arrives.

### 3.5 Chord set

```
Global
  Esc                close the focused pane's modal, else exit
  Ctrl+Q             quit (confirm if unsaved)
  Ctrl+S             save the library (with confirmation)
  Tab / Shift+Tab    cycle pane focus
  Ctrl+H / ?         field-aware help pane
  Ctrl+R             rollback picker (reuse config TUI's flow)

Prompts list (focused)
  ↑↓ / PgUp / PgDn   navigate
  Enter              activate (also auto-loads as cursor moves)
  a                  add new prompt — name prompt + empty template
  d                  delete focused prompt — confirm modal

Editor (focused)
  same chord set as the main editor pane
  (arrow movement, selection, clipboard, undo, etc.)

AI pane (focused)
  ↑↓ / PgUp / PgDn   scroll
  c                  clear the response

AI prompt (focused)
  type to edit
  Up / Down          history walk
  Enter              SEND (system = editor body; user = input)
  Ctrl+L             clear input
  Ctrl+K             clear input + history
```

## 4. Architecture

### 4.1 Module layout

New module: `src/prompts_tui/`

```
src/prompts_tui/
├── mod.rs           entry point (`run`)
├── app.rs           event loop + state + render
├── library.rs       prompts.hjson load / save +
                     atomic write + backups
├── ai.rs            send-to-LLM glue
                     (wraps spawn_chat_stream)
└── widgets.rs       reuse where possible; new
                     widgets only if absolutely
                     necessary
```

New CLI entry: `src/cli/prompts_editor_cmd.rs`,
registered under `inkhaven prompts-editor` in
`src/cli/mod.rs`.

### 4.2 State machine

```rust
pub struct App {
    project_root: PathBuf,
    prompts_path: PathBuf,
    cfg: Config,                  // for LLM
                                  // resolution
    ai: AiClient,
    library: PromptLibrary,       // existing struct
                                  // — load/save in
                                  // `library.rs`
    cursor: usize,                // selected
                                  // prompt index
                                  // in the list
    editor: tui_textarea::TextArea,
    editor_dirty: bool,
    /// User-typed test input.
    ai_input: TextInput,
    ai_history: Vec<String>,
    ai_history_cursor: Option<usize>,
    /// Most recent inference (None when idle).
    inference: Option<Inference>,
    /// Rendered transcript of the latest send.
    /// Single-shot in v1 — see §10 Q5.
    last_send: Option<Send>,
    focus: Focus,
    modal: Modal,
    saved_at_least_once: bool,
    /// Per-prompt staged changes (a save flushes
    /// every dirty entry at once).
    dirty_prompts: HashSet<String>,
}

pub enum Focus {
    List, Editor, Ai, AiPrompt,
}

pub enum Modal {
    None,
    Help { body: String },
    SaveConfirm { entries: Vec<String> },
    AddPrompt { buffer: String, error: Option<String> },
    DeletePromptConfirm { name: String },
    DiscardConfirm { unsaved: usize },
    Saved { message: String },
}

pub struct Send {
    pub system_prompt: String,    // editor body
                                  // at send time
    pub user_message: String,     // ai_input at
                                  // send time
    pub response: String,         // streamed
                                  // assistant
                                  // reply
    pub started_at: Instant,
    pub duration: Option<Duration>,
}
```

### 4.3 AI integration

Reuse the existing AI plumbing — no new client:

  * `AiClient::from_config(&cfg.llm)` — same the
    main TUI uses.
  * `spawn_chat_stream(client, model, None, [],
    rendered_text)` — `None` system prompt, the
    rendered template in the user role.  Matches
    `start_show_dont_tell_scan` and the F12
    critique flow.

**Payload composition** (Q2 resolution):

  1. Take the editor body as the template.
  2. If it contains `{{selection}}`: replace with
     the AI prompt input.  If it contains
     `{{context}}`: replace with empty string
     (the prompts editor isn't tied to a
     hierarchy, so there's no context to inject).
  3. If neither placeholder is present: append
     the AI prompt input to the template body on
     a fresh paragraph (`\n\n`) so a
     prompts-without-placeholders template still
     has the user's test text to operate on.
  4. Send the rendered text as the **user**
     message; `None` for the system prompt.

This mirrors how the same template would behave
when invoked from the main TUI's editor pane —
faithful reproduction of production semantics, not
a separate chat-style harness.

Single inference at a time.  Sending a second
prompt while the first is still streaming cancels
the first (drops the rx; the background task
finishes naturally and its events are ignored).

### 4.4 Save semantics

  * `Ctrl+S` writes the entire `prompts.hjson`
    library — same atomic
    (`.hjson.tmp` → rename) + timestamped backup
    (`<project>/.prompts-backups/prompts_YYYYMMDD_HHMMSS.hjson`)
    pipeline as the Config TUI.
  * Dirty marker: per-prompt boolean (the list
    pane chips dirty entries with a red `✱`).
    The library write happens en bloc — there's
    no partial-save concept since the file is
    one HJSON document.
  * Confirmation modal lists every dirty prompt
    name before writing.

## 5. UX details

### 5.1 Switching prompts with dirty edits

If the user moves the list cursor while the
current editor has unsaved changes, **the changes
stay staged** — they move into a per-prompt
"pending body" buffer.  Switching back picks up
where they left off.  Save flushes every pending
body to disk at once.

(Alternative: prompt "discard / save" on every
switch — feels heavy-handed for an iteration
workflow.  Going with the staging model.)

### 5.2 Empty library

`prompts.hjson` may not exist yet (the user is
about to write their first prompt).  Behaviour:

  * The list pane shows "(empty — `a` to add)".
  * Editor pane shows "(no prompt selected)".
  * `Ctrl+S` is a no-op until the user adds at
    least one prompt.
  * Optionally — see §10 Q4 — auto-populate the
    library on first launch with inkhaven's
    embedded default prompts (`critique-edit`,
    `critique-changes`, `grammar-check-en`,
    `show-dont-tell`).

### 5.3 LLM provider

Resolved via the existing `AiClient::resolve_provider(&cfg.llm, None)`
call — same way the main TUI picks a provider.
Shown in the AI pane's title bar (e.g.
`AI · gemini-2.5-pro`).  Cycling providers from
inside the prompts editor is **out of scope for
v1** — use `Ctrl+B 0` to switch the default and
re-launch.  (§10 Q3.)

### 5.4 Help pane

Same machinery as the Config TUI's `Ctrl+H` — a
floating pane.  Focused-pane-aware content:

  * On list pane → "chord summary for the prompts
    editor; `a` add, `d` delete, …"
  * On editor pane → "this is the same editor as
    the main app; common chord summary"
  * On AI pane → "transcript view, scroll keys"
  * On AI prompt pane → "history navigation,
    Enter sends"

Reusable bits: the existing `quickref.rs` chord
table.  No new docs file.

## 6. Reuse vs new

| Component                          | Source                          |
|------------------------------------|---------------------------------|
| `tui-textarea` editor mechanics    | already a dep                   |
| AI client + streaming              | `crate::ai::*`                  |
| `TextInput` (AI prompt line)       | `crate::tui::input`             |
| HJSON parse                        | `serde-hjson`                   |
| Atomic write + backups             | `config_tui::save` helpers      |
| Help pane infrastructure           | `config_tui::help` (refactor?)  |
| `Prompt` / `PromptLibrary` types   | `crate::ai::prompts`            |
| Help-text source                   | `assets/default_prompts.hjson`  |

Only new code: the four-pane layout + the per-prompt
dirty-set + the send-to-LLM action.

## 7. Streaming UX

When the user hits Enter in the AI prompt:

  1. Snapshot `(system, user)` into a `Send`
     struct; clear `last_send.response`.
  2. Spawn the chat stream; route `StreamMsg`s
     into `last_send.response` as they arrive.
  3. AI pane title shows a spinner +
     elapsed-seconds counter while streaming.
  4. On `StreamMsg::Done` the title chip flips
     to `1.2s · 412 tokens`.
  5. On `StreamMsg::Error(e)` the response field
     prepends `⚠ ERROR: {e}\n` and the title
     flips red.

The user can keep editing the system prompt mid-
stream; the in-flight send doesn't update —
they'll need a new send to see the effect.  This
is the same semantics the main TUI's AI pane has.

## 8. AI prompt history

Same plumbing as the main TUI's
`ai_prompt_history: Vec<String>` + Up/Down
history-cursor walk:

  * Each successful send pushes the input onto
    the history.
  * Up = previous (older); Down = newer.
  * Any keystroke clears the history cursor
    (you're now editing fresh).

Persistence?  v1 keeps the history in-session.
A `<project>/.prompts-ai-history` sidecar across
sessions is §10 Q6.

## 9. CLI surface

Reuse the global `--project / -p / --project-directory`
flag (added in 1.2.10).  No new flags for v1.

`inkhaven prompts-editor --help` documents the
chord set + layout from §3.

## 10. Resolved questions

Decisions made 2026-05-27 before implementation:

| #  | Question                                                                                              | **Decision**                                                                                          |
|----|-------------------------------------------------------------------------------------------------------|--------------------------------------------------------------------------------------------------------|
| Q1 | The user spec says *"Tab shifts between prompts"*.  Cycle panes, or move within the list?             | ✓ **Tab cycles panes** through `Prompts list → Editor → AI prompt → Prompts list` (three stops).  Within the list, `↑↓` navigates entries.  The AI response pane is display-only and not in the Tab cycle.   |
| Q2 | LLM payload composition.                                                                              | ✓ **Render the template + send as user message; no system prompt.**  `{{selection}}` is substituted with the AI prompt input; `{{context}}` is replaced with empty string; templates without either placeholder get the input appended on a fresh paragraph.  Mirrors how the same template behaves when invoked from the main TUI — faithful production semantics, not a separate chat harness. |
| Q3 | Provider switching from inside the prompts editor (Ctrl+B L equivalent)?                              | **Out of scope v1.**  Edit `llm.default` in `inkhaven.hjson` via `Ctrl+B 0` (or `inkhaven config`) and re-launch.                                                                                              |
| Q4 | First-launch with no `prompts.hjson` — auto-populate with inkhaven's embedded defaults?               | ✓ **Yes.**  Same `reseed_prompt_examples` policy the main TUI already runs.  User can `d` to drop anything they don't want.                                                                                  |
| Q5 | Single-shot per send or multi-turn conversation continuing across sends?                              | ✓ **Single-shot.**  This is *prompt assessment*, not a chat session — each send is an independent evaluation of how the current template behaves on the given input.  Multi-turn isn't planned.             |
| Q6 | AI prompt history: in-session only or persisted to a sidecar?                                          | **In-session v1.**  Persistence (`.prompts-ai-history.txt` keyed by project) is a Phase 4 polish item if it proves useful.                                                                                    |
| Q7 | Should the editor pane support the existing in-process style-warning overlays?                         | **No.**  Prompts aren't prose — flagging "really" / "very" in a prompt template would mislead.                                                                                                                |
| Q8 | Should `d` deletion in the list be reversible like map-entry delete (struck-through until save)?      | **Yes** — same staging UX from the config TUI for consistency.                                                                                                                                                |
| Q9 | Rollback retention?  Reuse `.config-backups/` policy of "keep all"?                                    | **Yes**, same policy.  Folder name is `.prompts-backups/` to avoid collision with config backups.                                                                                                              |
| Q10| Should the `{{selection}}` / `{{context}}` template variables be visible / explorable in the help?     | **Yes** — the help pane on the editor pane includes a one-line cheat sheet of recognised variables.                                                                                                            |

## 11. Implementation phases

### Phase 1 — read-only walk-through (1 day)

  * CLI plumbing (`inkhaven prompts-editor`).
  * Standalone TUI shell + four-pane layout.
  * Load `prompts.hjson` (or embedded defaults
    fallback).
  * List pane navigation + show-on-focus.
  * Editor pane displays the selected prompt
    read-only.
  * Help pane (Ctrl+H).

Useful immediately as a *prompt browser*.

### Phase 2 — editing + save (1–2 days)

  * Editor pane mutating (tui-textarea wired up).
  * `Ctrl+S` confirmation modal + atomic write +
    `.prompts-backups/` snapshot (reuse
    `config_tui::save::write_atomic` helpers).
  * `a` add prompt, `d` delete prompt (the same
    confirm/staging UX from the config TUI).
  * Per-prompt dirty tracking + status chips.
  * Restart-required overlay if any field in the
    library is referenced live by a running main
    TUI session — actually unnecessary since the
    main TUI reads prompts on demand.  Skip.

### Phase 3 — AI integration (1 day)

  * AI client init in `App::load`.
  * AI prompt input wired to `spawn_chat_stream`.
  * AI pane streams responses.
  * Provider chip in the AI pane title.
  * AI prompt history (in-session).
  * Spinner / elapsed-time / token-count chip.

### Phase 4 — rollback + polish (0.5 day)

  * `.prompts-backups/` rollback picker + preview.
  * Tab focus cycling with coloured borders.
  * Status-bar chord hints update with focus.
  * Tutorial + KEYBINDING.md row + RELEASE_NOTES
    write-up.

**Total estimate**: **3.5–4.5 days of focused work.**

## 12. Risks

| Risk                                                                                                  | Mitigation                                                                                     |
|--------------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------------------|
| `prompts.hjson` shape differs from what I assumed (looking at `assets/default_prompts.hjson`).        | Use the existing `PromptLibrary::load` / `save` types — they already model the on-disk shape. |
| Streaming UX feels janky if the LLM is slow (Ollama on a laptop).                                       | Spinner + elapsed-time chip + Esc cancellation of the in-flight stream.                       |
| User edits the system prompt while a stream is in flight — confusion about "which prompt was tested?". | Snapshot the editor body into the `Send` struct at the moment Enter fires; show the snapshot in the AI pane title chip if it differs from the current editor body. |
| Multi-line AI prompt input (some users want to type a paragraph-length test).                          | v1 keeps the single-line `TextInput`.  Multi-line input is §10 Q5's adjacent follow-up.        |
| Two prompts-editor sessions writing to the same `prompts.hjson` concurrently.                          | Mtime-watch on save: refuse to overwrite if disk mtime changed since load.  Surface a "file changed externally" reload modal.                                       |

## 13. Out of scope (v1)

  * Multi-turn conversation continuation.
  * Multi-line AI prompt input.
  * Provider switching inside the editor.
  * Render-prompt-with-`{{selection}}`-substituted preview.
  * Cost / token-budget warnings.

## 14. Status: ready for implementation

All four flagged questions (Q1 Tab cycles three
panes, Q2 template-rendered user message with no
system prompt, Q4 auto-populate defaults, Q5
single-shot — this is prompt assessment, not a
chat) are resolved.  Implementation can begin
against the phased plan in §11.
