# 14 — Tags

Tags are project-wide free-form labels you attach to paragraphs. They never embed in your manuscript text; they live as metadata. Use them for editing passes, beta-reader slices, POV tracking, plot threads — any cross-cutting concern that doesn't fit the tree.

## Adding tags

| Chord | What it does |
|-------|--------------|
| Ctrl+B ] | Open the tag picker for the open paragraph. |
| g (Tree) | Same picker for the tree-cursor's paragraph — works on multi-select. |
| Ctrl+B } | Project-wide tag search — every tag, every paragraph that uses it. |

![figure: tag-picker](images/tag-picker.png) — Tag picker. Checkmarked tags are on the open paragraph. Space toggles; T commits; A adds new; R renames; D deletes.

Tags are case-preserved at write time but the picker dedupes case-insensitively. Tag namespaces (e.g. `pov-aerin`, `thread-revenge`) work fine — they're just strings.

## Renaming tags project-wide

`R` inside the picker prompts for a new name and rewrites every paragraph carrying the old tag. If the new name already exists in the project, the rewrite **merges** — paragraphs that had both end up with just the destination.

## Tags in the tree pane

Paragraph rows display their first two tags as compact `#tag` chips with `+N` for additional ones:

![figure: tree-tag-pips](images/tree-tag-pips.png) — Tree paragraph rows with tag pips. `+N` shows when more than two tags are present.

## Tag-filtered export

```
inkhaven export pdf --tag draft
inkhaven export pdf --tag draft --status final
```

The first writes a PDF with only paragraphs tagged `draft`. The second AND-combines with the status filter. Useful for beta-reader slices ("here's the chapters I've marked beta-ready") and submission packages.

## Multi-select

Tags really shine when combined with tree multi-select:

| Chord | What it does |
|-------|--------------|
| Space (in tree) | Mark the cursor paragraph. |
| g (with marks) | Apply the tag picker to every marked paragraph at once. |

So you can mark 12 paragraphs across three chapters and add the `pov-aerin` tag to all of them in one operation.

## Bund — `ink.tag.*` stdlib

Five words plug tags into scripts:

```bund
                          ink.tag.list      ( -- list )
"intro/scene-1"           ink.tag.list_for  ( path -- list | NODATA )
"draft"                   ink.tag.search    ( tag -- list-of-paths )
"intro/scene-1" "draft"   ink.tag.add       ( path tag -- )
"intro/scene-1" "draft"   ink.tag.remove    ( path tag -- )
```

Policy: reads under `store_read` (default allowed); writes under `store_write` (default denied — opt in).

## Scrivener keyword import

`inkhaven import-scrivener` brings Scrivener's per-document keywords across as inkhaven tags. Both shapes are handled — the modern `<KeywordRef>`-against-registry form (Scrivener 3.x) and the older inline `<Keywords>foo, bar; baz</Keywords>` form. See Chapter 26.

## Recap

- Tags are project-wide free-form labels stored as metadata; never embed in prose.
- `Ctrl+B ]` / `g` (tree) — picker. `Ctrl+B }` — search.
- `R` inside the picker renames project-wide; merges into an existing tag if present.
- Tree pane shows first 2 tags as `#tag` pips + `+N`.
- `inkhaven export --tag <name>` filters export to a tag slice; combine with `--status`.
- Bund: `ink.tag.list / list_for / search / add / remove`.
- Scrivener keywords import automatically as tags.
