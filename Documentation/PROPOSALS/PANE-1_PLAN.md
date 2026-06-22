# RFC PANE-1 — Output Pane Architecture

| | |
|---|---|
| **RFC** | PANE-1 |
| **Title** | Output Pane Architecture |
| **Status** | Draft |
| **Created** | 2026-06-24 |
| **Author** | Vladimir Ulogov |
| **Target version** | 1.4.0 |
| **Depends on** | none |
| **Supersedes** | none |
| **Enables** | WORLD-1 (world simulation); retrofits LANG-3 (translation, 1.3.23); retargets Bund `print`; surfaces LANG-1 and LANG-2 proposal queues |

> **Status note.** **Greenlit for implementation in the current 1.3.x cycle**
> (user-directed 2026-06-24): PANE-1 *is* the planned TUI rearchitecture the
> earlier "postpone TUI work" directive was waiting for, so it is now a **go**.
> (The header's 1.4.0 target predates that decision; work proceeds incrementally
> on 1.3.24-dev.) It is also the home for LANG-3's deferred translation-pane
> routing (the engine/CLI/Bund surface that routing needs already shipped in
> 1.3.23).

> **Progress (1.3.24-dev).** *P0 data layer landed:* `src/pane/output/` — the
> universal `Message` envelope (`types.rs`: kind string + metadata JSON +
> `Severity`/`Lifetime`/`ActionId`) and the per-project DuckDB-backed
> `OutputStore` (`store.rs`: `<project>/output.db` on the same `StorageEngine` as
> the progress/blob stores; emit, `active`/`by_kind` queries pinned-then-newest,
> dismiss/pin/snooze, and cleanup that drops time-expired and trims `Session(N)`
> kinds). Headless CLI surface `inkhaven output show|emit|dismiss|clear` (RFC
> §10.1) wired as the first consumer. Validated live (emit → show with severity
> icons → JSON filter → dismiss). Timestamps are unix-secs (not the RFC's
> `TIMESTAMP`) and project scope is the file (no `project_id` column) — both to
> match the in-tree stores. Zero new deps; tests 1576 → 1582. The rest of the
> envelope/store API is intentionally complete ahead of its consumers (TUI pane,
> emitter routing — the next increments).
>
> *P1 — the `ink.io.*` Bund stdlib landed* (RFC §9): in `src/scripting/stdlib/io.rs`,
> `ink.io.print` / `ink.io.log` (emit `bund_print` / `bund_log`, store_read —
> always available per §9), `ink.io.notify` (arbitrary kind + metadata dict,
> fs_write), and `ink.io.message.list` / `count` / `dismiss` / `pin` / `unpin`
> (reads store_read, mutations fs_write). The active project's `output.db` is
> cached per-process (re-opening would clash a second DuckDB instance). Validated
> live: a Bund script emits, the CLI `output show` reads the same store
> cross-process. **Deliberately deferred: redirecting the *bare* `print` word to
> Output** — until the ratatui pane renders `output.db`, that would send TUI print
> output where nothing displays it; it's a one-liner once the pane lands, so it
> ships with the widget. tests 1582.
>
> *P0 pane widget + cycling landed* — the TUI integration. `src/pane/output/`
> gains a process-global install (`install`/`active`/`uninstall`, mirroring
> `crate::progress`) so the App and `ink.io.*` share **one** `output.db` instance
> (two would clash); the App installs it on open. `App.right_pane: RightPane`
> (Output | AI; default AI to preserve the launch view) is cycled by **`Ctrl+B
> Tab` / `Ctrl+B Shift+Tab`** (handled in `handle_meta_action` before the
> modifier check, since BackTab carries Shift), which toggles the pane and focuses
> the region. The main `draw` dispatches `body[2]` to `draw_output` or `draw_ai`;
> `draw_output` (in `render/panes.rs`) renders active messages two lines each
> (severity icon + kind, then text), the selected row marked + bold when focused.
> `handle_output_key` drives the pane when focused: ↑/↓ (`j`/`k`) select, `g`/`G`
> first/last, `d` dismiss, `p` pin/unpin. Compiles clean; the store/CLI/`ink.io`
> path is validated end-to-end (interactive render verified by construction —
> mirrors `draw_ai` / `change_focus` / the meta-chord ladder). tests 1582. *Next:
> P2 — route LANG-3 translation into Output (unblocks the deferred translate-pane
> integration); the bare-`print`→Output redirect (needs the `Ctrl+Z E` eval-buffer
> interaction handled); pane persistence (`pane_state` table) + the ask-AI bridge.*
>
> *P2 — translation → Output routing landed.* The active-aware emitter
> `pane::output::emit(&msg)` (no-op unless a store is installed = the TUI) plus
> `cli::language::emit_translation_output(...)` (a `translation_result`, plus a
> `translation_uncovered_word_report` ⚠ when words are uncovered), called from
> both the CLI `translate`/`reverse`/`cross` handlers **and** the Bund
> `lang.translate`/`reverse`/`cross` words (the TUI runs Bund). So a translation in
> a TUI script lands in the Output pane; a shell CLI translation emits nothing
> (no store) and prints to stdout as before. Validated live (covered →
> `translation_result`, uncovered → result + ⚠ report, read back via `output
> show`). The generic pane renderer shows each via its `text` summary; the
> per-word trace expansion, the `r` Remember / `e` Edit actions, the ask-AI
> bridge, and the dedicated `Ctrl+B U T/V/X/…` chord family are follow-ons.
> tests 1582.
>
> *The ask-AI bridge landed (§8.9).* `a` on an Output message takes its full
> structured detail into the AI conversation **by reference, not value**:
> `App::ask_ai_about_output` arms the message's metadata as the one-shot
> `pending_rag_prefix` (the existing RAG-injection the Ctrl+B P/C flows use —
> prepended to the next prompt on submit), places a short visible quote
> (`about [kind] "…" — `) in the AI input, switches to the AI pane, and focuses
> the prompt so the author just types their question. So the model sees the rich
> context while the conversation shows only the quote + question. The Output pane
> gains a footer hint (`↑↓ select · a ask AI · d dismiss · p pin · ^B Tab → AI`).
> **Deviation from the RFC:** uses the codebase's resolve-at-press
> `pending_rag_prefix` rather than a hidden `@output-msg:uuid` handle resolved on
> send — same outcome (structured context reaches the AI; conversation stays the
> question), and consistent with the existing RAG flows. Compiles clean; verified
> by construction (mirrors `start_lexicon_inference`'s arm-prefix-then-focus
> path). tests 1582.
>
> *Trace expansion + Remember landed.* The forward `translation_result` now
> carries `{ trace, alternatives }` in its metadata (`emit_translation_output`
> gained an `extra` param; `translation_trace_json(&Translation)` builds it;
> reverse/cross pass null). In the pane: **`o` / Space** toggles per-message
> expansion (`App.output_expanded: HashSet<Uuid>`), and `draw_output` renders the
> per-word trace (`bird → kira (lexicon, 0.90)`) + alternatives inline (or the
> remaining metadata fields for kinds without a trace), with the scroll now
> tracking the selected entry's true first line (entries vary in height).
> **`r`** Remember (`App::remember_output_translation`) commits a
> `translation_result`'s `source → target` to the language's LANG-3 translation
> memory (embedding the source via the project store), guarding against uncovered
> (`«…»`) targets. Footer hint updated. tests 1582. *Next: the `Ctrl+B U`
> translation chord family; pane persistence + Output-default; then P3 LANG-1/2
> emitter retarget, P4–P6.*

---

## 1. Summary

