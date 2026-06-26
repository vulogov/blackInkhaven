# Jinja Template Paragraphs (STRUCT-1)

A **Jinja template paragraph** is a paragraph whose `content_type` is `"jinja"`.
It is a [Jinja2](https://jinja.palletsprojects.com/en/stable/templates/)-compatible
template (rendered by [`minijinja`](https://docs.rs/minijinja)) that the
**assembler compiles to Typst** before `typst compile` runs.

Two layers, strictly sequential — never nested:

```
your .jinja paragraph  ──(assembly: minijinja)──▶  .typ  ──(typst compile)──▶  PDF
```

The assembler renders Jinja → Typst. Typst compiles Typst → PDF. The Jinja layer
is gone by the time Typst sees anything; the output is ordinary Typst.

It is **self-gating**: a project with no Jinja paragraphs behaves exactly as
before. There is no enable flag.

---

## Why

Some content is *structured*, not free prose:

- A **character sidebar** that reads a linked Characters entry —
  `{{ linked["aria"].name }}`, `{{ linked["aria"].species }}` — instead of
  copy-pasting the same facts and watching them drift.
- An **API endpoint table** that loops `{% for param in params %}` over fields
  from a linked HJSON entry.
- A **shared admonition macro** written once and pulled into many chapters with
  `{% include "snippets/warning.jinja" %}`.

Jinja paragraphs generate Typst from data. The data comes from linked HJSON
paragraphs and project metadata; the reusable fragments come from the **Snippets**
system book (the same book REUSE-1 introduced).

---

## Create one — `t` in the Tree pane

Put the tree cursor inside a **user book** (or one of its chapters) and press
**`t`**. Type a name, Enter. You get a `.jinja` paragraph seeded with a
documented starter template (it lists the variables available to you).

Put the cursor inside the **Snippets** system book instead, and `t` seeds a
**reusable fragment** — a template meant to be `{% include %}`-d from elsewhere
rather than rendered on its own.

`t` is rejected (with a status hint) anywhere else — other system books (Notes,
Characters, Glossary, …) don't take Jinja templates.

In the tree a Jinja paragraph shows a **`⟡`** glyph (vs `¶` prose, `❴` HJSON);
the editor header carries a **`[jinja]`** badge and the buffer is syntax-
highlighted for Jinja (comments, `{{ … }}`, `{% … %}`, strings, filters).

---

## The render context

Every Jinja paragraph is rendered with these variables:

| Variable | Meaning |
|---|---|
| `title`, `slug` | this paragraph's own title and slug |
| `book.title`, `book.slug`, `book.genre` | the enclosing book |
| `chapter.title`, `chapter.slug` | the enclosing chapter (if any) |
| `language`, `genre` | project language / declared genre |
| `linked["<slug>"].<field>` | HJSON data from a **linked** paragraph |

### `linked` — the data-injection mechanism

Link an HJSON paragraph to your Jinja paragraph with the normal add-link chord
**`Ctrl+V a`**. At assembly the linked paragraph's HJSON is parsed and exposed
under `linked["<that paragraph's slug>"]`.

So if you link `characters/01-aria.hjson` containing:

```hjson
{ name: "Aria", species: "fox", role: "scout" }
```

your template can read:

```jinja
= {{ linked["01-aria"].name }}

#block[A {{ linked["01-aria"].species }} who serves as {{ linked["01-aria"].role }}.]
```

Only **HJSON-bodied** linked paragraphs land in `linked` — a linked prose
paragraph is skipped (its raw Typst isn't meaningful template context). Raw-text
access from any linked paragraph is planned for a later revision.

---

## Reusable template fragments — the Snippets book

A `.jinja` paragraph **in the Snippets book** is registered as a named Jinja
template *before any rendering*, so manuscript templates (and other snippets)
can include it. The template name is the snippet's tree path, lowercased:

```
Snippets/
  Admonitions/        (chapter)
    warning           →  snippets/admonitions/warning.jinja
  Macros/
    Shared/           (subchapter)
      base-block      →  snippets/macros/shared/base-block.jinja
  sidebar             →  snippets/sidebar.jinja
```

Include them by that exact name:

```jinja
{% include "snippets/admonitions/warning.jinja" %}
{% include "snippets/sidebar.jinja" %}
```

The chapter/subchapter titles become path segments (lowercased, spaces →
hyphens); the paragraph slug is the filename. You read the path straight off the
tree.

> If two snippets resolve to the **same** template name (same slug under
> same-named chapters), the first in tree order wins and assembly logs a warning.

Snippet `.jinja` paragraphs are *also* rendered standalone to
`<artefacts>/<book>/snippets/<slug>.typ`, so a plain Typst REUSE-1
`#include "…/snippets/<slug>.typ"` keeps working alongside the Jinja
`{% include %}` path.

---

## Assembly order

At `Ctrl+B A` / `inkhaven build`:

1. **Register** every `.jinja` paragraph in the Snippets book as a Jinja template.
2. **Render snippets** → `<artefacts>/<book>/snippets/<slug>.typ`.
3. **Render the manuscript** — each `.jinja` paragraph renders to
   `<artefacts>/<book>/book/…/<NN-slug>.typ` (always a `.typ`, never `.jinja`, so
   the generated `index.typ` `#include`s a file Typst can compile).
4. `typst compile`.

Step 1 happens before steps 2–3, so every `{% include "snippets/…" %}` resolves
against an already-registered template — no two-pass rendering, no nesting.

---

## When a template doesn't render

By default a render failure (bad syntax, a missing include, a typo'd variable)
**aborts the whole assembly** with the offending paragraph and the error:

```
Error: jinja render failed in `Aria sidebar`: invalid syntax: unexpected `}}`
```

That's CI-safe: a broken template can't silently drop content from the PDF. To
keep assembling instead — writing a **visible** Typst error block into the
paragraph's place so you can fix templates one at a time — set:

```hjson
jinja: {
  continue_on_error: true
}
```

See [CONFIGURATION.md → Jinja templates](CONFIGURATION.md).

---

## Multilingual

STRUCT-1 has no language-specific behaviour of its own — it renders whatever you
write. Because the context exposes `language` and `genre`, **you** decide how a
template behaves per language:

```jinja
{% if language == "ru" %}Глава{% else %}Chapter{% endif %} {{ title }}
```

The `linked` HJSON values are passed through verbatim (Unicode-clean), so a
Russian/French/German/Spanish Characters entry renders exactly as written.

---

## What it is *not*

- **Not** the text-expansion snippet system (`tui/snippets.rs`, the `bund:`
  edit-time expansions). That injects a value at a keystroke *while you type*.
  Jinja paragraphs generate structured Typst *at assembly*. Both coexist.
- **Not** prose. Jinja paragraphs are skipped by the Inner Editor, Inner
  Socrates, and the idle fact-checker — style/argument analysis doesn't apply to
  markup.
- **Not** a new node kind or a database change — it's a `content_type` on an
  ordinary paragraph, `.jinja` on disk.

---

**See also:** [Tutorial 93 — Jinja templates](Tutorials/93-jinja-templates.md) ·
[CONFIGURATION.md](CONFIGURATION.md) · [KEYBINDING.md → `t` (Tree)](KEYBINDING.md) ·
[Reusable snippets (REUSE-1)](Tutorials/92-reusable-snippets.md).
