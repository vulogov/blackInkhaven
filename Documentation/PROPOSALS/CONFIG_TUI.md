# Proposal — Full TUI configuration editor (`inkhaven config`)

Status: **research / pre-implementation**.  Target
cycle: 1.2.10 (or 1.3 if scope grows).

## 1. Summary

A standalone TUI editor for `inkhaven.hjson`,
launched as `inkhaven config --project-directory
<dir>`.  Replaces hand-editing of HJSON with a typed,
schema-aware editor that surfaces defaults inline,
validates values per-type, versions every save, and
shows the matching CONFIGURATION.md section in a
floating help pane.

The existing in-app `Ctrl+B 0` HJSON editor stays —
it remains the power-user fallback for raw editing.
This new editor is the **structured** entry point;
the HJSON editor is the **textual** one.

## 2. Motivation

Today's config story:

  * `inkhaven.hjson` has ~30 top-level structs and
    ~240 leaf fields (from `src/config.rs`).
  * `Ctrl+B 0` opens an HJSON text editor — syntax-
    highlighted, but the writer is on their own for
    field names, default values, allowed enum
    variants, and type correctness.
  * `CONFIGURATION.md` documents ~97 rows but the
    writer has to context-switch to look up each
    one.
  * Default values live in `Default` impls in
    `src/config.rs`; the user can't see them
    without grepping source.
  * No version history of config changes — a
    typo today, no easy rollback tomorrow.

A schema-aware editor flips all four problems.

## 3. User-facing surface

### 3.1 Invocation

```
$ inkhaven config --project-directory ~/Books/aerin
$ inkhaven config -p ~/Books/aerin          # short form
$ inkhaven config                            # cwd default
```

Standalone TUI — exits to shell on `Esc` from the
top level or `Ctrl+Q`.  Same shutdown hooks as the
main editor (capture cleanup, terminal restore).

### 3.2 Layout

```
┌──────────────────┬──────────────────────────────────────────┐
│ Config tree      │ editor.style_warnings.show_dont_tell     │
│                  │                                          │
│ ▼ editor         │   enabled                  [x] true      │
│   theme: …       │   use_stemming             [x] true      │
│   ▼ style_warn…  │   english_emotion_adjs     [ list ]      │
│     ▼ show_dont… │   russian_emotion_adjs     [ list ]      │
│       enabled    │   …                                      │
│       linking_v… │                                          │
│   tts: …         │  (default · unset · annotated · changed) │
│ ▶ llm            │                                          │
│ ▶ theme          │                                          │
│ ▶ timeline       │                                          │
│ ▶ goals          │   Ctrl+S save · Ctrl+H help · Ctrl+R     │
│                  │   rollback · Ctrl+A annotate · Esc back  │
└──────────────────┴──────────────────────────────────────────┘
```

  * **Left pane** — config tree, behaves identically
    to the main inkhaven tree pane: `↑↓` move,
    `Enter` or `Space` expand/collapse, `/`
    incremental search.
  * **Right pane** — context-sensitive:
      - On a branch (`editor`, `editor.style_warnings`) →
        list of children with current values + status
        chips.
      - On a leaf (`editor.autosave_seconds`) → the
        widget for that field, plus its default
        value, current source (HJSON / default), and
        any annotation.

### 3.3 Status chips next to each field

| Chip          | Meaning                                                       |
|---------------|---------------------------------------------------------------|
| `default`     | Value is the built-in default (not present in HJSON).         |
| `unset`       | Optional field with no value (`Option<T>` is `None`).         |
| `annotated`   | User has attached a free-text note (see §7).                  |
| `changed`     | Value differs from disk — unsaved edit in this session.       |
| `invalid`     | Current value fails the type/constraint check.                |

### 3.4 Chord set

```
↑ ↓                 navigate
Enter / Space       expand / collapse / edit (context-aware)
/                   incremental search in tree
Tab                 focus right pane / back to tree
e                   edit selected leaf
r                   reset selected leaf to default
Ctrl+S              save (with confirmation modal)
Ctrl+R              open rollback picker
Ctrl+A              attach / edit annotation on the current field
Ctrl+B h            field help (floating CONFIGURATION.md slice)
Ctrl+Z              undo last edit (in-session)
Esc                 cancel current widget / focus tree / exit
Ctrl+Q              quit
```

Mirrors main inkhaven conventions wherever
possible.  `Ctrl+B h` for help matches the existing
context-help chord.

## 4. Architecture

### 4.1 Crate layout

New module: `src/config_tui/`

