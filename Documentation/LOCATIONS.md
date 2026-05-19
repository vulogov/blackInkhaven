# Locations (the Places system book)

Inkhaven ships every project with a **Places** book — one of the six
system books seeded by `inkhaven init`. It is where you record every
location, region, or setting your manuscript references. Two things
make Places special:

1. **Editor overlay.** Every word in your prose that matches a Place's
   title lights up in cyan. The match is **stemmed** to the project's
   configured `language`, so "Москва" lit up by an entry covers
   "Москве", "Москвы", "Москвою" etc. automatically.
2. **AI lookup chord.** `Ctrl+B P` in the editor picks the selection
   (or the word under the cursor), sweeps the Places book for matching
   paragraphs, and prepends their contents to the next AI prompt as
   RAG context. Use it when you forget a detail and want the model to
   answer from your own canon.

## Table of contents

- [How to populate the Places book](#how-to-populate-the-places-book)
- [The cyan overlay in the editor](#the-cyan-overlay-in-the-editor)
- [How stemming works](#how-stemming-works)
- [`Ctrl+B P` — Place RAG inference](#ctrlb-p--place-rag-inference)
- [Multilingual writing](#multilingual-writing)
- [Recommended organisation](#recommended-organisation)
- [Configuration knobs](#configuration-knobs)

## How to populate the Places book

The Places book lives in the Tree pane. Navigate to it:

```
├─ My Novel
├─ Notes
├─ Research
├─ Prompts
├─ Places         ← here
├─ Characters
└─ Help
```

You cannot delete or rename **Places** itself (it is a protected system
book) but everything inside is plain editable. The simplest workflow:

1. Move the cursor to the **Places** row and press `→` to expand it.
2. Press `+` to add a paragraph (or `C` first if you want chapter-level
   grouping like "Cities" / "Regions" / "Buildings").
3. The Add modal asks for a title. **The title is the place name.**
   `Москва`, `King's Landing`, `Apartment 12`, `San Francisco — 1906`.
4. Press Enter; the paragraph opens in the editor.
5. Type whatever you want to remember about the place — physical
   description, history, who lives there, plot relevance. This is
   regular Typst prose; headings (`=`), bold (`*foo*`), lists
   (`- one`), all work.
6. `Ctrl+S` to save.

You can mix paragraphs directly under **Places** with deeper structure:

```
├─ Places
│  ├─ Cities                    (chapter)
│  │  ├─ Major                   (subchapter)
│  │  │  ├─ ¶ Москва
│  │  │  └─ ¶ Санкт-Петербург
│  │  └─ Minor                   (subchapter)
│  │     └─ ¶ Воронеж
│  ├─ Regions                   (chapter)
│  │  ├─ ¶ Сибирь
│  │  └─ ¶ Урал
│  └─ ¶ Дача на Волге           (paragraph directly under the book)
```

All paragraphs anywhere in the subtree are picked up by the editor
overlay and the `Ctrl+B P` lookup.

## The cyan overlay in the editor

Open any paragraph in your manuscript. Every word that matches a Place
title (after stemming) renders in **cyan + bold**.

Example: with `Москва` recorded as a Place under a Russian project
(`language: russian`), prose like

```
Из Москвы поезд шёл всю ночь. К утру они были в Москве.
```

displays "Москвы" and "Москве" in cyan — even though neither matches
the literal entry title. The Russian Snowball stemmer reduces all
three forms to the same stem.

The colour is `theme.places_fg` in your config — see
[`CONFIGURATION.md`](CONFIGURATION.md#theme). Default `#89dceb` (sky
blue). Override to taste:

```hjson
theme: {
  places_fg: "#a6e3a1"   # green if cyan clashes with your other choices
}
```

The overlay refreshes:

- Live as you type — new paragraphs get checked against the lexicon on
  every render.
- After every save (`Ctrl+S`) — adding a new Place paragraph immediately
  starts highlighting its name elsewhere.
- On project open — the lexicon is compiled once at startup.

## How stemming works

Inkhaven uses [Snowball stemmers](https://snowballstem.org/) from the
`rust-stemmers` crate. The stemmer reduces inflected words to their
root form so a single entry covers every grammatical case.

The language is driven by:

1. **Top-level `language`** in `inkhaven.hjson` — wins when non-empty.
   Default `"english"`.
2. **`editor.stemming.languages`** — a fallback list of multiple
   languages used when `language` is empty.

Set `language` to the dominant language of your manuscript:

```hjson
language: russian
```

Supported languages:

```
arabic, danish, dutch, english, finnish, french, german, greek,
hungarian, italian, norwegian, portuguese, romanian, russian,
spanish, swedish, tamil, turkish
```

If you write in a language that isn't on this list, set `language: ""`
to disable stemming — the overlay falls back to **exact** word matches.

### Multi-word Places

Place titles can be multi-word: `King's Landing`, `North Tower`,
`Серая Гавань`. The matcher splits the title into tokens, stems each,
and looks for the same sequence of stems in the prose. So `King's
Landing` matches `King's Landing` in any sentence, with stemming applied
to each word independently.

## `Ctrl+B P` — Place RAG inference

The Place name overlay is passive. To **ask the AI** about a Place,
use the Place-RAG flow:

1. In the editor, select the place name in your prose (or place the
   cursor inside the word).
2. Press `Ctrl+B P`.

What happens next depends on whether the AI prompt bar has text:

- **Empty bar** — Inkhaven sweeps the Places book for paragraphs whose
  title contains the term (case-insensitive substring), builds a
  context block, **stashes** it as the next RAG prefix, and focuses
  the **AI prompt** bar. The status shows
  `Place RAG armed for 'Москва' — type your question and Enter`.
  Type your question (`What's the population? When was it founded?`),
  press Enter, and the model answers using the stashed context.
- **Non-empty bar** — the inference fires immediately with the context
  block prepended to your existing prompt. Focus moves to the **AI
  pane** so you can watch the answer stream in.

The context block looks like:

```
── Place context for `Москва` (1 match(es)) ──

── Place: Москва ──
= Москва

The capital of Russia, founded in 1147. Population ~12M.
Site of the Kremlin and Red Square.
── end place ──
```

The AI receives this prefix followed by your question. With **Inference
mode = Local** (toggle with `F10`), the model is constrained to use
only the context you supplied — useful when you want a faithful summary
of your canon and nothing else.

### When the lookup matches nothing

If your selection doesn't match any Place title, the status reports
`Place RAG: no entry titled like 'XYZ' in the Places book`. No
inference fires — drop a Place entry first.

## Multilingual writing

A single project can write in one language at a time (`language:
russian`), or you can mix languages by disabling stemming and using
exact-match Places. Two patterns:

### Single-language project (recommended)

```hjson
language: russian
```

Stemming is applied. `Москва` matches every inflection.

### Bilingual project (e.g. translation work)

```hjson
language: ""
editor: {
  stemming: { languages: ["english", "russian"] }
}
```

Both stemmers run; an entry `Moscow` lights up English inflections,
`Москва` lights up Russian ones. Recommended when you keep source and
target in two parallel books inside the same project.

## Recommended organisation

A few patterns that scale to long projects:

- **Group by hierarchy when names collide.** If multiple Smiths-Mansion
  exist across timelines, group under a chapter `Mansion variants`
  and give each paragraph a uniquely-qualified title (`Mansion (1890s)`,
  `Mansion (rebuilt)`).
- **Body content carries continuity.** Inkhaven only matches titles for
  the cyan overlay, but the `Ctrl+B P` lookup returns full paragraph
  bodies. Put history, ownership, residents, accidents — anything the
  AI should know — in the prose. The model can recall it back to you.
- **One Place per paragraph.** Keep things atomic. If you find yourself
  writing about two locations in one paragraph, split them.
- **Cross-link via Typst link syntax.** `#link("places/king-s-landing")[King's Landing]`
  renders in the export but is also human-readable in the editor.

## Configuration knobs

| Field | Default | What it controls |
| ----- | ------- | ---------------- |
| `language` | `"english"` | Snowball stemmer language used by the Places overlay. |
| `editor.stemming.languages` | `["english", "russian"]` | Fallback when `language` is empty; allows multi-stemmer matching. |
| `theme.places_fg` | `#89dceb` | Foreground colour of the cyan overlay. |

See [`CONFIGURATION.md`](CONFIGURATION.md) for the complete reference.
