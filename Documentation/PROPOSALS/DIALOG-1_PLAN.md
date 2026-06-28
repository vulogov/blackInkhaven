# DIALOG-1 — Dialogue Quality and Attribution Engine

| | |
|---|---|
| **RFC** | DIALOG-1 |
| **Title** | Language-aware dialogue detection; zero-attribution / said-bookism / talking-head findings; per-character dialogue fingerprint; `theatergoer` Inner Socrates persona; CLI + Bund parity |
| **Status** | In progress — 1.4.14 |
| **Author** | Vladimir Ulogov |
| **Depends on** | NARR-1 (1.4.12) — imports `crate::prose::ProseLanguage` + the modal word lists |
| **New dependency** | none |

Dialogue is the most technically demanding prose mode. DIALOG-1 adds a
deterministic, zero-AI, no-new-crate dialogue detector (5 languages, three
quotation conventions), three editorial findings (zero-attribution,
said-bookism density, talking-head sequences), a per-character dialogue
fingerprint in the story bible, a `theatergoer` Slow-track persona, and full
CLI + Bund parity — all backed by `.inkhaven/dialogue.duckdb`.

## Audit corrections (RFC vs reality — verified against the tree)

The RFC carried the usual cluster of fabricated/stale infra claims; corrected
before implementing:

- **Target 1.4.13 → 1.4.14.** 1.4.13 (OUTLINE-1) already shipped this cycle.
  Test baseline "2,026" is wrong; real current = **2005**, so +81 → ~2086.
- **"voice" namespace is fabricated — NARR-1 is `prose`.** Verified: `ink.prose.*`
  (no `ink.voice.*`), `.inkhaven/prose.duckdb` (no `voice.duckdb`), finding
  category `"prose"` / `kinds::PROSE_DRIFT` (no `VoiceFinding`, no `voice`
  category). "voice" is reserved for TTS. DIALOG-1 mirrors the **`prose`**
  module: `src/dialogue/`, `ink.dialogue.*`, `category:"dialogue"`,
  `kinds::DIALOGUE_*`. (Fixes RFC §9.2, §11.1, §12, §15.)
- **NARR-1 import API names wrong.** No `MODAL_VERBS[lang]` / `scan_phrases()`;
  the real surface is `prose::lexicon::CompiledLexicon` (`modal_unigrams/
  bigrams/trigrams`) via `for_language_with`. `ProseLanguage` is `pub(crate)`
  with `En/De/Es/Fr/Ru/Other(String)` — §4.4 dispatch is correct.
- **Story bible has no "tabs."** It's `Modal::StoryBible{rows:Vec<BibleRow>,
  cursor}` — a flat scrollable list. The fingerprint renders as additional
  `BibleRow`s under each character, not a tab. Its chord is `Ctrl+B Shift+L`
  (not `Ctrl+V Shift+L`). (Fixes RFC §6.3.)
- **`Ctrl+V D` is taken** (`d`=AiContinuationDraft, `Shift+d`=ViewThreadDoctor).
  New chord: **`Ctrl+V Shift+Q`** (Q = Quote — dialogue view), the only free
  Ctrl+V slot with a mnemonic. (Fixes RFC §6.4, §16.)
- **No "deep-refresh scheduler with a category selector."** The Output pane has
  a *filter*, not a per-category recompute trigger. DIALOG-1 follows NARR-1's
  model: its own `BgJobKind::DialogueCheck` + an idle-tick spawn + an explicit
  engage chord. (Fixes RFC §9.1 trigger 3, §11.1.)
- **No synchronous on-save fast-checker.** NARR-1 stays off the hot path
  (idle/ambient + explicit engage, zero on-save cost). DIALOG-1 does the same —
  no per-save synchronous detection. (Fixes RFC §9.1 trigger 2.)
- **No existing "character mention index."** Character *names* come from the
  Characters system book (`SYSTEM_TAG_CHARACTERS`) + `continuity_bible.rs`;
  attribution scans paragraph text against that name set (new code, not a
  reuse). (Fixes RFC §6.2/§6.4/§15.)
- **Per-feature `.inkhaven/dialogue.duckdb`** is consistent — `prose.duckdb`
  set this precedent (the RFC's "established pattern" is now actually true).

## Phases

| Phase | Content |
|---|---|
| D-P0 | `src/dialogue/` scaffold: types (`DialogueConvention`/`AttributionConfidence`/`TagVerbClass`/`DialogueSpan`/`CharacterDialogueFingerprint`), `dialogue_convention(&ProseLanguage)`, neutral + said-bookism verb lists (5 langs, const slices + lookup) |
| D-P1 | Convention detectors A (QuotePair) / B (GuillemetsAndDash) / C (Hybrid) + FR inline-tag stripper |
| D-P2 | Attribution cascade (Certain/Inferred/None) over Characters-book names |
| D-P3 | `dialogue.duckdb` schema + `DialogueStore` (mirror `ProseStore`) |
| D-P4 | Chapter detection pipeline + metrics (zero-attribution, said-bookism density, talking-head) + finding emit |
| D-P5 | Per-character fingerprint builder (incremental) |
| D-P6 | TUI: `BgJobKind::DialogueCheck` + idle/explicit engage + Output findings + story-bible rows + `Ctrl+V Shift+Q` |
| D-P7 | CLI `inkhaven dialogue scan\|profile\|refresh\|suggest` |
| D-P8 | Bund `ink.dialogue.{stats,fingerprint,violations,spans,refresh}` + policy |
| D-P9 | `theatergoer` Slow-track persona (15th) |
| D-P10 | `dialogue:` config block + docs (KEYBINDING, CONFIGURATION, tutorial 97) |

## Non-goals (per RFC §3)

Coreference resolution; subtext scoring; dialogue-act classification;
cross-chapter voice consistency (DIALOG-2); sentiment.

## Target

+81 tests (2005 → ~2086). No new runtime crates. No new system books. One new
`.inkhaven/dialogue.duckdb`. One new Inner Socrates persona. One new chord
(`Ctrl+V Shift+Q`).
