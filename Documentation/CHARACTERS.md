# Characters (the Characters system book)

The **Characters** system book is the people-shaped twin of the Places
book. You record every named character your manuscript references; the
editor lights mentions of those names in **yellow**, and `Ctrl+B C`
asks the AI about a selected character with RAG context drawn from this
book.

If you have read [`LOCATIONS.md`](LOCATIONS.md) the mental model is
identical — only the colour, the chord, and the system tag differ. This
file is included separately so each reference doc covers exactly one
topic.

## Table of contents

- [How to populate the Characters book](#how-to-populate-the-characters-book)
- [The yellow overlay in the editor](#the-yellow-overlay-in-the-editor)
- [Stemming and name variants](#stemming-and-name-variants)
- [`Ctrl+B C` — Character RAG inference](#ctrlb-c--character-rag-inference)
- [Recommended organisation](#recommended-organisation)
- [Pronouns, nicknames, aliases](#pronouns-nicknames-aliases)
- [Configuration knobs](#configuration-knobs)

## How to populate the Characters book

The Characters book is a protected system book seeded by `inkhaven
init`. It sits below Places in the root level of the Tree pane and
cannot be deleted or renamed. Everything inside is editable.

To add a character:

1. Move the tree cursor to the **Characters** row.
2. Press `→` to expand it.
3. Press `+` to add a paragraph under it.
4. In the Add modal, type the character's **canonical name** — this
   is what the editor overlay will match. `Aragorn`, `Anna Karenina`,
   `Robb Stark`, `Дмитрий`. Press Enter.
5. The new paragraph opens; type whatever you want to remember.
   Backstory, physical description, relationships, voice notes,
   plot role. This is plain Typst prose — headings (`= Section`),
   lists (`- birth: 980`), bold (`*important*`) all work.
6. `Ctrl+S` to save.

Group by chapter / subchapter if you have many characters:

```
├─ Characters
│  ├─ Main characters             (chapter)
│  │  ├─ ¶ Aragorn
│  │  ├─ ¶ Frodo
│  │  └─ ¶ Sam
│  ├─ Supporting                  (chapter)
│  │  ├─ ¶ Boromir
│  │  └─ ¶ Faramir
│  ├─ Antagonists                 (chapter)
│  │  └─ ¶ Sauron
│  └─ ¶ Random villager           (paragraph directly under book)
```

All paragraphs anywhere in the subtree feed both the overlay and
`Ctrl+B C` lookup.

## The yellow overlay in the editor

Every word in your prose that matches a Character's title (after
stemming) renders in `theme.characters_fg` — **yellow + bold** by
default. Look at any paragraph in your manuscript; mentions of the
people you have recorded pop out.

When a Character name **and** a Place name overlap on the same column
(rare, but happens with surnames that are also place names — `Stark`
in some manuscripts), Place wins by design. Use distinct names or
distinct titles to avoid the collision.

Override the colour in `inkhaven.hjson`:

```hjson
theme: {
  characters_fg: "#fab387"   # peach if you want a warmer tone
}
```

Refresh cadence is the same as Places: live as you type, on every
save, and at project open.

## Stemming and name variants

Names inflect. "Aragorn" / "Aragorn's" / "Aragorne" (medieval forms),
"Анна" / "Анне" / "Анной" / "Анну" / "Анны" / "Анне" — six forms in
Russian alone. The Snowball stemmer reduces all of them to the same
stem so one entry covers every form.

The stemmer language comes from your top-level `language`:

```hjson
language: russian
```

Without stemming you would need a separate paragraph for every case
form — clearly not the workflow. See
[`CONFIGURATION.md`](CONFIGURATION.md#prompts_file-and-language) for
the supported language list.

### Multi-word names

`Robb Stark`, `Anna Karenina`, `Hermione Granger` — multi-word titles
match as a sequence of stems. So a Russian entry `Анна Каренина`
matches `Анна Каренина` in any case in the prose; the matcher splits
the title into two tokens, stems each, and looks for the same sequence
in the buffer.

### First name / surname

A single paragraph for `Anna Karenina` highlights the full name, but
**not** the standalone `Anna` or `Karenina` in isolation. If both forms
matter, either:

- Add two paragraphs (one for the full name, one for the first name
  alone), OR
- Use a single short title (e.g. just `Anna`) and rely on the body
  text for surname disambiguation.

The choice depends on how often each form appears. For a protagonist
whose first name is unique in the manuscript, a short title is enough.

## `Ctrl+B C` — Character RAG inference

The yellow overlay is passive. To **ask the AI** about a character,
use the chord:

1. In the editor, select the character name in your prose (or place
   the cursor inside the word).
2. Press `Ctrl+B C`.

Behaviour mirrors `Ctrl+B P`:

- **AI prompt bar empty** — the matching paragraphs from the
  Characters book are stashed as the next RAG prefix; focus jumps to
  the AI prompt. Status reads
  `Character RAG armed for 'Aragorn' — type your question and Enter`.
- **AI prompt bar has text** — inference fires immediately with the
  Character context prepended to your existing prompt. Focus moves
  to the AI pane.

The context block looks like:

```
── Character context for `Aragorn` (1 match(es)) ──

── Character: Aragorn ──
= Aragorn

Son of Arathorn, last heir of Isildur. Hides as a Ranger named
Strider in the wilderlands before claiming the throne of Gondor.
Carries Andúril, reforged from the shards of Narsil.
── end character ──
```

The model receives this followed by whatever you type. With **F10 =
Local**, the answer is constrained to the context (no fanfic
invention from training data). Switch to **Full** if you want the
model to add references to the wider source canon.

### Empty matches

If the selection doesn't match any Character title, you get
`Character RAG: no entry titled like 'XYZ' in the Characters book`.
No inference fires.

## Recommended organisation

Patterns from long-form projects:

### Group by role

```
├─ Characters
│  ├─ Protagonists
│  ├─ Antagonists
│  ├─ Supporting
│  └─ Background
```

### Group by chronology

```
├─ Characters
│  ├─ Part I — Childhood
│  ├─ Part II — University years
│  └─ Part III — Returning home
```

### Group by family

```
├─ Characters
│  ├─ House Stark
│  │  ├─ ¶ Eddard
│  │  ├─ ¶ Catelyn
│  │  ├─ ¶ Robb
│  │  └─ ¶ Sansa
│  ├─ House Lannister
│  └─ House Targaryen
```

### What to put in the body

The body is what the AI sees when you `Ctrl+B C`. Useful sections:

- **Biography** — birth, family, formative events.
- **Physical** — what they look like; the AI can use this to keep
  descriptions consistent.
- **Voice** — speech patterns, vocabulary, accent. Tells the model
  how to write their dialogue if you ask.
- **Relationships** — who they love, hate, owe.
- **Plot involvement** — at which chapters / subchapters they enter
  and exit.
- **Author notes** — things the reader should never know but you
  want to remember. Plot twists hinging on this character.

A single paragraph 200 – 1000 words is comfortable. Use Typst
headings (`= Biography`, `== Voice`) so the body is scannable in the
editor.

## Pronouns, nicknames, aliases

The matcher only highlights the literal title (after stemming).
Nicknames and aliases need their own paragraphs or you need to put
them in the title.

### Pattern 1: one paragraph per alias

Useful when each alias has distinct lore:

```
├─ Characters
│  ├─ ¶ Aragorn
│  ├─ ¶ Strider
│  └─ ¶ Elessar
```

Each paragraph stands alone; cross-link them in the body
(`See also: #link[Strider]`).

### Pattern 2: title carries the canonical alias

If a character is usually called by their alias in the prose, set the
title to the alias and mention the real name in the body:

```
Title: Strider
Body:
  = Strider
  Alias of Aragorn — see also the entry titled `Aragorn`.
  Carries Andúril…
```

This highlights `Strider` in the editor but not `Aragorn`. Useful
when the surface text in your manuscript primarily uses the alias.

### Pattern 3: pronouns

Pronouns are not character names and aren't highlighted. If you want a
"who is `she`?" lookup, the AI scope modes (`F9` Selection /
Paragraph) plus a question to the chat work — Inkhaven's lexicon is
deliberately a noun-phrase index, not a coreference resolver.

## Configuration knobs

| Field | Default | What it controls |
| ----- | ------- | ---------------- |
| `language` | `"english"` | Snowball stemmer used for the overlay. |
| `editor.stemming.languages` | `["english", "russian"]` | Fallback when `language` is empty. |
| `theme.characters_fg` | `#f9e2af` | Yellow overlay colour. |

See [`CONFIGURATION.md`](CONFIGURATION.md) for the full surface.
