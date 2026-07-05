# BUGFIX_PLAN 1.6.1 — stability pass over the 1.6.0 "Living World" surface

A four-track adversarial audit (WORLD compile layers · WORLD desk+CLI · TUI
scene-context · cross-cutting/non-WORLD) over everything 1.6.0 added. Findings
ranked; each fix lands with a regression test.

## P1 — correctness / user-visible-wrong / broken recovery

- **BUG-1 · RTF recovery still crashes the host.** `scrivener/rtf.rs:36` uses a
  raw `catch_unwind`, but the global panic hook (installed at `main.rs:86`) fires
  on the unwinding thread *first* — so a malformed `.rtf` still writes a crash
  report, prints "inkhaven crashed", and (from the TUI) tears the terminal down,
  even though the body is recovered. Fix: wrap in `crash::suppress_panic_report`.
  *(The 1.6.0 fix was incomplete.)*
- **BUG-2 · A declared nation is silently dropped.** `polities_layer.rs:87` skips
  a `NationDef` when its nearest settlement is already taken by an earlier
  declared capital; `k_target` still counted it, so a *generated* realm fills the
  slot and the authored nation (and its relations) vanish. Fix: fall back to the
  nearest *unused* settlement.
- **BUG-3 · Scene-world cache never invalidates.** `app.rs:14894` fills
  `scene_world` once and no path ever resets it — accepted place-links and
  mid-session `world.hjson` edits are ignored until restart, and `Ctrl+B W` shows
  a stale scene beside a fresh overview. Fix: invalidate on world.hjson mtime +
  on place-link acceptance.
- **BUG-4 · `realworld travel` overstates north–south distance ~1.5×.**
  `travel.rs:44` derives `km_per_cell` from `grid_width` only, then applies it to
  an isotropic cell distance — but the model grid is 160×120 equirectangular
  (2.25°/x-cell vs 1.5°/y-cell). A correct N–S march is flagged "implausible".
  Fix: separate x/y cell-km and scale `dx`/`dy` independently. (Same root fixes
  **BUG-12**, `scene.rs:93` nearest-realm.)

## P2 — medium (misleading output, robustness, perf)

- **BUG-5 · Project lock misses real writers.** `cli/mod.rs:4888`
  `command_mutates` omits `ImportHelp`, `ImportTypstHelp`, `Event(add)`,
  `Sources(import)`, `Thread(add)`, `Language(init)`, `Recover` — the advisory
  lock is silently skipped for them.
- **BUG-6 · Insert-time "contradiction" false positives.** `research/app.rs:5283`
  — `numbers()` flags a more-detailed fact (`1200` vs `1200`+`20`), comma/decimal
  grouping (`1,000,000`), and `net_negated()` trips on idiomatic "no" ("no doubt
  the treaty was ratified"). Fix: subset-aware numeric compare + drop the crude
  negation-parity heuristic (or gate it behind a real antonym signal).
- **BUG-7 · Unchecked heightmap index.** `materialize.rs:398` indexes
  `heightmap[y*width+x]` without a length check — a short vector panics instead of
  a clean `Error`. Fix: bounds-guard.
- **BUG-8 · `Ctrl+B W` compiles the whole chain on the UI thread** (`app.rs:13409`)
  — a visible freeze on a DEM/large-grid world. Fix: cache the overview compile
  keyed on world.hjson mtime (one-shot today, but the open janks). *(Assess cost;
  may defer the full async move — at minimum reuse the BUG-3 cache.)*
- **BUG-9 · Partial declared ecology wipes generated life.** `ecology_layer.rs:99`
  replaces flora/fauna wholesale — a biome pinned with only a `keystone` loses its
  fauna and can emit an empty keystone. Fix: fall back to generated fields when a
  declared field is empty.
- **BUG-10 · `dist2` overflow on extreme declared coords.**
  `polities_layer.rs:34` — a billions-range capital typo panics (debug) / gives a
  garbage nearest and suppresses the wilderness lint (release). Fix: saturating /
  f64 distance.
- **BUG-11 · Unknown travel mode silently becomes foot.** `travel.rs:19` — `boat`
  is assessed at 30 km/day and flagged implausible. Fix: report the mode was
  unrecognized (and broaden the synonym table).
- **PERF · scene tick** double-walks the hierarchy (`gather_events` twice) and
  retries the world-file read every paragraph switch on a no-world project.

## P3 — low (cosmetic / hardening / author-error)

- **BUG-13 · `inkhaven style --book-name` matches title only, not slug**
  (`style.rs:57`) — diverges from every sibling command's title-or-slug.
- **BUG-14 · Duplicate declared nation names** bind relations to the first only.
- **BUG-15 · EPUB image import** has no size cap (OOM on a bomb) + an empty-basename
  edge on an href ending in `/`.
- **BUG-16 · `row_to_latitude` vs climate** disagree by a half-cell (~0.75°).

## Cleared (checked, no bug)
Determinism of the compile layers (no HashMap iteration into output); `weather_at`
divisor/NaN guards; zero-settlement worlds; `build_timeline_calendar`; `validate`
reads the right layer per pass; `deadlinks` async lifetime + classification;
`resolve_href` cannot escape the archive root; `materialize_history` itself.
