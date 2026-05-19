# Maintenance

Inkhaven's database is robust under normal use, but a manuscript is too
valuable to leave to chance. This document covers the four maintenance
operations you should know about:

1. **Backup** — create a `.zip` snapshot of an entire project.
2. **Restore** — unpack a backup into a fresh directory.
3. **Auto-backup on exit** — the TUI's safety net for users who forget.
4. **Reindex** — reconcile bdslib with the `.typ` files on disk after
   external edits or recovery.

Plus a few smaller tools: the log file, embedding model cache, and a
short troubleshooting checklist.

## Table of contents

- [Backup: `inkhaven backup`](#backup-inkhaven-backup)
- [Restore: `inkhaven restore`](#restore-inkhaven-restore)
- [Auto-backup on TUI exit](#auto-backup-on-tui-exit)
- [Reindex: reconcile with disk](#reindex-reconcile-with-disk)
- [The runtime log file](#the-runtime-log-file)
- [Embedding model cache](#embedding-model-cache)
- [Troubleshooting](#troubleshooting)
- [Recovery patterns](#recovery-patterns)

## Backup: `inkhaven backup`

```bash
inkhaven --project ~/Books/my-novel backup --out ~/Backups
```

What this does:

- Walks every file under the project root.
- Skips `.inkhaven.log` (the runtime log; not interesting in a backup)
  and any directory you have configured as the backup output target
  (avoids zipping zips of zips of …).
- Streams the rest into a deflate-compressed zip named
  `blackinkhaven_YYYYDDMM_HHMMSS.zip` inside `--out`.
- Updates `.inkhaven-backup.json` in the project root with the
  current timestamp so the on-exit auto-backup hook knows the last
  successful run.
- Prints `wrote backup: <path>` on success.

The backup is filesystem-level — it does **not** open the bdslib store.
That is by design: backups should be safe to run while the TUI is
closed, and the database files (`metadata.db`, `blobs.db`, `vectors/`)
ship as-is. A restore reproduces an exact working tree.

### Backup format

Each archive contains, relative to its root:

```
inkhaven.hjson           ← restore uses this as the "is this an Inkhaven backup?" marker
prompts.hjson
metadata.db
blobs.db
frequency.db
vectors/…
books/…
.session.json            ← cursor + open paragraph state (optional)
.inkhaven-backup.json    ← timestamp marker
```

The archive is portable — drag it to another machine, restore, and the
project comes back with identical UUIDs, identical paragraph paths,
identical embeddings.

### Tips

- Run `inkhaven backup` before a destructive operation (`rm books/foo`,
  switching `embeddings.model`, a major reorganisation).
- Pipe `backup --out` to a directory under version control (git-lfs is
  comfortable with the binary blobs) for offsite history.
- The filename format is `YYYYDDMM_HHMMSS` (not ISO order) — this
  matches the spec but does not sort chronologically across months
  on filename alone. The on-disk modification time still does.

## Restore: `inkhaven restore`

```bash
inkhaven restore ~/Backups/blackinkhaven_20260519_143010.zip --to ~/Books/my-novel-restored
```

What this does:

- Opens the archive. If it does not contain `inkhaven.hjson` at the
  root, the restore aborts: this protects you from accidentally
  unpacking a non-Inkhaven zip on top of your work.
- Refuses if `--to` already contains an `inkhaven.hjson` (i.e. another
  Inkhaven project). Pick a fresh directory or wipe the old one first.
- Creates `--to` if missing.
- Extracts every entry preserving paths.
- Reports `restored backup … into …` on success.

The restored project is independent of the source — UUIDs are
identical, but the two projects share no state going forward.

If you want to restore in place over the same directory, do it
manually:

```bash
inkhaven --project ~/Books/my-novel backup --out /tmp        # safety net
rm -rf ~/Books/my-novel
inkhaven restore /tmp/blackinkhaven_<date>.zip --to ~/Books/my-novel
```

## Auto-backup on TUI exit

The TUI checks `.inkhaven-backup.json` against `backup.max_age` from
your config on every clean exit:

```hjson
backup: {
  out_dir: "backups"
  max_age: "7d"
}
```

If the last successful backup is older than `max_age`, the exit
sequence:

1. Drops the App (which drops the Store handle, flushing DuckDB
   checkpoints and the HNSW WAL).
2. Renders a centred splash with a live progress bar:
   ```
   ┌── Inkhaven · backup ──────────────────┐
   │  Performing database backup…          │
   │  Project: /home/you/Books/my-novel    │
   │  [████████····]  321/512 ( 63%)       │
   └───────────────────────────────────────┘
   ```
3. Streams every project file into the dated zip in `out_dir`.
4. Updates `.inkhaven-backup.json` with the new timestamp.
5. Tears down the terminal and returns to your shell.

If anything goes wrong during the backup, the error is written to the
runtime log file (`.inkhaven.log`) and the user is still returned to
the shell — the backup hook is a safety net, not a blocker.

### Disabling auto-backup

`out_dir: ""` (empty) or `max_age: "0s"` either turns it off entirely.
The manual `inkhaven backup` command still works regardless.

### Choosing `max_age`

The interval depends on how often you write:

- **Daily writer** — `"24h"` or `"12h"`
- **Weekly writer** — `"7d"` (default)
- **Long-form, sporadic** — `"30d"` if you also commit to a separate
  archive habit

Inkhaven also gives you per-paragraph snapshots (F5) for in-session
versioning; the zip backup is for project-level disaster recovery.

## Reindex: reconcile with disk

```bash
inkhaven --project ~/Books/my-novel reindex [--prune] [--adopt]
```

`reindex` walks the `.typ` files under `books/` and synchronises bdslib
with what it finds.

Plain `reindex` (no flags) re-reads every `.typ` file's content and
re-embeds it. This is what you run after switching `embeddings.model`
or after restoring a project and wanting to verify the index is fresh.

| Flag | Effect |
| ---- | ------ |
| `--prune` | Remove bdslib records whose `.typ` file is missing from disk. Use after manually deleting files or directories outside the TUI. |
| `--adopt` | Find `.typ` files on disk that bdslib doesn't know about and register them as paragraphs under the deepest hierarchy branch whose path matches their parent directory. Use after dropping new files into `books/<some-chapter>/`. |

You can combine the flags: `--prune --adopt` does both passes.

### When you need it

- After editing a `.typ` file in a different editor.
- After a `git checkout` that brought back paragraphs the database had
  forgotten.
- After a manual `mv` / `rm` inside `books/` outside the TUI.
- After switching the embedding model in `inkhaven.hjson`.
- As a "are my files and database aligned?" sanity check.

`reindex` is idempotent and safe to run repeatedly.

## The runtime log file

When the TUI is running, the `tracing` subscriber writes to
`<project-root>/.inkhaven.log` (append mode, no ANSI colours). CLI
commands log to stderr instead — they don't disturb the TUI's
alternate-screen rendering.

What lands in the log:

- Provider warnings from genai (e.g.
  `WARN genai::adapter::adapters::openai::streamer: EMPTY CHOICE CONTENT`
  is a normal DeepSeek streaming quirk and can be ignored).
- bdslib operations whose timing or error message is worth recording.
- Auto-backup failures, focus-loss-save errors, and similar non-fatal
  background errors.

If something goes wrong, look at the tail of `.inkhaven.log` first:

```bash
tail -n 200 ~/Books/my-novel/.inkhaven.log
```

To turn the log up to debug:

```bash
INKHAVEN_LOG=inkhaven=debug,bdslib=info inkhaven --project ~/Books/my-novel
```

(The TUI honours the standard `RUST_LOG` style filter via tracing's
`EnvFilter`. The actual env var name is whatever you have set as
`RUST_LOG` — `INKHAVEN_LOG` is just a convention.)

## Embedding model cache

The first time you initialise a project, fastembed downloads the chosen
embedding model. It lands in a per-user cache:

| OS      | Cache location |
| ------- | -------------- |
| macOS   | `~/Library/Caches/dev.inkhaven.inkhaven/embeddings/` |
| Linux   | `$XDG_CACHE_HOME/inkhaven/embeddings/` (defaults to `~/.cache/inkhaven/`) |
| Windows | `%LOCALAPPDATA%\inkhaven\inkhaven\cache\embeddings\` |

Subsequent projects reuse the cache. Switching `embeddings.model`
downloads the new model the next time you open a project; the old one
stays on disk until you `rm -rf` the cache directory.

If the download hangs or fails (slow connection, server hiccup), the
splash screen reports the elapsed time. `Ctrl+Q` aborts startup so you
can retry.

## Troubleshooting

### "command not found: inkhaven"

The binary is in `./target/release/inkhaven` after `cargo build`. Either
invoke it with the full path, install it to a directory on your
`PATH`, or alias `inkhaven=~/path/to/inkhaven`.

### "first-run model download is hanging"

- Confirm internet connectivity.
- Look at the splash screen's elapsed counter — anything under 2 min
  on a normal connection is expected for `MultilingualE5Small`.
- `Ctrl+Q` to abort; restart later.
- For an air-gapped install, pre-populate the embedding cache from
  another machine.

### "config error: missing field `foo`"

You opened a config written by an older release. Either add the
missing field by hand (see [`CONFIGURATION.md`](CONFIGURATION.md)) or
delete the obsolete file and run `inkhaven init --force` to regenerate
from the current template.

### Tree pane and disk are out of sync

```bash
inkhaven --project <root> reindex --prune --adopt
```

### Editor pane shows old content

A previous session crashed without flushing. Run:

```bash
inkhaven --project <root> reindex
```

to re-read the `.typ` files into the database. The session-state file
(`.session.json`) will reopen the previously-focused paragraph next
launch.

### Search results look wrong after switching embedding models

```bash
inkhaven --project <root> reindex
```

re-embeds every paragraph with the new model.

### "no usable AI" or 401 errors

Check the relevant env var is set (`echo $GEMINI_API_KEY`). The status
bar reports which one is missing when an inference fails. See
[`CONFIGURATION.md`](CONFIGURATION.md#llm).

### Database file got corrupted

```bash
# 1. Take a fresh manual backup (just in case)
mv ~/Books/my-novel/metadata.db ~/Books/my-novel/metadata.db.corrupt
mv ~/Books/my-novel/blobs.db    ~/Books/my-novel/blobs.db.corrupt

# 2. Restore a known-good backup
inkhaven restore ~/Backups/blackinkhaven_<date>.zip --to ~/Books/my-novel-restored

# 3. If you have no backup but the books/ directory survived:
mkdir ~/Books/my-novel-rebuild
cp -r ~/Books/my-novel/books ~/Books/my-novel-rebuild/
inkhaven init --force ~/Books/my-novel-rebuild
inkhaven --project ~/Books/my-novel-rebuild reindex --adopt
```

The `--adopt` pass registers every `.typ` it finds under the matching
hierarchy branch derived from the directory layout — your prose is
recovered, hierarchy regenerated.

## Recovery patterns

### Restoring to a different machine

```bash
# On the source machine
inkhaven --project ~/Books/my-novel backup --out /tmp

# Copy /tmp/blackinkhaven_<date>.zip to the new machine

# On the new machine
inkhaven restore /tmp/blackinkhaven_<date>.zip --to ~/Books/my-novel
```

The embedding model cache is per-user — the first launch on the new
machine downloads the model again unless you pre-seeded the cache.

### Branching: derive a new project from an existing one

```bash
inkhaven --project ~/Books/my-novel backup --out /tmp
inkhaven restore /tmp/blackinkhaven_<date>.zip --to ~/Books/my-novel-fork
```

Both projects then evolve independently. UUIDs are identical, which
means if you ever merge content back you can correlate paragraphs by
ID.

### Rebuilding from `.typ` files alone

If you have lost the database but kept the `books/` tree, point
`reindex --adopt` at a fresh init:

```bash
mkdir ~/Books/rebuild
cp -r path/to/old/books ~/Books/rebuild/books
inkhaven init --force ~/Books/rebuild
inkhaven --project ~/Books/rebuild reindex --adopt
```

The reindexer scans every `.typ`, finds the deepest legal branch in the
existing hierarchy whose filesystem path matches the orphan file's
parent directory, and registers the paragraph there. New UUIDs are
assigned — this is a fresh project.
