# 23 — Importing a Scrivener project

Inkhaven 1.2.4 adds a one-shot `inkhaven import-scrivener`
command that ingests a Scrivener `.scriv` package into the
current project. Everything stays in one binary — no
Scrivener install, no `pandoc`, no Apple `textutil`. Pure-Rust
quick-xml + rtf-parser-tt all the way down.

## TL;DR

```sh
cd ~/Projects/my-inkhaven-project
inkhaven import-scrivener ~/Scrivener/Tides.scriv --dry-run
inkhaven import-scrivener ~/Scrivener/Tides.scriv
```

Dry run reports the mapping decision and per-paragraph
RTF→Typst conversion outcome without writing anything. The
second run actually creates the nodes.

## Flags

| Flag | What |
|------|------|
| `<path>` (positional) | The `.scriv` package directory. Scrivener stores projects as a package — on macOS it looks like a single file but is a directory. Pass that directory. |
| `--draft-as-book <title>` | Override the user-book title taken from Scrivener's "Draft" folder. By default the imported book's title comes from the Draft folder's own title in the binder. |
| `--skip-research` | Skip everything outside the Draft folder — Research, Characters, Places, Notes, Trash. Useful for "just give me the manuscript". |
| `--dry-run` | Parse, classify, convert RTF, but create no nodes. Reports the same counters the real run would emit. |

## What gets imported and where it goes

Scrivener's binder is a forest of typed items. The importer
maps them into inkhaven's `Book / Chapter / Subchapter /
Paragraph` hierarchy by this rule table:

| Scrivener position | Scrivener kind | Mapping in inkhaven |
|--------------------|----------------|----------------------|
| `Draft` (root container) | Folder | User Book (titled by Draft, or `--draft-as-book`) |
| Inside Draft, depth 1 | Folder | Chapter |
| Inside Draft, depth ≥ 2 | Folder | Subchapter (Scrivener nesting is collapsed to flat) |
| Inside Draft, any depth | Text | Paragraph (RTF body → Typst) |
| Outside Draft, top-level Folder | Folder named `Places` / `Locations` / `Settings` | System book `Places` |
| Outside Draft, top-level Folder | Folder named `Characters` / `Cast` | System book `Characters` |
| Outside Draft, top-level Folder | Folder named `Notes` / `Research` | System book `Notes` (Research merges into Notes — they're both reference material in inkhaven) |
| Outside Draft, top-level Folder | Folder named `Artefacts` / `Artifacts` / `Items` | System book `Artefacts` |
| Outside Draft | Anything else | Skipped (with the item's children also skipped — `SkipSubtree`) |
| Trash, search-results items, project notepad | — | Skipped |

The "depth 1 folder = chapter" rule is the only one that
might surprise: many Scrivener projects use Folder for both
top-level Parts/Books and inner sections. If your project has
that shape, do the dry-run first to verify, then either fold
the binder in Scrivener before importing, or accept that
inner folders become subchapters (the manuscript content is
preserved either way — only the organizational hierarchy
flattens).

## RTF → Typst conversion

Each Text document's RTF body is parsed with `rtf-parser-tt`
and walked to emit Typst markup:

- **Bold** runs → `**bold text**`
- *Italic* runs → `_italic text_`
- Paragraph breaks → blank lines
- Typst-meta characters (`*`, `_`, `@`, `<`, `>`, `#`, `\`)
  are escaped with a leading `\`
- Tables, footnotes, embedded images: silently dropped. The
  prose survives; embedded media doesn't. (If you need this,
  please file an issue with a sample document — single-file
  Scrivener has a deep grammar and the importer focuses on
  what novelists actually use.)
- Anything the RTF parser refuses outright falls back to a
  `strip_to_plain_text` pass — UUIDs and titles still land
  even if the body is malformed RTF.

Every conversion error is collected into a per-paragraph
error list and reported at the end; one bad RTF blob doesn't
abort the whole import.

## Stable UUIDs

Modern Scrivener (3.x+) stores UUIDs in the binder XML. The
importer reuses them directly so the same import twice writes
the same UUIDs.

Older Scrivener (1.x/2.x integer IDs) gets `deterministic_uuid`
treatment — UUID v5 hashed from the project name + integer
ID. Stable across re-imports of the same project, but won't
collide with a different project's IDs.

This is what makes `--dry-run` actually useful: you can dry-
run, eyeball the report, then run for real and the UUIDs are
the same ones that were in the dry-run report.

## Report

```
Scrivener import complete:
  books: 1
  chapters: 12
  subchapters: 37
  paragraphs: 408
  skipped: 5
  errors (2):
    · paragraph 'Chapter 7 / scene 3': RTF parse error: ...
    · paragraph 'Chapter 11 / scene 1': RTF parse error: ...
```

`skipped` includes both "outside-Draft items the mapping rules
ignored" and "items the conversion failed on so badly we
abandoned the row entirely". Errors are RTF→Typst failures
where the title made it but the body was lost.

## After import

1. `inkhaven reindex` — rebuilds search embeddings and the
   on-disk slug paths for the new nodes.
2. Walk the tree (`inkhaven` then `Ctrl+T`). The imported Book
   lives next to anything else you already had.
3. Run `inkhaven stats` to see word counts. The numbers come
   from inkhaven's own Typst-aware counter, not from
   Scrivener.
4. If you set up status ladders in inkhaven (see [`14-document-
   status.md`](14-document-status.md)), the imported
   paragraphs land with no status set; bulk-O in the tree
   (see [`22-tree-multiselect.md`](22-tree-multiselect.md)) is
   the fast way to triage.

## Limitations and design choices

- **One Scrivener Draft → one inkhaven Book.** Multi-Draft
  projects (rare) keep their first Draft folder; additional
  ones are skipped. File an issue if your workflow needs
  multi-Draft.
- **No round-trip.** This is import only — there's no
  `inkhaven export --as scrivener`. Once it's in inkhaven,
  inkhaven is the source of truth.
- **No external deps.** quick-xml + rtf-parser-tt are pure
  Rust and statically linked. The whole importer adds ~200
  KB to the binary.

## See also

- [`02-the-tree.md`](02-the-tree.md) — the hierarchy you just
  imported into.
- [`22-tree-multiselect.md`](22-tree-multiselect.md) — bulk
  status tagging is the fastest cleanup after a large import.
- `src/scrivener/` in the source tree — `binder.rs`, `rtf.rs`,
  `mapping.rs`, `import.rs` if you want to follow or extend
  the layers.
