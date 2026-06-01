# Tutorial 51 — Crash report writer and `inkhaven recover`

*Inkhaven 1.2.15+*

This tutorial walks through inkhaven's panic-survivability
layer.  The promise is concrete: if the editor panics or the
host dies (`kill -9`, power loss, segfault in a transitive
dep), every dirty buffer is flushed to a side-by-side rescue
file and the panic context is captured in a crash report you
can read later.  On the next run, `inkhaven recover` picks
the rescue back up.

You should never need this tutorial in practice.  When you
do, it should feel like a non-event.

## What gets written on a panic

Two artefacts:

* **`inkhaven-crash-<UTC>.hjson`** in the launch cwd — the
  crash report.  Pipe-friendly HJSON with the panic message,
  panic location, project root + open paragraph at the time
  of the crash, a manifest of rescued buffers, the last 50
  user actions you took, and an environment fingerprint
  (`$TERM`, `$LANG`, OS, inkhaven version).

* **`<paragraph>.typ.inkhaven-rescue`** next to each dirty
  paragraph file — the in-memory editor buffer flushed to
  disk atomically.  These are the actual prose; the report
  is the index.

Both files are written atomically (temp + fsync + rename +
parent-dir fsync).  Failures at any step are silent —
inkhaven would rather get partial state out than panic
during the panic handler.

## What is NOT written

The crash report is deliberately conservative.  It does
**not** include:

* LLM prompts or responses (privacy + size).
* Search queries (privacy).
* Snapshot bodies (size — these would dominate the file).
* The full project DB / index state (recover re-derives
  from the rescues + disk).
* Environment variables other than `$TERM` and `$LANG`.
* The full inkhaven.hjson (which may contain
  project-specific configuration the user doesn't want
  shared).

You can read the report yourself before sharing it in a bug
report.  See
[`Documentation/SECURITY_WARNING.md`](../SECURITY_WARNING.md)
§3.5 — the *recent-action ring* (`recent_actions` field) may
contain short structural details (slug names, anchor counts)
that you should glance at to make sure no secret leaked in
via a recent paste.

## Running `inkhaven recover`

After a crash, restart inkhaven on the same project, then
run from a shell:

```bash
$ inkhaven recover ./inkhaven-crash-20260601T032147.hjson

Crash report: ./inkhaven-crash-20260601T032147.hjson
  inkhaven version : 1.2.15
  panic at         : src/foo.rs:42:7
  generated at     : 2026-06-01T03:21:47Z
  message          : called Option::unwrap() on None

Found 3 rescued buffer(s).

  [1/3] manuscript/01-chapter-1/01-opening.typ
       rescue : .../01-opening.typ.inkhaven-rescue (4823 bytes)
       on-disk: .../01-opening.typ (4801 bytes, delta +22)
       apply? [y/N/diff]:
```

The recover walk shows each rescued buffer with its size +
the delta from the on-disk version.  Three responses:

* `y` — apply the rescue (atomically replace the on-disk
  file).  Before the replacement, the current on-disk
  contents are copied to `<original>.pre-recover-<UTC>`
  so you can roll back later by `mv`-ing that file back.
* `N` (or empty Enter) — skip this buffer.
* `d` / `diff` — show a side-by-side line diff between
  rescue and on-disk, then re-prompt.

After the walk, the crash report + rescue files are moved
into `<project>/.inkhaven/recovered/` so subsequent recover
invocations don't try to re-apply the same set.  Pass
`--keep` to leave them in place.

## Scripted recovery

For CI gates or scripted cleanup after a server-side panic:

```bash
$ inkhaven recover crash.hjson --yes        # apply every rescue
$ inkhaven recover crash.hjson --yes --keep # apply + leave files
```

`--yes` bypasses the per-buffer prompt and applies every
rescue whose rescue file is still present.

## Identical-content short-circuit

When the rescue and the on-disk file have identical bytes
(you saved before the crash; the rescue is redundant), the
walk skips the prompt and reports:

```
       on-disk: ... (4801 bytes, identical to rescue) — no action needed
       skipped.
```

No `.pre-recover-*` backup is created in this case — there's
nothing to back up.

## The path-traversal safety net

If a crash report contains a `paragraph_rel_path` that would
escape the project root (`../../etc/passwd` and friends),
the recover CLI rejects that entry and continues:

```
  [2/3] ../../etc/passwd: rejected (path traversal): absolute or escape segment
```

This is part of the 1.2.15 security hardening (S.6.H3) —
even a maliciously crafted crash report can't be used to
write arbitrary files via `inkhaven recover --yes`.

## When recovery isn't enough

If `kill -9` or a power loss landed between two debounce
ticks of the dirty-buffer mirror (the default is 2 seconds),
the most recent ~2 s of typing may not be in the rescue
file.  This is the worst-case window; everything older is
preserved.

For long-running sessions on important manuscripts, the
established backstops still apply:

* `Ctrl+S` saves the open paragraph immediately — atomic via
  `crate::io_atomic` (1.2.15+).
* `editor.autosave_seconds` flushes dirty buffers after the
  configured idle window.
* `inkhaven backup` (or `Ctrl+B Shift+B`) writes the entire
  project to a timestamped `.zip` and surfaces a warning in
  the `health` chip when it's overdue.

## See also

* [Tutorial 52 — Health monitor and doctor scan](52-health-and-doctor.md)
* [Tutorial 53 — Bund script trust gate](53-bund-trust-gate.md)
* `Documentation/SECURITY_WARNING.md` — risk disclosure
* `Documentation/RELEASE_NOTES/1.2.15.md` — phases R.1 + R.2
  (the proposal that became this layer)
