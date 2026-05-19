# 10 — Backups and recovery

Your manuscript is more valuable than the laptop it sits on. This
tutorial covers backing up Inkhaven projects, restoring from a
backup, configuring the auto-backup-on-exit hook, and recovering
from common failure modes.

For the reference manual see [`../MAINTENANCE.md`](../MAINTENANCE.md).
This tutorial is a hands-on guide.

## What a backup contains

`inkhaven backup` zips the **entire project directory** (with two
exceptions). That includes:

- `inkhaven.hjson`, `prompts.hjson`
- `metadata.db`, `blobs.db`, `frequency.db`, `vectors/`
- `books/` — every `.typ` file
- `.session.json` (cursor / open paragraph state)
- `.inkhaven-backup.json` (last backup timestamp)

Excluded:

- `.inkhaven.log` (runtime log; not interesting in a backup)
- The backup output directory itself, if you place it inside the
  project (avoids zipping zips of zips).

A restore reproduces this exact tree — identical UUIDs, identical
embeddings, identical filenames.

## Manual backup

```bash
$ inkhaven --project ~/Books/my-novel backup --out ~/Backups
```

Output:

```
wrote backup: /home/you/Backups/blackinkhaven_20260519_143010.zip
```

The filename pattern is `blackinkhaven_YYYYDDMM_HHMMSS.zip` —
**note the order**, it's YYYY then DD then MM, not ISO. The HHMMSS
suffix gives second-level uniqueness within the same day. Files
sort correctly by modification time on disk regardless.

Run this:

- Before any risky operation (`rm -rf books/old-chapter`, mass
  rename, switching `embeddings.model`, large reorganisation).
- As part of a daily / weekly cron if you don't trust the
  auto-backup hook to fire.
- Manually whenever you've reached a milestone (chapter complete,
  draft to a beta reader).

The output directory is created if missing. `--out` may be relative
(resolved against the current shell directory, not the project root)
or absolute.

## Auto-backup on TUI exit

Configured in `inkhaven.hjson`:

```hjson
backup: {
  out_dir: "backups"
  max_age: "7d"
}
```

- **`out_dir`** — where backups land. Relative paths resolve
  against the project root; absolute paths are used as-is. Empty
  string disables auto-backup.
- **`max_age`** — humantime duration. If the last successful backup
  is older than this when you quit the TUI, a fresh backup is
  created during exit. `"0s"` disables.

Defaults: `"backups"` directory inside the project, weekly
(`"7d"`). For more frequent backups: `"24h"`, `"12h"`, `"30m"`.

When the hook fires, you see a splash before exit:

```
┌── Inkhaven · backup ──────────────────┐
│  Performing database backup…          │
│  Project: /home/you/Books/my-novel    │
│  [████████····]  321/512 ( 63%)       │
└───────────────────────────────────────┘
```

The store handle is dropped first (so DuckDB checkpoints and HNSW
flushes), then the zip runs with a live progress callback updating
the bar at ~30 Hz. After completion the terminal restores.

### Disabling auto-backup

```hjson
backup: {
  out_dir: ""           # or out_dir: "backups", max_age: "0s"
}
```

The manual `inkhaven backup` command still works regardless.

### Backup failure on exit

If the zip fails (disk full, permission error), Inkhaven logs to
`.inkhaven.log` and continues the exit teardown — you still get
back to your shell promptly. The backup is a safety net, not a
blocker.

Check the tail of the log:

```bash
$ tail -20 ~/Books/my-novel/.inkhaven.log
```

…and re-run `inkhaven backup` manually after fixing the underlying
cause.

## Restore

```bash
$ inkhaven restore /path/to/blackinkhaven_*.zip --to ~/Books/restored
```

What this does:

- Opens the archive.
- Verifies it contains an `inkhaven.hjson` at the root (sanity
  check: refuses random zips).
- Refuses if `--to` already contains an `inkhaven.hjson` (so you
  can't accidentally clobber live work — pick a fresh directory or
  delete the old one first).
- Creates `--to` if missing.
- Extracts every entry preserving paths.

The restored project is a complete, runnable Inkhaven project.
Re-open it with `inkhaven --project ~/Books/restored` and the TUI
sees the same hierarchy, the same paragraphs, the same prompts.

### Restoring in place

Inkhaven does not overwrite an existing project on restore (it's a
safety feature). To restore over an existing directory:

```bash
$ inkhaven --project ~/Books/my-novel backup --out /tmp/safety
$ rm -rf ~/Books/my-novel
$ inkhaven restore /path/to/desired-backup.zip --to ~/Books/my-novel
```

The `safety` backup is your insurance in case the desired-backup is
wrong.

## Recovery patterns

### Database corruption

Symptoms: `inkhaven list` fails with a DuckDB error, the TUI crashes
on launch, or specific paragraphs are missing from search.

