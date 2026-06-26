# Tutorial 92 — Reusable Snippets

*Inkhaven 1.4.9*

Documentation and technical writing repeats itself: the same "back up your data
first" warning, the same support-contact block, the same admonition about a
breaking change — written out again in every place it's needed, and drifting a
little each time. REUSE-1 lets you write that prose **once** and reference it
from anywhere, using nothing but standard Typst.

It's self-gating: with no snippets defined, nothing about your project changes.

## Define a snippet

Snippets live in the **Snippets** system book (it sits among Notes, Sources,
Glossary, … in the tree). Put the cursor on it and press **`P`** to add a
paragraph — the title you type becomes the snippet's **slug** (its filename).
Write any Typst:

```
= warning-box

#block(fill: rgb("#fee"), inset: 8pt, radius: 4pt)[
  *Warning.* Back up your data before running this procedure.
]
```

Save it. That's the whole definition.

## Reference it in your prose

In a chapter paragraph, you reference a snippet with a normal Typst `#include`.
You *could* type the path by hand, but let inkhaven do it: press **`Ctrl+V x`**,
fuzzy-find `warning-box`, and press Enter. It inserts:

```
#include "../../snippets/warning-box.typ"
```

The `../../` is computed automatically from where the paragraph sits in the tree
— a paragraph directly under a book needs `../snippets/…`, one under a chapter
needs `../../snippets/…`, and so on. You never count directories.

Want to swap one snippet for another? Put the cursor **inside** an existing
`#include "…/snippets/…"` path and press `Ctrl+V x` again — the picker opens in
**Replace** mode (pre-selecting the current snippet) and swaps just the path.

## Assemble, and it just works

When you assemble the book (**`Ctrl+B A`** / `inkhaven build`), inkhaven copies
every snippet to a `snippets/` directory next to the assembled book:

```
<artefacts>/<book>/snippets/warning-box.typ
```

So your `#include` resolves and `typst compile` drops the snippet's content
inline — wherever you referenced it. There's no new Typst syntax and no special
node type; it's just `#include`, which Typst has always understood.

## Catch broken references early

Rename or delete a snippet and its references dangle. inkhaven catches this two
ways:

- **As you write:** save a paragraph whose `#include` points at a snippet slug
  that doesn't exist, and a diagnostic appears (status bar, `F8` list,
  `Ctrl+V N` to jump to it) — *before* you ever assemble.
- **Across the project:** `inkhaven snippets check` scans every reference and
  **exits non-zero** when any points at an undefined snippet — perfect for a
  pre-build / CI step. It also reports **orphaned** snippets (defined but never
  referenced) as warnings.

```sh
$ inkhaven snippets check
snippets check: 1 missing reference(s):
  guide/03-tokens/01-intro line 4: no snippet `ghost`
snippets check: 1 orphaned snippet(s) (defined, unreferenced):
  old-disclaimer
```

## See the whole library

**`Ctrl+V Shift+X`** opens the snippets overview: every snippet with how many
times it's used across the project, so you can spot the unused ones and the
heavily-relied-on ones at a glance. Enter jumps to a snippet's source.
`inkhaven snippets list` is the terminal equivalent.

## From a script

```
ink.snippets.list     ( -- list )    every snippet { slug, title }
ink.snippets.get      ( slug -- dict | NODATA )   { slug, title, body }
ink.snippets.check    ( -- list )    missing references { slug, path, line }
```

All read-only.

---

**See also:** [CONFIGURATION.md → Reusable content blocks](../CONFIGURATION.md) ·
[KEYBINDING.md → `Ctrl+V x`](../KEYBINDING.md) · `inkhaven snippets --help`.
