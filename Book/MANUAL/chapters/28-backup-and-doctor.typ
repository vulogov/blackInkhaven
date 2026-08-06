#import "../design.typ": *

#chapter(number: 28, title: "Keeping It Healthy")

A manuscript is the most valuable thing on your disk and the least replaceable.
Everything else in this book is about writing it; this chapter is about not
losing it. Inkhaven's store is robust under ordinary use, but "robust" is not a
plan, and the tool takes the view that a book you have spent a year on deserves
several independent safety nets rather than one. This chapter walks the whole
maintenance surface: the backup pipeline and how to restore from it, crash
recovery, the `reindex` command that reconciles the database with the files on
disk, the project *doctor* that scans for structural trouble, the advisory lock
that keeps two sessions from corrupting each other, and the log you read when
something drifts. None of it is glamorous. All of it is the difference between a
bad afternoon and a lost book.

#callout(label: "The shape of the safety net")[
  Four layers stand between you and a lost word, from most to least frequent:
  *focus-loss autosave* (every time attention leaves the Editor), *snapshots*
  (F5 / F6, per paragraph — chapter 6), the *crash mirror* (a side copy of every
  dirty buffer, chapter 6), and the *zip backup* (a whole-project archive, this
  chapter). Each catches what the one above it misses. This chapter is the
  outermost layer and the recovery tools that read the ones beneath it.
]

#section("Backups — a whole project in one archive")

The backup pipeline answers one question: if this directory vanished, could you
get the entire project back exactly as it was? The `backup` command produces a
single dated `.zip` that says yes.

