# 40 — Concordance view

`Ctrl+B Shift+L` opens a project-wide concordance:
every distinct word in your manuscript, ranked by
occurrence count, with up to three keyword-in-context
(KWIC) samples per row.

```
┌─ Concordance — project-wide ─────────────────────────┐
│ 1,247 distinct · 19,432 tokens · 84 paragraphs       │
│ filter: walk│  sort: count   (3 shown)               │
│ #     word                       count   variants    │
│   1   walked                       18    (walk, walking)
│ ▶ 2   walking                      11                │
│   3   walk                          7                │
│                                                      │
│ samples for "walked"  (18× total)                    │
│   aerin/chapter-1:l3   …Anna «walked» away into…     │
│   aerin/chapter-2:l8   The boy «walked» a hundred…   │
│   khaal/chapter-1:l1   He «walked» as if the stones… │
│                                                      │
│ ↑↓ navigate · type to filter · Ctrl+S sort · Esc     │
└──────────────────────────────────────────────────────┘
```

## What the pipeline does for you

When you open the modal, inkhaven:

1. Walks every Paragraph node in the project's
   hierarchy.
2. Strips the leading typst heading line so the
   title doesn't inflate counts.
3. Tokenises with `unicode-segmentation`'s UAX-#29
   word segmenter — Cyrillic / Latin / Greek /
   Devanagari boundaries handled identically.
4. Drops single-character tokens, pure-digit runs,
   and stop-words.  (Stop-words reuse the
   `editor.style_warnings.repeated_phrases.<lang>_stop_words`
   list — same multilingual setup as the repeated-
   phrase detector, no second list to tune.)
5. Stems each surviving token with the project's
   Snowball algorithm.  `walk`/`walked`/`walking`
   all key on a single stem; Russian `сказал`/
   `сказала`/`сказали` collapse the same way.
   Disable via
   `editor.style_warnings.repeated_phrases.use_stemming
   = false`.
6. Groups by stem-key, counts, and captures the
   first three encountered occurrences as KWIC
   samples (32-char half-width, `«match»` wrapping
   for visual contrast in monospace output).
7. Sorts by count descending; ties broken by
   headword ascending.

## Interactions

Inside the modal:

| Chord                     | Action                                            |
|---------------------------|---------------------------------------------------|
| Any printable character   | Append to filter (substring match on headword + variants) |
| `Backspace` / `Delete`    | Edit the filter buffer                            |
| `Ctrl+S`                  | Toggle sort: count ↔ alphabetical                 |
| `↑` `↓` / `PgUp` / `PgDn` | Navigate                                           |
| `Home` / `End`            | Jump to first / last visible row                  |
| `Esc`                     | Close                                              |

Plain `s` types into the filter — it doesn't toggle
sort.  Use `Ctrl+S` so headwords starting with `s`
stay typeable.

## Filter semantics

The filter substring-matches against the headword
**and** every kept variant.  So typing `walk`
surfaces the entry whose headword is `walked` (the
most-common surface form for that stem) but whose
variants list includes `walking` and `walk`.

Type more characters to narrow further — `walke`
narrows the same entry down further; `walker` would
land on a different stem entirely.

## Memory + performance

- Max 3 samples + 5 variants per stem, so even a
  Tolstoy-scale corpus stays bounded.
- Build completes well under a second on a 100k-word
  manuscript; the work runs synchronously on the UI
  thread.
- Filter + sort changes are instant after the
  initial build — `visible` is a cached `Vec<usize>`
  into `data.entries`.

## Use cases

- **Auditing your prose's lexical fingerprint**.
  The top of the list is your *real* voice — not
  the voice you imagine.  Surprises are common.
- **Spotting overworked vocabulary**.  When `glanced`
  shows up 47 times across the book, it's time to
  swap in `peered` / `looked up` / `caught sight of`.
- **Locating a character or place mention**.  Type
  the name in the filter; KWIC samples show every
  place it appears with breadcrumb-style slug paths
  (`aerin/chapter-2:l8`) — faster than full-text
  search if you already know the word.

## See also

- [`03-the-editor.md`](03-the-editor.md) — inline
  filter-word + repeated-phrase overlays (related
  style-checking layer that's always-on while you
  edit).
- [`16-similar-paragraphs.md`](16-similar-paragraphs.md)
  — vector-similarity picker (semantic version of
  "find me where I wrote about X").