```bash
# 1. Save what you have, just in case
$ mv ~/Books/my-novel/metadata.db ~/Books/my-novel/metadata.db.corrupt
$ mv ~/Books/my-novel/blobs.db    ~/Books/my-novel/blobs.db.corrupt
$ mv ~/Books/my-novel/frequency.db ~/Books/my-novel/frequency.db.corrupt

# 2a. If you have a recent backup, restore it (in a new directory)
$ inkhaven restore ~/Backups/blackinkhaven_<recent>.zip --to ~/Books/my-novel-restored

# 2b. If you have no backup but the books/ directory survived,
#     rebuild from the .typ files:
$ mkdir ~/Books/my-novel-rebuilt
$ cp -r ~/Books/my-novel/books ~/Books/my-novel-rebuilt/
$ cp ~/Books/my-novel/inkhaven.hjson ~/Books/my-novel-rebuilt/
$ cp ~/Books/my-novel/prompts.hjson  ~/Books/my-novel-rebuilt/
$ inkhaven init --force ~/Books/my-novel-rebuilt
$ inkhaven --project ~/Books/my-novel-rebuilt reindex --adopt
```

The `--adopt` flag walks the `.typ` files and registers each as a
paragraph under the matching hierarchy branch derived from its
parent-directory slug. UUIDs are fresh in this case (the database
is gone), but the prose is recovered intact.

### Drift between disk and database

Symptoms: you edited a `.typ` file in another editor (vim, VS Code,
…) and the Inkhaven TUI shows the old content, or search misses
the new sentences.

```bash
$ inkhaven --project ~/Books/my-novel reindex
```

Re-reads every `.typ` and updates the database and embeddings.

If you also added or removed files outside Inkhaven, use the flags:

```bash
$ inkhaven --project ~/Books/my-novel reindex --adopt --prune
```

`--adopt` registers orphan files; `--prune` removes records whose
files are missing.

### Tree shows a paragraph that's not on disk anymore

A `rm` happened outside Inkhaven, but the database still thinks the
paragraph exists.

```bash
$ inkhaven --project ~/Books/my-novel reindex --prune
```

### Want to start over without losing the prose

```bash
$ cp -r books inkhaven.hjson prompts.hjson /tmp/preserve
$ rm -rf ~/Books/my-novel
$ inkhaven init ~/Books/my-novel
$ cp -r /tmp/preserve/books ~/Books/my-novel/
$ cp /tmp/preserve/inkhaven.hjson /tmp/preserve/prompts.hjson ~/Books/my-novel/
$ inkhaven --project ~/Books/my-novel reindex --adopt
```

You now have a fresh database with the original prose registered.
Useful when the database has degraded or when you want to switch
embedding models cleanly.

### Migrating to a new machine

```bash
# Source machine:
$ inkhaven --project ~/Books/my-novel backup --out /tmp

# Copy /tmp/blackinkhaven_<date>.zip to the new machine

# New machine:
$ inkhaven restore /tmp/blackinkhaven_<date>.zip --to ~/Books/my-novel
```

First-launch on the new machine downloads the embedding model
(~120 MB) into the new user's cache directory — one-time cost.

### Branching the project

```bash
$ inkhaven --project ~/Books/my-novel backup --out /tmp
$ inkhaven restore /tmp/blackinkhaven_<date>.zip --to ~/Books/my-novel-experimental
```

Now you have two independent projects with identical history. The
UUIDs are shared, which is useful if you ever decide to merge content
back (you can correlate paragraphs).

## Pairing with version control

Inkhaven projects play well with git:

```bash
$ cd ~/Books/my-novel
$ git init
$ cat > .gitignore <<EOF
.inkhaven.log
.session.json
.inkhaven-backup.json
backups/
vectors/
*.db
EOF
$ git add -A
$ git commit -m "Initial commit"
```

What to track:

- **`inkhaven.hjson`**, **`prompts.hjson`** — yes; config is content.
- **`books/`** — yes; the prose is canonical.
- **`metadata.db`**, **`blobs.db`**, **`frequency.db`**, **`vectors/`**
  — **no**. These are derived; `inkhaven reindex --adopt` rebuilds
  them from `.typ` files on disk.
- **`backups/`** — no; they're build artefacts.
- **`.session.json`**, **`.inkhaven.log`**,
  **`.inkhaven-backup.json`** — no; runtime / per-user state.

A typical `.gitignore`:

```
.inkhaven.log
.session.json
.inkhaven-backup.json
backups/
vectors/
*.db
```

When cloning the project on a new machine:

```bash
$ git clone <repo> ~/Books/my-novel
$ cd ~/Books/my-novel
$ inkhaven reindex --adopt
```

The database rebuilds itself from the prose. First launch re-embeds
everything (this takes a minute or two for a medium manuscript on
modern hardware).

Pair git with `inkhaven backup` for two-tier protection: git
captures the prose history, backups capture full project snapshots
(including the database, so you can restore without re-embedding).

## What you have learned

- `inkhaven backup --out <dir>` zips the project; restore with
  `inkhaven restore <archive> --to <dir>`.
- Configure auto-backup via `backup.out_dir` and `backup.max_age` in
  `inkhaven.hjson`. The TUI fires it on exit when the last backup
  is older than `max_age`.
- `inkhaven reindex` reconciles the database with `.typ` files;
  `--prune` and `--adopt` handle drift.
- Recovery from corruption: restore from backup, OR rebuild from
  `.typ` files via `init --force` + `reindex --adopt`.
- Inkhaven projects play well with git; ignore the database files
  and rebuild via reindex.

## Next steps

- [`11-theming.md`](11-theming.md) — making the TUI yours.
- [`../MAINTENANCE.md`](../MAINTENANCE.md) — full reference.