#screen(caption: "Taking a manual backup")[```
$ inkhaven --project ~/Books/my-novel backup --out ~/Backups
wrote backup: ~/Backups/blackinkhaven_20260805_143010.zip
```]

What the command does, in order: it walks every file under the project root; it
skips the runtime log (`.inkhaven.log`, uninteresting in an archive) and any
directory you have configured as the backup output target (so an archive never
tries to zip earlier archives of itself); it streams the rest into a
deflate-compressed zip named `blackinkhaven_YYYYDDMM_HHMMSS.zip` inside `--out`;
it updates `.inkhaven-backup.json` in the project root with the moment it
finished, so the on-exit hook knows when the last good backup ran; and it prints
`wrote backup:` with the path. That is the whole operation.

#term("Filesystem-level backup")[
  The backup is taken at the level of *files*, not the database. It never opens
  the store — DuckDB and the vector index are copied as bytes, exactly as they
  sit on disk. That is deliberate: a backup must be safe to run while the TUI is
  closed, and a byte-for-byte copy reproduces an *exact* working tree — identical
  UUIDs, identical paragraph paths, identical embeddings — when it is restored.
]

#subsection("What an archive contains")

Relative to its root, every archive holds the marker config, the prompt library,
the three database files, the vector index, your books, and the small session
and backup-timestamp sidecars.

#screen(caption: "The layout inside a backup zip")[```
inkhaven.hjson         ← the "is this an Inkhaven backup?" marker
prompts.hjson
metadata.db            ← hierarchy: nodes, slugs, titles, tags
blobs.db               ← paragraph bodies + image bytes
frequency.db
vectors/…              ← the HNSW embedding index
books/…                ← your prose, as .typ files
.session.json          ← last-open paragraph + cursor (optional)
.inkhaven-backup.json  ← the last-good-backup timestamp
```]

Because the archive is a complete, portable tree, you can drag it to another
machine, restore it, and the project comes back whole. The one thing that does
not travel inside it is the embedding model itself — that lives in a per-user
cache, and the first launch on a new machine re-downloads it (chapter 1).

#subsection("Where backups live by default")

You can always name a destination with `--out`. When you omit it, the location
depends on `backup.out_dir` in your config: an *empty* value (the default) writes
to a sibling of the project, `<parent>/inkhaven-backups/<project-name>/`, keeping
archives out of the project tree so a backup never contains itself; a *relative*
value is resolved against the project root; an *absolute* value is used verbatim.
The manual `backup` command and the on-exit hook write to the same resolved
place.

#callout(label: "A note on the filename")[
  The stamp is `YYYYDDMM_HHMMSS` — year, *day*, month — which does not sort
  chronologically by name across a month boundary. The on-disk modification time
  always does, so sort by mtime (`ls -t`) when you want the newest, and do not
  trust the filename's lexical order alone.
]

#subsection("Keeping the backup directory from growing forever")

By default archives accumulate — every backup is kept, which is the safe choice
but not always the tidy one. Set `backup.keep_last` to a positive number and each
new backup prunes the oldest beyond that count, so a `keep_last: 10` holds a
rolling window of the ten most recent. `0` (the default) keeps everything. The
prune runs after a successful write, never before, so a failed backup can never
delete a good one.

#two_track(
  [Run a manual `backup` before anything irreversible — deleting a book,
  switching the embedding model, a big reorganisation — so the pre-change state
  is one `restore` away.],
  [Point `--out` at a directory under version control (git-lfs handles the binary
  blobs) and you get an offsite, dated history of the whole corpus for free.],
)

#section("Auto-backup on exit — the safety net for forgetting")

Manual backups depend on remembering to take them, so Inkhaven takes one *for*
you when it notices the last one is stale. On every clean exit the TUI compares
`.inkhaven-backup.json` against `backup.max_age`. If the last successful backup
is older than that window, the exit sequence drops the App (which flushes the
DuckDB checkpoints and the vector-index WAL), renders a small splash with a live
progress bar, streams every project file into a fresh dated zip, updates the
timestamp marker, and only then tears down the terminal and returns you to your
shell.

#screen(caption: "The auto-backup splash on exit")[```
┌── Inkhaven · backup ──────────────────┐
│  Performing database backup…          │
│  Project: /home/you/Books/my-novel    │
│  [████████····]  321/512 ( 63%)       │
└───────────────────────────────────────┘
```]

The hook is a *net*, not a gate. If anything goes wrong mid-backup, the error is
written to `.inkhaven.log` and you are still returned to the shell — a failed
safety backup never traps you inside the editor. By default the splash waits for
a keypress after it finishes (`backup.wait_for_key_after_backup`, default `true`)
so you can read the result before it dismisses; set it `false` for the old
auto-dismiss behaviour.

#subsection("Turning it off, and choosing the window")

There are three ways to disable the exit hook. The explicit one is
`backup.auto_backup_on_exit: false`, which turns off just the hook and leaves the
manual command working. The two older switches still work as side effects: an
empty `out_dir` or a `max_age` of `"0s"` also disable it. The window itself is a
`humantime` string, so pick it by how fast you write:

#chord_table((
  chord_row("\"24h\" / \"12h\"", "A daily writer — never more than a day at risk."),
  chord_row("\"7d\"", "The default — a comfortable weekly cadence."),
  chord_row("\"30d\"", "Sporadic long-form, if you also keep a separate archive habit."),
  chord_row("\"0s\" / \"\"", "Disable the exit hook (manual backup still works)."),
))

Remember that the exit backup is for project-level disaster recovery; the
per-paragraph snapshots (F5) are your in-session versioning. They are different
tools for different scales of loss, and you want both.

#section("Restoring from a backup")

Restoring unpacks an archive into a *fresh* directory. The command takes the zip
and a `--to` destination.

#screen(caption: "Restoring an archive into a new directory")[```
$ inkhaven restore ~/Backups/blackinkhaven_20260805_143010.zip \
      --to ~/Books/my-novel-restored
