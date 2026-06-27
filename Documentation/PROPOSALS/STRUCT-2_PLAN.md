# STRUCT-2 — Structural Paragraph Subtypes + Deletion Hardening

| | |
|---|---|
| **RFC** | STRUCT-2 |
| **Title** | Structural paragraph subtypes via `para:*` tags; type picker; deletion word-count display, branch kill-ring, pre-delete snapshots |
| **Status** | Shipped — 1.4.11 |
| **Author** | Vladimir Ulogov |
| **Depends on** | none — purely additive |
| **New dependency** | none |

Two features bundled by one motivation: making the tree a reliable, navigable
workspace for technical and nonfiction authors.

## Part A — Structural paragraph subtypes

A structural paragraph is an ordinary `.typ` paragraph carrying a **`para:*`
tag** (code / admonition{note,warning,tip,caution} / math / procedure / table).
The **tag, not a content_type**, marks it — file format and editor mode are
unchanged, and the tag is addable/removable via `Ctrl+B ]` without a morph.

| Tag | Glyph | Companions | Word count |
|---|---|---|---|
| `para:code` | `⌨` | skip | excluded |
| `para:admonition-*` | `⚠` | skip | excluded |
| `para:math` | `∫` | skip | excluded |
| `para:procedure` | `≡` | **run** (steps are prose) | excluded |
| `para:table` | `⊞` | skip | excluded |

### Implementation

- **Single source of truth** — `STRUCTURAL_TYPES: &[(tag, glyph, label, seed)]`
  in `tui/app.rs`, with eight `SEED_*` Typst boilerplate consts. `structural_glyph`,
  the `i` picker, and the creation seed all read from it. `is_structural_paragraph`
  (any `para:*` tag) and `is_structural_nonprose` (structural and **not**
  `para:procedure`) are the gate predicates.
- **Creation** — `i` in the Tree pane opens `Modal::StructuralTypePicker`; on
  Enter the chosen index is stashed in `App.pending_structural_type` and the
  standard `Adding` title prompt opens. `commit_add` consumes the flag, writes
  the seed (content_type stays typst), and stamps the `para:*` tag after the
  hierarchy reload. Mirrors the STRUCT-1 `e`/Jinja flow; the chord is a plain
  Tree letter (not `Ctrl+B Shift+T` — `t`/`j` were taken, `i` = "insert").
- **Display** — type glyph in `tree_row_lines`; structural paragraphs render
  `⌨ ⚠ ∫ ≡ ⊞`.
- **Gates** — the Inner Editor and Inner Socrates skip `is_structural_nonprose`
  paragraphs (procedure still runs). `compute_book_stats` counts structural
  paragraphs in a new `BookStats.structural` field, excluding them from
  `paragraphs` / `words` / `sentences`; Book Info shows a `structural: N` line.

## Part B — Deletion hardening

The `d`-in-Tree delete flow gained three recovery layers:

- **B-1 — word count in the confirmation.** `Modal::Deleting.word_count` sums the
  paragraph leaves of the subtree; the prompt shows `… (15,342 words)?`
  (omitted when zero).
- **B-2 — branch kill-ring.** `commit_delete` stashes every paragraph LEAF of a
  branch delete into the kill-ring (was single-paragraph only), in reverse tree
  order so the LIFO ring restores in original order via `Ctrl+B U`. Bodies read
  from disk before `delete_subtree`. The `stashed` count drives the
  failure-rollback and the status hint.
- **B-3 — pre-delete snapshots.** Before a branch delete,
  `create_snapshot_annotated` snapshots every paragraph leaf
  (`pre-delete: <title> · <date>`) so any can be recovered from the F6 picker
  after the kill-ring cycles. Taken before the delete — safe even on partial
  failure. Branch-only (single deletes would pollute the snapshot list).
- **B-4 — restore word count.** `DeletedParagraphStash` carries `word_count`;
  `undo_last_delete` reports it.

## Open questions (resolved)

- **Q1 — `para:code` seed format:** a readable `#figure` + triple-backtick raw
  block.
- **Q2 — procedure word count:** excluded from prose words, counted as
  structural; a `structural_include_words` knob deferred.
- **Q3 — kill-ring capacity on large branch deletes:** rely on the B-3
  pre-delete snapshots; kill-ring cap unchanged.
- **Q4 — snapshot volume:** one snapshot per paragraph; a grouped per-branch
  snapshot would need a store API change, deferred.

## What did not change

No new `NodeKind`, no `content_type`, no `ChildRef`, no DuckDB schema, no morph
cycle change, no new deps. A paragraph can be both `content_type: "jinja"` and
`para:code` (the jinja gate fires first). Single-paragraph delete behaviour is
unchanged.

See [STRUCTURAL_PARAGRAPHS.md](../STRUCTURAL_PARAGRAPHS.md) and
[Tutorial 94](../Tutorials/94-structural-paragraphs.md).
