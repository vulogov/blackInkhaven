# AUDIENCE-1 — Nonfiction reader personas (grounded plan, 1.4.6)

Four bundled nonfiction Inner-Socrates personas (skeptical-practitioner,
domain-newcomer, expert-reviewer, end-user), a genre-aware Socratic system
prompt, nonfiction arms in the Inner Editor genre-fragment table, and a
project-default-persona config key. **No new crates, no new DB tables, no new
modules.** The full RFC is the design intent; this records the grounding against
the real 1.4.6-dev tree + the phase map.

## Grounding — verified against the tree (corrections to the RFC)

| RFC claim | Verified reality | Note |
|---|---|---|
| `bundled()` returns 5 personas via a `p(…)` helper | ✓ `personas.rs:57-114`; helper `p(id,name,summary,notes,emph:&[(Category,f32)])` | Add 4 `p(…)` → 9 total. Absent categories default to `1.0` (`emphasis_for`), so mutes **must** set `0.0` explicitly. |
| `SLOW_SYSTEM` const hardcodes "fiction manuscript" at `slow.rs:17` | ✓ — but there are **two** `SLOW_SYSTEM` consts. The other (`world/fact_check_slow.rs:18`) is the **world fact-checker** and is **out of scope**. | Only `inner_socrates/slow.rs` changes. |
| "Two uses of `SLOW_SYSTEM` in `cli/inner_socrates.rs` (slow + timeline)" | ✗ **One** use (`cli/inner_socrates.rs:292`). The `cli/realworld.rs` uses import the *world* const, not ours. No test references the Socratic const directly. | Single call-site change. |
| `genre` key consumed by `genre_fragment()` | ✓ `Config.genre: Option<String>` (`config.rs:68`); `genre_fragment()` (`inner_editor/prompt.rs:180`) has **10** fiction arms. | Add 6 nonfiction arms → 16. |
| `inner_socrates_daily_call_cap` adjacency for the new config field | ✓ `config.rs:3299` (`= 150` default at `:3309`). | Add `inner_socrates_default_persona: Option<String>` adjacently. |
| "Update `personas::active()` to take `cfg` — update callers" | `active(project)` has **9 call sites** (scripting ×2, app.rs ×5, cli ×2), several without a `Config` in scope. | **Deviation:** instead of threading `cfg` through 9 sites, `active()` loads the project config **internally** in the `None` branch (signature + all callers unchanged). Strictly less invasive, equivalent behaviour. |
| Category enum | ✓ 15 members = 7 Fast + 8 Slow (5 prose + 3 timeline: DramatizationGap, ImplicationTracing, TemporalDensity). Slow prose pass emits only the 5 prose ids; `apply_persona_and_ledger` filters all findings by `emphasis ≤ 0`. | Muting DramatizationGap/TemporalDensity/UnattributedDialogue via `0.0` works wherever a finding originates. |
| Test baseline 1879 (1.4.3) | ✗ Now **1897** (1.4.5 shipped). | AUDIENCE-1 target ≥ +25 → **≥ 1922**. |

`load_all()` three-tier resolution, `by_id()`, `socratic_cycle_persona()`, the
persona wizard, and the `Ctrl+B J` chord family pick up the 4 new bundled
personas with **no change** — verified.

## Locked decisions (from the RFC, confirmed)

4 nonfiction personas mute DramatizationGap/TemporalDensity/UnattributedDialogue
· genre-aware `slow_system(genre)` replaces the const · nonfiction
`genre_fragment` arms (same key set, Editor-craft text) · project-default
persona via config (per-book routing is AUDIENCE-2) · 5 fiction personas
unchanged · Category enum unchanged.

## Phase map

- **A-P0 — Four bundled nonfiction personas (pure).** Add the 4 `p(…)` to
  `bundled()` with the character-sheet emphasis maps (mutes at `0.0`). Tests:
  9 distinct personas; all 4 nonfiction mute DramatizationGap + TemporalDensity
  + UnattributedDialogue; emphasis values match sheets; `by_id` resolves all 9.
- **A-P1 — Genre-aware `slow_system()`.** Replace `const SLOW_SYSTEM` with
  `pub fn slow_system(genre: Option<&str>) -> String` + `pub fn
  slow_genre_context(genre) -> Option<&'static str>`. Update the one call site
  (`cli/inner_socrates.rs:292`). Tests: None → neutral (no "fiction"); technical
  → "technical document"; fantasy → "fantasy"; unknown → neutral; all known
  genres distinct non-empty.
- **A-P2 — Nonfiction `genre_fragment`.** Add 6 arms (nonfiction/technical/
  documentation/academic/science/business) to `genre_fragment()`. Tests: each
  new key → `Some(…)` with a craft word; unknown still `None`; fiction coverage
  unchanged.
- **A-P3 — `inner_socrates_default_persona` config + overview hint.** Add the
  `Option<String>` field; `active()` consults it (internal config load) when the
  DB has no active persona; overview status shows "(project default from
  config)" when the active id came from config, not an explicit set. Tests:
  no-DB + config default → default; no-DB + no config → inner-socrates; explicit
  set beats config.
- **A-P4 — CLI + docs.** Confirm `inner-socrates persona list/set` surface the
  4 new personas; KEYBINDING (`Ctrl+B J → S` = 9 personas), CONFIGURATION
  (`inner_socrates_default_persona`), Tutorial 90. → **cut 1.4.6**.

## Non-goals

Per-book active-persona DB persistence (AUDIENCE-2) · new Fast detectors /
LLM output categories for nonfiction (AUDIENCE-2) · rewriting the 5 fiction
personas · removing DramatizationGap/TemporalDensity from the enum. No new
runtime crates; no new DuckDB tables (A-P3 adds one `Option<String>` field).

Test baseline 1897 (1.4.5); target ≥ ~1922.