restored backup `…143010.zip` into ~/Books/my-novel-restored
```]

Two guards protect you from mistakes. First, the archive must contain
`inkhaven.hjson` at its root — the marker that says "this really is an Inkhaven
backup" — or the restore aborts, so you cannot accidentally explode an unrelated
zip over your work. Second, it refuses if `--to` already holds a project (its own
`inkhaven.hjson`); pick a clean directory, or clear the old one first. The
destination is created if it does not exist. The restored project is fully
independent of the source: the UUIDs match, but the two share no state from that
point on, which is exactly what makes restore double as a *branch* tool — restore
the same archive twice into two directories and evolve them separately.

#subsection("Restoring in place, and rebuilding from prose alone")

To overwrite the same directory there is no in-place flag by design; you do it by
hand, taking a safety backup first, removing the old tree, and restoring onto it.
And if the database is gone but the `books/` tree survived, you can rebuild from
the `.typ` files alone — a fresh `init`, then a `reindex --adopt` that registers
every orphaned file under the matching branch of the hierarchy (this is the next
section's `--adopt`, doing recovery duty).

#screen(caption: "Rebuilding a project from surviving .typ files")[```
$ mkdir ~/Books/rebuild
$ cp -r ~/Books/my-novel/books ~/Books/rebuild/books
$ inkhaven init --force ~/Books/rebuild
$ inkhaven --project ~/Books/rebuild reindex --adopt
```]

This assigns fresh UUIDs — it is a new project, not the old one — but your prose
and its chapter structure come back from the directory layout.

#section("Crash recovery — inkhaven recover")

The backup answers "the directory is gone." Crash recovery answers the smaller,
commoner disaster: the process died — a panic, a lost power cable, a `kill -9` —
between two keystrokes, with unsaved edits in a buffer. Chapter 6 covers the
machinery in full; here is the maintenance-side summary.

While you edit, a background task mirrors every *dirty* buffer to disk on a short
cadence (`editor.crash_mirror_seconds`, two seconds by default), so a sudden
death can cost at most the last couple of seconds of typing. If Inkhaven panics,
its handler writes a *crash report* (`inkhaven-crash-<UTC>.hjson`, an index of
what was open and a manifest of rescued buffers) plus one *rescue file* per dirty
paragraph (`<paragraph>.typ.inkhaven-rescue`, the actual flushed prose) — both
written atomically, both silent on failure. See chapter 6 for exactly what the
report does and does not record.

#subsection("Walking the rescued buffers")

After a crash, reopen the project, then run `inkhaven recover` from a shell,
pointing it at the report. It walks each rescued buffer, prints its size and the
delta against what is currently on disk, and asks what to do — `y` to apply, `N`
(the default) to skip, `d` to see a unified diff first and then re-ask.

#screen(caption: "inkhaven recover — the per-buffer prompt")[```
$ inkhaven recover ./inkhaven-crash-20260805T0321.hjson

  Found 1 rescued buffer(s).

  [1/1] books/ch2/the-quay.typ
       rescue : …the-quay.typ.inkhaven-rescue (214 bytes)
       on-disk: books/ch2/the-quay.typ (188 bytes, delta +26)
       apply? [y/N/diff]: y
       applied.

Done: 1 applied, 0 skipped, 0 error(s).
```]

Applying a rescue is careful about the copy it overwrites: before writing, it
snapshots the current on-disk file as `<paragraph>.typ.pre-recover-<UTC>`, so the
pre-rescue version is always one `mv` away. A buffer whose rescue is byte-for-byte
identical to disk is reported as "no action needed" and left untouched. `--yes`
applies every rescue without prompting; `--keep` leaves the report and rescue
files in place. Without `--keep`, a clean run moves them into
`<project>/.inkhaven/recovered/` so your working tree is tidied. Two things worth
knowing: `recover` *never opens the database* — it works purely on file paths, so
it is safe to run while a fresh TUI session is open on the same project — and any
buffer that fails to apply leaves the whole report in place and exits non-zero,
so a `recover && rm report` habit can never silently discard unrecovered work.

#callout(label: "Path safety")[
  A crash report is just an HJSON file, so `recover` treats it as untrusted:
  every rescued path is resolved *within* the project root, and any `..`
  traversal is rejected rather than followed. A hand-crafted report cannot make
  `recover --yes` write outside the project.
]

#section("Reindex — reconciling the store with the disk")

The database and the `.typ` files on disk are meant to agree. Most of the time
they do, because the editor keeps them in sync on every save. They can drift when
something changes files *outside* the TUI — you edited a paragraph in another
editor, a `git checkout` brought back files the database had forgotten, you moved
or deleted files in `books/` from a shell, or you switched the embedding model.
`reindex` is the command that makes them agree again.

#screen(caption: "The three reindex modes")[```
$ inkhaven --project ~/Books/my-novel reindex
reindex: 3 updated, 511 unchanged, 0 missing, 0 orphan(s)

