# 1.3.32 — Project doctor: referential integrity + the test spine

The first 1.4.0 bookend (see [ROADMAP-1.4.0.md](ROADMAP-1.4.0.md)). Two deliverables:
a **data-integrity safety net** and the **test/bench harness** every later release
will lean on.

## What already exists

`inkhaven doctor --scan` (`src/cli/doctor_scan.rs`) is a mature scanner with 15
classes across two families:

- **Project-integrity** (disk-side): `ZeroByteFile`, `OrphanParagraphRow`,
  `MissingReferencedFile`, `CorruptCommentsSidecar`, `BdslibOnly`, `StaleSubmission`.
- **Editorial** (narrative): dropped characters, pacing, stalled threads, naming,
  echoes, numeric contradictions, continuity drift, unresolved tension, long
  paragraphs.

Architecture: `ScanClass` enum (`slug`/`from_slug`/`ALL`/`is_opt_in`/
`editorial_category`), `ScanSeverity` (Critical/Warning/Info), `ScanFinding {class,
severity, path, detail}`, `ScanReport`, and `scan_project(project, selected)` that
gates each `scan_*(...)` behind a `run(class)` guard. `--json`, `--class <slug>`,
`--autofix` already wired in `cli/mod.rs`; exit code 2 when any finding is
≥ Warning.

## The gap

Every existing project-integrity class is **disk-side** (file vs DB). None checks
**referential integrity** *within* the hierarchy — the dangling UUIDs that
accumulate from deletes, partial restores, and hand-edits. That's the genuine fsck
hole, and it's cheap: the whole node set is already in memory after
`Hierarchy::load`.

## Phases

- **P0 — referential-integrity scan classes.** Five new `ScanClass` variants, all
  pure over the loaded `Hierarchy` (no disk, no DB mutation), wired into
  `scan_project` and the default run. **(this increment)**
  - `BrokenParentRef` *(Critical)* — `parent_id` points at a node that doesn't
    exist → the node is detached/unreachable.
  - `DanglingParagraphLink` *(Warning)* — a `linked_paragraphs` UUID resolves to
    nothing (a deleted target).
  - `DanglingEventRef` *(Warning)* — an `event.characters` / `event.places` UUID
    resolves to nothing.
  - `SiblingSlugCollision` *(Warning)* — two children of the same parent share a
    slug → their on-disk `fs_path`s collide.
  - `DuplicateSystemBook` *(Warning)* — more than one Book carries the same
    `system_tag`.
- **P1 — the test/bench spine.** _Done._ Decision: crate dev-deps are fine (only
  external *application/binary* deps are off-limits). **criterion was already wired**
  (`benches/` + `[[bench]]` startup/search), so P1 only added **`proptest`**. Perf
  budgets land as in-process `#[test]`s with an `Instant` assertion (the right tool
  for CI regression gates — runs in the release-gate `cargo test`; criterion stays
  for manual profiling). Shipped: a referential-scan perf-budget test (1000 nodes
  < 500ms, guards against O(n²) regressions) + a proptest harness smoke (3
  `levenshtein`-is-a-metric properties). Full suite 1758 → 1762.
- **P2 — parser property tests.** First property tests over the panic-prone parsers
  (calendar `parse`, HJSON config layering) on the P1 spine.
- **P3 — autofix for the safe classes.** Prune dangling `linked_paragraphs` /
  `event` refs and reconcile under `doctor --autofix` (store mutation; the existing
  autofix path). Detection-only in P0.

## Non-goals

No change to the existing scan classes, the report schema, the `--json` / `--class`
/ `--autofix` surface, or the editorial scanner. No new runtime dependencies.

## Increment log

- **P0** — _done._ Five referential-integrity `ScanClass` variants added to
  `doctor_scan.rs` (`BrokenParentRef` Critical; `DanglingParagraphLink`,
  `DanglingEventRef`, `SiblingSlugCollision`, `DuplicateSystemBook` Warning), each a
  pure `scan_*(&Hierarchy)` pass wired into `scan_project` + the default run, with
  `slug`/`from_slug`/`ALL` updated and a detection-only `apply_fix` arm. 6 unit
  tests (via `Hierarchy::from_nodes_for_test`) + live smoke. Full suite 1752 → 1758.
</content>
