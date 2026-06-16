# PLANNING-2 — The Planning Board, fluent & complete (1.3.3)

_Status: planning. Target: **1.3.3**. Finishes the Planning Board that
1.3.2 (PLANNING-1) opened: make it **fluent** (map without hand-editing
HJSON) and **complete** (the blank-page / plan-first mode), then sweep the
small deferred items._

## Why

1.3.2 shipped the **analyze + pacing** slice — it diagnoses a draft you
already have. Two gaps remain:

1. **It isn't fluent.** Mapping a beat means hunting slugs and editing
   HJSON in the editor (the friction that surfaced the `plan check` slug
   listing). The outline view is read-only.
2. **It only does the retrofit half.** Pillar B promised the *front* of
   the lifecycle — planning before the prose exists. 1.3.2 has no
   plan-first mode.

1.3.3 closes both, and pays down the loose ends from the 1.3 feature run.

## Builds on (already in tree)

- **PLANNING-1** — the `Beat` model + `beat_body`/`parse_beat` (the
  write-back primitives), the `plan check` report, the `Ctrl+V Shift+K`
  outline modal.
- **The Threads write-back pattern** — parse an HJSON paragraph, mutate a
  field, re-render, `update_paragraph_content` + disk write.
- **The LLM stack** — `AiClient` + `collect_blocking`; the 3-tier
  `resolve_prompt_template`.
- **`create_node`** — for scaffolding chapter shells.
- **doctor scan classes** (`src/cli/doctor_scan.rs`) — for the
  `stale-submission` check.

## Dependencies

**None.** Write-back is `serde_hjson` over the existing store; plan-first
reuses the LLM stack; scaffolding is `create_node`.

## Phases

### P0 — `plan map` / `unmap` (the write-back foundation)

`inkhaven plan map <beat> <chapter> [--threads a,b] [--status drafted]` and
`inkhaven plan unmap <beat>`: resolve the beat by name/slug under Planning,
**validate** the chapter slug exists (and thread slugs exist in Threads),
load its HJSON body, set the field(s), re-render via `beat_body`, and save
(`update_paragraph_content` + disk — the Threads pattern). Deterministic,
fully testable end-to-end (unlike the TUI), and useful headlessly /
scriptably on its own. Tests: map sets `mapped_chapter` + round-trips;
unknown chapter / thread slug errors; unmap clears it.

### P1 — interactive mapping in the outline view (make it fluent)

The `Ctrl+V Shift+K` outline becomes interactive (reusing P0's write-back):

- **`m`** on a beat → a chapter picker (the report's chapter list) → writes
  `mapped_chapter` and refreshes the report in place.
- **`Enter`** → jump to the beat's mapped chapter in the editor.
- **`s`** → cycle the beat's `status` (planned → drafted → done).
- **`t`** → a thread picker to link/unlink an arc.

No more leaving the view to edit HJSON. The modal carries the beat node ids
so it can write back.

### P2 — plan-first: beat intentions from a premise (the headline)

`inkhaven plan scaffold --premise "<logline>" [--framework]`: with no draft
to digest, the LLM expands **each framework beat into a 1–2 sentence
intention** for *this* premise, written into each beat's `notes`. One
structured call (delimited per-beat output) → distributed to the beats. The
author now has a filled beat sheet to write against. Prompt resolves
through the 3-tier resolver (`plan-scaffold` slug). The TUI surfaces it in
the outline (a beat's intention shown on selection).

### P3 — plan-first: scaffold chapter shells from beats

`inkhaven plan scaffold --chapters [--book]`: materialize the structure —
create a chapter node per beat (or per act) under the **manuscript** book,
titled by the beat and seeded with its intention + a `mapped_chapter`
back-link. **Opt-in and guarded**: refuses if the book already has chapters
(never clobbers existing prose). The blank page becomes a navigable
skeleton.

### P4 — consolidation sweep (the loose ends)

- **`actions/checkout@v5`** across the three workflows (the Node-20
  deprecation, overdue).
- **`stale-submission` doctor scan** — sent submissions with no response
  past N days → a doctor finding (`--json` parity).
- **CLI Prompts-book prompt tier** — the submission / plan generators
  resolve `prompts.hjson → built-in` in the CLI; add the Prompts-book
  paragraph tier so the CLI matches the TUI's 3 tiers.
- **`I`-lift** — from the AI pane, lift a streamed analysis / generator
  draft into its system book (Planning / Submissions) — closing the loop
  the 1.3.1/1.3.2 streaming chords left open.

### P5 — docs + release

Tutorial 67 update (interactive mapping + plan-first); KEYBINDING (the new
outline keys); RELEASE_NOTES/1.3.3 finalize, README, version bump, signed
tag, `cargo publish`, merge to main.

## Risks / decisions

1. **Write-back races the editor.** If a beat paragraph is open in the
   editor, `plan map` / interactive mapping writes the store + disk; the
   editor's external-change watcher reloads it. Pin the disk-first ordering
   (the Threads pattern) so the on-disk `.typ` and the store agree.
2. **Plan-first response parsing.** A per-beat delimited format is
   fragile — validate the count, fall back to leaving a beat's `notes`
   empty rather than mis-assigning. Frame intentions as *suggestions*.
3. **Chapter scaffolding is destructive-adjacent.** Strictly opt-in,
   refuse on a non-empty book, and report exactly what it created.
4. **Don't over-build the pickers.** The chapter / thread pickers reuse the
   existing list-modal pattern; no new widget framework.

## Out of scope (1.3.3)

- **Tension-curve overlay** — still needs a tension model (PLANNING-1
  P4.2); not until that's designed.
- **Scene cards** (per-scene goal / conflict / disaster) — a finer grain;
  its own layer.
- **The Whole-Book AI Editor** → 1.4.

## Sequencing

P0→P1 is the shippable spine — it makes the existing Board fluent, the
highest-leverage fix, and benefits every current user. P2→P3 add the
plan-first capability (the headline). P4 sweeps the loose ends. A partial
1.3.3 (P0–P1 + P4) is still a real, valuable release.