$ inkhaven --project ~/Books/my-novel reindex --prune --adopt
reindex: 0 updated, 514 unchanged, 1 missing, 2 orphan(s)
  pruned 1 missing record(s) from the store
  adopted 2 orphan .typ file(s) into the hierarchy
```]

Plain `reindex` re-reads every `.typ` file the database knows about; where the
file's bytes differ from the stored content it updates the record and re-embeds
the paragraph, leaving unchanged ones alone. That bare form is what you run after
switching `embeddings.model` (to re-embed the whole book with the new model), or
after a restore, or simply as an "are my files and database aligned?" check. The
two flags handle the drift the bare form only *reports*:

#chord_table((
  chord_row("--prune", "Remove store records whose .typ file is missing from disk — after deleting files or folders outside the TUI."),
  chord_row("--adopt", "Register .typ files on disk the store doesn't know about, under the deepest branch whose path matches their parent folder — after dropping new files into books/."),
))

Run bare, `reindex` prints how many records point at missing files and lists any
orphan `.typ` files, then tells you to re-run with `--prune` or `--adopt` to act
on them; nothing is deleted or created until you ask. You can combine the flags —
`--prune --adopt` does both passes — and the command is idempotent, so running it
twice is harmless.

#callout(label: "Reindex vs. doctor")[
  `reindex` reconciles *content* — file bytes against stored bytes, plus
  add/remove of whole records. The *doctor* (next) inspects *structure* — broken
  parent links, dangling references, zero-byte files, corrupt sidecars. Reach for
  `reindex` after external file edits; reach for `doctor` when you suspect the
  store itself is bent.
]

#section("The project doctor")

The doctor is Inkhaven's health check. In its plain form, `inkhaven doctor`
prints a `brew doctor`-style report — the binary version and toolchain, the Typst
engine summary and whether an external `typst` is on `PATH`, the package cache
path and size, the project's shape and word count, and a *Notes* section flagging
things worth acting on (engine set to external with no `typst` installed, both
font sources disabled, and so on). That informational dump is useful but not the
interesting part. The interesting part is the *scan*.

#subsection("Scanning for trouble — doctor --scan")

`inkhaven doctor --scan` walks the hierarchy and the on-disk tree and emits
structured *findings*, each carrying a *class*, a *severity*, an optional *path*,
and a one-line detail. `--json` emits the same report as JSON (it implies
`--scan`), which is what makes the doctor a CI gate.

#screen(caption: "A doctor scan with findings")[```
$ inkhaven doctor --scan
Project scan
  generated_at  : 2026-08-05T14:31:09Z
  project_root  : /home/you/Books/my-novel
  findings      : 2

  [1] critical · broken-parent-ref          · ch3/lost-scene
        `Lost Scene` (id …) has parent_id …, which
        doesn't exist — the node is detached from the tree
  [2] warning  · sibling-slug-collision      · the-quay
        2 siblings under `Chapter 2` share slug `the-quay`
        (The Quay, The Quay) — their files collide; rename one
