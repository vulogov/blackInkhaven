# Proposal — Full TUI configuration editor (`inkhaven config`)

Status: **research / approved — ready for
implementation**.  Target cycle: 1.2.10.  All four
flagged open questions resolved 2026-05-27 (§10).

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
Ctrl+B i            comment inspector (floating pane — see §8.4)
Ctrl+Z              undo last edit (in-session)
Esc                 cancel current widget / focus tree / exit
Ctrl+Q              quit
```

Mirrors main inkhaven conventions wherever
possible.  `Ctrl+B h` for help matches the existing
context-help chord; `Ctrl+B i` is new — *inspect*
the human-written comments attached to the focused
field's stanza.

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

**Decision (Q3): preserve comments via surgical
text rewrite.**

Naïve `serde_hjson::to_string(&edited)` blows away
every comment in the live file.  We adopt a
**path → byte-range index** approach:

  1. Parse the live `inkhaven.hjson` with a hand-
    rolled walker that records, for every leaf
    key, the byte range of its **value** in the
    source text (not the key, not the surrounding
    whitespace) and the byte range of any leading
    comments attached to that field.
  2. Live in `App` as `source: String` (original
    bytes) + `index: HashMap<String, ValueSpan>`
    + `edited_json: serde_json::Value` (working
    copy of the JSON shape).
  3. On `Ctrl+S`, compute the diff of
    `edited_json` against the disk JSON.  For
    every changed leaf, splice the new HJSON
    representation into `source` at
    `index[path].value_range`.  For every newly-
    present leaf (e.g. an `Option<T>` flipped
    from `None` to `Some`, or a default
    explicitly committed by the user), append at
    the end of the appropriate stanza.
  4. Write the spliced `source` atomically (tmp
    + rename).

Trade-offs:

  * Comments **and** unknown fields (see §6.6)
    survive untouched.
  * Annotation-driven comments emitted on save
    (§7) live as their own marked blocks
    (`# annotation:` prefix) so the editor can
    update them in place without disturbing
    hand-written comments above them.
  * The parser is hand-rolled.  ~300 LOC; tested
    against every existing project's HJSON in a
    fixture corpus + the synthetic edge cases
    (trailing commas, multi-line strings,
    `#` vs `//` comments, nested braces inside
    strings).

The existing `Ctrl+B 0` HJSON editor stays as the
power-user fallback for raw editing — both
editors round-trip cleanly because both preserve
comments.

### 6.6 Unknown fields

The user may add fields outside the inkhaven
schema (forward-compat, external tooling
integration, custom Bund hooks).  Policy:

  * The TUI **does NOT display** unknown fields
    in the tree.
  * The TUI **DOES NOT EDIT** unknown fields.
  * The surgical-rewrite save path (§6.5)
    incidentally **preserves** unknown fields:
    the splice only touches byte ranges the
    schema knows about, leaving everything else
    untouched.

Surfaced as a chip on the top status bar when
unknown fields are detected:
*`3 unknown fields preserved as-is`*.

The user is on the hook for unknown-field
correctness — inkhaven makes no guarantees beyond
"we didn't touch them".  Documented prominently
on the TUI's first-launch splash.

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

### 8.4 Comment inspector (`Ctrl+B i`) — new

A second floating pane, complementary to `Ctrl+B h`.
While `Ctrl+B h` shows the **author** (inkhaven)
documentation for the field, `Ctrl+B i` shows the
**user's** comments — the `#` / `//` lines in the
live HJSON file attached to the focused field's
stanza.

