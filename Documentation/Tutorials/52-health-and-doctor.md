# Tutorial 52 — Background health monitor and project doctor

*Inkhaven 1.2.15+*

Two related 1.2.15 features:

* **Background health monitor** (Theme H) — a tokio task
  that periodically checks the project's integrity invariants
  and surfaces findings via a status-bar chip.
* **Project doctor** (Theme D) — an on-demand scan + repair
  for problem classes that drift across the project tree.

The two share a worldview: an inkhaven project is "healthy"
when its DB rows, on-disk files, sidecars, and rescue
companions all agree.  The monitor watches the cheap signals
in the background; the doctor walks the whole project on
demand.

## The health chip in the status bar

When enabled (see HJSON below), a single-glyph chip appears
in the status bar between the focus chip and the POV chip.
Four states:

| Glyph | Colour | Meaning |
|-------|--------|---------|
| `✓` | green | Last tick clean |
| `✎` | amber | Last tick auto-repaired something |
| `⚠` | yellow | Warning open (user attention recommended) |
| `✗` | red | Error open (user must intervene) |

The chip stays hidden when no monitor is running.

## Enabling the monitor

The monitor is off by default in 1.2.15 to avoid surprising
existing projects with a background task.  To enable, add to
`<project>/inkhaven.hjson`:

```hjson
{
  health: {
    enabled: true
  }
}
```

Restart inkhaven (the spawn is a startup task).  The chip
appears after the first check completes (~30 seconds in).

## What the monitor checks (1.2.15)

Three checks, each with its own cadence so they don't all
fire on the same tick:

* **Project root reachable** (90 seconds) — confirms
  `metadata()` on the project directory still succeeds.
  Critical on Err (project moved/unmounted), Warning on
  type mismatch (the path resolves to something that isn't
  a directory).
* **Backup freshness** (5 minutes) — walks the configured
  backup directory for `.zip` files, finds the newest
  mtime, compares to `backup.max_age`.  Warning when the
  newest backup is older than the configured threshold;
  recommends `Ctrl+B Shift+B` or `inkhaven backup`.
* **Rescue file orphans** (1 hour) — walks the project
  tree for `*.inkhaven-rescue` files older than 7 days.
  These are R.1 panic-hook leftovers the user dismissed
  without running `inkhaven recover`.  Warning with a
  count + an example path.

Additional integrity checks (DuckDB PRAGMA, HNSW vector
parity vs. DB row count, textarea-vs-disk sync, tree
parent-pointers, disk free %) need cross-thread access to
the project Store and are deferred to 1.2.16 (Phase P.4).

Note: inkhaven's search subsystem is HNSW + fastembed
embeddings only — there is no inverted full-text index
(Tantivy or otherwise).  See `Documentation/MAINTENANCE.md`
§ "Search model" for the full picture.

## Auto-repair (opt-in)

For the rescue-orphan check, the monitor can auto-delete
files older than 30 days (stricter than the 7-day warning
threshold, so there's a multi-week window between "warning"
and "auto-cleanup"):

```hjson
{
  health: {
    enabled: true
    auto_repair: {
      rescue_orphans: true
    }
  }
}
```

Every auto-repair is logged to `<project>/.inkhaven/health.log`
with a `Repaired|<class>|<note>` line so you have an audit
trail even when the TUI is closed.

## `inkhaven doctor --scan`

The doctor is the synchronous on-demand counterpart.  Run
it from a shell against a project:

```bash
$ inkhaven --project ~/Books/my-novel doctor --scan
Project scan
  generated_at  : 2026-06-01T03:14:05Z
  project_root  : /Users/.../my-novel
  findings      : 2

  [1]  warning · orphan-paragraph-row     · /…/missing.typ
        paragraph row `missing` points at missing file ...
        and bdslib has no content either
  [2]     info · bdslib-only              · /…/from-store.typ
        paragraph `from-store` has no disk file but bdslib
        holds 1247 bytes — recoverable
```

The scan distinguishes:

* **`zero-byte-file`** — disk file is 0 bytes AND bdslib has
  no content.  Critical: prose lost.
* **`orphan-paragraph-row`** — DB row but no disk file AND
  no bdslib content.  Warning: real data inconsistency.
* **`missing-referenced-file`** — same shape, tagged when
  the rel-path looks malformed.
* **`corrupt-comments-sidecar`** — `.comments.json` that
  doesn't parse as JSON.  Warning.
* **`bdslib-only`** — disk file missing but bdslib has
  content.  Info: recoverable.  This is the common case
  for system-book paragraphs that were never materialised
  to disk (Prompts, Help, Typst seed paragraphs), or for
  files the user manually deleted from the shell.

## `--json` for CI gates

```bash
$ inkhaven doctor --json | jq -e '.findings == []'
```

Exit code 2 when any finding at Warning or above shipped —
matches conventional doctor / linter behaviour.

```bash
$ inkhaven doctor --json | jq '.findings | length'
```

The JSON shape uses kebab-case class names so the strings
match what `--class <name>` accepts.

## `--autofix` for scripted cleanup

```bash
$ inkhaven doctor --autofix       # prompts y/N per finding
$ inkhaven doctor --autofix --yes # apply everything
$ inkhaven doctor --autofix --class bdslib-only --yes
                                  # rematerialize bdslib-only,
                                  # don't touch the other classes
```

Each repair is logged to `<project>/.inkhaven/doctor.log`
with a `OK|<class>|<note>` or `ERR|<class>|<message>` line.

Per-class actions:

* **`zero-byte-file`** / **`orphan-paragraph-row`** /
  **`missing-referenced-file`** — delete the DB row + the
  file.  Irreversible.
* **`corrupt-comments-sidecar`** — rename to
  `<sidecar>.corrupt-<UTC>.bak`.  Data preserved.
* **`bdslib-only`** — rematerialize the disk file from
  bdslib content via `crate::io_atomic::write`.
  Non-destructive: refuses to overwrite a non-empty disk
  file if one materialises between scan and fix.

## `Ctrl+B Shift+0` — doctor panel in the TUI

The same scan + repair flow lives in a TUI modal.  Press
`Ctrl+B Shift+0` (paired mnemonically with `Ctrl+B 0`
config-editor) to open the panel.  Inside:

* `↑↓` — navigate findings
* `r` — repair the highlighted finding
* `R` — repair every finding
* `Esc` — close

Status messages after each repair land on the status bar
so you can see what happened without scrolling the modal.

## See also

* [Tutorial 51 — Crash report writer and recover](51-crash-report-and-recover.md)
* [Tutorial 53 — Bund script trust gate](53-bund-trust-gate.md)
* `Documentation/CONFIGURATION.md` — `health.*` HJSON
  reference
* `Documentation/RELEASE_NOTES/1.2.15.md` — Themes H + D
