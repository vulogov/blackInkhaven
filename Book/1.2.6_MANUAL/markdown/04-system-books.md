# 4 — System books

Every inkhaven project ships with eight pre-created books that live alongside your manuscript. They handle a specific role in the writing pipeline; the tree pane shows them after your user books.

## The full set

| Book | Role |
|------|------|
| Notes | Free-form notes, outlines, side thoughts. The default search target for `Ctrl+B G` (note-RAG). |
| Research | World-research / source material. Searchable by F1 if you add it to the Help index. |
| Prompts | AI prompt templates. Override the embedded defaults by renaming a paragraph to drop its `.example` suffix. |
| Places | Locations referenced by the manuscript. Powers the yellow place-highlight overlay in the editor. |
| Characters | Character cards. Powers the cyan character-highlight overlay. |
| Artefacts | Physical objects, magic items, recurring symbols. Same overlay machinery as Places / Characters. |
| Typst | Read-only. Bundled typst reference that powers F1 typst-help queries. |
| Help | Read-only. Bundled how-to text. F1 RAG searches here. |

There's also a per-user-book **Timeline** chapter that materialises lazily on first `event add` (Chapter 17). It's tagged `system_tag: book_timeline` so renames don't break it.

## Why they exist

Each system book pays its rent in features:

- **Prompts:** When `Ctrl+B G` (grammar check), `Ctrl+F12` (explain diagnostic), or `F12` (critique) sends a prompt to the AI, it resolves the prompt template by looking up a paragraph in this book first, then falling back to `prompts.hjson`, then to the embedded default.
- **Places / Characters / Artefacts:** Inkhaven scans the open paragraph against the titles in these books and highlights matches inline. `Ctrl+B P` / `Ctrl+B C` / `Ctrl+B A` open a RAG flow against the matched entry. See Chapter 13.
- **Notes:** `Ctrl+B G` (note RAG) sends a query against every note's content with the AI hooked up — useful when you've scribbled something three weeks ago and can't remember where.
- **Typst / Help:** F1 query against either one.

## Treating them as ordinary books

System books look special in the tree (a `(system)` suffix + they're protected from `D` delete on the book itself) but their paragraphs are ordinary paragraphs. Tag them, link them, give them word targets — the same metadata stack applies. So a Character card can have status `Final`, a target of 800 words, and tags `[protagonist, main, draft]`.

> **If you import a Scrivener project:** Inkhaven's importer maps Scrivener's "Characters" / "Places" / "Notes" folders to the corresponding system books automatically. See Chapter 26.

## The Prompts book

The Prompts book gets its own chapter (Chapter 20) but the short version: on `inkhaven init`, five paragraph templates get seeded automatically with the `.example` suffix:

```
Prompts/
├── grammar-check.example
├── explain-diagnostic.example
├── critique-edit.example
├── critique-changes.example
└── timeline-health.example
```

Open one (`Enter`), edit the body, then rename (`F2`) to drop the `.example` suffix. From that moment inkhaven uses your version instead of the embedded fallback.

## Recap

- Eight system books: Notes, Research, Prompts, Places, Characters, Artefacts, Typst, Help.
- Each pays for itself by powering a specific feature (RAG, lexicon highlight, prompt resolution).
- System books are protected at the book level but their paragraphs are ordinary.
- The lazy Timeline chapter is per-book — Chapter 17.
- Customise AI prompts by editing the `.example` seeds in the Prompts book and dropping the suffix.