Introduce a new right-side pane, **Output**, for structured one-way notifications from every Inkhaven subsystem. Output complements the existing AI pane: AI remains a conversation; Output absorbs everything Inkhaven needs to tell the author about without expecting a reply. A single common **message envelope** (kind, timestamp, metadata, actions, severity, lifetime, group key) lets every emitter — translation (1.3.23), Bund `print`/`log`, LANG-1 lexicon proposals, LANG-2 variety renderings, the future World simulator's fact-checker, future Consistency Sentinel — write through one channel.

Right-side pane cycling is `Ctrl+B Tab` / `Ctrl+B Shift-Tab` — always works regardless of focus, shifts focus to the cycled pane. Persistence is project-scoped, DuckDB-backed, and survives session boundaries. An "ask AI about this" gesture uses a hybrid quote-plus-reference handle so structured context reaches the AI on demand without polluting the conversation history.

PANE-1 is the foundation RFC. WORLD-1 depends on it. LANG-3's held-back TUI integration is unblocked by it. The Bund `print` situation (currently emitting to nowhere reliably visible) is fixed by it. And the broader architectural question of "where does structured feedback live in Inkhaven" gets a permanent, principled answer.

## 2. Motivation

Multiple subsystems in Inkhaven need to communicate structured, non-conversational information to the author:

- **LANG-3 translation (1.3.23, shipped).** The translation engine, CLI, and Bund surface are complete; the release notes explicitly hold the TUI integration for a "planned TUI rearchitecture." That rearchitecture is this one.
- **Bund `print` and `log`.** Currently emit to the terminal in ways that interact poorly with the TUI. Authors who write Bund scripts can't see their script output reliably without leaving the editor.
- **LANG-1 lexicon proposals.** When the AI proposes new lexicon entries, the proposal queue currently lives in a one-off overlay specific to LANG-1. The same pattern recurs elsewhere.
- **LANG-2 variety renderings.** `language lect` produces a rendering (`kata → kada`) that today lives in CLI stdout; the TUI gestures for variety rendering need a home.
- **WORLD-1 fact-checker (planned).** Will produce a continuous trickle of structured warnings as the author writes. Without a dedicated surface, those warnings either pollute the AI pane (the "feature creep on chat" failure mode) or invent a one-off pane just for fact-checking (creating UI debt).
- **Future Consistency Sentinel.** Same need as the fact-checker for character / place / artefact continuity.

Today's AI pane is a single timeline that mixes conversation with whatever else happens to be routed through it. Adding more kinds of content to that timeline scales badly — each new kind needs a tag and a filter, and the chat history becomes unreadable without filters that the author has to remember to apply. The honest architectural move is to recognize that **conversation** and **structured notification** are different UI patterns and give each its own surface.

PANE-1 makes that split. The AI pane keeps doing what it does. Output is the new home for everything else.

## 3. Goals

### 3.1 The pane

1. **A new right-side pane, Output**, separate from the AI pane, holding structured notifications from any subsystem.
2. **A common message envelope** — kind, timestamp, metadata (kind-specific), actions (subset declared per kind), severity, lifetime, group key, optional source references — used by every emitter.
3. **Flat-stream content model** with auto-grouping of bursts (5+ same-kind messages within a short window collapse into a single expandable header).
4. **Seven interaction primitives** (`d` dismiss, `Enter` primary action, `p` pin, `a` ask-AI, `P` promote, `s` snooze, `o`/`Space` expand details), with each message kind declaring the subset it supports.
5. **Severity model**: `info` / `warning` / `contradiction` / `progress` with consistent visual treatment.
6. **Lifetime policy per emitter**: each message carries a lifetime hint (session-bounded N, hourly, until-acted-on, until-paragraph-edited, never-expire), set at emission and configurable per source.
7. **Project-scoped persistence** in DuckDB; messages restore across sessions and snapshots.
8. **Cross-project isolation**: per-project Output channels; Bund follows the active project.

### 3.2 The cycling and focus

9. **`Ctrl+B Tab` cycles forward** through the right-side panes (Output → AI → … → Output) and shifts focus to the cycled pane.
10. **`Ctrl+B Shift-Tab` cycles backward** symmetrically.
11. **Cycling always works** regardless of current focus — editor, tree, overlays, anywhere.
12. **Implicit pane switching**: focusing the AI prompt input (by typing into it, or by an explicit chord that targets it) automatically switches the right-side pane to AI.
13. **Editor's `Tab` is preserved** — text input semantics are unchanged.

### 3.3 The bridges

14. **The ask-AI gesture (`a`)** on any Output message inserts a visible quote into the AI input box plus a hidden structured reference handle, so the AI prompt-builder retrieves full metadata when generating the reply — context flows by reference, not by ambient timeline mixing.
15. **AI-to-Output**: when a long-running AI task completes (batch translation, full-chapter regeneration, grammar-book build), a single completion notification lands in Output. AI conversational content otherwise stays in the AI pane.

### 3.4 The retargets

16. **Bund `print` and `log` route to Output** as `bund_print` / `bund_log` messages with the script source line carried in metadata for jump-to-source.
17. **LANG-3 translation results** (translate, reverse, cross, memory listings, corpus progress, eval, export) route to Output as appropriate kinds.
18. **LANG-1 lexicon proposals** and **LANG-2 variety renderings** route to Output as `lexicon_proposal` and `variety_rendering` kinds.
19. **The status line** sheds multi-line and persistent content; reduces to transient single-line state only (location, word count, active chord prefix, brief connection state).

### 3.5 The foundation

20. **Bund stdlib `ink.io.*`** for emitting structured messages from any user script.
21. **Documented kinds and extensibility**: future subsystems (World, Consistency Sentinel) declare their kinds against this envelope.
22. **No new external dependencies.** Reuse the in-tree DuckDB store, ratatui rendering stack, Bund VM, and existing chord-handling machinery.

## 4. Non-goals

- **Rich-media messages** (images, video, audio). Messages are text and structured data only. The image-preview pane covers image use cases via existing machinery.
- **Audio notifications.** Terminals can't reliably make sound; Output is visual only.
- **Cross-project shared Output channel.** Each project has its own; Bund follows the active project.
- **Multi-user collaborative dismissal.** Single-user system everywhere else in Inkhaven; same here.
- **AI training on dismissal patterns.** Promotions of dismissals to ledger entries are explicit author actions, not learned.
- **Reordering messages.** Apart from auto-grouping bursts (sequential collapse) and pinning (forced top), message order is timestamp-based and not user-customizable.
- **A dedicated translation pane** for active token-by-token sessions. LANG-3's retrieval architecture doesn't need one (per Decision 9 in the prior design rounds); the question reopens only if a future interactive translation flow is added.
- **An IDE-style "problems panel" with code-quality severities** beyond the four declared here. Output is a notification surface, not a linter dashboard.
- **External notification systems** (desktop notifications, browser tabs, mobile push). Inkhaven is terminal-native; Output is the terminal-native answer.

## 5. Constraints

- **Single binary, zero new dependencies.** Output reuses DuckDB, ratatui, tui-textarea, the existing Bund VM, the existing chord-handling and keymap-table machinery. No new crates.
- **Pure Rust.** All new code is pure Rust against in-tree dependencies.
- **Backwards-compatible with the existing AI pane.** Every existing AI chord and behavior is preserved; the AI pane only loses its accidental role as catch-all for non-conversational output.
- **No changes to project-on-disk layout.** Output state lives in DuckDB under `.inkhaven/`; no new top-level files.
- **Bund sandbox respected.** New `ink.io.*` words obey the existing sandbox categories (`fs_write` for state mutations like dismiss/pin; reads are unrestricted).
- **No breaking changes to LANG-1, LANG-2, LANG-3 CLI or Bund surfaces.** Routing those emitters into Output is additive — CLI and Bund behavior preserved exactly; the TUI gains the new surface.
- **DuckDB-only persistence.** No new database, no new file format. The Output message store is a small set of new tables in the existing project database.
- **ratatui-only rendering.** No new UI framework; the Output pane is a ratatui widget composed of existing primitives.