```
src/config_tui/
├── mod.rs              entry (`run`)
├── schema.rs           ConfigField, ConfigSchema, type catalog
├── schema_build.rs     hand-rolled schema for every Config field
├── tree.rs             tree view + flatten + expand state
├── widgets/
│   ├── mod.rs
│   ├── bool_toggle.rs
│   ├── int_input.rs
│   ├── text_input.rs
│   ├── enum_select.rs
│   ├── color_picker.rs
│   ├── path_picker.rs
│   └── list_editor.rs
├── help.rs             CONFIGURATION.md slicer + floating pane
├── annotations.rs      sidecar load/save
├── backup.rs           timestamped HJSON snapshots + rollback
└── app.rs              event loop, modal stack, dispatcher
```

New CLI entry: `src/cli/config_cmd.rs` registered
under `inkhaven config` in `src/cli/mod.rs`.

### 4.2 Schema model

The core data structure:

```rust
pub enum ConfigType {
    Bool,
    Int { min: Option<i64>, max: Option<i64> },
    Float { min: Option<f64>, max: Option<f64> },
    String { multiline: bool },
    Color,                    // "#RRGGBB"
    Path { kind: PathKind },   // PathKind::File | Dir | Any
    Enum(Vec<&'static str>),   // e.g. ["external","inprocess"]
    StringList,
    Stanza(Vec<ConfigField>),  // nested struct
    Map(Box<ConfigType>),      // for HashMap<String, T> stanzas
}

pub struct ConfigField {
    pub path: String,          // "editor.style_warnings.enabled"
    pub display: &'static str, // "enabled"
    pub help_anchor: &'static str, // matches CONFIGURATION.md key
    pub ty: ConfigType,
    pub default: ConfigValue,
    pub since_version: &'static str,
    pub deprecated_since: Option<&'static str>,
}
```

### 4.3 Building the schema

**Three options considered**:

| Approach                          | Pro                                 | Con                                       |
|-----------------------------------|-------------------------------------|-------------------------------------------|
| Proc-macro on `Config` structs    | Compile-time correctness; auto-sync | Heavy infrastructure; one more dep        |
| Runtime reflection via `serde`    | Zero macro                          | Serde reflects shape, not enums/ranges    |
| **Hand-rolled `schema_build.rs`** | Full control, explicit defaults     | Has to be kept in sync with `config.rs`   |

**Recommendation**: hand-rolled.  ~240 lines of
table data is cheap; the macro overhead would dwarf
it.  A `cargo test` post-build pass compares the
schema's leaf paths against `serde_value::Value` of
`Config::default()` to catch any field that was
added to `Config` but forgotten in the schema —
this is the drift trap and a one-line CI assertion
fixes it.

### 4.4 State machine

```
App {
    schema: ConfigSchema,        // built once at startup
    on_disk: serde_json::Value,  // last loaded HJSON
    edited:  serde_json::Value,  // working copy
    annotations: HashMap<String, String>,
    tree_focus: Focus,
    tree_cursor: usize,
    collapsed: HashSet<String>,  // path -> collapsed
    modal: Modal,                // edit widget / help / rollback / save-confirm
    backups: Vec<BackupEntry>,
}
```

`edited` is the live JSON; widgets mutate it
directly via path-based set.  Saving serialises
`edited` back to HJSON with comment preservation
(see §6).

## 5. Widget catalog

| Type             | Widget                                                          |
|------------------|-----------------------------------------------------------------|
| `bool`           | Inline `[x] true` / `[ ] false`, Space toggles.                 |
| `int`            | Number input with min/max validation, `↑↓` increments by 1.     |
| `float`          | Same shape, decimal-aware.                                      |
| `String`         | Single-line text input; multiline opens a modal with textarea.  |
| `Color`          | Hex input + live swatch ████; HSL slider modal on Enter.        |
| `Path`           | Text input with existence check; F3 file picker on Enter.       |
| `Enum`           | Up/Down selector showing all variants; current marked `▶`.       |
| `Vec<String>`    | Vertical list: `↑↓` select, `a` add, `d` delete, `Enter` edit.  |
| `Stanza`         | (Not a leaf — descend into tree.)                                |

Validation runs on every keystroke; the `invalid`
chip lights up and `Ctrl+S` is blocked until every
field validates.

## 6. Save flow + backup

### 6.1 `Ctrl+S` flow

1. Validate all fields.  If any `invalid`, modal
   shows the failing paths and refuses to save.
2. Confirmation modal: *"Save 7 changes to
   `<dir>/inkhaven.hjson`?  [y/N]"*.
3. On `y`: write `inkhaven.hjson` (atomic — write
   to `inkhaven.hjson.tmp` then rename).
4. Simultaneously copy the post-save file to
   `.config-backups/inkhaven_YYYYMMDD_HHMMSS.hjson`.
5. Re-load `on_disk` from the written file; reset
   `edited` to match; clear the `changed` chips.
