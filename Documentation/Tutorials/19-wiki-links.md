# 19 — Paragraph links and backlinks

> Earlier docs and release notes called these "paragraph links".
> That name is misleading — there's no `[[X]]` inline
> syntax, no parser scanning prose, no MediaWiki-style
> resolution.  These are **paragraph cross-references**
> stored purely as metadata.  1.2.9+ uses the name
> "paragraph links" throughout; the chords (`Ctrl+V A` /
> `I` / `L` / `K`) and the underlying `linked_paragraphs`
> field are unchanged.

Inkhaven 1.2.4 added typed references between paragraphs as
**metadata**, not as embedded markers in the typst source. The
link never leaks into your PDF / EPUB; it lives in the
paragraph's metadata next to `status`, `target_words`, etc.,
and surfaces through dedicated chords + the AI inference path.

The metadata-only choice is deliberate: no inline `[[X]]`
syntax leaks into export; renaming a paragraph never breaks
the link (UUIDs are stable); and the AI inference path can
read structured references without parsing markup.

## The four chords

| Chord     | Direction | What it does |
|-----------|-----------|--------------|
| `Ctrl+V A` | Outgoing  | Add a link FROM the open paragraph TO a picked paragraph. Tree pane switches to "select paragraph to link" mode; Enter confirms. |
| `Ctrl+V I` | Incoming  | Add a link FROM a picked paragraph TO the open paragraph. Reverse of `Ctrl+V A` — useful for "this scene should reference X". |
| `Ctrl+V L` | Outgoing  | Open the floating "linked paragraphs" picker. Enter opens the chosen one; D removes the link. |
| `Ctrl+V K` | Incoming  | Open the floating "backlinks" picker — paragraphs that link TO the open one. Same Enter / D semantics; D removes the SOURCE's outgoing link. |

All four are bind-table actions (`view.add_link`,
`view.add_incoming_link`, `view.list_links`,
`view.list_backlinks`) so HJSON `keys.bindings` + `ink.key.bind`
can rewrite them.

## Adding a link

The cleanest flow for adding an outgoing link:

1. Open the source paragraph in the editor.
2. `Ctrl+V A`.
3. Tree pane gets a title flip:
   ```
   ┌── Tree · select paragraph to link · Esc cancels ──────┐
   ```
4. Navigate the tree (arrows, slash filter via `Ctrl+/` etc.)
   to the target paragraph.
5. `Enter` adds the link and returns focus to the editor.
6. `Esc` at any point cancels.

For incoming, the same flow with `Ctrl+V I` — the title text
changes to "Tree · select paragraph that will link to current"
and the chosen paragraph's metadata is the one that grows the
new outgoing link.

## Guards

Three checks fire at `add_link` time:

- **Self-link**: `owner == target` is rejected with
  `"can't link a paragraph to itself"`.
- **Duplicate**: if the link is already present, the chord
  reports `"already linked"` and does nothing.
- **Cycle**: a DFS over the candidate target's outgoing
  closure looks for the owner. If found, the chord refuses
  with `"You can not create circular references"` and returns
  focus to the editor. So `A→B` then `B→A` is blocked;
  `A→B→C→A` is also blocked; harmless DAG-shaped graphs
  pass through.

The cycle check is what makes outgoing-link metadata safe to
walk for AI inference — there's no infinite-recursion risk
because there's no cycle.

## Listing + removing

`Ctrl+V L` and `Ctrl+V K` open identical-looking modals that
list the open paragraph's outgoing / incoming links:

```
┌── Linked paragraphs (3) ───────────────────────────────────┐
│  → The storm    story/01-arrival/the-storm                │
│  → Lightning    story/01-arrival/lightning                │
│  → Bell tower   story/01-arrival/bell-tower               │
│ ↑↓ select · Enter opens · D removes · Esc closes  (1/3)   │
└────────────────────────────────────────────────────────────┘
```

- **`Enter`** — close the modal, save the current buffer, load
  the chosen paragraph into the editor (tree cursor follows).
- **`D`** / `Delete` — remove the link. For outgoing (L), pulls
  the row's UUID out of the open paragraph's `linked_paragraphs`.
  For incoming (K), removes the SOURCE's outgoing link to
  current — symmetric "delete from the side that owns the
  metadata".
- **`Esc`** — close.

The modal auto-closes when the last row is removed so you
never end up staring at an empty pane.

The backlinks modal renders rows with `←` instead of `→` so
the direction is unambiguous.

## Status-bar count

When the open paragraph has at least one outgoing link, the
status-bar widget appends `links N`:

```
today 1,247w · 45m · streak 3d · links 4
```

No equivalent count for backlinks in the widget today — they
can be seen via `Ctrl+V K`.

## AI inference integration

When the AI scope is `Paragraph` (F9 cycles), inkhaven
appends each linked paragraph's body to the prompt context
after the main paragraph, wrapped in `── Linked paragraph:
<title> ──` / `── end linked paragraph ──` delimiters. So
the model sees the explicit related-material you curated,
not just the editor selection.

Direct outgoing only — matches the status-bar count and keeps
the prompt size predictable. If you want transitive inclusion
(`A→B→C` flowing all into context), curate the chain on `A`
directly with `Ctrl+V A` four times rather than relying on
graph traversal.

## When to use what

| Scenario | Chord |
|----------|-------|
| "This scene should be aware of that older scene" | `Ctrl+V A` from the new scene → older one |
| "When the user reads this, they should be able to see what depends on it" | `Ctrl+V K` to inspect backlinks |
| "Let me jump back to the source of this incoming link" | `Ctrl+V K`, `Enter` on the row |
| "I want the AI to see this related material when I prompt with scope=Paragraph" | `Ctrl+V A` to add the relation; F9 to set scope; send a prompt |
| "I forgot which scenes depend on this one" | `Ctrl+V K` |

## Anti-patterns

- **Don't put `[[ ]]` in the prose**. Inkhaven won't pick it up —
  the link metadata is the source of truth. If you want a
  visible cross-reference in the rendered PDF, write `see
  Chapter 3` in plain Typst alongside the metadata link.
- **Don't expect the graph to fan out automatically**. Direct
  outgoing only at AI-inference time. If you want a richer
  context window, link explicitly to every paragraph the AI
  should see.

## Storage details

* Field: `Node.linked_paragraphs: Vec<Uuid>`, serde-default empty.
* Persisted via the standard `Store::raw().update_metadata` path.
* Stale targets (paragraph deleted) are silently filtered from
  picker entries — the UUID stays in the metadata but doesn't
  render. `inkhaven reindex` is the cleanup escape hatch.

## See also

- [`14-document-status.md`](14-document-status.md) — the
  status ladder that pairs well with the link graph for "show
  me only Ready paragraphs and their dependencies".
- [`17-writing-goals.md`](17-writing-goals.md) — outgoing links
  affect the AI context, not the word count.