## 6. Audience

**Primary**: All Inkhaven users who interact with any subsystem that produces structured feedback. After PANE-1, that's effectively everyone — translation (already shipped), Bund script authors, lexicon proposals, varieties — and after WORLD-1, every author writing fiction in a built world.

**Secondary**: Future feature implementers (WORLD-1 author, Consistency Sentinel author, anyone adding a new structured emitter) who need a place to put their warnings, proposals, and progress reports without inventing a new UI each time.

**Tertiary**: Bund script authors who currently lose `print` output to nowhere visible inside the TUI. PANE-1 gives them a real, reliable surface.

## 7. Design overview

### 7.1 The right-side region

Inkhaven's existing layout has three regions: a tree pane on the left, an editor pane in the center, and a right-side region. Today the right-side region holds the AI pane. PANE-1 generalizes the right-side region to hold one of several **panes**:

- **Output** (default on first launch)
- **AI** (the existing conversation pane)
- **Translation** (only when a future active-session translation flow is implemented; reserved name, not built in PANE-1)

Only one right-side pane is visible at a time. The user cycles between them with `Ctrl+B Tab` / `Ctrl+B Shift-Tab`. The active right-side pane is per-project, persisted in session state.

### 7.2 The message envelope

Every emitter produces messages with the same outer shape:

```rust
pub struct Message {
    pub id: MessageId,                     // UUIDv7
    pub kind: MessageKind,                 // e.g. translation_result, bund_print, ...
    pub timestamp: DateTime,
    pub project_id: ProjectId,
    pub source_paragraph_id: Option<ParagraphId>,
    pub source_language_id: Option<LanguageId>,
    pub metadata: serde_json::Value,       // kind-specific shape
    pub actions: Vec<ActionId>,            // subset of the seven primitives
    pub severity: Severity,                // Info | Warning | Contradiction | Progress
    pub lifetime: Lifetime,                // policy at emission
    pub group_key: Option<GroupKey>,       // for auto-grouping bursts
    pub pinned: bool,                      // forced top
    pub snoozed_until: Option<DateTime>,   // hidden until this timestamp
    pub dismissed: bool,
    pub dismissed_at: Option<DateTime>,
    pub expires_at: Option<DateTime>,      // computed from lifetime at insert
    pub trace_id: Option<TraceId>,         // optional: for cross-message correlation
}
```

The `metadata` field is JSON whose shape depends on `kind`. Each kind documents its metadata schema; the envelope is universal.

### 7.3 The seven interaction primitives

| Key | Action | Notes |
|---|---|---|
| `Enter` | Primary action | Varies by kind: `insert_at_cursor`, `jump_to_source`, `accept_proposal`, etc. |
| `d` | Dismiss | Mark dismissed; remove from default view |
| `p` | Pin | Force to top; immune to auto-expiry |
| `a` | Ask AI | Quote into AI input with reference handle; switch panes |
| `P` | Promote | For warnings: promote to magic-ledger rule or permanent dismissal pattern |
| `s` | Snooze | Hide for N minutes (small inline prompt for N) |
| `o` or `Space` | Expand details | Show full metadata, trace, citations inline |

Each message kind declares which subset of these it supports. The Output pane renders a footer hint per focused message showing only applicable keys.

### 7.4 Severity and lifetime

**Severity** drives the visual treatment (color, icon, default position in the list). Four levels:

- `Info` — neutral notification. Translation results, Bund print, completed AI tasks.
- `Warning` — author attention recommended. Fact-checker subtle issues, uncovered translation words.
- `Contradiction` — direct conflict with declared facts or physics. Fact-checker hard violations.
- `Progress` — long-running operation status. Corpus seeding, training (when applicable), book assembly.

**Lifetime** is policy declared at emission and converted to an `expires_at` timestamp at insert:

- `Session(N)` — bounded cache of the most recent N messages of this kind.
- `Hours(N)` — auto-expire N hours after emission.
- `UntilActedOn` — persist until accepted, rejected, or dismissed.
- `UntilParagraphEdited` — auto-expire when the referenced paragraph changes (the warning was likely addressed).
- `Never` — persist until explicitly dismissed.

Defaults per emitter are listed in §8.2.

### 7.5 The cycling chord

`Ctrl+B Tab` / `Ctrl+B Shift-Tab` is a global chord that always:

1. Shifts focus to the right-side region (if not already there).
2. Cycles to the next (forward) or previous (backward) pane.
3. Persists the new active pane as the project's session state.

The chord is registered as a global handler in the keymap table, ranked above editor Tab handling. Editor `Tab` (insert tab) is preserved; only the `Ctrl+B Tab` chord targets pane cycling.

When only the Output and AI panes exist (the PANE-1 baseline), cycling alternates between them. When the Translation pane is added (deferred), it joins the cycle.

### 7.6 Persistence

Output messages are stored in DuckDB tables (§8.8) under `.inkhaven/db/`. Storage is project-scoped: each project has its own message store. Snapshots include the message store, so a snapshot restore brings back the Output state at that point in time.

Auto-cleanup runs on project open and every 5 minutes thereafter: messages past their `expires_at` and not pinned are deleted. Pinned messages, dismissed-but-archived messages (with optional archive retention), and never-expire messages persist indefinitely.

### 7.7 The Bund stdlib

A new family `ink.io.*` provides Bund-callable emitters:

- `ink.io.print` — write a `bund_print` message.
- `ink.io.log` — write a `bund_log` message with severity.
- `ink.io.notify` — emit a structured message of arbitrary kind.
- `ink.io.message.list / dismiss / pin / filter` — query and manipulate.

This makes Output usable from user scripts and lets future Bund-defined features hook in without modifying Rust.

## 8. Detailed design

### 8.1 Public API

```rust
pub mod pane {
    pub mod output {
        pub use super::types::*;
        pub use super::store::*;
        pub use super::emit::*;
        pub use super::query::*;
        pub use super::pane::*;
    }
}

pub mod types {
    pub struct Message { /* §7.2 */ }
    pub enum MessageKind { /* §8.2 */ }
    pub enum Severity { Info, Warning, Contradiction, Progress }
    pub enum Lifetime {
        Session(usize),
        Hours(f32),
        UntilActedOn,
        UntilParagraphEdited(ParagraphId),
        Never,
    }
    pub enum ActionId {
        Dismiss, Primary, Pin, AskAi, Promote, Snooze, Expand,
    }
}

pub mod emit {
    pub fn emit(message: Message) -> Result<MessageId>;
    pub fn emit_batch(messages: Vec<Message>) -> Result<Vec<MessageId>>;
}

pub mod query {
    pub fn active(project_id: ProjectId) -> Vec<Message>;
    pub fn by_kind(project_id: ProjectId, kind: MessageKind) -> Vec<Message>;
    pub fn by_source_paragraph(paragraph_id: ParagraphId) -> Vec<Message>;
    pub fn pending_for_kind(project_id: ProjectId, kind: MessageKind) -> usize;
}
```

Every emitter (translation engine, Bund VM, world compiler, fact-checker, etc.) calls `emit()` with a well-formed `Message`. The Output pane is a view over `active()`.

### 8.2 Message kinds at PANE-1 launch

