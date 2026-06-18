# 11 — Backups and recovery

Inkhaven's backup story is dead-simple zip archives of your project directory. There's nothing proprietary; you could untar one in any directory and have a working project. Backups travel.

## `inkhaven backup`

```
inkhaven backup
# → ./inkhaven-backups/<project-basename>/<project>-YYYYMMDD-HHMM.zip

inkhaven backup --out /backups/inkhaven/
# → /backups/inkhaven/<project>-YYYYMMDD-HHMM.zip
```

The zip contains everything: `books/`, `metadata.db`, `vectors/`, `inkhaven.hjson`, and any artefacts you've generated. Snapshot history is in `metadata.db` so it travels too.

## Auto-backup on exit

When you `Ctrl+Q` and the last backup is older than `backup.max_age`, inkhaven runs a backup automatically before the TUI tears down. Configured in `inkhaven.hjson`:

```hjson
backup: {
  out_dir: ""                  # empty → sibling
                                # `inkhaven-backups/<project>/`
  max_age: "24h"               # how long since the last
                                # backup before exit triggers one
}
```

`max_age: "0s"` disables auto-backup. The manual command still works.

![figure: exit-backup-splash](images/exit-backup-splash.png) — Ctrl+Q with stale backup: splash + progress bar while the zip is written. Esc cancels.

## `inkhaven restore`

```
inkhaven restore --from inkhaven-backups/my-book/my-book-20260520-1432.zip
inkhaven restore --from … --target /tmp/my-book-test
```

Without `--target` the restore is to the current project directory (refusing to overwrite without `--force`). With `--target` you can spin up an isolated copy for testing.

> **Restoring on top of work:** If you've made changes since the backup, `--force` overwrites them. Run `inkhaven backup` first if you want both versions on disk.

## Recovery from drift

Three things can drift inside a project:

1. **On-disk vs DB** — you edited a `.typ` file outside inkhaven, the DB doesn't know. Fix: `inkhaven reindex` walks every `.typ`, re-reads it, updates the DB record.
2. **DB vs vectors** — DB knows about a paragraph but the vector store doesn't have its embedding (or has a stale one). Fix: `inkhaven reindex` also re-embeds.
3. **DB vs metadata.db sanity** — the DB itself has a broken record (extremely rare; usually power loss mid-write). Fix: `inkhaven reindex --prune` drops records pointing at missing files; `--adopt` adopts files the DB doesn't know about.

| Command | What it does |
|---------|--------------|
| `inkhaven reindex` | Re-read every paragraph from disk, re-embed. |
| `inkhaven reindex --prune` | Also drop DB records whose file is missing. |
| `inkhaven reindex --adopt` | Add DB records for `.typ` files in the books tree that aren't tracked. |

## `inkhaven doctor`

A pre-flight health check:

```
inkhaven doctor
```

Reports the binary version, typst engine (external vs in-process), font availability, package cache state, project layout sanity, and any actionable warnings. Run this when something feels off.

![figure: doctor-output](images/doctor-output.png) — `inkhaven doctor`: health report. Green check + actionable warnings (yellow) + errors (red).

## Recap

- `inkhaven backup` writes a zip; `restore` reverses it.
- Auto-backup-on-exit triggered by `backup.max_age` (default 24h).
- `inkhaven reindex` heals on-disk ↔ DB ↔ vector drift.
- `--prune` drops dead DB records; `--adopt` claims orphaned files.
- `inkhaven doctor` is the pre-flight check before a real bug hunt.
