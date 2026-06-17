# WORLD-1 — World consistency: internal facts, anachronism, the story bible, series canon (1.3.8)

_Status: planning. Target: **1.3.8**. A breadth cycle deepening the
world-consistency tools (Facts 1.2.21 + continuity 1.2.19): does the world
contradict ITSELF, does the prose fit its era, can the author SEE the whole
world at a glance, and can a series share one canon._

## Why

`facts scan` checks the prose against the Facts book; `continuity-drift`
catches a character attribute changing across chapters. But nothing checks
whether the **Facts book contradicts itself**, whether the prose contains
**anachronisms** for its setting, nor gives the author a **consolidated view**
of the world they've built — and a **series** has to duplicate its canon book
by book. 1.3.8 closes those four gaps, each building on the Facts /
continuity / overlay machinery that already ships.

## Builds on (already in tree)

- **The Facts book** (1.2.21) — `SYSTEM_TAG_FACTS`, prose paragraphs;
  `facts scan` / `extract` / `init`, the `FactScanReport` sidecar, the
  3-tier fact prompts, the Facts semantic search.
- **The continuity bible** (1.2.19) — `continuity_bible::ContinuityBible`
  (`.inkhaven/continuity.json`) + `detect_drift`.
- **The style-warning overlay pattern** — `tui::style_warnings`
  (config-built per-line detectors + the editor underline), now `pub(crate)`.
- **The Editorial Pass** (1.3.6/1.3.7) — `inkhaven edit` + the cockpit;
  new findings drop straight in.
- **The HNSW semantic index** — local embeddings + search (the divergent-
  description check).

## Dependencies

**None.** The internal-facts check + the bible reuse the LLM / embedding
stacks; the anachronism overlay reuses the style-warning detector pattern;
series canon is a config + a layered read.

## Phases

### P0 — Facts internal-consistency (`facts check`)

`inkhaven facts check [--provider] [--json]`: an AI pass that reads the
whole Facts book and flags **self-contradicting fact pairs** ("winters are
mild" in climate vs "the harbor freezes" in geography) — distinct from
`facts scan`, which checks prose against facts. Cached in
`.inkhaven/facts_check.json` (content-hash invalidated). Output structured
(`FactConflict { a, b, detail }`); folded into the Editorial Pass as a
`world` category. The Facts book is small, so a single whole-book pass is
enough; the parser is pure + tested.

### P1 — Anachronism overlay

A new `anachronism:` config block — the manuscript's `era` (e.g. a year or
a named period) + a curated default lexicon of period-bound terms (each with
an earliest plausible era: "wristwatch" ≥ 1900s, "okay" ≥ 1840s, …),
user-extendable. A deterministic `AnachronismDetector` (a `style_warnings`
sibling) flags terms that postdate the setting; an always-on editor overlay
(its own theme colour) + an `anachronism` editorial finding. Off by default
(no era set → no flags); the lexicon ships English-first.

### P2 — The story-bible view + divergent-description check

A TUI modal (`Ctrl+V Shift+B`, **B**ible) consolidating the world the author
has built: **characters** (their continuity-bible attributes, per chapter),
**facts** (grouped by category), **places / artefacts**. `↑↓` navigate,
`Enter` jumps to a source. Plus an embedding-based **divergent-description**
flag: paragraphs describing the same entity whose embeddings have drifted
apart surface as a continuity hint (reuses the HNSW index). Read/navigate.

### P3 — Series-shared facts

A `facts.shared_path` config pointing at a shared Facts source (a directory,
or a sibling project's Facts book). `facts scan` / `facts check` / the
fact-check chord layer **shared + local** facts (local wins on conflict), so
the books of a series share one canon without duplication. `inkhaven facts
import --from <path>` copies a snapshot for projects that prefer a hard copy
over a live link.

### P4 — docs + the 1.3.8 release cut

A new tutorial (69, *World consistency*); KEYBINDING (`Ctrl+V Shift+B`);
finalize `RELEASE_NOTES/1.3.8` + index + README; version bump
`1.3.8-dev → 1.3.8`; signed tag `v1.3.8`; `cargo publish`; merge to main;
open the next cycle.

## Non-goals (deferred)

- **AI-resolved fact conflicts** — `facts check` reports conflicts; it
  doesn't auto-rewrite a fact to resolve them (author judgment).
- **Per-language anachronism lexicons** — English-first; other languages
  layer in via the config like the style-warning lists.
- **A series-wide build / cross-book search** — series canon is facts-only
  this cycle, not a multi-book index.
- **The Whole-Book AI Editor** — still the 1.4 headline.

## Test posture

P0's conflict-parser + the `facts_check.json` cache shape are pure and
tested. P1's `AnachronismDetector` (term + era → hit) is pure and tested
over synthetic prose (the `style_warnings` test pattern). P3's shared+local
fact layering (precedence, dedup) is pure and tested. The TUI bible view
(P2) follows the established read-state pattern; covered by keybind-
regression + render-smoke tests.
