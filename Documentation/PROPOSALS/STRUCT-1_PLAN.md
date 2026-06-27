# STRUCT-1 — Jinja Template Paragraphs

| | |
|---|---|
| **RFC** | STRUCT-1 |
| **Title** | Jinja template paragraphs — `content_type: "jinja"`, minijinja assembly pre-pass, Snippets book template library |
| **Status** | Shipped — 1.4.10 |
| **Author** | Vladimir Ulogov |
| **Depends on** | REUSE-1 (Snippets system book and `emit_snippets_directory` assembly pre-pass) |
| **New dependency** | `minijinja = "2"` (one crate; its transitive deps were already in the tree) |

## What shipped

A paragraph with `content_type: "jinja"` is a Jinja2-compatible template (rendered
by `minijinja`) that the assembler compiles to a `.typ` file **before**
`typst compile` runs. Two layers, strictly sequential, never nested:

```
.jinja paragraph ──(assembly: minijinja)──▶ .typ ──(typst compile)──▶ PDF
```

Self-gating: a project with no Jinja paragraphs is unaffected.

### Assembly (S-P1)

A new pre-pass in `assemble_book()` runs before any rendering:

1. **`build_jinja_environment()`** — every `content_type: "jinja"` paragraph in
   the **Snippets** system book is registered as a named `minijinja` template.
   The name is the hierarchy slug path, lowercased: `Snippets/Macros/warning` →
   `snippets/macros/warning.jinja`. Duplicate names → first-write-wins + a logged
   warning (Q2). Empty env when no Snippets book / no Jinja snippets.
2. **`emit_snippets_directory()`** (extended) — Jinja snippets render standalone
   to `<artefacts>/<book>/snippets/<slug>.typ` (REUSE-1 Typst-level includes keep
   working alongside the Jinja `{% include %}` path).
3. **`write_branch()`** (extended) — each manuscript `.jinja` paragraph renders to
   `<artefacts>/<book>/book/…/<NN-slug>.typ`. The output is always `.typ` (never
   `.jinja`), and the emitted `ChildRef` carries the `.typ` name so the generated
   `index.typ` `#include`s a file Typst can compile.

**Render context** (`jinja_context_for_node`): `title`, `slug`,
`book.{title,slug,genre}`, `chapter.{title,slug}`, `language`, `genre`, and
`linked["<slug>"].<field>` — HJSON data from paragraphs linked with `Ctrl+V a`
(parsed `serde_hjson → serde_json::Value → minijinja::Value`). Non-HJSON links
are skipped; raw-text access from any linked paragraph is deferred to STRUCT-2.

**Error handling (Q1):** a render failure **aborts the whole assembly** by
default with the offending paragraph + error (CI-safe — no silently-dropped
content). `jinja.continue_on_error: true` writes a visible Typst error block into
the paragraph's place and keeps assembling.

### Creation & editing (S-P2, S-P3, chord rework)

- **Create:** `e` in the Tree pane (mnemonic: t**e**mplate) opens the Add modal
  for a seeded `.jinja` paragraph — a manuscript template under a user book, or a
  reusable `{% include %}` fragment under the Snippets book; rejected elsewhere.
  (`t`/`j` were both already taken in the Tree pane.)
- **Convert:** the plain `t`/`T` node-type morph cycle gained a `jinja` rung:
  `typst → hjson → jinja → bund`. A single `next_leaf_type()` helper is the source
  of truth for both the single and bulk morph paths.
- **Display:** tree glyph `⟡`, editor `[jinja]` badge, a hand-rolled Jinja
  highlighter (`jinja_highlight.rs`) mirroring the HJSON one — `{# #}` / `{{ }}` /
  `{% %}`, strings, `| filter` names.
- **Guards:** Jinja paragraphs are skipped by the Inner Editor, Inner Socrates,
  and the idle fact-checker (markup, not prose). The Typst diagnostics /
  `check_includes` / `Ctrl+V R` render paths already excluded non-Typst via their
  whitelist guards.

### Config

```hjson
jinja: {
  continue_on_error: false   // abort on render error (default) vs. visible-block-and-continue
}
```

### Multilingual

No prompts or word-lists; the template renders whatever the author writes and
exposes `language`/`genre` for self-branching (`{% if language == "ru" %}…`).
Linked HJSON values pass through UTF-8-clean (Cyrillic verified by test); the
highlighter is char-based.

## Open questions (resolved)

- **Q1 — render errors:** abort by default + `jinja.continue_on_error` opt-in.
- **Q2 — name collisions:** first-write-wins + assembly warning.
- **Q3 — `linked_text` (raw prose from any linked paragraph):** deferred to STRUCT-2.
- **Q4 — edit-time render preview:** deferred.
- **Q5 — `t` reserved:** superseded — creation is `e`; `t`/`T` is the morph cycle.

## What did not change

No new `NodeKind`, no `ChildRef` variant, no DuckDB schema change — a
`content_type` on an ordinary paragraph, `.jinja` on disk. Orthogonal to the
text-expansion snippet system (`tui/snippets.rs`).

See [JINJA_TEMPLATES.md](../JINJA_TEMPLATES.md) and
[Tutorial 93](../Tutorials/93-jinja-templates.md).
