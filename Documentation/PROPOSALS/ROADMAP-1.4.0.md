# Roadmap — the road to 1.4.0

After the 1.3.x feature run (ConLang Suite, WORLD-4/5, INNER_SOCRATES-1,
TIMELINE-2-INTEGRATION), 1.4.0 is the **consolidation & hardening** milestone: make
the four big systems feel like one product, add the daily-use quality-of-life a
feature-piled tree is missing, and harden the whole surface before the cut.

Everything here ships across **1.3.32 → 1.3.39**; **1.4.0 is the cut** once the
definition of done below is met.

## Two tracks

**Track A — Features (consolidation + flow).** Connective tissue between the big
systems plus the writer-flow QoL that was deferred during the RFC run.

**Track B — Stability (the spine).** A rider on *every* release, anchored by two
**bookends**: a data-integrity safety net at the **start** (`inkhaven doctor` + the
test/bench harness) and a full hardening sweep at the **end**. Principle: stand up
the test spine early so every feature lands on a verified base; re-sweep the larger
surface right before the cut.

## Backlogs

### Track A — Features
- **Command palette** — `Ctrl+P` fuzzy finder over every command/chord; `?`
  context-sensitive keybinding overlay.
- **Output pane filtering** — by provenance / severity / source-paragraph; saved
  filters; header counts.
- **`inkhaven check`** — one review pass running every checker (fact-check, Socratic
  fast, timeline critique, conlang coverage) over a scope → Output, with a summary;
  tree per-chapter finding badges.
- **Unified AI cost dashboard** — `inkhaven cost` + a panel over the scattered
  WORLD-slow / Socratic-slow / timeline-elaboration daily caps.
- **Word-count goals** — targets / per-chapter goals / streaks.
- **Snapshot browser** — browse snapshots + diff between them.
- **EPUB export** — the marquee new subsystem (PDF / Typst / Shunn already exist).
- **Git helper** — thin `inkhaven diff` / per-paragraph blame over the plain files.

### Track B — Stability
- **`inkhaven doctor`** — store fsck: dangling paragraph links, orphan nodes, dead
  event refs, DB↔disk `.typ` drift, system-tag integrity.
- **proptest harness** over the panic-prone parsers (calendar, HJSON layering,
  Typst assembly, language detection).
- **criterion benchmarks + enforced perf budgets** (1k-paragraph / 1k-event
  fixtures).
- **project lock + session/crash recovery** (multi-instance safety).
- **error-message quality pass**; golden/snapshot tests for rendering & exports.

## Release sequence

| Ver | Headline | Track A payload | Track B rider |
|---|---|---|---|
| **1.3.32** | **Project doctor** | — | `inkhaven doctor` fsck **+** stand up proptest/criterion harness; first calendar+HJSON property tests |
| **1.3.33** | **Command palette** | `Ctrl+P` palette + `?` keybinding overlay | command/keymap registry proptests; CLI-dispatch error pass |
| **1.3.34** | **Output, filtered** | Output filters + saved filters + counts | Output-store query bench @1k msgs; golden Output-render tests |
| **1.3.35** | **One review pass** | `inkhaven check` + chord + tree badges | full-project check perf budget; cross-checker integration test |
| **1.3.36** | **Cost in one place** | unified cost dashboard | cap-enforcement + daily-tally integrity tests; no-provider degrade |
| **1.3.37** | **Writing goals** | word-count goals/streaks + snapshot browser/diff | word-count/manuscript golden math; large-tree perf |
| **1.3.38** | **Hardening** | _(small)_ git diff/blame helper | **the sweep:** parser fuzzing, bench regression gates, project lock + crash/session recovery |
| **1.3.39** | **EPUB** | EPUB export | export round-trip tests; doctor validates exports |
| → | **cut 1.4.0** | | |

## Ordering rationale

- **Doctor + harness first (1.3.32)** — a data-integrity net plus the test/bench
  spine is the cheapest insurance; latent store bugs are found before six more
  releases pile on top.
- **Filtering (34) before `check` (35)** — a consolidated findings view is only
  usable once the pane can filter by provenance.
- **Palette early (33)** — independent, high daily value; builds the command
  registry the overlay and `check` discoverability reuse.
- **Hardening sweep last (38)** — re-runs fuzz/bench across the now-larger surface
  and adds locking/recovery once the feature shape is final.
- **EPUB (39)** — the one genuinely new subsystem and the marquee milestone feature;
  nothing else depends on it, so it sits last with room to polish.

## Definition of done (the 1.4.0 cut criteria)

- `doctor` reports clean on the dogfood project; no known data-loss path (project
  lock + crash/session recovery in place).
- All four examined-authorship systems reachable from **one** `check` and **one**
  Output filter model.
- One cost view, one command palette.
- Fuzz + bench suites green; perf budgets enforced as gates.
- EPUB export round-trips a representative book.
- `KEYBINDING.md` regenerated; tutorials + release notes current.

## Status

- **1.3.32** — _in progress_ (doctor + harness).
</content>
