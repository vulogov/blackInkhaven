# INNER_EDITOR-1 — implementation plan (grounded)

Records RFC **INNER_EDITOR-1** (Inner Editor: literary & stylistic companion),
corrected against the real 1.4.2-dev codebase, with the locked decisions and a
two-release phase map. The RFC is the design intent; this file is what we build.

## What Inner Editor is

The **second member of the Inner family** (sibling to Inner Socrates). Where
Inner Socrates interrogates **facts/structure** through multiple Reader Personas
(Notice/Inquiry/Probe, ◇ purple, `Ctrl+B J`), Inner Editor observes **literacy
and style** through a **single configurable persona** (Praise/Note/Concern, ✎
warm-earth, `Ctrl+B E`). LLM-only (no Fast track). Paragraph scope (current ¶ +
N preceding). Fires on a paragraph-pause idle timer and on a manual chord.
Eight observation categories. Coexists with Inner Socrates on the same trigger;
findings are visually distinct in the Output pane.

## Grounding — RFC claims vs. the real codebase

Verified by survey of `src/inner_socrates/`, `src/pane/output/`,
`src/tui/inference.rs`, `src/tui/keybind.rs`, `src/ai/usage.rs`, `src/config.rs`.

| RFC assumes | Reality | Consequence |
|---|---|---|
| Target 1.4.0 | We're at 1.4.2-dev (1.4.1 shipped) | Retarget across **1.4.2 + 1.4.3** |
| Inner Socrates established **two** F9 scopes (Socrates, Suggestion) + a "Magic Ledger" scope | `AiMode` = None/Selection/Paragraph/Subchapter/Chapter/Book/Facts/**Socratic** only | We **add one** Editor scope; no "Suggestion"/"Magic Ledger" exist |
| Intent ledger lives in `Intent/02-ledger/` files; `inner_editor_llm_usage` already exists | Ledger = `intent_entries` table in `inner_socrates.db`; usage tables are **per-feature** | We create `inner_editor.db` (findings + cooldown + usage); consult the ledger via a **raw string-level read** (its `coverage` is typed to *Socratic* `Category`) |
| 10 bundled genres from Inner Socrates; genre in project config | 5 CLI-only genre strings (`facts init`), **not persisted**, no Inner Socrates genre logic | We add a **persisted project `genre`** + a few Editor genre fragments (minimal, R1) |
| Per-¶ language persisted on the paragraph record | In-memory `OpenedDoc.detected_language` (whatlang at open) | Use the in-memory value + `active_prompt_language()` |
| PANE-1 kinds are namespaced per subsystem | Flat string consts in `pane::output::kinds`; filter `message_source` keys off `kind` | Add `inner_editor_observation` const + `"inner-editor"` source |
| Cost caps **disable on reach** (§3.9) | Inkhaven permissive principle: caps **inform, never block** (durable rule; Inner Socrates' cap is soft) | Caps are **informative** — warn + continue. Principled deviation from the RFC. |
| Prompts as a `Prompts/inner_editor/` .typ tree (~40 files) | Inner Socrates hardcodes `SLOW_SYSTEM`; Book-RAG hardcodes localized `system_prompt` consts | **Hardcoded localized consts** (EN/RU/ES/FR/DE) + `resolve_prompt("inner-editor-system", …)` override hook |
| `Ctrl+B E` may be taken | `Ctrl+B Shift+E` = OpenReaderPace; **plain `Ctrl+B E` is free** | Use `Ctrl+B E` overview → E/S/C/F/U subkeys (mirrors `Ctrl+B J`) |

## Locked decisions

- **R1 (1.4.2) ships manual + ambient** — both the `Ctrl+B E E` manual engage and
  the paragraph-pause auto-fire (with cooldown + informative caps).
- **Genre: minimal in R1** — persisted project `genre` field + a handful of
  Editor genre-fragment hints in the prompt.
- **Prompts: hardcoded localized consts + override hook** — EN first, then
  RU/ES/FR/DE; overridable via the standard `resolve_prompt` chain.
- **Cost caps inform, never block** (overrides RFC §3.9 per standing principle).
- **One LLM call per engagement** producing findings across all enabled
  categories (mirrors Inner Socrates' slow track), not eight separate calls.
- **Severity**: Praise→Info, Note→Warning, Concern→Contradiction (PANE-1), with
  the default visible threshold at **Note** (Praise hidden, filter to reveal).

## Reuse map (what we mirror, not rebuild)

- **Store**: `StorageEngine` + `<project>/inner_editor.db` (mirror `InnerSocratesStore`):
  `execute_with` / `select_all_with` / `now_secs()`.
- **Output emit**: `Message::new(kind, sev, Lifetime::UntilActedOn, json).with_source_paragraph(id)` → `pane::output::emit`.
- **Cost**: `ai::usage::record("inner_editor")` + per-feature `inner_editor_llm_usage(day, sub_budget, calls)`; sub-budgets `editor_engagement` / `conversation`. Surfaces in `inkhaven cost` automatically.
- **Multilingual**: `active_prompt_language()`, `iso_from_long`, `resolve_prompt`.
- **Intent ledger**: a new `InnerSocratesStore::list_intent_rows_raw()` (raw `kind`/`description`/`scope`/`coverage:Vec<String>`); Editor matches its category ids against `coverage` + reuses `IntentScope::applies_to`. New `IntentKind` variants added additively when the "record as intent" outcome lands (R2).
- **Chord**: `OpenInnerEditorOverview` meta_sub on `Ctrl+B E`; overview modal captures E/S/C/F/U.
- **Idle**: mirror the fingerprint + activity-timestamp + idle-threshold pump used by the Socratic ambient / fact-check idle slow-track; add same-paragraph cooldown.

## Phase map

### Release 1.4.2 — Part A (the editorial core)

- **IE-P0 Foundation** — `src/inner_editor/` module; `types.rs` (EditorSeverity
  Praise/Note/Concern, EditorCategory ×8, EditorFinding, PersonaTuning); the
  `InnerEditorConfig` block in `config.rs` (+ persisted project `genre`);
  `storage.rs` (`inner_editor.db`: `editor_findings`, `editor_cooldown_state`,
  `inner_editor_llm_usage`). Pure, unit-tested.
- **IE-P1 Persona + prompts** — the single Editor voice-notes; the localized
  system prompt + structured-output category guidance (EN/RU/ES/FR/DE consts);
  tuning-knob application (tone/verbosity/praise-frequency/genre) + override hook.
- **IE-P2 Engagement engine** — context builder (¶ + 3 preceding); prompt
  builder with tuning; one LLM call via the existing client; response parser →
  `Vec<EditorFinding>`; intent-ledger consultation (raw read, suppress by
  coverage+scope); cost tracking (informative caps); persist findings.
- **IE-P3 Surfaces** — `inner_editor_observation` kind + `"inner-editor"` filter
  source + ✎/warm-earth row rendering + Note-threshold; `Ctrl+B E` overview &
  `Ctrl+B E E` engage; `inkhaven inner-editor engage|findings|config|usage` CLI.
- **IE-P4 Ambient + cooldown** — paragraph-pause idle trigger (fingerprint +
  activity + `idle_threshold_seconds`), same-¶ cooldown with edit-reset, the
  `Ctrl+B E S` settings overlay. → **cut 1.4.2**.

### Release 1.4.3 — Part B (conversation, integration, polish)

- **IE-P5 Conversation mode** — new `AiMode::EditorConversation` F9 scope +
  `Ctrl+B E C`; curated opening; per-finding outcomes (dismiss/address/intent/
  note); record-as-intent writes new additive `IntentKind`s; resumability.
- **IE-P6 Snapshot + history + promotion** — snapshot id at emission/resolution;
  `findings history`; dismissal→promotion (reuse Inner Socrates' mechanism).
- **IE-P7 Bund + genre + usage overlay** — `ink.inner_editor.*` stdlib; finish
  genre fragments; `Ctrl+B E U` usage; optional book-take findings export.
- **IE-P8 Multilingual review + polish + docs** — native-review pass; tutorial
  (`Documentation/Tutorials/88-inner-editor.md`); CONFIGURATION.md `inner_editor`
  block; risk/edge tests. → **cut 1.4.3**.

## Non-goals carried verbatim from the RFC

No prescriptive editing (observe + qualify, never command — structural, not a
toggle). No grammar/syntax/fact checking. No Fast track. No chapter/book scope.
No AI co-writing. No multi-persona. No new external dependencies. Praise must be
specific and grounded — generic encouragement forbidden by prompt design.
