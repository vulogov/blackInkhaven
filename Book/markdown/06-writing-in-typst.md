# 6 — Writing in Typst

You don't have to learn Typst to write in inkhaven. Plain prose just works — the bundled templates handle every typographic decision. But the moment you want bold, emphasis, or a section break, Typst is the syntax.

This chapter is a survival kit: the dozen Typst markers you'll actually meet writing prose. The full Typst language goes much further (math, plots, custom functions); see typst.app/docs when you need it.

## Block-level markers

| Marker | What it does |
|--------|---------------|
| `= Title` | Top-level heading (chapter). |
| `== Title` | Second level (subchapter). |
| `=== Title` | Third level (section inside a chapter). |
| `- item` | Bulleted list item. |
| `+ item` | Numbered list item (the typst form; `1. item` also works). |
| `> quote` | Block quote — single line OR several with leading `>` on each. |
| `// comment` | Comment. Skipped by the typesetter; visible in the editor. |

Paragraph break = blank line. Single newlines inside a paragraph are treated as soft wraps and joined.

## Inline markers

| Marker | What it does |
|--------|---------------|
| `*bold*` | Bold text — single asterisks. |
| `_italic_` | Italic — single underscores. |
| `` `code` `` | Inline code — backticks. |
| `"smart quotes"` | Curly quotes when the editor's language is set; straight when set to `none`. |
| `---` | Em dash. `--` is en dash. |
| `[link](url)` | Hyperlink in markdown form (Typst accepts both `#link()` and the bracket form). |
| `#emph[text]` | Function form of italic — useful inside complex content. |
| `#strong[text]` | Function form of bold. |

## The skeleton inkhaven creates

When you create a paragraph (`+` in the tree or `inkhaven add paragraph`), inkhaven writes a tiny skeleton:

```typst
= My paragraph title

```

The `= Title` line is what gets used as the slug seed + appears at the top of the rendered output. Add prose below the blank line. The title line is yours to edit; F2 rename in the tree updates it.

## Functions you'll actually use

A handful of Typst function calls cover most prose needs:

```typst
#image("../images/cover.jpg", width: 80%)

#footnote[A quick aside.]

#emph[delicate emphasis] vs *strong emphasis*

#text(style: "italic", "rendered italic without the inline _")

#pagebreak()
```

The `#` prefix is Typst's "this is a function call" marker. Anything you write in pure prose without `#` is treated as prose; everything with `#` is computed.

> **Imports happen automatically:** Inkhaven assembles the book by walking the tree, pulling each paragraph's `.typ` body, and wrapping with the book's `index.typ` + `settings.typ` + `globals.typ`. So functions you define in `globals.typ` are usable from any paragraph without an `#import` line.

## When things go wrong

Type something Typst doesn't like — `#undefined_function()` or an unmatched `*` — and the editor's gutter shows a red `●` on the offending line (1.2.6+). Chapter 24 covers the diagnostic surface in depth.

The render preview (`Ctrl+V R`, Chapter 24) lets you see what a paragraph will look like in the final book without running the full build.

## Markdown? Sort of.

Inkhaven's AI pane talks markdown by default — code blocks in answers use triple backticks, headings use `#`, lists use `-` or `*`. When you apply an AI answer to a paragraph via `r` / `i` / `t` / `b`, inkhaven converts markdown to Typst on the fly. It's not lossless but covers the basics: `# H` becomes `= H`, `**bold**` becomes `*bold*`, fenced code stays a code block.

If you write your own prose in markdown habits, that's fine — most of it round-trips through the Typst engine cleanly.

## Recap

- Plain prose works. Typst markup is opt-in for the typographic moments.
- `= == ===` headings; `*bold*`; `_italic_`; `` `code` ``; `---` em dash.
- `#function[arg]` for typst calls; `#image()`, `#footnote[]`, `#emph[]`.
- `= Title` line at the top of each paragraph is what inkhaven uses; F2 renames it.
- Diagnostics (gutter `●`) and render preview (`Ctrl+V R`) catch problems early.