6. Status bar: `saved · backup #14 · 0 unsaved`.

### 6.2 Filename format

The user spec said `inkhaven_YYYYDDMM_HHMMSS.hjson`.
**Recommend**: `inkhaven_YYYYMMDD_HHMMSS.hjson`
(ISO 8601 year-month-day) so sort-by-name lines up
with chronological order.  Flag this as Q1 in §10.

### 6.3 Storage location

`<project>/.config-backups/` — hidden subdir,
gitignored by default.  Reasoning:

  * Keeps the project root clean.
  * Matches existing `.session.json` / `progress.db`
    convention.
  * Easy to exclude from backups + cloud sync if the
    user wants.

### 6.4 Retention policy

Default: keep all (config files are tiny — hundreds
of K-byte files don't add up).  Optional HJSON knob
`config_editor.backup_retention: <integer>` to cap
the count; oldest dropped on overflow.

### 6.5 HJSON write — comment preservation

Naïve `serde_hjson::to_string(&edited)` blows away
every comment in the live file.  Two strategies:

  * **Strategy A — comment rebind**.  Parse HJSON
    with a comment-preserving parser (`deser-hjson`
    + a fork; or a hand-rolled walker), match
    comments to fields by line proximity, re-emit
    with comments at the same paths.  Brittle.
  * **Strategy B — annotations replace comments**
    (recommended).  Editor *owns* comments: any
    in-file `#` / `//` lines outside annotations
    get stripped on first save, replaced by
    annotation-driven comments on subsequent
    saves.  Simpler.  Tradeoff: hand-edited
    comments are lost the first time the user
    saves from the TUI — surface this in a
    one-time "migration" modal.

**Recommend Strategy B**.  The HJSON editor
(`Ctrl+B 0`) stays the comment-preserving path for
users who want raw control.

## 7. Annotations

Annotations are free-text notes attached to a
config field path.  Storage:

  * **Live HJSON**: rendered as `# annotation:
    <text>` above the field on every save.
  * **Sidecar**: `<project>/.config-annotations.hjson`
    keyed by path (`{"editor.autosave_seconds":
    "5s feels fast — bumped from 10s after testing"}`).

The sidecar is the canonical store; HJSON
re-emission is for human-readable diffs.  This
sidesteps the comment-preservation problem in §6.5.

UI: `Ctrl+A` on a field opens a single-line input
(or multiline modal); enter saves to the sidecar
and updates the field's `annotated` chip.

## 8. Help integration

### 8.1 `Ctrl+B h` on a field

Opens a floating pane (~70% width, ~60% height,
centred) showing the relevant CONFIGURATION.md
slice.  Same layout convention as the main app's
Ctrl+B h quickref.

### 8.2 Mapping field → doc section

CONFIGURATION.md is a table.  Each row's left
column is a config path (`style_warnings.enabled`,
`pov_chip_enabled`, …).  Two-pass plan:

  1. At build time, parse CONFIGURATION.md into a
     `HashMap<String, String>` (path → markdown
     body for that row).  Embed via
     `include_str!` so the binary is self-
     contained.
  2. On `Ctrl+B h`, look up the current field's
     `help_anchor`, fall back to the parent stanza's
     anchor if not found, fall back to a generic
     "see CONFIGURATION.md" link if neither hits.

The `help_anchor` field on `ConfigField` keeps the
mapping explicit so a docs reorganisation can
re-target a help link without changing TUI logic.

### 8.3 Rendering

Reuse the existing markdown renderer
(`pulldown-cmark` + the Help-book viewer plumbing
from 1.2.8).  Read-only pane; `Esc` closes.

## 9. Rollback (`Ctrl+R`)

Modal listing every backup in
`.config-backups/`, newest first:

```
┌── Config rollback ────────────────────────────┐
│  ▶  2026-05-27 10:41:12   (12 minutes ago)    │
│     2026-05-27 10:08:35   (45 minutes ago)    │
│     2026-05-26 18:22:09   (yesterday)         │
│     2026-05-24 09:14:17   (3 days ago)        │
│     …                                          │
│                                                │
│  Enter to load · v to preview · d to delete    │
│  Esc to cancel                                 │
└────────────────────────────────────────────────┘
```

  * **Enter**: load the backup into `edited` (not
    onto disk yet — the user reviews the changes,
    then `Ctrl+S` commits).
  * **v**: split-pane diff of the backup vs current
    on-disk file (reuse F6 snapshot-diff plumbing).
  * **d**: delete a backup with confirm.

## 10. Open questions

| #  | Question                                                                              | Recommendation                                                |
|----|---------------------------------------------------------------------------------------|---------------------------------------------------------------|
| Q1 | User spec says `YYYYDDMM_HHMMSS`; was that a typo for `YYYYMMDD_HHMMSS`?               | Confirm — ISO ordering sorts correctly chronologically.       |
| Q2 | Should `inkhaven config` open the main project, or be CWD-agnostic?                   | `--project-directory` required when not in a project dir.     |
| Q3 | Comment preservation in HJSON: tolerate one-time loss (Strategy B) or invest in a comment-preserving parser? | Strategy B — see §6.5.                                        |
| Q4 | Should the rollback list display annotations attached at backup time?                 | Yes — render in DIM beside the timestamp.                      |
| Q5 | Should `inkhaven config` work for projects that haven't run `inkhaven init` yet?      | Allow; treat as "no project, system defaults only" — useful for tuning a fresh config before init. |
| Q6 | `theme` stanza has ~80 colour fields.  One-screen-per-colour or grouped sub-screens? | Group by surface (Editor / Tree / Modal / Style warnings).    |
| Q7 | Should saving trigger a `restart required` banner like Ctrl+B 0 does?                 | Yes — re-use the existing overlay; same triggers.              |
| Q8 | Edit-history undo: in-session only, or persisted across launches?                     | In-session only; persistence belongs to the backup system.    |
| Q9 | `LlmConfig.providers` is a `HashMap<String, LlmProvider>` — needs map-editor widget; in scope for v1? | Defer to v2; v1 treats providers as opaque, edit via HJSON.   |
| Q10| Should the TUI verify `inkhaven.hjson` against a JSON Schema, or trust serde?         | Serde validation + per-field constraints is enough.            |

## 11. Implementation phases

### Phase 1 — read-only walk-through (1 day)

Goal: a non-mutating TUI that can be shipped behind
a feature flag.

  * CLI plumbing (`inkhaven config`).
  * Schema construction (hand-rolled, all ~240
    fields).
  * Tree pane + leaf detail pane rendering.
  * Read live HJSON, overlay defaults.
  * Help pane (Ctrl+B h).

No widgets that mutate; no save; no backup.  Already
useful as a *config explorer*.

### Phase 2 — typed widgets + save (2–3 days)

  * Per-type widget catalog (§5).
  * Validation pipeline.
  * `Ctrl+S` save flow with confirmation modal.
  * Backup snapshot on save.
  * Restart-required overlay.

### Phase 3 — rollback + annotations (1–2 days)

  * `.config-backups/` lister + preview.
  * Sidecar annotations + chip rendering.
  * Annotation edit modal (Ctrl+A).

### Phase 4 — polish (1 day)

  * Tree-pane incremental search.
  * Better diff in the rollback preview.
  * Documentation: new tutorial 44 in
    `Documentation/Tutorials/`.
  * KEYBINDING.md row.
  * Mention in `Documentation/CONFIGURATION.md`
    header.

Total estimate: **5–7 days of focused work**.

## 12. Risks

| Risk                                                                                                  | Mitigation                                                                          |
|--------------------------------------------------------------------------------------------------------|--------------------------------------------------------------------------------------|
| Schema drifts from `Config` struct as new fields land.                                                | One-test schema-completeness assertion run in CI.                                  |
| Comment-loss on first save surprises users.                                                            | One-time migration modal that explains + offers to keep HJSON editor as default.   |
| `theme` stanza dwarfs every other section in field count, dominates the tree.                          | Group rendering by surface (Q6); collapse by default.                              |
| `LlmConfig.providers` map shape is harder than scalars.                                                | Defer (Q9); document that "Providers" branch is read-only in v1.                    |
| Users editing both Ctrl+B 0 and `inkhaven config` against the same file see divergent state.           | Mtime-watch like the main editor; reload on external change with red warning.       |
| Backup directory grows unbounded.                                                                      | Optional retention knob (§6.4).                                                     |

## 13. Out of scope (v1)

  * **HJSON comment round-trip** — Ctrl+B 0 keeps
    that responsibility.
  * **Editing LLM provider blocks** — keep that
    surface in the HJSON editor for now.
  * **Theme preview** — colour picker shows a swatch;
    no live overlay on a sample editor.  v2.
  * **Bund-script config hooks** — out of scope.
  * **Network operations** — fully offline.

## 14. Open thread → user

Items I'd like a green light on before starting
implementation:

  * The Q1 typo (`YYYYDDMM` → `YYYYMMDD`).
  * Strategy B for HJSON comments (§6.5) — accept
    that first save from the TUI may drop
    hand-written comments.
  * Sidecar `.config-annotations.hjson` location
    (§7) — alternative: in the HJSON file itself as
    `# annotation:` lines.
  * Defer `LlmConfig.providers` editor to v2.
