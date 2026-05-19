# 7 — Places and Characters

The **Places** and **Characters** system books are Inkhaven's
worldbuilding companions. Recording an entry does three things:

1. **Cyan / yellow overlay** — mentions of the entry's name light up
   in the editor everywhere they appear in your prose.
2. **`Ctrl+B P` / `Ctrl+B C`** — select a name, press the chord, and
   the AI receives that entry's content as RAG context. Ask it
   questions about your canon and the model answers from your own
   lore.
3. **Stemmed matching** — "Москва" entered once matches "Москве",
   "Москвы", "Москвою", etc. (Russian stemmer); "city" matches
   "cities" (English stemmer).

This tutorial is a hands-on companion to
[`../LOCATIONS.md`](../LOCATIONS.md) and
[`../CHARACTERS.md`](../CHARACTERS.md) (which are reference docs).

## Set the project language

Both overlays use Snowball stemmers driven by the top-level `language`
field. Open `inkhaven.hjson` and confirm:

```hjson
language: english     # or: russian, german, french, spanish, …
```

Default is `english`. Set it to the dominant language of your
manuscript. See [`../CONFIGURATION.md`](../CONFIGURATION.md#prompts_file-and-language)
for the full supported list.

## Record a Place

1. In the Tree pane, expand the **Places** book (`→` on the row).
2. Optionally add a chapter to group entries (`C`):
   ```
   Cities
   Regions
   Buildings
   ```
3. With the cursor on a chapter (or directly on Places), press `+`
   to add a paragraph.
4. In the Add modal, **enter the place name as the title**:
   `Москва`, `King's Landing`, `Apartment 12`, `Lighthouse Cove`.
5. Press Enter. The paragraph opens in the editor.
6. Type whatever you want to remember:
   ```typst
   = Москва

   = History
   Столица России. Основана в 1147 году.

   = Geography
   На реке Москве, в Центральной России.

   = Recent events
   Действие глав 3 и 7 разворачивается здесь.
   ```
7. `Ctrl+S` to save.

## See the overlay in action

Open any other paragraph in your manuscript. Any mention of
`Москва` — in any case (`Москвы`, `Москве`, `Москвою`, `Москве`,
`Москвой`) — lights up in **cyan + bold**.

The colour is configurable: `theme.places_fg` (default `#89dceb`).

The overlay refreshes:

- Live as you type — adding a new sentence about Москва lights it up
  on the next render.
- After saving a new Place — the entry is added to the lexicon at the
  same moment the save completes.
- On project open — the lexicon is compiled from scratch.

## Record a Character

Exact same workflow, except in the **Characters** book and the
overlay is **yellow** (`theme.characters_fg`, default `#f9e2af`).

```
├─ Characters
│  ├─ Main characters
│  │  ├─ ¶ Aragorn
│  │  ├─ ¶ Frodo
│  │  └─ ¶ Sam
│  ├─ Supporting
│  │  └─ ¶ Strider             (alias for Aragorn — separate entry)
│  └─ ¶ Random villager
```

Each paragraph's title is the canonical name. Mentions of that name
in your prose light up in yellow with stemming applied.

## Multi-word names

Both books handle multi-word titles. `Anna Karenina` matches
`Анна Каренина` in any case form; `King's Landing` matches verbatim;
`North Tower` matches `North Tower`. The matcher tokenises the title,
stems each token, and looks for the same sequence in the buffer.

Caveat: a multi-word title does **not** match its individual words in
isolation. If both forms matter, add two entries:

```
├─ ¶ Anna Karenina        (the full name)
├─ ¶ Anna                 (when she's referred to as just "Anna")
```

## `Ctrl+B P` — Place RAG inference

The overlay is passive. To ask the AI about a place, use the chord:

1. In the editor, **select** the place name (or place the cursor
   inside the word).
2. Press **`Ctrl+B P`**.

Two outcomes depending on the AI prompt bar:

### AI prompt empty

The Place entries matching your selection are stashed as the next RAG
prefix. Focus jumps to the **AI prompt bar**. Status reports:

```
Place RAG armed for `Москва` — type your question and Enter
```

Type a question:

```
> When was it founded and who lived there in 1812?
```

Enter. The inference fires with the Place context **prepended** to
your question. Focus moves to the AI pane to watch the answer stream.

### AI prompt non-empty

The inference fires **immediately**. The Place context is prepended
to whatever you had typed in the bar. Focus moves to AI pane.

### What the model sees

The Place context block looks like:

```
── Place context for `Москва` (1 match(es)) ──

── Place: Москва ──
= Москва

= History
Столица России. Основана в 1147 году.

= Geography
На реке Москве, в Центральной России.

= Recent events
Действие глав 3 и 7 разворачивается здесь.
── end place ──
```

…followed by your question. With **F10 = Local**, the model is
constrained to use only this context (no general knowledge fallback).

### When the lookup matches nothing

Status: `Place RAG: no entry titled like 'XYZ' in the Places book`.
No inference fires. Add a Place entry first.

## `Ctrl+B C` — Character RAG inference

Identical to `Ctrl+B P` but against the **Characters** book.

```
Place the cursor inside Aragorn's name → Ctrl+B C → "What weapon does he carry?" → Enter
```

Returns:

```
── Character context for `Aragorn` (1 match(es)) ──

── Character: Aragorn ──
= Aragorn
Son of Arathorn, last heir of Isildur. Carries Andúril, reforged from
the shards of Narsil…
── end character ──
```

The chord works in any focus (it goes through the meta-prefix
dispatcher), but selecting in the editor first is the typical flow.

## Workflow patterns

### Drafting from worldbuilding

Open the editor with a paragraph that needs a city description. You
remember you wrote details about that city last week but don't recall
the specifics. `Ctrl+B P` on the city's name; "Describe the streets
at dawn"; Enter. The model writes a description from your own canon.

### Continuity check

Hit a scene where a character does something you're not sure fits
their backstory. `Ctrl+B C` on the character's name; "Is X consistent
with their background?"; Enter. The model gives you a yes/no with
specifics drawn from the entry.

### Scope + lexicon

For richer answers, combine F9 scope with `Ctrl+B P`/`Ctrl+B C`:

1. Move cursor inside `Москва` in your prose.
2. `F9` (scope = Selection or Paragraph or Chapter, depending on how
   much context you want the model to see).
3. `Ctrl+B P` — adds the Place context.
4. Type "How does she feel about being back?"; Enter.

The model sees: [Chapter content as scope] + [Place: Москва context]
+ [your question]. Strong grounding.

### Local-first reading

Set `default: ollama` in `inkhaven.hjson`, pull `llama3.2`, and your
entire worldbuilding loop runs locally with no API costs and no data
leaving your machine. Great for sensitive manuscripts (clients,
unpublished work) and for travel without good internet.

## Pronouns and aliases

The matcher only highlights literal title matches (after stemming).
For aliases (Strider, Anna's nickname "Anya"), either:

- Add a separate paragraph per alias (separate yellow overlays).
- Or use the title for the most-frequent form and mention aliases in
  the body (no overlay for the alias, but `Ctrl+B C` on the canonical
  name still surfaces the entry).

Pronouns (he, she, they) are not character names and are deliberately
not highlighted. Use F9 scope to give the AI enough context to figure
out who "she" is.

## Tuning the colours

If yellow / cyan clash with your terminal theme, override in
`inkhaven.hjson`:

```hjson
theme: {
  places_fg:      "#a6e3a1"   # green instead of cyan
  characters_fg:  "#fab387"   # peach instead of yellow
}
```

Restart the TUI. See [`11-theming.md`](11-theming.md) for the full
palette.

## What you have learned

- The Places and Characters system books are pre-seeded; populate them
  with paragraphs whose titles are the entity names.
- Editor highlights mentions in cyan / yellow, with multilingual
  stemming.
- `Ctrl+B P` / `Ctrl+B C` ask the AI about a selected name, drawing
  RAG context from the respective book.
- Empty AI prompt → armed prefix + focus to prompt bar; non-empty →
  immediate inference + focus to AI pane.
- Combine F9 scope with Ctrl+B P/C for richer grounding.
- Aliases need separate entries; pronouns rely on F9 scope.

## Next steps

- [`08-importing-existing-docs.md`](08-importing-existing-docs.md) —
  ingesting existing worldbuilding notes into Inkhaven.
- [`../PROMPTS.md`](../PROMPTS.md) — pairing custom prompts with
  Ctrl+B P / C flows.
