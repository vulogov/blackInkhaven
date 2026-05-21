# 22 — Tree multi-select and bulk chords

Tree-pane multi-select lets you mark a set of paragraphs (or
any nodes) and apply one action to all of them in one chord.
1.2.4 ships the multi-select primitive plus two new tree-pane
chords that work on either the single cursor row OR the
multi-selection if one exists.

## Marking

Focus the tree pane, then:

| Key | Effect |
|-----|--------|
| `Space` | Toggle mark on the cursor row. The row's title gets a leading `✓` and the row count appears in the status bar. |
| `Esc` | Clear all marks. |
| `Space` (on a row that's already marked) | Unmark. |

Marks survive cursor movement, expand/collapse, and any
search/filter — they're keyed by UUID, not by visible row.
Marks are cleared on tree refresh after a Reindex, or by Esc.

## T — cycle node type

With the tree focused, press `T`:

- **No marks**: cycles the cursor row's `NodeKind` through the
  small leaf rotation `paragraph → json → script → paragraph`.
  Folders (book / chapter / subchapter) are skipped silently —
  T is a leaf-only chord.
- **With marks**: cycles every marked LEAF row's `NodeKind` by
  one step in the same rotation. Non-leaf marked rows are
  skipped. The cursor doesn't move. Status bar reports
  `cycled type on N node(s)`.

T-with-marks is the easy way to convert a batch of paragraphs
into scripts (or back). Before 1.2.4 this needed individual
edits via metadata or a Bund script.

## O — cycle status

`O` with the tree focused:

- **No marks**: advances the cursor paragraph one rung up the
  status ladder (`napkin → first → second → third → final →
  ready → napkin`, configurable via `goals.status_ladder`).
- **With marks**: advances every marked paragraph by one rung.
  Status bar reports `promoted N paragraph(s) to <status>` —
  bulk move tied to the explicit user gesture.

Bulk O is the answer to "I had a writing session, mark all
those scenes as second-draft". The same chord exists in the
editor as `Ctrl+V T` for the open paragraph; tree-pane `O` is
the multi-target variant.

## Other chords that respect the mark set

Three earlier chords were upgraded in 1.2.4 to read the mark
set when present:

| Chord | Solo behaviour | With marks |
|-------|----------------|------------|
| `Ctrl+B R` | Rename the cursor row. | Disabled — renames are per-row by design (rename a batch with `inkhaven` CLI scripting instead). |
| `Ctrl+B I` | Reindex from the cursor subtree. | Reindex from each marked subtree (deduped — overlapping marks roll up to the deepest common ancestor). |
| Delete (`Del`) | Delete the cursor row (with confirm modal). | Delete every marked row (single confirm modal; descendants collapse normally). |

If you want a chord to work on marks that doesn't yet, ask in
the issue tracker — multi-select-aware actions are cheap to
add once the mark set is the source of truth.

## A worked example

You finished a Saturday session. Five paragraphs were
napkin-status going in; three are now firmly second-draft. You
want to bump exactly those three. Walk the tree with `Space`
on each row (or `Ctrl+/` filter → Space → next), then:

```
Tree pane
  ✓ The storm           [napkin]
  ✓ Bell tower          [napkin]
  ✓ Lightning           [napkin]
    Three weeks         [napkin]
    Morning ritual      [napkin]

O               → promoted 3 paragraph(s) to first
O               → promoted 3 paragraph(s) to second
```

Then `Esc` to clear marks. The other two paragraphs stay at
napkin — you didn't touch them.

## When NOT to mark

Single-row chords like `Ctrl+B R` (rename) intentionally
ignore marks. Anything fundamentally per-row — rename, jump
into editor, copy slug to clipboard — stays per-row even when
marks exist, so the mark set never accidentally amplifies an
error.

When in doubt, the status bar tells you. If a chord operated
on the mark set, the report uses `N node(s)`; if it operated
on the cursor row alone, it uses the row's title.

## Implementation notes

* `App.tree_marked: HashSet<Uuid>`.
* `cycle_status_single` and `cycle_leaf_type_single` for the
  no-marks paths; `cycle_leaf_type_bulk` for the marks path.
  Single and bulk share the rotation table — one source of
  truth for "what comes next".
* The status-bar renders the mark count when ≥ 1:
  `marked 3` between the active-time widget and the link count.

## See also

- [`02-the-tree.md`](02-the-tree.md) — basic tree navigation
  and the existing `Ctrl+B` chord family.
- [`14-document-status.md`](14-document-status.md) — the status
  ladder and its `goals.status_ladder` configuration.
- [`17-writing-goals.md`](17-writing-goals.md) — `auto_promote_on_target`
  also calls into `cycle_status_single` once a target is hit.
