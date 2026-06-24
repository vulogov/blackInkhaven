# Snapshot browser + the hardening sweep

Road to 1.4.0, cycle **1.3.36**. Bundles the snapshot browser (Track A) with the
first slice of the hardening sweep (Track B). Per-feature premise: the snapshot
*engine* and the *stability spine* already exist — this surfaces one and closes the
real gaps in the other.

## Premise

**Snapshots** are fully built per-paragraph: `Snapshot{id,parent_id,created_at,
word_count,preview,annotation}`, `create_snapshot[_annotated]`,
`list_snapshots(parent_id)`, the **F6** picker, `compute_line_diff` +
`SnapshotDiff` modal, pre-restore safety snapshots, dedup. What's missing is a
**project-wide** view — F6 only lists the open paragraph's history.

**Stability** is already strong: crash rescue (`.inkhaven-rescue` mirrors + crash
report), atomic session persistence, criterion CI gates (20% regression threshold
on startup/search), and never-panic proptests on 6 parsers (config/calendar/
lang-detect/levenshtein/filter/palette). The real gaps: **no project lock**
(two instances can race `metadata.db` / `.session.json`) and **three unguarded
parsers** (Scrivener RTF, EPUB reader, Typst check).

## Increments

### P1 — Snapshot browser (Ctrl+F6)
`Store::list_all_snapshots() -> Vec<(String /*parent_title*/, Snapshot)>` (the
`list_snapshots` body minus the parent_id filter; `parent_title` is already in the
metadata). New `Modal::SnapshotBrowser`: every snapshot project-wide, newest-first,
grouped by paragraph, with the F6 picker's annotation filter. `V` diffs a snapshot
against its paragraph's current text (reuse `compute_line_diff` + the `SnapshotDiff`
modal, returning to the browser); `Enter` jumps to that paragraph and opens its F6
picker. Bound **Ctrl+F6** (Editor/Any), palette + quickref + KEYBINDING.

### P2 — Parser fuzz sweep
Never-panic proptests for the three uncovered parsers — Scrivener **RTF**, **EPUB**
reader, **Typst** syntax check — plus round-trip/idempotence where cheap. Pure
dev-only test additions, zero runtime risk. Mirrors the existing never-panic sweep
style in `calendar.rs` / `config.rs`.

### P3 — Project lock (advisory, permissive)
A single-instance gate so two processes don't corrupt the shared store. **Honors the
permissive principle**: it *informs*, it does not hard-block. A stale lock (dead PID
/ old timestamp) is reclaimed automatically; a live one warns and offers `--force` /
a confirmation rather than refusing outright. Data-safety, not lockout.

## Cut criteria
Each increment signed with tests; KEYBINDING + quickref updated for P1; the lock's
override path documented. Folds into the 1.3.36 cut.
