# 25 — Tag workflows

Inkhaven 1.2.5 added free-form **tags** as first-class metadata
on every paragraph. The 1.2.6 cycle filled out the management
surface: project-wide rename, tag-filtered export, tree-pane
pips, Bund stdlib, Scrivener keyword import. This tutorial
walks through the whole flow end to end.

Tags are stored alongside `status`, `linked_paragraphs`, etc.
in the paragraph's metadata blob — never embedded in the
manuscript text. They survive renames (UUIDs are stable),
travel with backups, and stay invisible in every export.

## The two chords

| Chord       | Mode    | What it opens |
|-------------|---------|----------------|
| `Ctrl+B ]`  | Editor  | Tag picker scoped to the open paragraph. Toggle existing tags, add new ones, merge. |
| `Ctrl+B }`  | Anywhere | Project-wide tag search. Enter on a tag → a list of every paragraph carrying it; Enter on a paragraph opens it in the editor. |
| `g`         | Tree    | Same as `Ctrl+B ]`, applied to the tree cursor's paragraph (or — with marks — to every marked paragraph at once). |

`Ctrl+B ]` and `g` open the same `Modal::TagPicker`. The
project-wide search variant (`Ctrl+B }`) starts from the
same picker shape but Enter changes meaning: it lists the
paragraphs instead of toggling the tag.

## Adding a tag

Open a paragraph. `Ctrl+B ]`:

```
┌── Tags · 4 in this project · open paragraph: The Storm ─┐
│ → ✓ draft                                                │
│     plot-twist                                            │
│   ✓ weather                                               │
│     worldbuild                                            │
│ ↑↓ select · Space marks · T applies · A adds · R renames │
│ · D deletes · Esc closes                                  │
└──────────────────────────────────────────────────────────┘
```

`Space` toggles the cursor tag; `T` (or Enter in editor mode)
applies the marked set onto the paragraph. The tag picker is
**multi-select** — you can flip several tags before committing.

**`A` adds a brand-new tag**: a tiny prompt pops over the
picker, you type the name, Enter commits. The new tag
becomes part of the project-wide set and gets a checkmark on
the open paragraph automatically.

## Project-wide rename (1.2.6+)

`R` inside the picker prompts for a new name for the cursor
tag and rewrites every node that carries the old name:

```
┌── Rename tag — R ─────────────────────────────────────────┐
│                                                            │
│ Rename tag `wether` (12 paragraph(s)):                     │
│ › weather│                                                 │
│                                                            │
│   Enter commits (merges if name exists) · Esc cancels      │
└────────────────────────────────────────────────────────────┘
```

If the new name already exists in the project (e.g. you're
consolidating `wether` and `weather`), the rewrite **merges**
— the destination tag deduplicates per paragraph. So a row
that had both `wether` and `weather` ends up with just
`weather`.

The status bar reports the blast radius:
`tag renamed: \`wether\` → \`weather\` · touched 12 paragraph(s)`.

## Deleting a tag (1.2.5+)

`D` inside the picker pops a confirm-and-delete modal:

```
┌── Delete tag — y / n ────────────────────────────────────┐
│                                                           │
│  Delete tag `weather` project-wide?                       │
│     Will be removed from 12 paragraph(s).                 │
│                                                           │
│   y / Enter confirm · n / Esc cancel                      │
└───────────────────────────────────────────────────────────┘
```

Project-wide — removes the tag from every paragraph carrying
it AND drops the tag from the project-wide set. The status
bar reports the actual count of nodes touched.

## Tag pips in the tree pane (1.2.6+)

Paragraph rows in the tree pane show their first two tags as
compact `#tag` chips after the per-paragraph progress dot:

```
│  ¶ The storm                ●  #draft #weather
│  ¶ Bell tower               ◑  #plot-twist +1
```

Each chip truncates to ~9 chars + ellipsis so a heavily-tagged
paragraph doesn't push everything off the pane. When more than
two tags are present, a `+N` suffix tells you to open
`Ctrl+B ]` to see the rest.

## Tag-filtered export (1.2.6+)

`inkhaven export pdf --tag draft` ships only paragraphs that
carry the named tag. Combines with `--status`:

```bash
# All paragraphs tagged `draft`, regardless of status.
inkhaven export pdf --tag draft

# Paragraphs tagged `draft` AND at Status:Final or above.
inkhaven export pdf --tag draft --status final

# Multi-book project: scope to one book.
inkhaven export markdown --book-name "Aerin Saga" --tag flashback
```

Case-insensitive match against `Node.tags`. A paragraph must
pass both `--tag` AND `--status` predicates to be exported —
the two are AND-combined.

Useful workflows:

- **Mid-book draft slice for a beta reader** — `--tag beta` to
  ship only the chapters you've stamped beta-ready.
- **One POV at a time** — tag every Aerin POV paragraph
  `aerin`, every Brann POV `brann`; export each separately.
- **Submission package** — keep a `submission` tag for the
  paragraphs you want to send to an agent; rest of the book
  stays out of the artefact.

## Searching by tag (`Ctrl+B }`)

From anywhere in the TUI:

```
┌── Tags · search mode · 4 in project ─────────────────────┐
│ → draft                                                   │
│   plot-twist                                              │
│   weather                                                 │
│   worldbuild                                              │
│ ↑↓ select · Enter shows paragraphs · R renames · D deletes│
│ · Esc closes                                              │
└──────────────────────────────────────────────────────────┘
```

Enter on a tag opens the per-tag results modal — every
paragraph carrying it. Enter on a paragraph there opens it in
the editor. Useful for cross-cutting passes: "show me every
paragraph tagged `weather` and read them as a sequence."

## Scrivener keyword import (1.2.6+)

When you import a Scrivener `.scriv` project, every keyword
becomes an inkhaven tag automatically. Both keyword shapes are
recognised:

1. **Modern Scrivener 3.x** — project-level `<Keywords>` registry
   plus per-binder-item `<KeywordRef ID="N"/>` references.
2. **Older / lighter exports** — inline
   `<MetaData><Keywords>foo, bar; baz</Keywords></MetaData>`
   with comma / semicolon / newline-separated names.

Both end up on `Node.tags` after import — order from the source,
case preserved, duplicates dropped. Scope: paragraphs only
(Scrivener allows keywords on folders too, but inkhaven's tag
picker is paragraph-focused so we don't surprise the user).

```bash
inkhaven import-scrivener --source ~/Documents/Aerin.scriv
# … converting 412 documents … importing 41 unique keywords …
```

After import, `Ctrl+B }` lists every Scrivener keyword as an
inkhaven tag, ready for the same picker / rename / export
flow.

## Bund stdlib (1.2.6+)

Five `ink.tag.*` words plug tags into scripts:

```bund
                ink.tag.list      ( -- list )
"intro/scene-1" ink.tag.list_for  ( path -- list | NODATA )
"draft"         ink.tag.search    ( tag -- list of paragraph-paths )
"intro/scene-1" "draft" ink.tag.add    ( path tag -- )
"intro/scene-1" "draft" ink.tag.remove ( path tag -- )
```

Policy: reads under `store_read` (default-allowed); writes
under `store_write` (default-denied — opt in via HJSON
`scripting.enabled_categories`).

Example — tag every paragraph in a chapter as `editing-pass-2`:

```bund
"intro" ink.node.children          // ( list )
{
  dup ink.node.get "kind" get      // ( node-hash kind )
  "Paragraph" =                    // ( node-hash bool )
  {
    "path" get                     // ( path )
    "editing-pass-2"
    ink.tag.add
  }
  { drop } ifelse
} each
```

## Cleanup on delete (1.2.6+)

When a paragraph is deleted, its `linked_paragraphs` references
across the rest of the project get scrubbed automatically (1.2.6
AC). The same scrub now also walks every event's `characters` /
`places` link list and prunes deleted UUIDs — no manual reindex
needed after a delete.

## Recap

- Tags are project-wide metadata, never embedded in the manuscript.
- `Ctrl+B ]` (editor) / `g` (tree) / `Ctrl+B }` (search) — three
  ways into the same picker.
- `R` renames project-wide; merges when the destination exists.
- Tree pane shows first 2 tags as `#tag` pips + `+N` count.
- `inkhaven export --tag <name>` filters the export to just
  that slice.
- Scrivener keywords import as tags automatically.
- `ink.tag.*` Bund stdlib for scripted tag management.
