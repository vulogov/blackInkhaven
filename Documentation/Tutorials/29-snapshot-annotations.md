# 29 — Snapshot annotations

Inkhaven shipped `F5` (create snapshot) and `F6` (snapshot
picker) back in 1.2.0. 1.2.6 added an **annotation prompt** to
the create flow — a one-line note describing what this version
captures. The picker then displays the annotation alongside
the preview, so a long history reads as a labelled list of
revision states rather than a wall of timestamps.

## Why annotations

A typical paragraph accumulates dozens of snapshots over a
revision cycle. Without labels:

```
2026-05-22 14:32:05 +0200   145w   "Aerin pulled her cloak…"
2026-05-22 14:18:11 +0200   141w   "Aerin pulled her cloak…"
2026-05-22 13:55:47 +0200   122w   "She tightened the cloak…"
2026-05-22 13:31:02 +0200   118w   "She tightened the cloak…"
…
```

You can't tell which one was "before the lighthouse rewrite"
or "the version the beta reader praised". With annotations:

```
2026-05-22 14:32:05 +0200   145w   "Aerin pulled her cloak…"
       ✎ after the lighthouse rewrite
2026-05-22 14:18:11 +0200   141w   "Aerin pulled her cloak…"
       ✎ first complete draft
…
```

## The F5 flow

`F5` now opens a small annotation prompt over the editor:

```
┌── Snapshot annotation — F5 ──────────────────────────────┐
│                                                            │
│  Snapshot `The Storm` — annotation:                        │
│  › before the lighthouse rewrite│                         │
│                                                            │
│   Enter commits (empty = no note) · Esc cancels            │
└────────────────────────────────────────────────────────────┘
```

| Keys                | Effect |
|---------------------|--------|
| Type any character  | Accumulates into the annotation. |
| `Enter`             | Commits the snapshot with the typed note. |
| `Enter` (empty)     | Commits without a note — same as the pre-1.2.6 path. |
| `Esc`               | Cancels — no snapshot written. |

The buffer to snapshot is captured **at prompt-open time**, so
you can keep typing in the editor without affecting what gets
saved. The annotation lives in the snapshot's metadata
(`Snapshot.annotation: String`), not the body content.

A noisy F5 reflex of "F5, Enter" still works as fast as the
pre-1.2.6 path — the annotation just stays empty for that
snapshot.

## The F6 picker

`F6` lists every snapshot of the open paragraph. Annotated
snapshots render with the note on a second italic-cyan
indented line beneath the preview:

```
┌── Snapshots — The Storm ─────────────────────────────────┐
│                                                            │
│  2026-05-22 14:32:05 +0200   145w   The storm rolled…    │
│         ✎ after the lighthouse rewrite                    │
│  2026-05-22 14:18:11 +0200   141w   The storm rolled…    │
│         ✎ first complete draft                            │
│  2026-05-22 13:55:47 +0200   122w   She tightened her…   │
│  2026-05-22 13:31:02 +0200   118w   She tightened her…   │
│                                                            │
│   ↑↓ navigate · Enter loads · V diff vs current ·         │
│   D / Del delete · Esc cancel                             │
└────────────────────────────────────────────────────────────┘
```

Un-annotated rows stay single-line. The `✎` glyph + italic
indent makes annotated rows visually distinct so you can scan
for labels without reading every preview.

| Chord                | Effect |
|----------------------|--------|
| `↑` / `↓`            | Move cursor. |
| `Home` / `End`       | Jump to newest / oldest. |
| `Enter`              | Pre-restore safety snapshot of the current buffer + restore the selected snapshot. |
| `V`                  | Side-by-side diff: snapshot vs current buffer (see [`20-snapshot-diff.md`](20-snapshot-diff.md)). |
| `D` / `Del`          | Confirm + delete the selected snapshot. |
| `Esc`                | Close. |

## When to annotate

Annotations don't have to be on every snapshot. They earn
their keep on the **decision-point snapshots** — moments you
might want to go back to:

- Before a structural rewrite — "before the lighthouse rewrite".
- After a complete draft — "first complete draft, before
  editing pass 1".
- A version that landed well — "version Maria loved".
- A milestone — "submission draft v1".
- A risky experiment — "trying the second-person POV".

The annotation is searchable in your `.inkhaven.log` (it's
included in the snapshot's metadata) but the F6 picker doesn't
filter on it yet — that's a 1.2.7+ task.

## Recovering an annotated state

Same as the un-annotated flow:

1. `F6` to open the picker.
2. Navigate to the labelled snapshot.
3. `V` to diff against current — eyeball that this is really
   the state you want.
4. `Esc` to back out, `Enter` to commit.
5. The current buffer auto-snapshots (un-annotated) so the
   restore is reversible.
6. Selected snapshot loads as the new editor body. F5 a fresh
   annotated marker if you want a labelled return point.

Step 5 is critical — Inkhaven never overwrites your live work
silently. Every F6 restore creates a pre-restore safety
snapshot first.

## Bund — fire-time tag stamping

The existing `hook.on_snapshot` (1.2.1+) fires with `( parent_uuid
snapshot_uuid -- )` and isn't aware of the annotation. To add
auto-annotated snapshots (e.g. tag every F5 fire with the
current `status:` workflow level), a Bund script can post-stamp:

```bund
"hook.on_snapshot" {
  // ( parent_uuid snapshot_uuid -- )
  swap drop                                 // ( snapshot_uuid )
  // Read current paragraph status from parent ...
  // ... compose annotation, then attach via
  //     ink.snapshot.set_annotation (future word; 1.2.7+).
  drop
} register
```

`ink.snapshot.set_annotation` doesn't exist yet — Phase 4 of
the snapshot work in 1.2.7+ adds it. For now annotations live
purely in the F5 prompt path.

## Recap

- **F5** — now pops a one-line annotation prompt. Enter on
  empty commits without a note (same as pre-1.2.6).
- **F6** — picker shows annotations as a `✎`-indented second
  line under the row.
- Annotations don't change the snapshot's body content; they're
  pure metadata.
- Pre-restore safety snapshot still fires automatically — restores
  remain reversible.
- Diff vs current (`V`) still works the same way; see
  [`20-snapshot-diff.md`](20-snapshot-diff.md).
