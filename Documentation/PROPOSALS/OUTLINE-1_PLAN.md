# OUTLINE-1 — Full-screen manuscript Outline pane

| | |
|---|---|
| **RFC** | OUTLINE-1 |
| **Title** | Full-screen foldable Outline pane; node reorder / promote / demote; cross-parent paragraph copy/move; CLI + Bund parity |
| **Status** | Shipped — 1.4.13 |
| **Author** | Vladimir Ulogov |
| **Depends on** | none — purely additive |
| **New dependency** | none |

The side Tree pane navigates; it does not restructure comfortably. OUTLINE-1
adds a full-screen, foldable view of the whole manuscript with structural
editing — reorder, promote/demote, and **cross-parent paragraph copy/move** —
plus CLI and Bund parity. Every mutation runs on the same filesystem-aware
store primitives the Tree pane uses.

## Surfaces

- **Activation** — `Ctrl+2` (and its `Ctrl+@` / `KeyCode::Null` re-encodings)
  opens the Outline; `Ctrl+B Shift+O` is the rebindable backup (`outline.open`).
  `Ctrl+T` keeps focusing the side Tree pane.
- **Modal::Outline** — a unit marker; the persisted view state lives on
  `App.outline_state` and round-trips to `.inkhaven/outline-state.json`
  (tolerant load, atomic save, excluded from backup).
- **Keys** — `j`/`k`/`g`/`G` navigate; `Enter`/`l`/`h`/`Space` fold/step;
  `Shift+J`/`Shift+K` reorder; `<`/`>` promote/demote; `y`/`m`/`f` copy/move/
  affix; `/` filters; `Esc` is staged (exit-edit → clear-filter → save+close).

## Store primitives (the spine)

| Method | Used by |
|---|---|
| `swap_siblings` (existing) | reorder — Outline `Shift+J/K`, Tree `U/J`, `inkhaven mv` |
| `move_node_to_parent` (new) | promote/demote, `y`+`f` move, `paragraph move`, `ink.outline.paragraph_move` |
| `copy_paragraph_to_parent` (new) | `y`+`f` copy, `paragraph copy`, `ink.outline.paragraph_copy` |

**Childless-only reparenting.** `move_node_to_parent` is restricted to nodes
with no children (paragraphs / leaves / empty branches). A branch move would
also have to rewrite every descendant's stored `path` depth-key (the load-time
sort key) plus their `file` paths — that's the Tree pane's domain and out of
scope. Promote/demote and paragraph move are all childless, so one primitive
covers them. Copy carries the prose metadata (tags / status / target /
content-type / outgoing links) but **not** the timeline event — a copy must
never mint a duplicate event.

## Phases

| Phase | Content |
|---|---|
| O-P0 | `OutlineState` + `.inkhaven/outline-state.json` sidecar |
| O-P1 | Pure row model — default-view seed, `visible_rows`, cursor nav (unit-tested) |
| O-P2 | Open/close + full-screen draw + nav/fold; `Ctrl+2` reroute + `Ctrl+B Shift+O` |
| O-P3 | Reorder (`Shift+J/K`) + promote/demote (`</>`); `move_node_to_parent` |
| O-P4 | Cross-parent paragraph copy/move (`y`/`m`/`f`) in both Outline and Tree |
| O-P5 | Inline `/` filter (Unicode-aware path-to-match) + detail panel |
| O-P6 | CLI: `inkhaven outline`, `inkhaven paragraph copy\|move` |
| O-P7 | Bund: `ink.outline.{print, paragraph_copy, paragraph_move}` (policy-gated) |
| O-P8 | Docs: KEYBINDING §9, in-app quickref, Tutorial 96 |

## Audit notes (RFC vs reality)

The RFC carried several claims that didn't match the code; corrected before
implementing:

- **`sort_order` field** → the real field is `Node.order: u32`.
- **`node.created_at`** → does not exist (only `modified_at`); the detail panel
  uses `modified_at`.
- **`src/tui/tree.rs` / `NodeTree` / `TreeWidget` / `TreeWidgetState`** →
  fabricated. The renderer was built fresh over the live `Hierarchy`
  (`flatten_with_collapsed`), with its own row model.
- **`compute_node_badge`** → the real helper is `compute_tree_badges`.
- **K/J reorder "to add"** → already existed as `move_current(MoveDir)` /
  `swap_siblings`; reused.
- **"files may or may not carry a numeric prefix"** → files always carry the
  `{:02}-` order prefix, so a reorder always renames.

## Verification

- Pure model unit-tested (default view, expand reveals, seed no-op, cursor
  clamp/track, reanchor, filter keeps matches + ancestors).
- Reparent + copy primitives verified end-to-end on a temp project: move
  relocates with no stale file and reassigned `NN-` prefixes; copy duplicates
  with identical body and a fresh uuid.
- `ink.outline.print` renders via `inkhaven bund`; the store_write mutators are
  correctly deny-gated.
- Tests 1996 → 2005.

## Non-goals / out of scope

- Branch (chapter/subchapter) cross-parent move — Tree-pane territory.
- Multi-select bulk copy/move in the Outline (single cursor node for now).
- Precise insert-at-position on affix (lands at end of the effective parent;
  follow with `Shift+J/K`).