Each kind specifies its metadata shape, default actions, default severity, default lifetime, and which emitter produces it.

#### `bund_print`

```hjson
metadata: {
    text: "..."
    script_path: "books/Scripts/example.bund"
    script_line: 42
}
```

Emitted by: Bund VM, when a script calls `ink.io.print` or the bare `print` word.
Default actions: `Dismiss`, `Primary` (jump to script line), `Pin`, `AskAi`, `Expand`.
Default severity: `Info`.
Default lifetime: `Session(100)`.

#### `bund_log`

```hjson
metadata: {
    text: "..."
    level: "info" | "warn" | "error"
    script_path: "..."
    script_line: 42
}
```

Default actions: `Dismiss`, `Primary`, `Pin`, `AskAi`, `Expand`.
Default severity: from `level` (info→Info, warn→Warning, error→Contradiction).
Default lifetime: `Session(200)`.

#### `translation_result`

Per LANG-3 (1.3.23). Source/target text, per-word confidence, decision trace, alternatives, uncovered words. See §9 in the LANG-3 release notes and the previous integration plan for the full shape.

Default actions: `Dismiss`, `Primary` (insert at cursor), `Pin`, `AskAi`, `Expand`, plus kind-specific `Remember` (extends the seven; see §8.3).
Default severity: `Info`.
Default lifetime: `Session(500)`.

#### `translation_memory_listing`

A listing of remembered pairs, typically as a single grouped message containing N pair-entries in the metadata.

Default actions: `Dismiss`, `Primary` (open pair for editing), `Expand`.
Default severity: `Info`.
Default lifetime: `UntilActedOn`.

#### `translation_corpus_progress`

Long-running corpus seeding. Initially emitted with `Progress` severity; updated in-place as the operation proceeds; converted to `Info` on completion.

Default actions: `Dismiss`, `Expand`.
Default severity: `Progress` → `Info` on completion.
Default lifetime: `Hours(24)` after completion.

#### `translation_eval_result`

```hjson
metadata: {
    language_id: "qya"
    roundtrip_similarity: 0.78
    coverage: 0.93
    sample_size: 200
    threshold_passed: true
    top_missing_concepts: ["journey", "courage", ...]
}
```

Default actions: `Dismiss`, `Pin`, `Expand`, `AskAi`.
Default severity: `Info`.
Default lifetime: `UntilActedOn`.

#### `translation_export_result`

```hjson
metadata: {
    bundle_path: "build/qya-pack.itm"
    size_bytes: 38421
    languages: ["qya"]
}
```

Default actions: `Dismiss`, `Primary` (reveal in filesystem), `Expand`.
Default severity: `Info`.
Default lifetime: `Hours(72)`.

#### `translation_uncovered_word_report`

```hjson
metadata: {
    language_id: "qya"
    source_text: "the warrior raised his sword"
    uncovered_words: ["warrior"]
    paragraph_id: "ch07-p042"
}
```

Default actions: `Dismiss`, `Primary` (open lexicon editor pre-filled), `AskAi`, `Expand`.
Default severity: `Warning`.
Default lifetime: `UntilParagraphEdited(paragraph_id)`.

#### `lexicon_proposal`

Emitted by LANG-1's AI lexicon-generation pipeline (§8.10 in LANG-1). Each proposed entry becomes a message.

```hjson
metadata: {
    language_id: "qya"
    proposed_headword: "karak"
    proposed_gloss: "stone"
    proposed_ipa: "kaˈrak"
    proposed_part_of_speech: "noun"
    rationale: "..."
}
```

Default actions: `Dismiss`, `Primary` (accept and commit), kind-specific `Edit`, `AskAi`, `Expand`.
Default severity: `Info`.
Default lifetime: `UntilActedOn`.

#### `variety_rendering`

LANG-2 (1.3.22) renderings of a base word in a variety/dialect.

```hjson
metadata: {
    language_id: "qya"
    variety: "northern_dialect"
    base_form: "kata"
    rendered_form: "kada"
    sound_changes_applied: [{ rule: "t > d / V_V", position: 2 }]
}
```

Default actions: `Dismiss`, `Primary` (insert at cursor), `AskAi`, `Expand`.
Default severity: `Info`.
Default lifetime: `Session(200)`.

#### `ai_task_complete`

Single-line notification when a long-running AI task finishes.

```hjson
metadata: {
    task: "grammar_book_chapter_regeneration"
    target: "books/Languages/Quenya/Grammar/04-morphology"
    elapsed_seconds: 47
    summary: "Regenerated chapter 04: morphology. 8 paragraphs updated."
}
```

Default actions: `Dismiss`, `Primary` (jump to target), `Pin`.
Default severity: `Info`.
Default lifetime: `Hours(12)`.

#### Future kinds (declared in future RFCs; not implemented in PANE-1)

- `world_compiler_proposal` — Place/Fact/Note proposals from `inkhaven realworld` (WORLD-1).
- `fact_check_warning` — Real-time fact-checker output (WORLD-1).
- `consistency_warning` — Future Consistency Sentinel (continuity across Characters/Places/Artefacts).
- `manuscript_scan_finding` — Undefined conlang word, broken wiki-link, etc.

Each future kind documents its metadata, actions, severity, and lifetime in its own RFC. PANE-1 only commits to the envelope and the rendering mechanics.

### 8.3 Kind-specific actions beyond the seven

A message kind may declare an action outside the primitive seven. Example: `translation_result` has a `Remember` action (key `r`) that commits the translation to memory. Such extra actions:

- Use a key letter not already in the seven (`r`, `e`, `c`, etc.).
- Are documented in the kind's specification.
- Render in the footer hint along with the primitive actions.
- Are wired through the same dispatch machinery as the primitives.

Output's UI shows the action keys in the footer of the focused message, prefixed by the kind name so the author sees what's available:

```
[translation_result]  Enter:insert  r:remember  e:edit+remember
                      a:ask AI       o:expand   d:dismiss
```

### 8.4 Auto-grouping bursts

When 5 or more messages of the same kind arrive within 30 seconds (configurable), they auto-collapse into a single group header:

```
[+] Bund print  ×12 messages in last 8s
    "loading lexicon..."
    "lexicon loaded: 487 entries"
    ...
```

Expanding the group (with `o` on the header) shows the individual messages. Solo messages and messages outside the burst window display normally.

The grouping uses `group_key`: emitters can provide one explicitly (forcing grouping of related messages), or it auto-derives from `kind + closest-second timestamp + paragraph_id`.

### 8.5 Filtering and search

The Output pane shows a filter bar above the message list, hidden by default and toggled with `/`. Filters:

- **By kind**: multi-select from currently-present kinds.
- **By severity**: include Info / Warning / Contradiction / Progress.
- **By time range**: last hour, last day, last week, all.
- **By source**: paragraph, language, script.
- **Text search**: substring match across message metadata.

Filters are session-scoped; resetting Inkhaven returns to default (everything visible).

### 8.6 The pane UI

```
┌─ Output ────────────────────────────────────────────────────────────────┐
│ [/] filter   [a]ll severities ▾   [k]inds: all ▾                        │
│ ─────────────────────────────────────────────────────────────────────── │
│                                                                         │
│ ● 15:23:14  translation_result  qya                            (0.86)   │
│   "the warrior raised his sword" → "I ohtar ortanë macilirya"           │
│                                                                         │
│ ⚠ 15:21:47  translation_uncovered_word_report  qya                      │
│   1 word couldn't be translated: «midnight»                             │
│                                                                         │
│ ● 15:18:02  bund_print                                                  │
│   "Generated 50 candidate words, accepted 22."                          │
│                                                                         │
│ [+] 15:15:31  Bund print  ×12 messages                                  │
│                                                                         │
│ ⚠ 15:14:08  lexicon_proposal  qya                                       │
│   karak (n.) — "stone"   Etymology: deterministic CVC                   │
│                                                                         │
│ ─────────────────────────────────────────────────────────────────────── │
│ [translation_result] Enter:insert  r:remember  a:ask AI                 │
│ o:expand  d:dismiss  Ctrl+B Tab:switch pane                             │
└─────────────────────────────────────────────────────────────────────────┘
```