```]

The scan exits `0` when clean, and `2` when any finding lands at *Warning* or
above — the conventional linter exit code — so a CI job can fail the build on a
structural regression:

#screen(caption: "Gating CI on a clean project")[```
$ inkhaven doctor --json | jq -e '.findings == []'
```]

#subsection("The finding classes")

The classes fall into three families. The first is *data integrity* — the ones
that mean prose is at risk or already lost, and the ones the doctor can actually
repair:

#chord_table((
  chord_row("zero-byte-file", "A .typ file is 0 bytes and the store has no content either — prose lost. CRITICAL."),
  chord_row("orphan-paragraph-row", "A store row points at a file that isn't on disk, and the store has no content for it. WARNING."),
  chord_row("missing-referenced-file", "As above, but the row's path is malformed (empty or contains ..). WARNING."),
  chord_row("bdslib-only", "The disk file is missing or 0 bytes but the store still holds the prose — recoverable. INFO."),
  chord_row("corrupt-comments-sidecar", "A paragraph's .comments.json doesn't parse — comments unreadable. WARNING."),
))

The second family is *referential integrity* — the structural links inside the
hierarchy that a partial delete or a botched restore can bend. These are
detection-only today (no autofix; you resolve them by hand):

#chord_table((
  chord_row("broken-parent-ref", "A node's parent_id points at a UUID not in the tree — the node is detached and invisible. CRITICAL."),
  chord_row("dangling-paragraph-link", "A paragraph links to another that was deleted — the link dangles. WARNING."),
  chord_row("dangling-event-ref", "A timeline event references a character or place that no longer exists. WARNING."),
  chord_row("sibling-slug-collision", "Two children of one parent share a slug, so their files collide on save. WARNING."),
  chord_row("duplicate-system-book", "Two Books carry the same system tag, making system-book lookups ambiguous. WARNING."),
))

The third family is *author judgment* — plot- and prose-level observations
(dropped character, pacing collapse, stalled thread, naming inconsistency, echo
repetition, numeric contradiction, continuity drift, paragraph too long, stale
submission). These are all *Info*, never carry an autofix, and belong properly to
the Editorial Pass (`inkhaven edit`, chapter 27) rather than to project
integrity — only you can decide whether a dropped character was deliberate. One
class, `unresolved-tension`, is *opt-in*: it never runs on a plain `--scan` and
only appears when you name it with `--class unresolved-tension`, because its
tagging is approximate and an open thread may be a deliberate hook. You can scope
any scan to a single class with `--class <name>`.

#subsection("Repairing — doctor --autofix")

`--autofix` applies the repairs the doctor knows how to make. It prompts `y/N`
per finding (add `--yes` to accept every one non-interactively), applies the fix,
and logs each outcome.

#screen(caption: "Autofix, one finding at a time")[```
$ inkhaven doctor --scan --autofix
Autofix — applying repairs.

  [1/1] warning · corrupt-comments-sidecar
        path: books/ch2/the-quay.typ.comments.json
        comments sidecar for `the-quay` doesn't parse as JSON
        apply repair? [y/N]: y
        applied: moved corrupt sidecar … → ….corrupt-20260805T143210.bak

Autofix done: 1 applied, 0 skipped, 0 error(s).
```]

The repairs are conservative and specific. A zero-byte, orphan, or
missing-referenced finding is fixed by *deleting the dead row and file* (there is
nothing left to save). A corrupt sidecar is *renamed* to a timestamped `.bak`
rather than deleted, so you can inspect it. A `bdslib-only` finding is
*rematerialised* — the store's content is written back to the missing disk file,
atomically, and only after re-checking that no real file has appeared in the
meantime, so a concurrent save is never clobbered. Author-judgment and
referential-integrity classes return "no autofix" and point you at the prose or
the structure to fix by hand.

#subsection("The doctor panel — Ctrl+B Shift+0")

Everything above has an in-editor twin. `Ctrl+B Shift+0` opens the *doctor panel*
from any pane: it runs the same project scan synchronously on open and shows every
finding in a modal table with its class, severity, path, and detail. Inside,
`↑↓` move the selection, `r` repairs the highlighted finding, `R` repairs every
finding it can, and `Esc` closes. A repaired finding drops out of the table so
you watch the list shrink as you go; a finding with no autofix reports why on the
status line and stays. Every repair — from the panel or the CLI — is appended to
`<project>/.inkhaven/doctor.log` as a timestamped `UTC | outcome | class | detail`
line, so there is an audit trail of what the doctor touched.

#screen(caption: "The doctor panel — Ctrl+B Shift+0")[```
┌─ Project doctor · 2 finding(s) ─────────────────────┐
│ ● critical  broken-parent-ref     ch3/lost-scene    │
│   `Lost Scene` detached — parent_id doesn't exist   │
│ ⚠ warning   corrupt-comments…     …the-quay.typ     │
│   comments sidecar for `the-quay` won't parse       │
├─────────────────────────────────────────────────────┤
│ ↑↓ select   r repair   R repair-all   Esc close     │
└─────────────────────────────────────────────────────┘
```]

The chord lives on the digit-`0` row deliberately: `Ctrl+B 0` opens the HJSON
config editor and `Ctrl+B Shift+0` opens the doctor — the "inspect the system"
cluster, paired in your muscle memory.

#callout(label: "Two doctors, one word")[
  Do not confuse the *project doctor* here (`Ctrl+B Shift+0`, `inkhaven doctor`)
  with the *thread doctor* of chapter 20 (`Shift+D` in the thread view) or the
  *Editorial Pass* of chapter 27 (`inkhaven edit`). This doctor is about
  *project integrity* — structure and files. Those are about the manuscript's
  craft. They share findings at the edges (the author-judgment classes) but the
  jobs are different.
]

#section("The advisory project lock")

Two TUI sessions open on the same project both write `metadata.db` and
`.session.json`; interleaved writes can corrupt the store. Inkhaven guards
against that with an *advisory* lock — and the word advisory is the whole design.
In keeping with the tool's permissive principle, the lock *informs*; it never
hard-blocks a writer who has decided to proceed.

#term("Advisory lock")[
  On launch, Inkhaven takes an OS-level advisory lock (`flock` / `LockFileEx`) on
  `<project>/.inkhaven.lock`. If a live session already holds it, the launcher
  *warns* and — by default — asks whether to open anyway. The kernel releases the
  lock automatically when the holding process exits, *including on a crash or
  `kill -9`*, so there is no stale-lock cleanup to get wrong: a dead session never
  locks you out.
]

When a second session finds the project busy, the default behaviour prints who
holds it (a friendly `PID … since HH:MM`, read from the lockfile purely to make
the message informative) and prompts:

#screen(caption: "The 'open anyway?' prompt")[```
⚠ Another inkhaven session may already have this project
  open (PID 4321 on mac since 14:02).
  Opening it twice at once can corrupt the project store.
  Open anyway? [y/N]
