# NARR-1 — Narrative Voice Profiling

| | |
|---|---|
| **RFC** | NARR-1 |
| **Title** | Deterministic, multilingual narrative-voice metrics (`inkhaven prose`) |
| **Status** | Shipped — 1.4.12 |
| **Author** | Vladimir Ulogov |
| **Depends on** | none (reuses existing infrastructure) |
| **New runtime crates** | none |
| **Languages** | EN, RU, DE, FR, ES (+ `Other` → Tier-1 rhythm only) |

## What shipped

`inkhaven prose` produces a deterministic, chapter-ordered **voice fingerprint**
for every book — sentence rhythm, lexical diversity, epistemic hedging,
interiority, sensory balance, passive ratio — **with no LLM, no parser, and no
new dependency**. It measures; it never prescribes (every finding is `info`).

### Decisions taken at implementation (vs. the original draft)

- **Namespace = `prose`, not `voice`.** `voice` already names the TTS
  speech-synthesis subsystem; `inkhaven prose` / `ink.prose.*` avoids the clash.
  (The `ProseLanguage` enum was already aptly named.)
- **Storage = a new `.inkhaven/prose.duckdb`** (the user's choice). The original
  RFC's "established per-feature DuckDB isolation pattern" did **not** exist —
  no `facts.duckdb`/`sources.duckdb`/… ever existed; computed-scan data is
  normally a hash-invalidated `.inkhaven/*.json` sidecar (`book_digest`,
  `drift`). NARR-1 stands up the **first** per-feature DuckDB file, built on the
  shared pooled `StorageEngine`; numeric columns are TEXT (the project's robust
  DuckDB pattern), `text_hash` is a `DefaultHasher` u64 (no xxhash crate).
- **Scheduling = the `start_bg_job` harness + on-demand**, not the "deep-refresh
  scheduler" (which is the LLM world-refresh — NARR-1 is zero-AI). The TUI check
  is content-hash lazy, so only edited chapters recompute.

### Audit fixes (the RFC's own math/regex were wrong)

- **Burstiness B** is described as order-sensitive and independent of CV, but
  with σ/μ over the length distribution `B = (CV−1)/(CV+1)` — a monotone
  transform of CV. Shipped per the schema, doc-commented; a real order metric is
  deferred.
- The RFC's **French passive regex** omitted the `-it` participle family
  (`construit`/`détruit`) + `-eint/-aint/-oint`; added so real FR passives match.
- `sentence_split()` is actually `continuity::split_sentences` (deliberately
  naive); NARR-1 ships its own multilingual, abbreviation/ellipsis/dialogue-aware
  splitter rather than regress the shared one.

## Architecture (`src/prose/`)

- **`mod.rs`** — `ProseLanguage` + resolution chain (`prose.language` → project
  language → EN+note); `SensoryChannel`; `CompiledLexicon` (HashSet/Vec lookups,
  `for_language_with` folds in `prose.extra_*`, leaked to `'static`); tokenizer.
- **`lexicon.rs`** — per-language curated word lists (modal uni/bi/trigram, FID
  interiority phrases, DE erlebte-Rede particles, sensory word→channel, passive
  exceptions) for all five languages.
- **`segment.rs`** — multilingual sentence splitter.
- **`metrics.rs`** — Tier-1 (percentiles, CV, burstiness, MATTR).
- **`lang_metrics.rs`** — modal density, interiority (+ DE particle density),
  sensory balance. **`passive.rs`** — per-language passive detection (regex).
- **`profile.rs`** — `VoiceProfile`/`VoiceTier2`/`VoiceScope` + `compute_profile_with`.
- **`store.rs`** — `ProseStore` over `.inkhaven/prose.duckdb`.
- **`pipeline.rs`** — chapter extraction (Jinja excluded, structural/Typst
  stripped) + `refresh_book` (content-hash lazy, language-change invalidation).
- **`violations.rs`** — threshold crossings vs baseline + `emit_violation`.

## Surfaces

- **CLI** — `prose profile [--deep] [--json] [--language]`, `prose refresh`,
  `prose drift [--mode] [--reference]`, `prose suggest`.
- **TUI** — `Ctrl+V V` engage (bg job → Output `prose` findings), `Ctrl+V Shift+V`
  ambient (off, `prose.ambient_cooldown_secs` floor).
- **Bund** — read-only `ink.prose.{profile,drift,violations,refresh}`.
- **Config** — `prose:` block (deep_metrics, mattr_window, baseline_chapter,
  language, thresholds, extra_modal_tokens, extra_interiority_phrases, ambient,
  ambient_cooldown_secs).

## Future (deferred)

NARR-2 global voice calibration; a genuine order/memory rhythm metric; FR/ES
subjunctive/conditionnel via a zero-dep morphological analyser; CHAR-1 /
DIALOG-1 reusing `ProseLanguage`. Tests 1952 → 1996.

See [PROSE_VOICE.md](../PROSE_VOICE.md) and
[Tutorial 95](../Tutorials/95-narrative-voice.md).