Visual conventions:

- **Severity icons**: `●` info, `⚠` warning, `⊗` contradiction, `↻` progress.
- **Color-coded left margin**: per-source per-kind.
- **Timestamp** in 24-hour HH:MM:SS, leading column.
- **Kind name** in muted color, after timestamp.
- **Severity-tinted background** for warnings and contradictions; neutral for info.
- **Focused message** has a brighter border and shows the action footer.
- **Group headers** show `[+]` collapsed, `[-]` expanded.

The pane scrolls; arrow keys navigate between messages; `gg` jumps to oldest, `G` to newest (matching established Inkhaven vim-style navigation in other panes).

### 8.7 The cycling chord (Option B locked)

`Ctrl+B Tab` is registered in the keymap table at the global-chord level. Behavior:

1. The handler reads the project's current right-side region pane.
2. Computes the next pane in the cycle order: `Output → AI → (Translation if exists) → Output → …`.
3. Switches the right-side region to the next pane.
4. Shifts focus to that pane (overriding whatever focus was previously held).
5. Persists the new active pane to session state.

`Ctrl+B Shift-Tab` is the same operation in reverse order.

Both chords work from any focus context: editor, tree, overlays, AI input box (where the chord prefix `Ctrl+B` already absorbs the keystroke). Existing chords with `Ctrl+B Tab` collision: none — `Tab` is not currently bound under the `Ctrl+B` prefix.

### 8.8 Implicit pane switching to AI

When the user focuses the AI prompt input box (by typing into it, or by an explicit chord that targets it — typically the existing `Ctrl+B K` or its evolution), the right-side region automatically switches to AI if it isn't already. This means the author's natural gesture of "let me ask the AI something" doesn't require an explicit pane switch.

