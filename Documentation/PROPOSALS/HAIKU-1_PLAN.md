# HAIKU-1 — Startup / new-paragraph / on-demand haiku

| | |
|---|---|
| **RFC** | HAIKU-1 |
| **Title** | Zero-AI, language-aware haiku in the Output pane at three moments |
| **Status** | Shipped — 1.4.17 |
| **Author** | Vladimir Ulogov |
| **New dependency** | none |

A small writer-delight feature: a hand-curated haiku, in the book's language, emitted to the Output
pane at startup (T1), when a new manuscript paragraph is created (T2), and on demand (T3, `Ctrl+Z p`).
All 25 poems (5 languages × 5) are `&'static str` baked into the binary — no AI, no network.

## Audit corrections (RFC was written against a fabricated surface)

- **Target 1.4.15 → 1.4.17.** 1.4.16 (CHAR-1) already shipped.
- **`Message` struct literal is fabricated.** The real type has no `summary`/`body`/`source`/
  `book_id`/`paragraph_id`/`created_at` fields. Use `Message::new(kind, severity, lifetime, json)`;
  the display string lives under `metadata["text"]` (the pane renders that key). `Lifetime::Session(1)`
  is real and means exactly "keep only the most recent 1 of this kind" — the pane self-prunes, so no
  manual dismiss loop is needed.
- **`Action::display_name()` → `Action::label()`.** Both `label()` and `description()` are exhaustive
  matches with no catch-all — each needs a new arm. `run_action` too.
- **`default_true` helper does not exist.** The pattern is a per-field `fn default_startup_haiku() ->
  bool { true }` plus an entry in the manual `impl Default for EditorConfig`.
- **`commit_add` guards:** `structural_pick` (`pending_structural_type`) and `seed_body_after_create`
  are real locals (RFC right). `parent_is_language` is not a usable variable, and is unneeded:
  `seed_body_after_create.is_none()` already excludes every seeded kind (Dictionary/Sources/Glossary/
  Snippets/Threads/Language-rule/Jinja). **T2 scope = user-book manuscript only:** gate on
  `self.book_of_node(new_id).is_some()` (returns `Some` only when the containing book is a user book).
- **Pane row model is one header + one text line; embedded `\n` won't render.** Store the three lines
  as a `haiku_lines` array in metadata and render them as three indented lines (plus a `✦` kind-glyph);
  keep an inline `"text"` (lines joined ` / `) for anything that reads the text key.
- **`Ctrl+Z p` in `bund_sub`** — confirmed correct: `bund_sub`'s prefix is `Ctrl+Z` and `p` is free.

## Phases

| Phase | Content |
|---|---|
| H-P1 | `src/haiku.rs` (table + `next_for_lang` + `emit_for_lang`), `mod haiku`, `kinds::HAIKU`, config knob, pane render (3-line + ✦ glyph) |
| H-P2 | T1 (`install_progress`), T2 (`commit_add`, user-book-only), T3 (`Action::ShowHaiku` + `bund_sub` `p` + `run_action`), docs (KEYBINDING / CONFIGURATION / tutorial 100) |

## Not in scope

Per-poem user customization (`haiku.custom`), cross-session rotation persistence, and more languages
(HAIKU-2: PT/IT/JA) — all deferred.