Use cases:

  * The HJSON file carries an explanatory comment
    above a field ("bumped from 10s to 5s after
    testing on a 5K-word manuscript") and the
    writer wants to recall the reasoning without
    leaving the structured editor.
  * The annotation system (§7) records short notes
    against fields, but rich multi-line context
    naturally lives as comments in the HJSON
    file — the inspector surfaces both.

UI:

```
┌── Comments — editor.autosave_seconds ────────────┐
│                                                  │
│ Lines 47-49 of inkhaven.hjson:                   │
│                                                  │
│   # bumped from 10s to 5s after testing on a    │
│   # 5K-word manuscript — 10s lost too much typing │
│   # under high-cadence editing                    │
│                                                  │
│ Annotation (§7):                                 │
│   "deliberate: feels right for novel pace"       │
│                                                  │
│ Esc closes                                       │
└──────────────────────────────────────────────────┘
```

The inspector reads the comment text from the
byte-range index built at file load time (§6.5).
Scope: comments immediately above the focused
field's key, contiguous block.  No re-parsing —
the index already carries the comment span.

When focused on a stanza (not a leaf), the
inspector shows comments at the stanza opener
plus a count of fields with their own comments:
*`5 of 12 fields carry comments`*.

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

## 10. Resolved questions

Decisions made 2026-05-27 before implementation:

| #  | Question                                                                              | **Decision**                                                          |
|----|---------------------------------------------------------------------------------------|------------------------------------------------------------------------|
| Q1 | User spec says `YYYYDDMM_HHMMSS`; was that a typo for `YYYYMMDD_HHMMSS`?               | ✓ **Confirmed typo** → `YYYYMMDD_HHMMSS` (ISO).                       |
| Q2 | Should `inkhaven config` open the main project, or be CWD-agnostic?                   | `--project-directory` required when not in a project dir.              |
| Q3 | Comment preservation in HJSON.                                                         | ✓ **Preserve.**  Surgical text rewrite via path→byte-range index (§6.5).  Comments surface via new `Ctrl+B i` inspector (§8.4). |
| Q4 | Should the rollback list display annotations attached at backup time?                 | Yes — render in DIM beside the timestamp.                              |
| Q5 | Should `inkhaven config` work for projects that haven't run `inkhaven init` yet?      | Allow; treat as "no project, system defaults only".                    |
| Q6 | `theme` stanza has ~80 colour fields.  One-screen-per-colour or grouped sub-screens? | Group by surface (Editor / Tree / Modal / Style warnings).             |
| Q7 | Should saving trigger a `restart required` banner like Ctrl+B 0 does?                 | ✓ **Yes** — reuse the existing 1.2.8 restart-required overlay.        |
| Q8 | Edit-history undo: in-session only, or persisted across launches?                     | In-session only; persistence belongs to the backup system.             |
| Q9 | `LlmConfig.providers` map editor — v1 or defer?                                       | ✓ **In scope for v1.**  Treated as a map of named stanzas; editing tracked under §11 Phase 2. |
| Q10| Should the TUI verify `inkhaven.hjson` against a JSON Schema, or trust serde?         | Serde validation + per-field constraints is enough.                    |
| Q11| What about unknown / user-added fields (forward-compat, external tooling)?            | ✓ **Preserve incidentally via surgical rewrite; do NOT edit.**  See §6.6.  Top-bar chip surfaces the count. |

## 11. Implementation phases

### Phase 1 — schema + read-only walk-through (1–2 days)

Goal: a non-mutating TUI that can be shipped behind
a feature flag.

  * CLI plumbing (`inkhaven config`).
  * Schema construction (hand-rolled, all ~240
    fields).
  * Schema-completeness CI assertion against
    `Config::default()`.
  * Tree pane + leaf detail pane rendering.
  * Read live HJSON, overlay defaults.
  * Help pane (`Ctrl+B h`).
  * Unknown-fields detection + top-bar chip
    (§6.6).

No widgets that mutate; no save; no backup.  Useful
as a *config explorer*.

### Phase 2 — typed widgets + comment-preserving save (3–4 days)

  * Per-type widget catalog (§5).
  * Validation pipeline.
  * **Hand-rolled HJSON walker** that builds the
    path → byte-range index for surgical rewrite
    (§6.5).  Tested against every fixture HJSON
    in the test corpus + edge cases (trailing
    commas, multi-line strings, `#` vs `//`
    comments, nested braces inside strings).
  * **Surgical splice** save pipeline that
    preserves comments + unknown fields.
  * `Ctrl+S` confirmation modal.
  * `.config-backups/inkhaven_YYYYMMDD_HHMMSS.hjson`
    snapshot on save.
  * Restart-required overlay (reuses 1.2.8 plumbing).
  * **`LlmConfig.providers` map editor** (Q9 in
    scope) — list of named provider stanzas,
    `a` add, `d` delete, `Enter` edit child fields.

### Phase 3 — rollback + annotations + comment inspector (2 days)

  * `.config-backups/` lister + preview + diff.
  * Sidecar `.config-annotations.hjson` + chip
    rendering.
  * Annotation edit modal (`Ctrl+A`).
  * **`Ctrl+B i` comment inspector** floating
    pane (§8.4) — reads from the byte-range
    index built in Phase 2.

### Phase 4 — polish + docs (1 day)

  * Tree-pane incremental search (`/`).
  * Better diff in the rollback preview.
  * New tutorial in `Documentation/Tutorials/`
    (next free number, currently 44).
  * KEYBINDING.md rows for `inkhaven config`'s
    chord set.
  * Mention in `Documentation/CONFIGURATION.md`
    header pointing at the new editor.
  * RELEASE_NOTES/1.2.10.md write-up.

**Total estimate**: **7–9 days of focused work**
(was 5–7 before scope grew with surgical rewrite +
providers map editor).

## 12. Risks

| Risk                                                                                                  | Mitigation                                                                          |
|--------------------------------------------------------------------------------------------------------|--------------------------------------------------------------------------------------|
| Schema drifts from `Config` struct as new fields land.                                                | One-test schema-completeness assertion run in CI (Phase 1).                         |
| Hand-rolled HJSON walker mis-parses an exotic file and corrupts on save.                              | Fixture corpus + edge-case tests (Phase 2).  Backup snapshot taken *before* the splice runs, so rollback is one chord away.  |
| `theme` stanza dwarfs every other section in field count, dominates the tree.                          | Group rendering by surface (§Q6); collapse by default.                              |
| Surgical-rewrite splice splice-target offsets shift mid-save (re-indexing bug).                       | After each splice, rebuild the index from the post-splice source; cheap at literary HJSON sizes.  |
| `LlmConfig.providers` map widget complexity bleeds into the timeline.                                 | Phase 2 ships a minimal map-editor (add / delete / descend); rich provider-specific UX is Phase 4 polish only if budget permits. |
| Users editing both `Ctrl+B 0` and `inkhaven config` against the same file see divergent state.        | Mtime-watch like the main editor; reload on external change with red warning.       |
| Backup directory grows unbounded.                                                                      | Optional retention knob (§6.4).                                                     |
| Unknown fields surprise the user when they realise the TUI ignored them.                              | Top-bar chip (§6.6) + first-launch splash mention.                                  |

## 13. Out of scope (v1)

  * **Theme preview** — colour picker shows a
    swatch; no live overlay on a sample editor.
    Future polish.
  * **Bund-script config hooks** (e.g.
    `hook.on_config_save`) — out of scope.
  * **Network operations** — fully offline.
  * **Diff-based partial-save** (commit only N of
    M changed fields) — v1 saves everything that
    changed; granular commits are a future refinement.

## 14. Status: ready for implementation

All four flagged questions (Q1, Q3, Q7, Q9) plus
the new unknown-fields policy (Q11) are resolved.
Implementation can begin against the phased plan in
§11.