The auto-switch is a one-way trigger: focusing AI input switches to AI. There is no inverse trigger (typing in the editor doesn't switch back to Output — the cycling chord handles that explicitly).

### 8.9 The ask-AI bridge

The `a` action on any Output message:

1. Captures the message's structured reference (its `id` and `kind`).
2. Builds a quote string from the message: a one-to-three-line text representation appropriate to the kind.
3. Switches the right-side pane to AI (implicit switch, §8.8 applies).
4. Inserts the quote into the AI input box with the reference handle hidden behind the visible quote:

```
> [translation_result] English → Quenya
> "the warrior raised his sword" → "I ohtar ortanë macilirya" (conf 0.86)
> RBMT, no memory match, alternatives: 1
@output-msg:01H7XYZ123ABC...

```

The `@output-msg:UUID` is hidden in normal rendering (zero-width or stylized as a low-contrast tag); visible only when the user explicitly inspects the input contents.

5. The cursor is positioned after the quote so the author types their question.
6. When the AI prompt is sent, the AI's RAG context-builder detects the reference handle, fetches the message's full metadata from DuckDB, and includes it as structured context for the LLM call. The user-visible prompt contains just the question; the AI's view of the prompt includes the full reference details.

The same machinery accommodates multiple references in one prompt (the author can ask about several Output messages at once).

### 8.10 AI-to-Output completion notifications

Long-running AI tasks (book chapter regeneration, batch translation, full grammar-book build, full-chapter fact-check, world-compiler runs, etc.) emit a single `ai_task_complete` message to Output when finished. This lets the author switch panes during a long task and still get notified.

Tasks that qualify (declared in their respective subsystems):

- `grammar_book_chapter_regeneration`
- `grammar_book_full_build`
- `lexicon_batch_generation`
- `world_compile_full` (when WORLD-1 lands)
- `batch_translation`
- `corpus_seed`

Conversational AI replies (the response to a user prompt) do **not** emit to Output. They're conversation, they stay in AI.

### 8.11 Status line scope reduction

With Output absorbing multi-line and persistent content, the status line is reduced to:

- **Current location** (`Book 1 > Chapter 7 > §2`).
- **Word count** for current paragraph.
- **Active language** (when conlang work is in progress).
- **Active chord prefix** (when a multi-key chord is in progress).
- **AI provider connection state** (e.g., `Claude` or `[disconnected]`).
- **One-line operation indicators** (`Saving…`, `Compiling…`, `Indexing…`).
- **Brief error toasts** for things not deserving full Output entries (`File not found`).

What no longer goes there: Bund script output, multi-line operation results, persistent warnings. Those move to Output.

This is mostly a clarification of existing intent rather than a breaking change; the status line was already moving in this direction.

### 8.12 The Output ↔ Bund relationship

Bund scripts can both produce and consume Output messages:

**Produce** via `ink.io.*`:

```bund
\ Print to Output
"Loading lexicon..." ink.io.print

\ Structured notify
:translation_result
    "source"  "I see the sky"          dict.set
    "target"  "Mira anar nan"          dict.set
    "confidence"  0.78                  dict.set
ink.io.notify
```

**Consume** via `ink.io.message.*`:

```bund
\ List all pending lexicon proposals
:lexicon_proposal ink.io.message.list

\ Dismiss all bund_print older than 1 hour
:bund_print 1 hours.ago ink.io.message.filter
[ ink.io.message.dismiss ] each
```

The Bund stdlib is sandbox-classified: read operations require no permission; write operations (emit, dismiss, pin) require `fs_write`.

### 8.13 Routing 1.3.23 translation through Output

LANG-3's existing engine, CLI, and Bund surfaces are preserved exactly. The only change is that when running inside the TUI, each translation command additionally emits a `translation_result` (or appropriate kind) to Output:

- `language translate <lang> "<en>"` → `translation_result`
- `language reverse <lang> "<surface>"` → `translation_result` (direction=reverse)
- `language cross <a> <b> "<surface>"` → `translation_result` (direction=cross)
- `language remember ...` → no Output message; the memory is updated silently (next translation that hits the memory carries `memory_match` metadata)
- `language corpus <lang>` → `translation_corpus_progress` (live-updated) → `translation_corpus_progress` on completion
- `language eval <lang>` → `translation_eval_result`
- `language export-translation <lang>` → `translation_export_result`
- `language memory list <lang>` → `translation_memory_listing`

Uncovered words in a `translation_result` additionally emit a paired `translation_uncovered_word_report` so the author has an actionable proposal to add them to the lexicon.

The TUI chord `Ctrl+B U T` (translate, scope = selection) and its siblings (`U V`, `U X`, `U M`, `U R`, `U S`, `U E`, `U P`) wrap the existing CLI invocations; results land in Output automatically.

### 8.14 Routing Bund `print` and `log` through Output

Currently Bund's `print` either writes to stdout (escaping the TUI screen) or to a transient strip somewhere; behavior is inconsistent. After PANE-1:

- `print` writes a `bund_print` message to Output.
- The script source line is captured in metadata so `Enter` on the message jumps the editor to the source.
- The previous stdout-escape path is preserved when running Bund outside the TUI (`inkhaven bund run script.bund` from a shell, where there's no Output to receive the message).
- Existing scripts that use `print` get the new behavior for free; no script changes needed.

### 8.15 Persistence semantics

Message storage is project-scoped: each project has its own message tables. Switching projects switches the visible Output.

Snapshots include the message store. A snapshot taken at time T captures the Output state at time T; restoring the snapshot restores that state. This is useful when the author wants to revert a session that produced many warnings or proposals.

The auto-cleanup background task runs on:

- Project open (clean up expired messages from the previous session).
- Every 5 minutes during a session.
- Project close (cleanup pass before snapshot or shutdown).

Cleanup deletes messages where `expires_at < now()` AND NOT `pinned`. Dismissed messages are also kept for a configurable retention window (default 30 days) so the dismissal-pattern promotion feature has data to work with, then deleted.

## 9. Bund stdlib

New family under `ink.io.*`. All sandbox-classified per the existing policy.

| Word | Sandbox | Description |
|---|---|---|
| `ink.io.print` | none | Emit a `bund_print` message |
| `ink.io.log` | none | Emit a `bund_log` message with explicit severity |
| `ink.io.notify` | `fs_write` | Emit a structured message of arbitrary kind |
| `ink.io.message.list` | none | List active messages (filterable by kind, project, severity) |
| `ink.io.message.get` | none | Get a single message by ID |
| `ink.io.message.dismiss` | `fs_write` | Dismiss a message |
| `ink.io.message.pin` | `fs_write` | Pin a message |
| `ink.io.message.unpin` | `fs_write` | Unpin |
| `ink.io.message.snooze` | `fs_write` | Snooze for N minutes |
| `ink.io.message.filter` | none | Filter active messages by criteria |
| `ink.io.message.count` | none | Count active messages, optionally by kind |
| `ink.io.kind.register` | `code_eval` | Declare a new kind from Bund (advanced) |

New hooks fire from the message store:

- `hook.on_message_emit` — fires after every emission (debounced 100ms).
- `hook.on_message_dismiss` — fires when a message is dismissed.
- `hook.on_message_promote` — fires when a dismissal is promoted to a pattern.
- `hook.on_pane_switch` — fires when the right-side pane changes.

These let advanced users automate workflows: e.g., a script that auto-dismisses `bund_print` older than 10 minutes, or that auto-pins any `contradiction`-severity message.

## 10. Surfaces

### 10.1 CLI

Minimal CLI surface — Output is primarily a TUI feature:

```
inkhaven output show [--kind <k>] [--severity <s>] [--limit N]
inkhaven output dismiss <message-id>
inkhaven output clear [--kind <k>] [--all]
inkhaven output emit <kind> --metadata <json>   # for testing / scripting
```

The `output show` command renders a plain-text view of active messages, suitable for piping to other tools or sshing into a project without a TUI.

### 10.2 TUI

The new pane plus the cycling chords. Specifically:

- `Ctrl+B Tab` / `Ctrl+B Shift-Tab` — pane cycling (§7.5).
- Inside Output: arrow keys to navigate messages, the seven primitive actions plus any kind-specific extras, `/` to toggle the filter bar, `gg` / `G` for first/last.
- Implicit pane switch to AI when the AI prompt input focuses (§8.8).
- The existing AI pane chords (`Ctrl+B K`, `Ctrl+B C`, etc.) preserved exactly.

### 10.3 Book-Take

No impact. Output state is session-scoped and not part of book artifacts. The `.itm` bundle from LANG-3 is a book-take format; Output messages about export progress flow through Output but the bundle itself ships via book-take as before.

## 11. Dependency selection

**No new direct dependencies.** Everything PANE-1 needs is already in-tree:

- `duckdb` — message store.
- `ratatui` + `tui-textarea` — pane rendering.
- `serde` + `serde_json` — metadata serialization.
- `chrono` — timestamps.
- `uuid` — message IDs.
- The existing keymap table — chord registration.
- The existing Bund VM — `ink.io.*` stdlib.
- The existing project / session / snapshot machinery — persistence.

This is the single tightest dependency budget of the four RFCs so far. PANE-1 is structural, not capability-adding; the work is wiring and rendering, not new algorithms.

## 12. Implementation phases

**P0 — Pane infrastructure (1 week).**

- New DuckDB tables (§16.A schema).
- The `Message` type, `emit()`, `query()` API.
- The Output pane ratatui widget.
- Severity icons, colors, footer hint rendering.
- Filter bar UI.
- Arrow-key navigation, action key dispatch.
- The cycling chord `Ctrl+B Tab` / `Ctrl+B Shift-Tab`.
- Implicit AI-pane switching on prompt focus.
- Auto-cleanup background task.
- Per-project state, snapshot integration.

**P1 — Bund `print` retargeting (3 days).**

- Bund VM's `print` word redirects to `ink.io.print`.
- `ink.io.print`, `ink.io.log`, `ink.io.notify` Bund stdlib.
- `ink.io.message.*` query and manipulation words.
- Tests on fixture scripts.

**P2 — LANG-3 translation routing (1 week).**

- Each translation command emits its appropriate kind to Output when running in the TUI.
- The TUI chord family `Ctrl+B U T/V/X/M/R/S/E/P` wired against the existing CLI/Bund surface.
- `translation_result` rendering with per-word confidence and trace expansion.
- The `Remember` (`r`) and `Edit+Remember` (`e`) kind-specific actions.
- The ask-AI bridge specifically for translation messages.
- `translation_uncovered_word_report` follow-up emission.

**P3 — LANG-1 / LANG-2 emitter retargeting (3 days).**

- LANG-1 lexicon proposals emit `lexicon_proposal` messages.
- LANG-2 variety renderings emit `variety_rendering` messages.
- Existing LANG-1/LANG-2 overlay code that previously rendered proposals inline migrates to consuming Output.

**P4 — Status line cleanup (3 days).**

- Multi-line content removed from status line.
- Transient single-line contract documented and enforced.
- Cosmetic pass on status line layout.

**P5 — AI completion notifications (2 days).**

- Long-running AI tasks emit `ai_task_complete` on finish.
- Six initial qualifying tasks wired (§8.10 list).

**P6 — Documentation, examples, integration tests (4 days).**

- `Documentation/OUTPUT_PANE.md` user guide.
- `Documentation/Tutorials/26-using-output-pane.md` walkthrough.
- Example Bund scripts demonstrating `ink.io.*`.
- End-to-end TUI tests for cycling, action dispatch, filtering, persistence.

**Total: ~3 weeks for one developer.**

P0 is the foundational week; everything else is wiring that depends on it. P1 (Bund print) is the smallest test that the new pipeline works correctly. P2 (translation) is the biggest functional win — it unblocks the 1.3.23 held-back TUI integration. P3 retrofits existing LANG-1/LANG-2 surfaces. P4–P6 are polish.

## 13. Testing strategy

- **Unit tests** on the message store: emit, query, filter, dismiss, pin, snooze, auto-cleanup. Property tests on lifetime expiration (`expires_at` computed correctly from `lifetime` and `created_at`).
- **Snapshot/restore tests**: emit messages, snapshot, modify, restore, verify Output state matches the snapshotted state.
- **Per-project isolation**: open two projects in sequence, emit different messages, switch between them, verify no leakage.
- **Cycling chord tests**: from each starting pane and focus context (editor, tree, AI input, overlays), verify `Ctrl+B Tab` lands on the next pane and shifts focus.
- **Implicit pane-switch tests**: focus the AI prompt input via various paths (chord, typing, mouse where supported); verify Output→AI auto-switch.
- **Action dispatch tests**: each of the seven primitives plus the kind-specific extras (`Remember`, `Edit`) dispatches correctly per kind; unsupported actions are not bound.
- **Auto-grouping tests**: emit 6 same-kind messages within 30s, verify group header; outside the window, verify no grouping.
- **Filter persistence**: set filters in one session, verify they reset on next project open (session-scoped).
- **Ask-AI bridge tests**: invoke `a` from a translation_result, verify AI input gets the quote + reference handle, verify AI prompt-builder retrieves full metadata on send.
- **Bund integration tests**: scripts emit via `ink.io.notify`; consumers query via `ink.io.message.list`; sandbox enforcement on writes.
- **Long-task completion tests**: kick off a grammar-book chapter regeneration, verify `ai_task_complete` lands on completion.
- **Performance**: 10,000-message store, query and render time <100ms; auto-cleanup of 1,000 expired messages <50ms.

## 14. Risks and alternatives

**Risk: Output becomes a dumping ground.** If every subsystem treats Output as "where I put stuff I don't know what to do with," the pane fills with noise. Mitigation: kind discipline. Each kind has a documented purpose, default severity, and default lifetime. New kinds require an RFC change. The filter bar handles signal-to-noise within a session.

**Risk: The cycling chord collides with future needs.** Reserving `Ctrl+B Tab` for cycling locks it; future features can't use it. Mitigation: cycling is a sufficiently universal gesture that locking it is justified. If a hypothetical future feature genuinely needs `Ctrl+B Tab`, the cycle gesture can move to another binding via the existing keymap-table mechanism without breaking anything else.

**Risk: Auto-cleanup deletes messages the author wanted.** If a message expires before the author saw it, that's information loss. Mitigation: per-source lifetime defaults are conservative (mostly `UntilActedOn` or `Session(N)` with N=500+); pinning is one keystroke; the dismissed-archive retention (30 days) gives an undo window. The auto-cleanup is also logged so the author can see what was cleaned.

**Risk: Implicit AI-pane switching surprises users.** Focusing the AI input shouldn't *also* switch the visible pane in a way the user didn't expect. Mitigation: the switch is deterministic and reversible (Ctrl+B Shift-Tab brings Output back); the AI input by definition takes focus only on a deliberate gesture (typing, chord); the pattern matches how IDE consoles auto-focus on output during a build.

**Risk: The ask-AI reference handle leaks into the AI's visible response.** If the LLM repeats the `@output-msg:UUID` token back in its reply, the conversation history gets uglier. Mitigation: the reference is system-side context; the user-visible prompt is just the quote text; the LLM is prompted to never reproduce reference handles in its replies (system prompt constraint).

**Risk: Per-project isolation is too strict for users who want a global Output.** A user with many projects might want a unified notification stream. Mitigation: not a real need based on the audience; if it ever becomes one, a "global Output" view can be added without changing the per-project storage.

**Risk: Bund print routing breaks scripts that relied on stdout.** Existing Bund scripts that called `print` for shell output (`inkhaven bund run` from a shell) might suddenly emit nothing visible. Mitigation: outside the TUI, `print` still writes stdout. Inside the TUI, it writes to Output. The distinction is auto-detected; scripts don't need changes.

**Risk: ratatui rendering performance with thousands of messages.** Naive rendering of large message lists is slow. Mitigation: ratatui's viewport renders only visible rows; the message store query is indexed and paginated.

**Alternative considered: keep mixing into the AI pane with tags and filters.** Rejected; explained in §2 and the prior design rounds. The mixing approach scales badly as more emitters are added.

**Alternative considered: contextual Tab (no `Ctrl+B` prefix).** Rejected per the explicit user decision; Option B (`Ctrl+B Tab`) was chosen because it's globally available, predictable, and doesn't depend on focus state.

**Alternative considered: a multi-pane right-side region (Output + AI visible simultaneously).** Rejected for current scope; the terminal width on typical laptops is too narrow to display two right-side panes plus tree + editor without sacrificing readability. Power users with wide terminals can revisit later via a configuration option.

**Alternative considered: messages as full first-class objects with their own DuckDB tree-position.** Rejected; messages are ephemeral by design and should not pollute the manuscript's structural tree. Project-scoped DuckDB tables are the right home.

## 15. Open questions

1. **Filter persistence scope.** Should filters reset per session or persist? Recommendation: session-scoped to avoid the "I forgot I had filters on" surprise. Confirm.

2. **Default sort direction.** Newest-first or oldest-first by default? Recommendation: newest-first (most common in notification UIs). Sortable per session.

3. **Pinned-message limit.** Should there be a cap on how many messages can be pinned? Recommendation: yes, default 20, configurable. Otherwise the pinned section can dominate.

4. **Snooze duration prompt.** When the user presses `s`, do we present a small prompt for the snooze duration (15min / 1hr / 4hr / custom), or use a fixed default (1 hour)? Recommendation: small prompt with sensible presets.

5. **Dismissal-pattern promotion UX.** When the user has dismissed N similar messages, do we proactively suggest "promote to a pattern"? Recommendation: yes, after 5 similar dismissals, surface a one-time suggestion as its own Output message. Configurable threshold.

6. **The `@output-msg:UUID` reference syntax.** Is UUIDv7 the right format here, or should we use a shorter human-readable handle (`@output-msg:42`)? Recommendation: UUIDv7 internally; display a short index in the UI for human reference if needed.

7. **Multi-message ask-AI.** Should the author be able to select multiple Output messages and ask the AI about all of them at once? Recommendation: yes, as a follow-up — visual-selection mode (`v`) in the Output pane, then `a` quotes all selected messages.

8. **Color theme integration.** Should severity colors come from the project's HJSON theme configuration or be hardcoded? Recommendation: configurable, with sensible defaults.

9. **Group expansion persistence.** When a burst-group is expanded by the user, should the expansion persist across sessions? Recommendation: yes, per-session at minimum; per-project across sessions as a follow-up.

10. **Background indicator for Output unread.** When the user is in AI pane and Output gets new messages, do we show a small badge ("3 new") on the right-side pane indicator? Recommendation: yes, in the status line: `Output (3)  AI`.

## 16. Appendices

### A. Full DuckDB schema

```sql
-- The main message store
CREATE TABLE pane_output_messages (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL,
    kind VARCHAR NOT NULL,
    timestamp TIMESTAMP NOT NULL,
    metadata_json JSON NOT NULL,
    actions_json JSON NOT NULL,         -- array of ActionId
    severity VARCHAR NOT NULL,           -- 'info' | 'warning' | 'contradiction' | 'progress'
    lifetime_kind VARCHAR NOT NULL,      -- 'session' | 'hours' | 'until_acted' | 'until_paragraph_edit' | 'never'
    lifetime_value VARCHAR,              -- N (for session/hours), paragraph_id (for until_paragraph_edit)
    expires_at TIMESTAMP,                -- computed at insert; NULL = never
    pinned BOOLEAN DEFAULT FALSE,
    dismissed BOOLEAN DEFAULT FALSE,
    dismissed_at TIMESTAMP,
    snoozed_until TIMESTAMP,
    group_key VARCHAR,
    source_paragraph_id UUID,
    source_language_id UUID,
    trace_id UUID,
    created_at TIMESTAMP DEFAULT NOW()
);

CREATE INDEX idx_messages_active
    ON pane_output_messages (project_id, dismissed, expires_at, timestamp DESC)
    WHERE NOT dismissed;

CREATE INDEX idx_messages_kind
    ON pane_output_messages (project_id, kind, timestamp DESC);

CREATE INDEX idx_messages_paragraph
    ON pane_output_messages (source_paragraph_id, dismissed)
    WHERE source_paragraph_id IS NOT NULL;

CREATE INDEX idx_messages_group
    ON pane_output_messages (project_id, group_key, timestamp DESC)
    WHERE group_key IS NOT NULL;

-- Dismissal patterns (promoted from individual dismissals)
CREATE TABLE pane_dismissal_patterns (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL,
    kind VARCHAR NOT NULL,
    pattern_json JSON NOT NULL,          -- structural matching criteria
    rationale TEXT,
    created_at TIMESTAMP DEFAULT NOW(),
    dismissal_count INTEGER DEFAULT 0    -- how many messages this pattern has silenced
);

CREATE INDEX idx_patterns_kind
    ON pane_dismissal_patterns (project_id, kind);

-- Per-project pane state (active pane, filter state)
CREATE TABLE pane_state (
    project_id UUID PRIMARY KEY,
    active_pane VARCHAR NOT NULL DEFAULT 'output',  -- 'output' | 'ai' | 'translation'
    filter_json JSON,                                -- active filter set
    updated_at TIMESTAMP DEFAULT NOW()
);
```

### B. Full HJSON configuration

```hjson
output_pane: {
    enabled: true                       // master switch

    layout: {
        default_active_pane: "output"   // on first launch
        cycle_order: ["output", "ai"]   // when only two panes exist
        show_unread_badge: true
    }

    grouping: {
        enabled: true
        burst_window_seconds: 30
        burst_threshold: 5
    }

    lifetime_defaults: {
        // Per-kind overrides
        bund_print: { kind: "session", value: "100" }
        bund_log: { kind: "session", value: "200" }
        translation_result: { kind: "session", value: "500" }
        translation_uncovered_word_report: { kind: "until_paragraph_edit" }
        lexicon_proposal: { kind: "until_acted" }
        variety_rendering: { kind: "session", value: "200" }
        ai_task_complete: { kind: "hours", value: "12" }
    }

    cleanup: {
        background_interval_minutes: 5
        dismissed_retention_days: 30
    }

    pinning: {
        max_pinned: 20
    }

    appearance: {
        severity_icons: { info: "●", warning: "⚠", contradiction: "⊗", progress: "↻" }
        severity_colors: { info: "white", warning: "yellow", contradiction: "red", progress: "cyan" }
        focused_border: "bright"
        timestamp_format: "HH:mm:ss"
    }

    actions: {
        // Key bindings for the seven primitives (overridable via standard keymap mechanism)
        primary: "Enter"
        dismiss: "d"
        pin: "p"
        ask_ai: "a"
        promote: "P"
        snooze: "s"
        expand: "o"                     // or "Space"
    }

    cycling: {
        forward: "Ctrl+B Tab"
        backward: "Ctrl+B Shift-Tab"
    }

    bridges: {
        implicit_switch_to_ai_on_prompt_focus: true
        ask_ai_quote_lines: 3           // max lines of quote in AI input
    }

    status_line: {
        scope: "transient_only"         // 'transient_only' | 'full' (legacy)
        show_output_unread_badge: true
    }

    bund: {
        print_route: "output"           // 'output' | 'stdout' | 'both'
        log_default_severity: "info"
    }
}
```

### C. Sample TUI overlay variations

**Output pane with filter bar active:**

```
┌─ Output ────────────────────────────────────────────────────────────────┐
│ Filter: severity=[warning, contradiction]  kind=[translation_*, fact_*] │
│ ─────────────────────────────────────────────────────────────────────── │
│                                                                         │
│ ⚠ 15:21:47  translation_uncovered_word_report  qya                      │
│   1 word couldn't be translated: «midnight»                             │
│                                                                         │
│ ⊗ 14:12:08  fact_check_warning (travel_time)  ch07-p042                │
│   612 km in 3 days = 204 km/day, exceeds pre-industrial pace            │
│                                                                         │
│ ─────────────────────────────────────────────────────────────────────── │
│ Showing 2 of 47 messages   /:close filter   Ctrl+B Tab:switch           │
└─────────────────────────────────────────────────────────────────────────┘
```

**Output pane with grouped burst expanded:**

```
┌─ Output ────────────────────────────────────────────────────────────────┐
│                                                                         │
│ [-] 15:18:30  Bund print  ×8 messages in last 4s                       │
│     "Starting lexicon scan..."                                          │
│     "Found 487 lexicon entries"                                         │
│     "Checking phonotactic validity..."                                  │
│     "All entries valid"                                                 │
│     "Building vector store..."                                          │
│     "Vector store built: 487 vectors"                                   │
│     "Indexing complete"                                                 │
│     "Total time: 1.2s"                                                  │
│                                                                         │
│ ─────────────────────────────────────────────────────────────────────── │
│ [+] collapse group   d:dismiss all   p:pin all                          │
└─────────────────────────────────────────────────────────────────────────┘
```

**Translation result expanded (with `o`):**

```
┌─ Output ────────────────────────────────────────────────────────────────┐
│                                                                         │
│ ● 15:23:14  translation_result  qya                              (0.86)│
│   "the warrior raised his sword"                                        │
│   → "I ohtar ortanë macilirya"                                          │
│                                                                         │
│   Per-word trace:                                                       │
│     the         → I             (lexicon: def_article, conf 0.95)       │
│     warrior     → ohtar         (lexicon: warrior_n, conf 0.92)         │
│     raised      → ortanë        (morphology: past_3sg_active, 0.84)     │
│     sword       → macilirya     (morphology: acc_poss_3sg, 0.78)        │
│                                                                         │
│   Alternatives:                                                         │
│     "I ohtar anta macilirya"    (rbmt_alt, 0.71)                        │
│                                                                         │
│   Tier: rbmt   Memory match: none                                       │
│                                                                         │
│ ─────────────────────────────────────────────────────────────────────── │
│ Enter:insert  r:remember  e:edit+remember  a:ask AI                     │
│ o:collapse  d:dismiss  p:pin                                            │
└─────────────────────────────────────────────────────────────────────────┘
```

### D. Migration notes

**For 1.3.23 translation users.** After PANE-1 ships:

- CLI: no change. `language translate ...` continues to work exactly as before from the shell, outputting to stdout.
- Bund: no change. `lang.translate` returns the same value.
- TUI: the held-back integration becomes real. `Ctrl+B U T` (and siblings) now opens a chord wrapper that invokes the underlying command and the result lands in Output.
- The release notes' "held for the planned TUI rearchitecture" caveat is removed in the 1.4 release notes.

**For Bund script authors.** After PANE-1 ships:

- Scripts using `print` and `log` will see their output in Output (when the TUI is running) instead of wherever it currently goes. This is generally an improvement; no scripts should break.
- Scripts that programmatically consume their own output (rare) need to consult the message store via `ink.io.message.list` instead of capturing stdout. A migration helper is documented.

**For LANG-1 / LANG-2 users.** After PANE-1 ships:

- The lexicon proposal queue overlay is replaced by `lexicon_proposal` messages in Output. The triage operations (accept/edit/reject) map to the seven primitives plus the kind-specific `Edit` extra. Existing chord muscle memory mostly transfers.
- Variety renderings appear in Output rather than CLI stdout when invoked from the TUI.

**For future feature authors.** Declaring a new kind:

1. Add the kind to the `MessageKind` enum in `pane::output::types`.
2. Document the metadata schema, default actions, severity, lifetime in an RFC.
3. Add lifetime default to `output_pane.lifetime_defaults` in HJSON.
4. Emit via `pane::output::emit::emit()` from your subsystem.
5. The Output pane renders the kind automatically using the envelope; no UI work needed unless the kind has unusual rendering needs (e.g., embedded charts).

---

**End of RFC PANE-1.**