```]

Answer `y` and you proceed *without* the lock, minding your own concurrent edits;
answer anything else and the launch is cancelled, leaving the existing session
untouched. The behaviour is set by `project_lock.on_conflict`: `"prompt"` (the
default) asks as above and warns-then-proceeds when launched non-interactively;
`"warn"` always warns and proceeds without asking; `"refuse"` never opens a
second session. Setting `project_lock.enabled: false` disables the guard
entirely. The same policy governs store-mutating *CLI* commands (a `reindex` or a
`doctor --autofix` run while a TUI holds the project): they consult the lock, warn
if it is busy, and proceed unless `on_conflict` is `refuse` — so concurrent
writers serialise rather than race. A filesystem that does not support advisory
locks (some network mounts) degrades to opening *unlocked* rather than locking you
out. The lockfile itself is left in place afterward as a harmless, empty anchor
reused on the next launch.

#section("Logs, drift, and first-run trouble")

When something behaves oddly, the log is the first place to look. While the TUI
runs, the `tracing` subscriber appends to `<project-root>/.inkhaven.log` (plain,
no colour); CLI commands log to stderr instead, so they never disturb the TUI's
alternate-screen rendering. What lands there: provider warnings from the AI layer
(some, like an empty streaming chunk from certain providers, are normal and
ignorable), notable store operations, and the non-fatal background errors — a
failed auto-backup, a focus-loss save that could not write — that would otherwise
be invisible.

#screen(caption: "Reading and raising the log")[```
$ tail -n 200 ~/Books/my-novel/.inkhaven.log

# turn the log up to debug for one run:
$ RUST_LOG=inkhaven=debug,bdslib=info \
      inkhaven --project ~/Books/my-novel
