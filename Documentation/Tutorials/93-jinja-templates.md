# Tutorial 93 — Jinja Templates

*Inkhaven 1.4.10*

Reusable snippets (Tutorial 92) let you write a block of Typst **once** and
`#include` it verbatim. But some content isn't fixed prose — it's *structured*
and *data-driven*: a character sidebar that should read the character's actual
name and species, an endpoint table built from a list of fields. STRUCT-1 adds
**Jinja template paragraphs** for exactly that.

A Jinja paragraph is rendered to Typst **at assembly time**, with access to your
project's data. Two layers, never nested: minijinja renders Jinja → Typst, then
Typst compiles Typst → PDF.

It's self-gating: with no Jinja paragraphs, nothing about your project changes.

## Create a template

Put the tree cursor inside one of your books (or a chapter) and press **`e`**
(mnemonic: t**e**mplate). Type a name, Enter. You get a `.jinja` paragraph — note
the **`⟡`** glyph in the tree and the **`[jinja]`** badge in the editor header —
seeded with a starter that documents the variables you can use:

```jinja
{#- Template paragraph — rendered to Typst at Ctrl+B A (Book assembly). -#}

= {{ title }}

#block[
  Edit this template. Link HJSON paragraphs with Ctrl+V a to populate `linked`.
]
```

`{{ … }}` is an expression, `{% … %}` a statement (if/for/include), `{# … #}` a
comment — all syntax-highlighted as you type. Everything else is plain Typst,
passed straight through.

## Pull in real data with `Ctrl+V a`

The interesting variable is `linked`. Make an HJSON paragraph somewhere — say a
Characters entry `01-aria` with body:

```hjson
{ name: "Aria", species: "fox", role: "scout" }
```

Open your Jinja paragraph and press **`Ctrl+V a`** to link `01-aria` to it. Now
the template can read its fields, keyed by the linked paragraph's slug:

```jinja
= {{ linked["01-aria"].name }}

#block[A {{ linked["01-aria"].species }} who serves as {{ linked["01-aria"].role }}.]
```

At assembly that renders to:

```typst
= Aria

#block[A fox who serves as scout.]
```

Change Aria's species in one place and every template that reads her re-renders.
You also get `title`, `slug`, `book.title`, `chapter.title`, `language`, and
`genre` for free.

## Share fragments via the Snippets book

A `.jinja` paragraph **in the Snippets book** becomes an includable template.
Press `t` with the cursor in Snippets and write, say, a `warning` macro. Its
template name is its tree path, lowercased — `Snippets/Admonitions/warning`
becomes `snippets/admonitions/warning.jinja`. Include it from any manuscript
template:

```jinja
{% include "snippets/admonitions/warning.jinja" %}
```

Inkhaven registers every snippet template **before** rendering anything, so the
include always resolves — even a snippet that includes another snippet.

## Assemble

Press **`Ctrl+B A`** (or run `inkhaven build`). Inkhaven renders each Jinja
paragraph to a `.typ` file in the assembled tree, then runs `typst compile`. The
PDF contains the rendered output; the `{{ … }}` tags are long gone.

## When a template breaks

By default, a bad template **stops the build** with a clear message:

```
Error: jinja render failed in `Aria sidebar`: invalid syntax: unexpected `}}`
```

— so a typo can't silently blank out a paragraph in the PDF. Prefer to keep
going and fix templates one at a time? Set `jinja.continue_on_error: true` in
`inkhaven.hjson`; failures then render as a visible red error block in place and
assembly continues.

## Works in any language

The template receives `language` and `genre`, so you can branch:

```jinja
{% if language == "ru" %}Глава{% else %}Chapter{% endif %} {{ title }}
```

and your linked HJSON values (Cyrillic, accented Latin, …) pass through exactly
as written.

## Jinja vs. text-expansion snippets

Don't confuse this with the `bund:` **text-expansion** snippets (Tutorial 18) —
those drop a value into your buffer *as you type*. Jinja paragraphs generate
structured Typst *at assembly*. Different jobs; use both.

---

**See also:** [JINJA_TEMPLATES.md](../JINJA_TEMPLATES.md) ·
[Reusable snippets (Tutorial 92)](92-reusable-snippets.md) ·
[KEYBINDING.md → `e` (Tree)](../KEYBINDING.md) ·
[CONFIGURATION.md → Jinja templates](../CONFIGURATION.md).