```]

The TUI honours a standard `RUST_LOG`-style filter through tracing's
`EnvFilter`, so you can scope the verbosity to the module you are chasing.

#subsection("Common drift and how to clear it")

Most "the tool and the disk disagree" symptoms have the same small set of cures,
and nearly all of them are a `reindex`:

#chord_table((
  chord_row("Tree and disk disagree", "reindex --prune --adopt — reconcile both directions at once."),
  chord_row("Editor shows stale content", "reindex — a previous session crashed without flushing; re-read the .typ files."),
  chord_row("Search looks wrong after a model switch", "reindex — re-embed every paragraph with the new model."),
  chord_row("Structure looks bent", "doctor --scan — inspect the hierarchy's referential integrity."),
  chord_row("A database file is corrupt", "restore a backup, or rebuild from books/ with init --force + reindex --adopt."),
))

#subsection("First-run model download")

The one piece of setup that reaches the network is the embedding model: the first
time you open a project, `fastembed` downloads the chosen model into a per-user
cache (its location per OS is in chapter 1), and every later project reuses it.
If that download hangs — a slow connection, a server hiccup — the startup splash
shows the elapsed time, and `Ctrl+Q` aborts so you can retry later. For an
air-gapped install, pre-seed the cache from another machine. Switching
`embeddings.model` later downloads the new model on the next open and leaves the
old one on disk until you clear the cache directory yourself. Chapter 1 has the
full first-run walkthrough.

#section("The configuration that governs all this")

Four small config blocks tune everything in this chapter. Here they are together,
at their defaults, with the fields that matter for maintenance.

#screen(caption: "The maintenance-related config blocks")[```
backup: {
  out_dir: ""            // ""=sibling inkhaven-backups/; rel=under
                         //   project; abs=verbatim
  max_age: "7d"          // exit-hook staleness window; "0s"/"" off
  auto_backup_on_exit: true
  wait_for_key_after_backup: true
  keep_last: 0           // 0=keep all; N=rolling window of N
  amber_threshold: 0.5   // status chip goes amber at this
                         //   fraction of max_age
}

project_lock: {
  enabled: true
  on_conflict: "prompt"  // "prompt" | "warn" | "refuse"
}

health: {
  enabled: false         // background health monitor (opt-in)
  auto_repair: {
    rescue_orphans: false // delete *.inkhaven-rescue files >30d old
  }
}
```]

A few notes on the less obvious ones. `backup.amber_threshold` controls when the
optional status-bar freshness chip turns amber (a soft "plan a backup soon"
heads-up) before it turns to the hard warning past `max_age`; `0.0` disables the
amber stage. The `health` block is an *opt-in* background monitor, off by default
so no existing project inherits a new background task silently — and even with it
on, every auto-repair is individually off until you enable it, so turning on the
monitor never grants it permission to mutate your project. Its per-check cadences
(a project scan every 90s, a backup-freshness check every 300s, a rescue-orphan
sweep hourly) are tuned in code rather than exposed as HJSON. The only auto-repair
that exists today, `rescue_orphans`, cleans up stray `.inkhaven-rescue` files
older than thirty days — the debris of crashes you already recovered from.

#recap((
  [`inkhaven backup` writes a dated, filesystem-level `.zip` of the *whole*
  project — databases, vectors, and prose — that `restore` reproduces exactly;
  the *exit hook* takes one automatically when the last is older than
  `backup.max_age`, and `keep_last` bounds how many are retained.],
  [`restore` unpacks into a *fresh* directory (refusing to clobber a project or
  unpack a non-Inkhaven zip); the same command doubles as a *branch* tool, and a
  lost database can be rebuilt from surviving `books/` with `init --force` +
  `reindex --adopt`.],
  [`inkhaven recover` walks a crash report's rescued buffers (`y`/`N`/`diff`,
  `--yes`, `--keep`), snapshots each file as `.pre-recover-<UTC>` before
  overwriting, never opens the database, and refuses path traversal — the crash
  mirror itself is chapter 6.],
  [`reindex` reconciles *content*: bare re-reads and re-embeds changed files;
  `--prune` drops records for missing files; `--adopt` registers orphan `.typ`
  files under their matching branch.],
  [The *doctor* inspects *structure*: `doctor --scan` (`--json`, `--class`,
  exit 2 on Warning+) reports data-integrity, referential-integrity, and
  author-judgment classes; `--autofix` repairs the safe ones; `Ctrl+B Shift+0`
  is the in-editor panel (`r`/`R` repair, logged to `.inkhaven/doctor.log`).],
  [The *advisory lock* (`.inkhaven.lock`) informs but never blocks — it warns and
  asks "open anyway?", the kernel releases it even on a crash, and
  `project_lock.on_conflict` chooses `prompt` / `warn` / `refuse`.],
  [`.inkhaven.log` (raise it with `RUST_LOG`) is the first stop for drift; most
  disk/store disagreements clear with a `reindex`, and first-run model-download
  trouble is covered in chapter 1.],
))
