# RFC LANG-3 — Local Translation Models for ConLangs

| | |
|---|---|
| **RFC** | LANG-3 |
| **Title** | Local Translation Models for ConLangs |
| **Status** | Draft |
| **Created** | 2026-06-20 |
| **Author** | Vladimir Ulogov |
| **Target version** | 1.5.0 |
| **Depends on** | LANG-1 (ConLang Development Suite) |
| **Supersedes** | none |
| **Companion RFCs** | PDF-1 (book imposition), LANG-1 (conlang suite), LANG-2 (sociolinguistics & contact) |

> **Numbering note.** This RFC was authored as "LANG-2"; the LANG-2 code was
> already in use for the Sociolinguistics & Contact track (dialects, borrowing,
> areal features, speech communities — shipped 1.3.22), so the translation track
> is filed here as **LANG-3**. Content is otherwise the author's original.

> **Status (1.3.23-dev): P0.1 — Tier 1 (RBMT) core landed.** The pure-Rust,
> offline, deterministic spine is in `src/conlang/translate/` (`english.rs` a
> dependency-free English analyzer, `lexmap.rs` gloss→headword mapping,
> `mod.rs` the `translate()` orchestrator). It reuses the LANG-1 syntax engine
> (`syntax::assemble`) wholesale for word order / case / inflection / agreement,
> so a translation reorders and inflects for free. Surfaces: CLI
> `inkhaven language translate <lang> "<text>" [--trace] [--json]` and Bund
> `lang.translate` (`store_read`). Per-word confidence + decision trace;
> untranslatable words marked `«word»` and listed. Validated live (SVO→SOV
> reorder, pronoun person, plural, unknown-word handling). Zero new deps; tests
> 1548 → 1559. **Deliberate deviation from the RFC's P0:** the heavy neural
> DistilBERT English parser is deferred — the first cut uses a small rule-based
> analyzer (the RFC's documented fallback) behind the same `analyze` interface,
> keeping LANG-3 dependency-light and offline-testable until the ML tiers (P1+)
> are scoped.
>
> **P0.2 — `reverse` + `cross` landed.** `src/conlang/translate/reverse.rs`:
> `reverse` (conlang→English) un-inflects each surface word against the lexicon's
> paradigm-generated forms (a `ReverseIndex` over headwords + explicit inflections
> + every `paradigm::generate` form), reads roles off `word_order`, and generates
> a plain English clause; `cross` (conlang A→B) reverses A then forwards into B
> through the English pivot (exposed; confidence = product of the two passes).
> CLI `language reverse <lang> "<surface>"` and `language cross <from> <to>
> "<surface>"`; Bund `lang.reverse` / `lang.cross` (`store_read`). Validated live
> (Eldar SVO `kira nami pata` → English → Sindar SOV `turi moki vela`). tests
> 1559 → 1563.
>
> **P0.3 — richer parsing + agreement landed; ★ P0 (Tier 1) complete.**
> Source parsing is now **lexicon-aware**: a word's POS comes from the lexicon, so
> the verb is found by meaning (not position) and an attributive **adjective** is
> recovered on a noun phrase (`the bright bird` → `mira kira`, adjective agreeing
> with its noun). Number uses the `sg`/`pl` paradigm convention, so forward
> **inflects** (`the birds see the stones` → `kirai nami patai`) and `reverse`
> **re-agrees** English number and tense (plural nouns pluralized, a singular
> subject takes the 3sg verb). Full round-trip verified live. `english::prepare`
> + `GlossIndex::has_sense` + `structure()` drive the lexicon-aware pass, with the
> positional analyzer kept as the all-unknown fallback. tests 1563 → 1565.
>
> **Deferred to the neural tier (not Tier-1 RBMT work):** subordinate clauses and
> multi-word phrases beyond a single adjective need real dependency parsing — they
> arrive when the neural POS+dep parser swaps in behind `english`'s interface. The
> **routing/merge layer** is a no-op while only Tier 1 exists and is introduced
> with Tier 2 (P1+), when there is a second tier to route between. **Superseded
> for the neural specifics by Amendment A1 below — see it before reading §3.2 /
> §8.4 / §8.6 / §11.**

---

## Amendment A1 — Retrieval-first Tier 2 (Python-free, training-free)

| | |
|---|---|
| **Created** | 2026-06-21 |
| **Author** | Vladimir Ulogov |
| **Decision** | Adopt **Option A** — Tier 2 is *retrieval-augmented RBMT*, not a fine-tuned NMT |
| **Supersedes** | §3.2 (Tier-2 goals), §8.4 (fine-tuned NMT), §8.4.3 (Python training), §8.6 (correction loop = retrain), and the candle/NLLB lines of §11 |
| **Status** | accepted; P1 in progress on 1.3.23-dev |

### Why

The original Tier 2 reached for Python (PyTorch + `transformers` + `peft`) only
because it assumed Tier 2 must be a **fine-tuned** model — and fine-tuning needs a
training loop. Two facts make that assumption unnecessary:

1. **A conlang is a closed, author-controlled, synthetic domain.** The synthetic
   corpus the RBMT manufactures *is* the knowledge. Baking it into adapter
   weights is one way to use it; **retrieving from it at translation time** is
   another, and for a closed domain the second is both simpler and more faithful.
2. **The retrieval stack already ships in the binary.** `fastembed` (multilingual
   embeddings), `hnsw_rs` / `vecstore` (the `VectorEngine`), `tokenizers`,
   `safetensors`, and `ort` (ONNX Runtime, via fastembed) are all already direct
   or transitive dependencies. A retrieval Tier 2 adds **zero new crates**.

So Tier 2 becomes a **semantic translation memory layered over the Tier-1 RBMT
spine** (the well-known kNN-MT / TM-augmented-MT design). Python and the entire
training stack leave the critical path.

### The re-architecture

- **Datastore.** Each synthetic `(English → conlang)` pair is stored with the
  English embedded — exactly the `VectorEngine.store_documents_batch` /
  `search` shape already used for manuscript semantic search.
- **At translation time.** Embed the English input; pull the *k* nearest stored
  examples. An **exact / near-exact** hit is translation memory (return the
  remembered conlang directly). A **partial** hit supplies word-choice / idiom /
  phrasing exemplars that steer the RBMT. A **miss** falls back to pure RBMT. The
  merge policy is the one design-critical piece (it is what TM systems already
  do); output gains an `alternatives` list so the author always sees the
  candidates and chooses.
- **Correction loop = `store_document(...)`.** A correction is *appended to the
  datastore* and takes effect on the very next sentence — strictly better than
  the original design, which batched 100 corrections and a training run to
  achieve less. "Refresh" becomes optional datastore compaction/dedup, never a
  training run.
- **The shippable artifact** (RFC §8.9) becomes the **datastore bundle** (vectors
  + targets + the language data) — smaller than an adapter, and regenerable.

### What stays from the original RFC

Tier 1 RBMT (done, P0); the optional **Tier 3** resolver (reuses `src/ai/` —
already proven by LANG-2 `propose-dialect`); the **evaluation harness** (§8.8 —
round-trip similarity, grammar pass-rate, author-acceptance all still apply, and
round-trip now reuses `reverse`); **cross-conlang** (§8.7, done); the
**grammar-constrained** check (§8.4.6 — reuses the RBMT inflected-form set, now in
the merge step instead of at decode).

### Reuse map

| Need | Reused existing code | New |
|---|---|---|
| RBMT spine | `src/conlang/translate/` (P0) | — |
| Datastore | `src/storage/vector.rs` `VectorEngine` | a TM collection + schema |
| Embeddings | `src/storage/embedding.rs` `EmbeddingEngine` | — |
| Async corpus build | bg-job harness (`src/tui/app.rs`) | corpus generator (RBMT over a pool) |
| Tier 3 (optional) | `src/ai/` `AiClient` + `collect_blocking` | — |
| Tables | DuckDB store | corpus / correction / memory tables |

### Revised phase plan

- **P1 — corpus + translation memory (no deps, no downloads).** ✅ *Landed
  1.3.23-dev.* The translation-memory datastore (`memory.rs`: exact + lexical-fuzzy
  matching; `.inkhaven/` sidecar), the retrieve→merge policy (`apply_memory`, with
  an `alternatives`-bearing output), the correction primitive (`language remember`
  → reused on the next call, no retrain; `language memory` lists), and the
  synthetic-corpus generator (`corpus.rs`: RBMT over a **bundled** English pool —
  `assets/conlang/english-pool-v1.txt`, no download — gated on full lexicon
  coverage; `language corpus [--pool] [--yes]` seeds memory and reports acceptance
  rate + top-missing words as a lexicon-maturity signal). Tests through 1572.
- **P2 — semantic retrieval + pane cutover.** *Semantic retrieval landed
  1.3.23-dev:* the strategy now lives in `memory::TranslationMemory::best`
  (exact → semantic → lexical); each remembered English source is embedded once
  via the in-tree `fastembed` (cached in the sidecar) and a translation embeds
  only the query, matching by cosine (threshold 0.82). `apply_memory` takes an
  optional query embedding; the CLI/Bund supply it from `Store::embed_batch` when
  the memory is non-empty (exact hits skip it). Validated live (a lexical
  paraphrase recalled a remembered line at 92%). Semantic hits stay *advisory*
  (alternatives, never silent overrides). **Remaining P2:** route the TUI
  translate pane through the pipeline, and the eval harness (round-trip
  similarity via `reverse` + `fastembed`, grammar pass-rate, acceptance rate).
- **P3 — correction loop + export + cross.** Correction = datastore append
  (immediate); `.itm`-style datastore bundle export; cross-conlang already lands
  in P0.
- **P-opt (deferred, optional) — neural fluency.** *Only if* RBMT + retrieval
  proves insufficient on real conlangs: a base-NLLB run via the in-tree `ort`
  (no training, no new crate, a hosted pre-exported ONNX asset so the user never
  touches Python), and/or candle-native LoRA training (the only path that adds
  candle crates). These are upgrades on the same corpus + datastore spine, not
  requirements — so the heavy-deps decision is deferred until there is evidence
  it is worth it.

**Net effect:** the LANG-3 core ships **Python-free, training-free, with zero new
dependencies and no required model downloads**, fully offline, reusing the
embedding + vector + background-job + AI infrastructure already in the binary.

---

## 1. Summary

Add a three-tier hybrid translation stack to Inkhaven that produces a
persistent, local, per-language text-to-text translation model from a
LANG-1 conlang description. The stack consists of:

- **Tier 1 — RBMT spine.** A pure-Rust rule-based machine-translation
  engine driven by the conlang's phonology, morphology, grammar tags, and
  lexicon. Always-on, deterministic, sub-millisecond. Available the
  moment a language has any lexicon at all.

- **Tier 2 — Per-language fine-tuned NMT.** A small neural model
  (NLLB-200-distilled-600M base + LoRA adapter per language, quantized to
  Q4_K_M GGUF, served via `candle`) trained on a synthetic corpus that
  the RBMT manufactures by translating a bundled English source pool.
  Replaces Tier 1 as the default once trained. Improves over time as
  author corrections accumulate.

- **Tier 3 — Optional tiny-LLM resolver.** A small RAG-style pass over
  lexicon, idioms, and metaphor inventory, used for ambiguity and
  register-sensitive cases when Tier 2 confidence is low. Optional
  per-language; off by default.

The marquee contribution is the **RBMT-bootstrapped synthetic
backtranslation pipeline**: the formal grammar that LANG-1 already stores
becomes an active data generator, producing the parallel corpus that
conlangs structurally lack. The result is that an author who finishes
their language in Inkhaven also finishes with a real translation system
they can ship alongside their published work.

## 2. Motivation

LANG-1 §8.18 ships an interactive translation pane that asks an external
LLM (whatever provider the user has configured) to translate token by
token using the lexicon and grammar as RAG context. It works well for
the daily-use case --- an author drafting prose --- but has three
structural limitations:

1. **It depends on an external LLM.** Inkhaven's single-binary, offline-
   capable design is broken at the moment a provider is required.
2. **It is not a persistent artifact.** Each session restarts from a
   prompt; nothing accumulates inside the project.
3. **It does not learn from corrections.** The author's overrides
   disappear when the session ends; the model behaves identically the
   next time the same English appears.

LANG-3 fixes all three. Translation becomes:

- **Always available** without a network connection or external
  account, because Tier 1 (RBMT) ships in the binary and Tier 2 (small
  NMT model) lives in the project.
- **A real artifact** that snapshots, backs up, exports, and ships with
  the published work.
- **Cumulative** --- every accepted or corrected translation in the
  editor becomes a training pair that the next refresh incorporates.

The reason this is feasible for conlangs (where it is not feasible for
arbitrary natural languages) is that **a LANG-1 conlang has a complete
formal description**. The phoneme inventory, the phonotactic templates,
the morphology spec, the typological grammar tags, the lexicon with
senses and registers, the metaphor inventory --- all of these are
machine-readable, normalized, and queryable. For a natural language,
extracting these features from corpora is the central research problem;
for a LANG-1 conlang, they are the source of truth. The whole pipeline
hinges on exploiting that.

## 3. Goals

Goals are grouped by tier and by the cross-cutting infrastructure they
share.

### 3.1 Tier 1 — RBMT spine

1. **Pure-Rust English-to-conlang translation** using only the LANG-1
   data model. No external API. No GPU.
2. **English source-side parsing**: POS tagging and dependency parsing
   sufficient to drive word-order reordering and lemma-to-conlang
   mapping. Bundled as a one-time-download asset shared across all
   conlangs in the project.
3. **Word-sense disambiguation** by heuristic (POS, surrounding lemmas)
   and by semantic-feature match (using the LANG-1 semantic-feature
   tags on lexicon entries).
4. **Lemma-to-conlang mapping** by lexicon lookup with fallback to
   morphologically derived forms and finally to AI-proposed coinages
   (which are queued for review rather than committed).
5. **Reordering** driven by LANG-1's typological feature tags (word
   order, adjective placement, possessive position, etc.).
6. **Morphological inflection** by running the lemma through the
   LANG-1 morphology spec for the inferred grammatical features.
7. **Phonological post-processing** via the LANG-1 allophony, stress,
   and tone evaluators.
8. **Romanization** via the language's default romanization scheme.
9. **Confidence and trace output**: each translation carries a
   per-token confidence score and a structured trace recording which
   rule made which decision. Used by the pane and by Tier 2 fallback.
10. **Sub-millisecond latency** on short sentences.

### 3.2 Tier 2 — per-language fine-tuned NMT

11. **Synthetic-corpus generation** by running RBMT over a bundled
    English source pool, filtering for lexicon coverage and grammatical
    validity. Default target size: 100K parallel pairs.
12. **Bundled source pool**: a curated subset of Tatoeba
    (~500K sentences, ~50MB compressed, CC-BY licensed), pre-tokenized
    and deduplicated, shipped as an Inkhaven asset.
13. **Per-language LoRA adapters** over a shared base model
    (NLLB-200-distilled-600M), rank 16, alpha 32. Adapter size ~20MB.
14. **Quantized inference** via candle: base model Q4_K_M GGUF
    (~150MB), adapters loaded on demand into an LRU cache.
15. **Sub-500ms latency** on short sentences on CPU; sub-50ms on
    consumer GPU.
16. **Grammar-constrained decoding** (optional flag) that masks the
    token distribution against a per-language whitelist, catching the
    rare hallucination.
17. **Training-in-Rust where possible, training-in-Python as escape
    hatch.** The candle training stack is improving; for P1 we train
    via Python (PyTorch + transformers + peft) and convert the
    resulting LoRA adapter weights to candle's format. As candle
    training matures, the loop moves in-process.

### 3.3 Tier 3 — tiny-LLM resolver (optional)

18. **Low-confidence ambiguity resolution** using a small local LLM
    (Phi-3 mini, Llama 3.2 1B, or Qwen 2.5 1.5B; user choice) with
    RAG context built from the relevant lexicon senses, idioms, and
    metaphor entries.
19. **Off by default**, opt-in per language via configuration.

### 3.4 Cross-cutting

20. **Correction loop**: every author override or correction in the
    translation pane is logged as a training pair.
21. **Refresh policy**: when a threshold of corrections accumulates,
    or on explicit command, a background training run incorporates
    them into the adapter via a new LoRA snapshot.
22. **Versioned adapters**: each refresh produces a numbered adapter
    snapshot, integrated with Inkhaven's snapshot system; rollback
    works on adapters as it does on prose.
23. **Cross-conlang translation** for projects with multiple
    conlangs: a shared encoder pass plus per-language adapters allows
    A→B translation without dedicated A↔B training.
24. **Evaluation harness**: round-trip translation similarity, grammar
    pass rate, and author-acceptance rate, all computable without a
    human reference corpus.
25. **Export of trained models** so an author can ship a translation
    system with the published work --- bundled as a single file with
    inference code.
26. **Single binary** (apart from the optional model assets). The
    inference engine and the bundled English parser are in-binary;
    base models and source pools are downloadable assets cached in
    `~/.inkhaven/` like fonts.
27. **Three surfaces**: CLI subcommand tree, TUI integration
    (extends the LANG-1 translation pane `Ctrl+B U T`), and
    `ink.conlang.translate.*` Bund stdlib.
28. **Backwards compatible**: the LANG-1 translation pane interface
    is preserved; internally it now routes through this pipeline
    rather than calling out to an external LLM. Authors who prefer
    the external-LLM path can configure that as the active backend
    explicitly.

## 4. Non-goals

- **Speech-to-text or text-to-speech for conlangs.** Inkhaven does not
  synthesize audio of the conlang, even though the phonology is fully
  specified. Pure-Rust phoneme synthesis is not at usable quality.
- **Translation between two arbitrary natural languages.** The tool is
  English-to-conlang and conlang-to-English. Adding non-English source
  or target languages is feasible (NLLB is multilingual) but out of
  scope for LANG-3.
- **Real-time streaming translation** during a live phone call or
  video conference. The pipeline is request-response.
- **Training a model from scratch** for a conlang. The bootstrap
  always starts from a pretrained multilingual base model.
- **Federated or multi-user training.** All training is local to one
  machine; corrections are project-local.
- **OCR of handwritten conlang.** Different feature; out of scope.
- **Direct deployment as a web service.** The trained model can be
  exported and a user can serve it via their own infrastructure, but
  Inkhaven itself does not host translation endpoints.
- **Conlang-to-conlang translation via direct supervision.** Cross-
  conlang translation goes through the English semantic representation
  (the shared encoder); direct A↔B pairs are not trained.

## 5. Constraints

- **Single binary** for the translation logic itself. Models are
  downloaded assets, not bundled in the binary (the binary stays
  small; users opt in to model download).
- **Pure Rust** for inference, RBMT, source-pool indexing, corpus
  generation, and the correction loop. Training may use Python in P1;
  P3 documents the candle-native training plan.
- **No external service dependency at inference time.** Once a model
  is trained and on disk, no network is required.
- **LANG-1 is a hard dependency.** This RFC presupposes the full LANG-1
  data model (phonology, morphology spec, grammar tags, lexicon with
  senses + features + registers, idioms, metaphor inventory). LANG-3
  cannot be implemented without LANG-1.
- **Storage budget.** Bundled English parser plus base model plus
  source pool total ~260 MB on first install. Each additional trained
  conlang adds ~50 MB (adapter + cached corpus). All cached files are
  re-derivable, so users can delete and regenerate.
- **License compliance.** Tatoeba corpus is CC-BY; attribution shipped
  in `assets/THIRD_PARTY.md`. NLLB-200-distilled-600M is CC-BY-NC 4.0;
  documented as a non-commercial bundled asset, with a commercial-use
  warning at first activation and an `--alternative-base` option for
  users with commercial requirements (mT5-small, Apache 2.0, used as
  the documented commercial-friendly substitute).
- **Bund sandbox respect.** No new capability categories; reuse the
  existing `fs_read` / `fs_write` / `net` (for asset download) /
  `code_eval` set.
- **No mutation without consent.** Every Tier 2 output flows through
  the same author-review machinery as LANG-1 §8.10 proposals. The
  trained model is a *suggestion engine*; the author decides what is
  canonical.

## 6. Audience

**Primary**: Inkhaven users with one or more conlangs of meaningful
size (50+ lexicon entries) who want a translation system that lives
with their project and improves with use. Typical user: a fantasy
novelist with a finished conlang who wants to translate dialogue
quickly while drafting, share a translation system with readers, and
not depend on an external LLM provider.

**Secondary**: Inkhaven users with small conlangs (under 50 entries)
who benefit from Tier 1 (RBMT) only. Tier 2 training is skipped or
deferred; the same translation pane works with the deterministic
spine alone.

**Tertiary**: Conlang enthusiasts publishing in non-Inkhaven contexts
(podcasts, fan-fiction, games) who want the exported translation
artifact as a deliverable.

**Out of scope as audience**: production-quality natural-language
translation users (DeepL, Google Translate territory), real-time
interpretation users, low-resource natural-language MT researchers
needing reproducibility infrastructure.

## 7. Design overview

### 7.1 The three-tier pipeline

Every translation request flows through a routing layer that chooses
which tier(s) to invoke and merges their outputs:

```
                       English input
                              │
                              ▼
                  ┌──────────────────────┐
                  │  Routing layer       │
                  │  - look up language  │
                  │  - check model state │
                  │  - choose tier(s)    │
                  └──────────┬───────────┘
                             │
        ┌────────────────────┼────────────────────┐
        ▼                    ▼                    ▼
  ┌──────────┐         ┌──────────┐         ┌──────────┐
  │  Tier 1  │         │  Tier 2  │         │  Tier 3  │
  │  RBMT    │         │  NMT     │         │  Resolver│
  │  always  │         │  if      │         │  if      │
  │  on      │         │  trained │         │  enabled │
  └─────┬────┘         └─────┬────┘         └─────┬────┘
        │                    │                    │
        └────────────────────┼────────────────────┘
                             ▼
                  ┌──────────────────────┐
                  │  Merge + confidence  │
                  │  - primary output    │
                  │  - alternatives      │
                  │  - per-token trace   │
                  └──────────┬───────────┘
                             │
                             ▼
                       Translation
                       (text + meta)
```

**Routing logic.** The tier choice depends on language state:

- *No Tier 2 model exists:* Tier 1 is the primary; Tier 3 is consulted
  for low-confidence tokens if enabled.
- *Tier 2 model exists:* Tier 2 is the primary; Tier 1 runs in parallel
  as a sanity check; major divergence (BLEU < threshold against Tier 1)
  triggers a warning and surfaces Tier 1 as an alternative.
- *Tier 2 confidence is below threshold and Tier 3 is enabled:* Tier 3
  resolves; Tier 2's output and Tier 1's output are passed as RAG
  context.

The merge layer never silently picks; if the tiers disagree
substantively, the output flags the ambiguity and surfaces both as
alternatives the author chooses between.

### 7.2 Module layout

```
src/conlang/translate/
    mod.rs                          -- public API, types
    routing.rs                      -- tier choice logic
    merge.rs                        -- merge tier outputs
    pipeline.rs                     -- orchestrates a translation

    rbmt/
        mod.rs                      -- RBMT entry point
        parser.rs                   -- English POS + dep parsing
        disambiguate.rs             -- word-sense disambiguation
        lexicon_map.rs              -- English lemma → conlang lemma
        reorder.rs                  -- typological-tag-driven reordering
        inflect.rs                  -- apply morphology spec
        phonology_apply.rs          -- allophony + stress + tone
        confidence.rs               -- per-step confidence accumulation
        trace.rs                    -- structured rule-decision log

    corpus/
        mod.rs
        source_pool.rs              -- access to bundled English pool
        generator.rs                -- run RBMT over pool, filter
        filter.rs                   -- CorpusFilter (coverage, grammar)
        storage.rs                  -- corpus on disk + DuckDB index
        stats.rs                    -- inspect a generated corpus

    nmt/
        mod.rs
        base_model.rs               -- NLLB load / quantize / cache
        adapter.rs                  -- LoRA load / save / metadata
        inference.rs                -- candle generation loop
        constrained_decode.rs       -- grammar mask at decode time
        cross.rs                    -- cross-conlang via shared encoder
        training_offline.rs         -- Python training driver (P1)
        training_native.rs          -- candle training (P3+)
        eval.rs                     -- evaluation harness

    resolver/
        mod.rs
        tiny_llm.rs                 -- local LLM load (Phi-3 / Llama-3.2)
        rag.rs                      -- RAG context assembly from lexicon

    correction/
        mod.rs
        capture.rs                  -- hook into translation pane
        store.rs                    -- correction storage in DuckDB
        trigger.rs                  -- when to schedule a refresh
        scheduler.rs                -- background training trigger

    export/
        mod.rs
        package.rs                  -- bundle adapter + tokenizer + readme
        runtime.rs                  -- minimal standalone inference code

    asset/
        mod.rs                      -- manage downloaded base models
        parser_asset.rs             -- the bundled English parser
        pool_asset.rs               -- Tatoeba bundle
        download.rs                 -- progress + integrity check

    cli.rs                          -- translate subcommands
    tui.rs                          -- extends Ctrl+B U T pane
    bund.rs                         -- ink.conlang.translate.* stdlib
```

### 7.3 Data model

Translation introduces six new DuckDB tables (full schema in §16
Appendix A) plus filesystem artifacts:

**DuckDB tables**:

- `translation_models` --- one row per (language, model) pair, recording
  adapter file path, base model identifier, training epoch count, the
  corpus version trained on, the timestamp, and the active flag.
- `translation_corpora` --- one row per generated synthetic corpus,
  recording size, coverage statistics, source-pool revision, RBMT
  version, on-disk path.
- `translation_corrections` --- one row per author override, with the
  English source, the system output, the corrected output, the
  timestamp, and a weight.
- `translation_alternatives` --- when the merge layer surfaces multiple
  candidates, this records which the author picked.
- `translation_inferences` --- optional audit log of recent
  translations (capped at 10k rows; for debugging only).
- `translation_eval_runs` --- one row per evaluation pass, recording
  scores and the corpus that was scored.

**Filesystem artifacts** (under `assets/conlang/<lang_id>/translate/`):

- `corpus/<version>.tsv` --- synthetic parallel corpus, TSV format.
- `adapter/<version>.safetensors` --- LoRA adapter weights.
- `adapter/<version>.json` --- adapter metadata (rank, alpha, target
  modules, training config).
- `eval/<version>.json` --- evaluation report.

Base models and source pools are shared and live under
`~/.inkhaven/assets/`, not per-project.

### 7.4 Surfaces

Three concurrent entry points, all routed through `pipeline.rs`:

- **CLI**: `inkhaven conlang translate <subcommand>` --- full operation
  tree for scripting and batch.
- **TUI**: the existing `Ctrl+B U T` translation pane (LANG-1 §8.18)
  is extended with model status and training controls; new pane
  sub-keys for corpus generation, training kick-off, and evaluation.
- **Bund**: `ink.conlang.translate.*` for scripted release pipelines,
  per-language refresh policies, and automated evaluation.

## 8. Detailed design

### 8.1 The `Translation` value

```rust
pub struct Translation {
    pub source: String,             // English input
    pub target: String,             // conlang output (primary)
    pub language_id: LanguageId,
    pub alternatives: Vec<Alternative>,
    pub confidence: f32,            // 0.0..=1.0
    pub trace: Vec<TraceEntry>,     // per-token decision log
    pub tier_used: TierMix,         // which tier produced what
    pub elapsed_ms: u32,
}

pub struct Alternative {
    pub text: String,
    pub source_tier: Tier,
    pub confidence: f32,
    pub rationale: String,          // why this is an alternative
}

pub enum Tier { Rbmt, Nmt, Resolver }

pub struct TierMix {
    pub primary: Tier,
    pub consulted: Vec<Tier>,
}

pub struct TraceEntry {
    pub source_token: String,
    pub target_token: String,
    pub decision: Decision,
    pub confidence: f32,
}

pub enum Decision {
    LexiconLookup { entry_id: EntryId, sense_index: usize },
    MorphologicalInflection { paradigm_cell: ParadigmCellId },
    PhonologicalChange { rule_id: RuleId },
    AmbiguityResolved { picked_from: Vec<String>, by: String },
    Coined { rationale: String },   // not in lexicon; queued for review
    Inherited { from_tier: Tier },
}
```

The trace is what makes the system debuggable. Every output is
inspectable: the author can click a word in the pane and see which
lexicon entry, which paradigm cell, and which sound rule produced it.

### 8.2 Tier 1 — RBMT

The deterministic spine. Six stages, executed in order:

#### 8.2.1 English parsing

A small BERT-based POS + dependency parser, bundled as a candle-format
asset (~60 MB on first activation). The parser produces:

```rust
pub struct EnglishParse {
    pub tokens: Vec<Token>,
    pub pos: Vec<PosTag>,
    pub dependencies: Vec<Dependency>,
    pub lemmas: Vec<String>,
    pub features: Vec<MorphologicalFeatures>,  // tense, number, person, etc.
    pub clauses: Vec<ClauseStructure>,
}
```

**Candidate parsers** (P0 decision):
- `rust-bert`'s POS+NER pipeline. Large (~250 MB) but well-tested.
- A DistilBERT POS tagger via candle, trained on Universal
  Dependencies English. ~60 MB, comparable accuracy on news/prose text.
- A Brill-style rule-based tagger (~5 MB) with lower quality on
  out-of-distribution input.

**P0 chooses** the candle DistilBERT POS+dep parser as the bundled
asset, with the rust-bert option as a fallback users can install
explicitly via `inkhaven conlang translate parser install rust-bert`.

#### 8.2.2 Word-sense disambiguation

For each lemma, pick which sense of which lexicon entry is meant. Three
strategies cascading:

1. **POS-narrowing.** If a lemma has senses across multiple parts of
   speech, eliminate POS mismatches with the parsed tag.
2. **Semantic-feature match.** For each remaining candidate, score by
   how many of its declared semantic features match features inferable
   from the syntactic context (subject of "drink" suggests `[+liquid]`,
   etc.).
3. **Tier-3 query (if enabled).** Ambiguous after 1+2: hand to the
   resolver.

When tier 3 is disabled and steps 1+2 leave ambiguity, the disambiguator
picks the highest-frequency sense in manuscript usage (data from LANG-1
§8.23 lexicon usage statistics).

#### 8.2.3 Lexicon mapping

Each disambiguated English lemma maps to a conlang lemma:

```rust
pub fn map_lemma(en: &str, sense: SenseSelector, lang: &Language)
    -> LemmaMapping;

pub enum LemmaMapping {
    Direct(EntryId),
    Derived { from: EntryId, derivation: DerivationRuleId },
    Compound { components: Vec<EntryId> },
    Idiomatic { idiom_id: IdiomId, original_offset: usize },
    Coined { proposed_form: String, rationale: String },
    Untranslatable { reason: String },
}
```

The mapping consults the lexicon, then the morphology spec for
productive derivation, then the idiom inventory for multi-word matches,
then a coining proposal if all else fails. Coined forms generate a
LANG-1 §8.10 proposal queue entry --- they are never silently committed.

#### 8.2.4 Reordering

Walk the dependency tree; emit conlang tokens in the order dictated by
the grammar tags. The mapping from English dependency-tree order to
conlang surface order is a function of the grammar's typological
features:

- `WordOrder_SVO_OSV_etc` chooses subject/verb/object placement.
- `AdjectivePosition_PreNominal_PostNominal` controls where adjectives
  go relative to their noun.
- `PossessivePosition` controls genitive constructions.
- `NegationStrategy`, `QuestionFormation`, and others handle
  clause-level operations.

Each reordering rule is a small declarative function over the parse
tree. The full set (~30 rules) corresponds 1:1 to the WALS features
that LANG-1 stores.

#### 8.2.5 Inflection

For each conlang lemma in the reordered sequence, apply the morphology
spec for the relevant grammatical features:

```rust
pub fn inflect(
    lemma: &LexiconEntry,
    features: &MorphologicalFeatures,
    morphology: &Morphology,
) -> Result<String>;
```

This calls into the existing LANG-1 `morphology::paradigm` machinery.
Feature mismatches (e.g., English present tense in a conlang that
doesn't grammaticalize tense) are silently dropped with a trace entry.

#### 8.2.6 Phonological post-processing and romanization

Apply allophony rules, compute stress and tone, then romanize using the
language's default scheme. All LANG-1 §8.3--8.6 functions.

### 8.3 Synthetic corpus generation

The bridge from Tier 1 to Tier 2. Five components.

#### 8.3.1 The source pool

A bundled, pre-tokenized, deduplicated subset of Tatoeba:

```
assets/conlang/translate/source_pool/
    pool_v1.parquet                 -- 500K English sentences
    pool_v1.manifest.hjson          -- counts, license, hashes
    pool_v1.coverage_index.parquet  -- inverted index of common lemmas
```

Selection criteria for the bundled subset:
- Sentence length 4–30 tokens (short enough to translate cleanly,
  long enough to be informative).
- Vocabulary in the most-frequent 50K English lemmas.
- Diverse topics (filtered against a topic-distribution target).
- Deduplicated by 5-gram overlap.

**Format**: Parquet for fast scan-and-filter operations; ~50 MB
compressed.

**License**: CC-BY 4.0; attribution shipped in `THIRD_PARTY.md`.

**Download**: lazy, on first invocation of corpus generation. Cached at
`~/.inkhaven/assets/conlang/translate/source_pool/`. Verified by SHA-256
against a manifest checked into the Inkhaven repo.

#### 8.3.2 The generator

```rust
pub struct CorpusGenerator {
    pool: SourcePool,
    rbmt: Rbmt,
    filter: CorpusFilter,
    target_size: usize,
    parallelism: usize,
}

impl CorpusGenerator {
    pub fn generate(&self, lang: &Language) -> Result<SyntheticCorpus> {
        let mut accepted: Vec<(String, String)> = Vec::new();
        let scanner = self.pool.scan_shuffled(self.lang_seed(lang));
        for batch in scanner.batches(1024) {
            let translations = batch
                .par_iter()
                .map(|en| (en.clone(), self.rbmt.translate(en, lang)))
                .filter(|(en, t)| self.filter.accept(en, t))
                .map(|(en, t)| (en, t.target))
                .collect::<Vec<_>>();
            accepted.extend(translations);
            if accepted.len() >= self.target_size { break; }
        }
        Ok(SyntheticCorpus::new(accepted, lang.id, self.metadata()))
    }
}
```

Rayon-parallelized across English source sentences; RBMT is stateless
and per-sentence so this scales linearly with cores.

#### 8.3.3 The corpus filter

```rust
pub struct CorpusFilter {
    pub min_lexicon_coverage: f32,    // every content lemma must resolve
    pub max_coining_rate: f32,        // <5% coined forms acceptable
    pub max_inflection_failures: f32, // <2% failed inflections
    pub require_grammar_valid: bool,  // output must round-trip parse
    pub require_phonotactic_valid: bool, // surface form passes constraints
    pub diversity_window: usize,      // no near-duplicates within N
}
```

A typical filter accepts 10–30% of source sentences; the rest are
dropped silently. Acceptance rate is reported in the corpus metadata
so the author knows whether their language is mature enough to bootstrap
a useful corpus.

For a target of 100K pairs, expect to scan 300K–1M source sentences.
On a modern laptop (16 cores), generation runs at ~5K sentences/sec
through RBMT, so a full 100K corpus generates in 1–5 minutes.

#### 8.3.4 Stats and inspection

```
inkhaven conlang translate corpus stats <lang>
```

Reports: total size, acceptance rate, average source length, average
target length, lexicon coverage distribution, top 20 most-used lexicon
entries, top 20 most-frequently-failed source sentences. The last is
diagnostic: it tells the author what their language *can't* translate,
which often points to gaps in the lexicon or the grammar.

#### 8.3.5 Versioning

Each generated corpus is identified by a hash of:
- The source pool version.
- The RBMT version (changes when LANG-3 RBMT code changes).
- The language's `Language` value hash (changes when phonology,
  morphology, lexicon, or grammar changes).
- The filter configuration.

This identity ties a trained adapter unambiguously to the corpus that
produced it. Refresh detection compares the current language hash to
the corpus's recorded hash; mismatch flags the model as stale.

### 8.4 Tier 2 — fine-tuned NMT

The fluency layer. Six components.

#### 8.4.1 Base model

**NLLB-200-distilled-600M** is the recommended base. Reasons:

- Designed for low-resource MT; the architecture is fitted to the task.
- Already multilingual (200 languages); the encoder side has rich
  semantic representations that transfer to conlang outputs.
- The "X-eng" and "eng-X" task identifiers extend naturally to a new
  language code.
- Distilled-600M is the smallest variant; quantized to Q4_K_M GGUF it
  runs in ~150 MB.

**Commercial-friendly alternative**: **mT5-small** (300M params,
Apache 2.0). Lower quality on MT benchmarks but no commercial licensing
constraints. Selected via `--base mt5-small` on training.

**File format**: GGUF for quantized inference. Conversion from the
original safetensors checkpoints uses a documented one-time script in
P1; in P2+ we ship pre-quantized checkpoints as assets.

**Asset path**: `~/.inkhaven/assets/conlang/translate/base/nllb_600m_q4.gguf`.

**Download**: lazy on first training or first inference. ~150 MB; about
2 minutes on a residential broadband connection.

#### 8.4.2 LoRA adapter format

```rust
pub struct LoraAdapter {
    pub language_id: LanguageId,
    pub base_model: BaseModelKind,
    pub version: u32,
    pub trained_on_corpus: CorpusHash,
    pub training_config: TrainingConfig,
    pub created_at: DateTime,
    pub weights: HashMap<String, Tensor>,  // layer name → low-rank pair
}

pub struct TrainingConfig {
    pub rank: u32,                    // 16
    pub alpha: f32,                   // 32
    pub target_modules: Vec<String>,  // ["q_proj", "v_proj", ...]
    pub epochs: u32,
    pub learning_rate: f32,
    pub batch_size: u32,
    pub correction_data_mix: f32,     // 0.0..=1.0
}
```

Stored as a directory:

```
adapter/v3/
    weights.safetensors     -- low-rank matrices, ~20 MB
    metadata.json           -- config + provenance
    tokenizer_extension.json -- new language code added to NLLB tokenizer
```

#### 8.4.3 Training driver (P1, Python)

A documented Python script under `tools/train_translation.py`:

```
python tools/train_translation.py \
    --corpus assets/conlang/qya/translate/corpus/v5.tsv \
    --base nllb_600m \
    --lang qya \
    --rank 16 --alpha 32 \
    --epochs 3 \
    --output assets/conlang/qya/translate/adapter/v5/
```

Driven by `transformers` + `peft`. Reads the TSV, sets up the LoRA,
trains, writes the adapter directory in the format above.

The script is invoked by Inkhaven via `std::process::Command` *only*
when the user explicitly opts in (the operation prints a warning that
Python is being invoked and exits with an error if Python is not on
PATH). This is the one place LANG-3 violates the single-binary
constraint and we accept it for P1 with a path to P3 elimination.

**Training cost**: ~2 hours on a single A4000 / RTX 3060 with 100K
parallel pairs, 3 epochs. ~12 hours on CPU. Memory: ~10 GB GPU, ~16 GB
RAM CPU.

#### 8.4.4 Training driver (P3, candle)

When candle's training stack matures sufficiently (specifically: when
LoRA-on-frozen-base with cross-entropy is well-supported), the loop
moves in-process. The interface from outside is unchanged:

```rust
ink.conlang.translate.train <lang>
```

just no longer shells out. P3 documents the migration explicitly.

#### 8.4.5 Inference

```rust
pub struct InferenceEngine {
    base: Arc<QuantizedModel>,
    adapter_cache: LruCache<LanguageId, Arc<LoraAdapter>>,
    tokenizer: NLLBTokenizer,
}

impl InferenceEngine {
    pub fn translate(
        &mut self,
        en: &str,
        lang: LanguageId,
        opts: InferenceOptions,
    ) -> Result<NmtResult> {
        let adapter = self.adapter_cache.get_or_load(lang, &self.base)?;
        let prompt = self.tokenizer.encode_translation(en, lang.iso_code());
        let constrained = if opts.grammar_constrained {
            Some(self.build_constraint_mask(lang)?)
        } else { None };
        let logits_stream = self.base.generate_with_adapter(
            prompt, adapter, opts.into_decoding_config(), constrained,
        );
        let target = self.tokenizer.decode_until_eos(logits_stream)?;
        Ok(NmtResult {
            target,
            confidence: ...,
            alternatives: ...,
        })
    }
}
```

The LRU cache holds up to 4 adapters resident (configurable). Swapping
an adapter takes ~100 ms; this is amortized over a session.

**Latency targets** (NLLB-600M Q4 on consumer CPU, short sentences,
beam=4):
- Adapter cache hit: 200–400 ms.
- Adapter cache miss (load): +100 ms.
- Grammar-constrained decoding: +20–50 ms.

GPU latency is 5–10× faster; not assumed.

#### 8.4.6 Grammar-constrained decoding

Optional decoding mode that enforces only valid conlang surface tokens.
Implementation:

```rust
pub fn build_constraint_mask(lang: &Language) -> Result<DecodeMask> {
    let mut allowed = HashSet::new();
    // All inflected forms of all lexicon entries
    for entry in lang.lexicon.iter() {
        for paradigm_cell in entry.paradigm_cells(lang) {
            let surface = inflect(entry, paradigm_cell.features, &lang.morphology)?;
            allowed.insert(self.tokenize(&surface));
        }
    }
    // Plus punctuation, numerals, special tokens
    allowed.extend(self.special_tokens());
    Ok(DecodeMask::from(allowed))
}
```

The mask is built once per session per language and cached. The decoder
applies it at each step by setting logits of non-allowed tokens to
-infinity before softmax.

This is the same machinery as JSON-schema-constrained generation, just
with a per-language token whitelist. It catches the rare hallucination
(where the NMT proposes a conlang-shaped word that isn't actually in
the lexicon) at the cost of slight decoding overhead.

**Trade-off**: constrained decoding can hurt fluency for genuinely
productive constructions (e.g., compounding) that aren't in the
pre-computed mask. Off by default; opt-in per language.

### 8.5 Tier 3 — resolver (optional)

A small local LLM consulted when Tier 2 confidence is low or when the
metaphor inventory suggests idiomatic substitution.

#### 8.5.1 Model choice

User picks one of:
- **Phi-3 mini (3.8B Q4)** --- best quality, ~2.5 GB. MIT license.
- **Llama 3.2 1B (Q4)** --- balanced, ~700 MB. Llama 3.2 community
  license.
- **Qwen 2.5 1.5B (Q4)** --- multilingual, ~1 GB. Apache 2.0.

All run via candle. Per-language opt-in:

```hjson
translate: {
    qya: {
        resolver: {
            enabled: true
            model: "qwen-2.5-1.5b-q4"
            confidence_threshold: 0.5
        }
    }
}
```

#### 8.5.2 RAG context

When invoked, the resolver receives:
- The English source.
- The Tier 1 and Tier 2 candidate outputs with their per-token confidence.
- Top-5 lexicon entries for each ambiguous source token (by semantic
  feature match).
- Any idioms whose component lemmas overlap with the source.
- Any metaphors whose source domain matches the input topic.

Prompt template lives in the LANG-1 Prompts system book, editable by
the user, with default supplied.

#### 8.5.3 When to call the resolver

Three triggers:
1. **Token-level Tier 2 confidence** below per-language threshold.
2. **Tier 1 ↔ Tier 2 disagreement** beyond a similarity threshold on
   the BLEU/chrF metric.
3. **Idiom match in the source**: when the input contains a phrase
   matching an entry in the source-language idiom database (a small
   bundled list of common English idioms), the resolver is invoked to
   handle the metaphor substitution explicitly.

### 8.6 Correction loop

The closing of the loop: editor corrections become training data.

#### 8.6.1 Capture

The LANG-1 translation pane (§8.18) already captures every override.
LANG-3 wraps that capture into a structured record:

```rust
pub struct UserCorrection {
    pub id: CorrectionId,
    pub language_id: LanguageId,
    pub source: String,
    pub system_output: String,
    pub corrected_output: String,
    pub tier_used: TierMix,
    pub confidence_at_output: f32,
    pub timestamp: DateTime,
    pub paragraph_context: Option<ParagraphId>,
    pub weight: f32,
}
```

The `weight` field defaults to 1.0; the author can mark a correction
as "high-confidence canonical" (weight 2.0) or "minor preference"
(weight 0.5) in the pane. The author can also retroactively delete a
correction if they change their mind.

#### 8.6.2 Refresh triggers

A refresh is the act of incorporating recent corrections into a new
adapter snapshot. Three triggers:

1. **Threshold-based** (default): after `correction_refresh_threshold`
   new corrections accumulate (default 100), the scheduler queues a
   refresh.
2. **Time-based**: opt-in `correction_refresh_interval` (default off)
   triggers a refresh every N days regardless of count.
3. **Explicit**: the user runs `inkhaven conlang translate refresh
   <lang>` or invokes Bund `ink.conlang.translate.refresh`.

The scheduler does not actually start training; it produces a notification
that a refresh is recommended, plus an estimated training time. The user
confirms; training then runs as a background job, with progress visible
in a status overlay (`Ctrl+B U T` shows it in the pane footer).

#### 8.6.3 Mix ratio

Each refresh trains on a mix of:
- Existing synthetic corpus (the foundation).
- New corrections (the personalization).

The mix ratio defaults to 80/20 synthetic/correction --- enough
personalization to matter, but with the synthetic base dominant to
prevent catastrophic forgetting. The ratio is adjustable per language.

When corrections accumulate enough to outnumber synthetic data, the
mix shifts gradually toward corrections (linear interpolation), but a
floor at 20% synthetic is maintained.

#### 8.6.4 Versioning and rollback

Each refresh produces a new adapter version. Old adapters are retained
under `adapter/vN/` for at least the last 3 versions, so the author can
roll back if a refresh makes things worse:

```
inkhaven conlang translate model rollback <lang> --to v4
```

The active version is tracked in `translation_models` (DuckDB).

### 8.7 Cross-conlang translation

For projects with multiple conlangs sharing a base model, A→B
translation goes through the shared encoder:

```rust
pub fn translate_cross(
    text: &str,
    from: LanguageId,
    to: LanguageId,
) -> Result<Translation> {
    // 1. From → English via reverse-direction Tier 2
    let en = self.reverse_translate(text, from)?;
    // 2. English → To via forward Tier 2
    self.forward_translate(&en, to)
}
```

This is a degraded path (two translation steps with cumulative error)
but it works without dedicated training. The English intermediate is
exposed in the trace so the author can see where meaning was lost.

For related languages (sister languages sharing a proto-language and
significant cognate vocabulary), a P3+ enhancement trains a small
"interlingua adapter" that bypasses the English intermediate. Out of
scope for P0–P2; documented as future work.

### 8.8 Evaluation harness

Conlangs have no human reference translations, so standard BLEU/chrF
against ground truth is impossible. Three substitutes, all of which
are reproducible from project data alone:

#### 8.8.1 Round-trip semantic similarity

Translate English → conlang → English. The two English strings should
be semantically similar:

```rust
pub fn roundtrip_score(en_source: &str, lang: &Language) -> f32 {
    let conlang = forward(en_source, lang);
    let en_recovered = reverse(&conlang, lang);
    embedding_similarity(en_source, &en_recovered)
}
```

Uses fastembed for the similarity computation. Score range 0..1; healthy
languages should score >0.7 on a held-out test set.

#### 8.8.2 Grammar pass rate

Translate, then check the output against the LANG-1 phonotactic and
morphological validators:

```rust
pub fn grammar_pass_rate(test_set: &[&str], lang: &Language) -> f32 {
    test_set
        .iter()
        .filter(|en| {
            let output = forward(en, lang);
            lang.validate_grammar(&output).is_ok()
        })
        .count() as f32 / test_set.len() as f32
}
```

A trained Tier 2 model should pass 95%+ of generated outputs. The
remainder are typically partial inflection failures or coining
proposals, both of which are diagnostically useful.

#### 8.8.3 Author-acceptance rate

The simplest and most honest metric: of all translation pane outputs,
how often did the author accept (Tab) versus override (type
something different)? Stored in `translation_alternatives` over time;
charted in the analysis dashboard.

A healthy trained model converges toward 70–85% acceptance after a
few refresh cycles.

#### 8.8.4 Eval CLI

```
inkhaven conlang translate eval <lang> [--metric roundtrip|grammar|all]
                                       [--test-set <path>]
                                       [--against-tier rbmt|nmt|both]
```

Run by the author manually after refresh, or scheduled by Bund. Results
saved to `eval/<version>.json` and surfaced in the model info pane.

### 8.9 Exporting a trained model

The artifact a user might want to ship with a published work:

```
inkhaven conlang translate model export <lang> --out <file>
```

Produces a single ZIP-like bundle:

```
qya-translation-v5.itm/    (Inkhaven Translation Model)
    manifest.hjson         -- language metadata, model version, license info
    base.gguf              -- quantized base model (or reference to download)
    adapter.safetensors    -- the LoRA adapter
    tokenizer.json         -- tokenizer including conlang-specific tokens
    lexicon.tsv            -- the lexicon as TSV (for human reference)
    runtime/
        README.md          -- how to invoke
        infer.py           -- minimal standalone Python inference script
        infer.rs           -- minimal standalone Rust inference example
```

The `runtime/` directory makes the bundle usable without Inkhaven ---
the author can publish the model as an asset on the book's website,
and a reader with Python or Rust can run translations against it.

**Bundle size**: ~170 MB if base is included; ~20 MB if base is
referenced for download. The author chooses on export.

**Licensing note in manifest**: clearly states the base model's
license (CC-BY-NC for NLLB; Apache 2.0 for mT5-small). The author is
responsible for compliance.

## 9. Bund stdlib

The `ink.conlang.translate.*` family. All words sandbox-gated under
the existing capability set.

| Word | Sandbox | Description |
|---|---|---|
| `ink.conlang.translate` | none | Translate English to conlang |
| `ink.conlang.translate.rbmt` | none | Force Tier 1 only |
| `ink.conlang.translate.nmt` | none | Force Tier 2 only (errors if no model) |
| `ink.conlang.translate.reverse` | none | Conlang to English |
| `ink.conlang.translate.cross` | none | Cross-conlang translation |
| `ink.conlang.translate.confidence` | none | Confidence of a translation |
| `ink.conlang.translate.trace` | none | Get the trace of a translation |
| `ink.conlang.translate.alternatives` | none | List alternative outputs |
| `ink.conlang.translate.corpus.generate` | `fs_write` | Build synthetic corpus |
| `ink.conlang.translate.corpus.stats` | none | Inspect a corpus |
| `ink.conlang.translate.corpus.size` | none | Corpus row count |
| `ink.conlang.translate.corpus.delete` | `fs_write` | Delete a corpus |
| `ink.conlang.translate.train` | `fs_write`+`code_eval` | Start training |
| `ink.conlang.translate.train.status` | none | Training progress |
| `ink.conlang.translate.train.cancel` | `code_eval` | Cancel running training |
| `ink.conlang.translate.refresh` | `fs_write`+`code_eval` | Incorporate corrections |
| `ink.conlang.translate.refresh.recommended` | none | True if refresh would help |
| `ink.conlang.translate.eval` | none | Run evaluation harness |
| `ink.conlang.translate.eval.results` | none | Last eval results |
| `ink.conlang.translate.correction.add` | `fs_write` | Manually log a correction |
| `ink.conlang.translate.correction.list` | none | List corrections |
| `ink.conlang.translate.correction.delete` | `fs_write` | Delete a correction |
| `ink.conlang.translate.correction.count` | none | Pending correction count |
| `ink.conlang.translate.model.exists` | none | Check for trained model |
| `ink.conlang.translate.model.info` | none | Model metadata |
| `ink.conlang.translate.model.list` | none | All trained models in project |
| `ink.conlang.translate.model.versions` | none | List adapter versions |
| `ink.conlang.translate.model.activate` | `fs_write` | Set active version |
| `ink.conlang.translate.model.rollback` | `fs_write` | Roll back to previous |
| `ink.conlang.translate.model.delete` | `fs_write` | Remove model + corpus |
| `ink.conlang.translate.model.export` | `fs_write` | Export as `.itm` bundle |
| `ink.conlang.translate.resolver.enable` | `fs_write` | Turn on Tier 3 |
| `ink.conlang.translate.resolver.disable` | `fs_write` | Turn off Tier 3 |
| `ink.conlang.translate.resolver.model` | `fs_write` | Set resolver model |
| `ink.conlang.translate.asset.download` | `net`+`fs_write` | Pull base or pool |
| `ink.conlang.translate.asset.verify` | none | SHA check assets |

New hooks:

- `hook.on_translate` --- fires after every translation (debounced).
- `hook.on_correction_add` --- fires when an author overrides output.
- `hook.on_train_complete` --- fires when a training run finishes.
- `hook.on_refresh_recommended` --- fires when threshold crossed.

These let advanced users wire automated release scripts: e.g., refresh
nightly, evaluate, commit the new adapter if scores improved.

## 10. Surfaces

### 10.1 CLI

```
inkhaven conlang translate <text> [--from en --to <lang>] [--tier rbmt|nmt|auto]
inkhaven conlang translate reverse <text> --lang <lang>
inkhaven conlang translate cross --from <a> --to <b> <text>

inkhaven conlang translate corpus generate <lang> [--target N]
inkhaven conlang translate corpus stats <lang>
inkhaven conlang translate corpus delete <lang>

inkhaven conlang translate train <lang> [--epochs N] [--base nllb|mt5]
inkhaven conlang translate train status [--lang <lang>]
inkhaven conlang translate train cancel [--lang <lang>]

inkhaven conlang translate refresh <lang>
inkhaven conlang translate refresh recommended

inkhaven conlang translate eval <lang> [--metric roundtrip|grammar|acceptance|all]

inkhaven conlang translate correction add ...
inkhaven conlang translate correction list <lang>
inkhaven conlang translate correction delete <id>

inkhaven conlang translate model list
inkhaven conlang translate model info <lang>
inkhaven conlang translate model versions <lang>
inkhaven conlang translate model activate <lang> --version <v>
inkhaven conlang translate model rollback <lang> [--to <v>]
inkhaven conlang translate model delete <lang>
inkhaven conlang translate model export <lang> [--out <file>] [--include-base]

inkhaven conlang translate resolver enable <lang> --model <m>
inkhaven conlang translate resolver disable <lang>

inkhaven conlang translate asset download [base|pool|parser|resolver-<m>]
inkhaven conlang translate asset list
inkhaven conlang translate asset verify

inkhaven conlang translate parser install [distilbert|rust-bert]
```

### 10.2 TUI

The LANG-1 `Ctrl+B U T` translation pane is extended with:

- **Model status footer**: shows which tier produced the output,
  confidence, and trained-model version (if any).
- **Tier override toggle** (key `t`): cycle primary tier across RBMT,
  NMT, AUTO.
- **Model menu** (key `m`): submenu for corpus generation, training
  status, refresh, and rollback.
- **Trace inspector** (key `v`): selecting a target token shows the
  rule-decision trace for that token in a side panel.
- **Alternatives panel** (key `a`): shows all alternative outputs from
  consulted tiers, with one-key selection.
- **Eval status** (key `e`): shows the latest evaluation report.

The pane keyboard map is documented inline at the bottom of the pane
itself; LANG-1's existing keys (Tab to accept, Esc to reject, arrows
to step) are unchanged.

### 10.3 Book-Take integration

The book-take pipeline gains:

```hjson
book: {
    take: {
        formats: [pdf, epub, latex, grammar_book_pdf, translation_model]
        translation_model_languages: ["qya"]
    }
}
```

`Ctrl+B O` can now produce the manuscript PDF, grammar book, and an
exported translation model bundle in one chord. Useful for authors
shipping a self-contained "language pack" alongside their book.

## 11. Dependency selection

New direct dependencies (all pure Rust unless noted):

| Crate | Purpose | License | Pure Rust |
|---|---|---|---|
| `candle-core` | Tensor library for inference | Apache-2.0 / MIT | Yes |
| `candle-nn` | Neural network primitives | Apache-2.0 / MIT | Yes |
| `candle-transformers` | NLLB / mT5 / Phi-3 / Qwen / Llama | Apache-2.0 / MIT | Yes |
| `tokenizers` | HF-compatible tokenization | Apache-2.0 | Yes |
| `safetensors` | Adapter weight format | Apache-2.0 | Yes |
| `parquet` | Source pool storage | Apache-2.0 | Yes |
| `rayon` | Parallel corpus generation | MIT / Apache-2.0 | Yes (already in tree) |

Already in `Cargo.toml` and reused:

- `fastembed` --- for round-trip semantic similarity in eval.
- `duckdb` --- corpus + correction + model tables.
- `serde-hjson` --- model metadata.
- `regex` --- tokenization fallbacks.
- `lopdf` --- exported eval reports as PDF.
- `aho-corasick` --- idiom detection in source text.

Bundled assets (downloaded lazily, not in binary):

- DistilBERT English POS+dep model: ~60 MB.
- Tatoeba source pool (Parquet, filtered): ~50 MB.
- NLLB-200-distilled-600M Q4 GGUF: ~150 MB.
- mT5-small Q4 GGUF (alternative): ~80 MB.
- Resolver models (opt-in): ~700 MB to ~2.5 GB.

**Not used**:

- PyTorch / transformers in the binary --- only invoked via subprocess
  during P1 training (escape hatch, eliminated in P3).
- onnxruntime --- candle is the chosen runtime; ONNX adds a native dep.
- ggml directly --- candle's quantized inference covers the same ground
  in pure Rust.
- llama.cpp wrappers --- same reasoning.

## 12. Implementation phases

**P0 — Tier 1 (RBMT) (6 weeks).**
- `conlang::translate::rbmt` module complete.
- English parser asset pipeline (DistilBERT via candle).
- Lemma mapping, disambiguation, reordering, inflection, post-processing.
- Confidence and trace machinery.
- Pipeline routing layer (just Tier 1 for now).
- Translation pane backend wiring (LANG-1 §8.18 routes through this).
- CLI: `translate`, `reverse`, `cross` (cross uses two Tier 1 passes).
- Bund: `ink.conlang.translate`, `ink.conlang.translate.rbmt`,
  `ink.conlang.translate.confidence`, `ink.conlang.translate.trace`,
  `ink.conlang.translate.parser.install`.
- Tests: golden corpus of English → conlang pairs for fixture languages;
  trace validation; latency benchmarks.

**P1 — Corpus + offline training (4 weeks).**
- `conlang::translate::corpus` complete.
- Tatoeba bundle build script + asset hosting.
- `conlang::translate::nmt::training_offline` (Python driver,
  documented).
- CLI: corpus subcommands, `train`, `train status`.
- Bund: `corpus.*`, `train.*`, `model.*` (info, list).
- Documentation: `Documentation/TRANSLATION_TRAINING.md` covering the
  Python toolchain requirements.
- Tests: corpus generation on fixture languages; deterministic seed
  reproducibility.

**P2 — Tier 2 inference + cutover (5 weeks).**
- `conlang::translate::nmt::inference` (candle).
- Quantized model loading, LoRA adapter loading and caching.
- Grammar-constrained decoding.
- Pipeline routing: Tier 2 becomes primary when trained, Tier 1 as
  fallback / sanity check.
- Translation pane: model status footer, tier override, alternatives
  panel.
- CLI: tier-specific flags, `model activate`, `model rollback`,
  `model delete`.
- Bund: full surface available.
- Evaluation harness: `eval` subcommand with all three metrics.
- Asset download manager: progress, integrity check, resume.
- Tests: end-to-end on three fixture languages; latency on CPU and
  GPU; quantization quality regression.

**P3 — Correction loop + cross-conlang transfer (3 weeks).**
- Correction capture from translation pane.
- `correction::trigger`, `correction::scheduler` for refresh decisions.
- Refresh CLI + Bund + TUI integration.
- Adapter versioning, rollback.
- Cross-conlang translation (English-pivot).
- Export `.itm` bundle format with runtime examples.
- Hooks: `hook.on_translate`, `hook.on_correction_add`,
  `hook.on_train_complete`, `hook.on_refresh_recommended`.
- Tier 3 resolver implementation.
- Tests: full refresh cycle; rollback; cross-conlang on
  proto-language-related fixtures; export bundle round-trips.

**P4 — Polish (2 weeks).**
- `inkhaven conlang translate tutorial` interactive walkthrough.
- Documentation: `Documentation/TRANSLATION.md` user guide.
- Performance pass on adapter cache + inference loop.
- Asset CDN setup or GitHub Release hosting for base models.

**Total: ~20 weeks (~5 months) for one developer.**

P0 alone delivers immediate value: an RBMT translation system that
works the day after LANG-1 lands. P1+P2 add the trained-model
capability. P3 closes the loop and unlocks the publication story.

## 13. Testing strategy

- **Unit tests** on every deterministic RBMT component: lemma mapping,
  disambiguation, reorder, inflect, phonological apply, confidence
  accumulation.
- **Property tests** on the routing layer: tier choice respects
  configuration; merge layer never silently drops alternatives;
  confidence is monotonic in known cases.
- **Golden translation tests**: for each of 3 fixture languages
  (Quenya-like, Klingon-like, Japanese-like), 100 English source
  sentences with expected RBMT outputs. Diffs checked structurally,
  not character-equal.
- **Corpus generation**: deterministic seed produces identical
  corpus; coverage statistics within expected ranges.
- **Adapter format round-trip**: train a tiny model, save, reload,
  inference output identical to bytes.
- **Latency benchmarks**: median and p99 for short, medium, and long
  sentences; tracked in CI as regression gates.
- **Quantization quality**: F32 reference inference vs Q4 inference;
  BLEU divergence flagged above threshold.
- **Cross-conlang**: Quenya → Sindarin via English; semantic similarity
  preserved above threshold.
- **Refresh determinism**: same corrections, same corpus, same seed,
  same adapter weights.
- **Asset integrity**: SHA verification fails detect corruption.
- **Export round-trip**: exported `.itm` bundle, decompressed, loaded
  in a standalone process, produces identical inference to in-Inkhaven.
- **Resolver fallback**: when resolver disabled, low-confidence outputs
  still surface alternatives; when enabled, resolver outputs replace
  alternatives without dropping them.

## 14. Risks and alternatives

**Risk: candle's training stack is not mature enough in P3.** Mitigated
by the Python escape hatch for P1; if candle hasn't caught up by P3,
we keep the Python driver and ship a "training requires Python"
documentation note. The user-facing CLI doesn't change.

**Risk: NLLB-600M license (CC-BY-NC) limits commercial users.** Mitigated
by the `mT5-small` alternative under Apache 2.0. First-launch warning
about commercial vs. non-commercial use, surfaced both on download and
on export.

**Risk: synthetic corpus quality bounds Tier 2 quality.** This is the
foundational empirical question. Mitigations: (a) the corpus filter
rejects low-coverage outputs, so we don't train on garbage; (b) the
correction loop progressively improves the model beyond the RBMT
baseline; (c) the evaluation harness catches regressions early. If
real-world quality is disappointing, fall back to "Tier 1 + Tier 3" as
the primary pipeline and treat Tier 2 as optional polish.

**Risk: catastrophic forgetting from corrections drift.** Mitigated by
the 80/20 mix ratio with a 20% synthetic floor, and by versioned
adapters that allow rollback. Eval-harness regressions trigger a
rollback recommendation automatically.

**Risk: English parser misidentifies a verb.** Every downstream stage
is then wrong. Mitigations: parser quality benchmarked against
Universal Dependencies test set in CI; trace makes parser errors
visible to the author; rust-bert option for users who want a heavier
parser.

**Risk: storage scaling for many conlangs.** A user with 20 conlangs
would consume ~1 GB just for adapters. Mitigations: lazy training
(adapters only created when an author crosses a usage threshold);
clear deletion path; corpus is regeneratable so caching is opt-in.

**Risk: quantization breaks rare-token handling.** Conlang vocab is
exactly rare tokens. Mitigation: the constrained-decoding option
guarantees only valid forms emit; quantization quality is gated in CI
with a held-out test set; users can opt out of quantization at the
cost of larger models.

**Risk: download dependencies make the pipeline less robust.** A user
without network can't bootstrap. Mitigation: all required assets cached
locally after first download; manual asset paths (`--asset-path
<dir>`) for fully offline use; the Tier 1 RBMT works without any of
these downloads, so the pipeline degrades gracefully.

**Alternative considered: skip Tier 2 entirely and go RBMT + RAG-LLM
(approach 5 from the research).** Simpler, smaller footprint, but the
trained Tier 2 model is what gives the system its distinctive value
(persistence, learning from corrections, export as artifact). RAG
alone doesn't accumulate. Rejected.

**Alternative considered: train the entire base model from scratch
rather than fine-tune via LoRA.** Higher quality ceiling, intractable
training cost. Rejected.

**Alternative considered: distill Tier 2 into a fully Tier-1-shaped
RBMT (effectively learning new rules from the trained model).** Tempting
because it would produce an editable rules artifact. Out of scope for
LANG-3; revisit if there's a research path that makes it tractable.

## 15. Open questions

1. **English parser quality benchmarking.** Need to confirm DistilBERT
   POS+dep is good enough on author-prose (which differs from news
   text in style). Run UD test set + a manual prose sample.

2. **mT5-small vs NLLB-600M quality gap.** Empirical question to settle
   in P1: do both produce usable Tier 2 quality, or is one clearly
   better? May influence the commercial-friendly path's perception.

3. **Resolver default model.** Which to recommend? Phi-3 mini is best
   quality but largest; Qwen 2.5 1.5B is balanced; Llama 3.2 1B is
   smallest. Probably user choice with Qwen 2.5 as recommended default.

4. **Corpus refresh policy on lexicon edits.** When the author adds 50
   new words, should the corpus auto-regenerate? Probably no by
   default (too noisy); the stale-model warning surfaces in the model
   info pane.

5. **Cross-conlang adapter sharing.** For sister languages (Quenya and
   Sindarin both derived from Proto-Eldarin), would a shared
   "Eldarin-family" adapter outperform two independent adapters?
   Research direction; out of scope for P3.

6. **Author-acceptance metric integrity.** What if the author accepts
   bad outputs because they're tired? The metric is noisy. Mitigation:
   eval harness reports all three metrics, never just acceptance.

7. **Adapter export licensing.** When the user exports a `.itm` bundle
   containing an NLLB-derived adapter, are they distributing a
   derivative work? Legally murky for CC-BY-NC. Recommendation:
   manifest explicitly states licensing; default `--include-base` is
   off (the bundle references the base by hash for download), so the
   distribution is just the adapter, which is the user's own.

8. **Editor pane backwards compatibility.** Users who configured an
   external LLM in LANG-1's translation pane: do we silently route
   through LANG-3's pipeline, or preserve the external-LLM path?
   Recommendation: preserve, with a config flag
   `translate.backend = local|external` defaulting to `local`.

9. **Confidence calibration.** Tier 2's softmax probabilities are
   notoriously miscalibrated. Should we run temperature scaling?
   Probably yes; add as a post-training step.

10. **Long-document handling.** NLLB has a 512-token context limit. For
    paragraph-length translations (which the pane supports), we need
    chunking with overlap. Document the chunking strategy in P2.

## 16. Appendices

### A. Full DuckDB schema

```sql
CREATE TABLE translation_models (
    id UUID PRIMARY KEY,
    language_id UUID REFERENCES conlang_languages(id),
    version INTEGER,
    base_model VARCHAR,                  -- 'nllb-600m-q4' | 'mt5-small-q4'
    adapter_path VARCHAR,
    trained_on_corpus_hash VARCHAR,
    training_config_json JSON,
    created_at TIMESTAMP,
    is_active BOOLEAN,
    eval_score_roundtrip REAL,
    eval_score_grammar REAL,
    eval_score_acceptance REAL,
    UNIQUE (language_id, version)
);

CREATE TABLE translation_corpora (
    id UUID PRIMARY KEY,
    language_id UUID,
    corpus_hash VARCHAR,
    source_pool_version VARCHAR,
    rbmt_version VARCHAR,
    language_value_hash VARCHAR,
    filter_config_json JSON,
    size_rows INTEGER,
    on_disk_path VARCHAR,
    acceptance_rate REAL,
    avg_source_length REAL,
    avg_target_length REAL,
    coverage_distribution_json JSON,
    created_at TIMESTAMP
);

CREATE TABLE translation_corrections (
    id UUID PRIMARY KEY,
    language_id UUID,
    source TEXT,
    system_output TEXT,
    corrected_output TEXT,
    tier_used VARCHAR,                   -- 'rbmt' | 'nmt' | 'mix'
    confidence_at_output REAL,
    paragraph_id UUID,
    weight REAL,
    incorporated_in_version INTEGER,     -- NULL until incorporated
    created_at TIMESTAMP
);

CREATE TABLE translation_alternatives (
    id UUID PRIMARY KEY,
    correction_id UUID,                  -- if author picked an alternative
    rendered_at TIMESTAMP,
    alternatives_json JSON,
    picked_index INTEGER,                -- which alternative the author picked
    picked_tier VARCHAR
);

CREATE TABLE translation_inferences (
    id UUID PRIMARY KEY,
    language_id UUID,
    source TEXT,
    target TEXT,
    tier_used VARCHAR,
    confidence REAL,
    elapsed_ms INTEGER,
    created_at TIMESTAMP
    -- Capped at 10k rows; oldest evicted; debugging only
);

CREATE TABLE translation_eval_runs (
    id UUID PRIMARY KEY,
    language_id UUID,
    model_version INTEGER,
    metric VARCHAR,                      -- 'roundtrip' | 'grammar' | 'acceptance'
    score REAL,
    test_set_path VARCHAR,
    test_set_size INTEGER,
    details_json JSON,
    created_at TIMESTAMP
);
```

### B. Full HJSON config schema

```hjson
translate: {
    enabled: true

    routing: {
        default_tier: "auto"                  // auto | rbmt | nmt
        tier_disagreement_threshold: 0.7      // BLEU; below triggers warning
        tier3_confidence_threshold: 0.5
    }

    rbmt: {
        parser: "distilbert"                  // distilbert | rust-bert
        disambiguator_strategy: "cascade"
        coining_proposals: true               // queue, don't commit
    }

    corpus: {
        target_size: 100000
        source_pool_version: "v1"
        filter: {
            min_lexicon_coverage: 0.9
            max_coining_rate: 0.05
            max_inflection_failures: 0.02
            require_grammar_valid: true
            require_phonotactic_valid: true
            diversity_window: 100
        }
    }

    training: {
        base_model: "nllb-600m"               // nllb-600m | mt5-small
        rank: 16
        alpha: 32
        epochs: 3
        learning_rate: 5e-5
        batch_size: 32
        correction_mix_ratio: 0.2
        synthetic_floor_ratio: 0.2
        driver: "python"                      // python (P1) | candle (P3+)
    }

    inference: {
        adapter_cache_size: 4
        decoding: {
            beam_width: 4
            max_length: 512
            grammar_constrained: false        // opt-in per language
            temperature: 1.0
        }
    }

    correction: {
        refresh_threshold: 100
        refresh_interval_days: null           // off by default
        weight_default: 1.0
    }

    resolver: {
        default_model: "qwen-2.5-1.5b-q4"
        // Per-language opt-in:
        qya: { enabled: true }
    }

    export: {
        include_base_by_default: false
        runtime_examples: ["python", "rust"]
    }

    assets: {
        cache_dir: "~/.inkhaven/assets/conlang/translate/"
        download_on_first_use: true
        verify_sha: true
    }
}
```

### C. Sample TUI overlay

**Translation pane with model status footer (`Ctrl+B U T`):**

```
┌─ Translate (English → Quenya) ──────────────────────────────────────────┐
│  English                          │  Quenya                              │
│  ─────                            │  ──────                              │
│  The warrior raised his sword     │  I ohtar ortanë macilirya           │
│  to the rising sun and spoke      │  i anarello, ar quentë              │
│  the words of his fathers.        │  i quettar i atarinwaron.           │
│                                   │                                      │
│  ─────────────────────────────────┴────────────────────────────────      │
│  Tier: NMT v5 (corpus v4)   Confidence: 0.86   Elapsed: 312 ms          │
│  Alternatives (a): 2 from RBMT, 1 from Resolver                          │
│  Trace (v): cursor on a token to see derivation                          │
│  Refresh recommended: 127 corrections pending since v5                   │
│                                                                          │
│  Tab: accept   Esc: reject   a: alternatives   v: trace   t: tier        │
│  m: model menu   e: eval status                                          │
└──────────────────────────────────────────────────────────────────────────┘
```

**Model menu (key `m` from pane):**

```
┌─ Quenya Translation Model ──────────────────────────────────────────────┐
│  Active version: v5                                                     │
│  Base:           NLLB-200-distilled-600M (Q4_K_M)                       │
│  Trained on:     corpus v4 (98,432 pairs)                               │
│  Adapter size:   19.4 MB                                                │
│  Created:        12 days ago                                            │
│                                                                         │
│  Eval scores (last run, 3 days ago):                                    │
│    Round-trip:   0.78  (target: > 0.70)  ✓                             │
│    Grammar:      0.96  (target: > 0.95)  ✓                             │
│    Acceptance:   0.74  (improving, was 0.68 at v4)                      │
│                                                                         │
│  Pending corrections: 127  (refresh recommended at 100)                  │
│  Estimated refresh time: 1h 50m on CPU, 12 min on GPU                   │
│                                                                         │
│  All versions:                                                          │
│    v1   training initial,    44 days ago,   acceptance 0.61             │
│    v2   refresh,             32 days ago,   acceptance 0.65             │
│    v3   refresh,             24 days ago,   acceptance 0.66  (regressed)│
│    v4   refresh,             18 days ago,   acceptance 0.68             │
│  ▶ v5   refresh,             12 days ago,   acceptance 0.74             │
│                                                                         │
│  ─────────────────────────────────────────────────────────────────      │
│  r: refresh now    R: rollback    e: run eval    x: export bundle       │
│  d: delete model   Esc: close                                           │
└─────────────────────────────────────────────────────────────────────────┘
```

### D. End-to-end workflow

A worked example for an author of a Quenya conlang who wants a
translation model.

```
1. Quenya has been built in LANG-1 with 480 lexicon entries,
   complete phonology, morphology, and grammar tags.

2. The author opens the translation pane: `Ctrl+B U T`.
   The footer shows "Tier: RBMT (no trained model). Confidence: 0.62"
   --- Tier 1 is already working.

3. The author drafts a chapter using the pane for dialogue.
   Over a week, ~30 translations happen, ~15 are corrected.

4. The author runs `inkhaven conlang translate corpus generate qya`.
   First time: prompted to download the source pool (~50 MB), then
   parser (~60 MB). Generation runs in ~3 min, produces a corpus of
   84,000 parallel pairs (acceptance rate 17% on a 500K pool scan).

5. The author runs `inkhaven conlang translate train qya`. First time:
   prompted to download NLLB-600M base (~150 MB). Training runs
   overnight on the author's GPU (1h 50m) or via cloud GPU rental
   (~$2). Adapter v1 emerges, ~20 MB.

6. The author opens the translation pane again. Footer now shows
   "Tier: NMT v1. Confidence: 0.83." Outputs are noticeably more
   fluent.

7. Over the next month, the author drafts further chapters.
   ~150 corrections accumulate.

8. `Ctrl+B U T` shows "Refresh recommended: 127 corrections pending."
   The author runs refresh; v2 trains in 1h on a smaller delta.

9. Eval runs automatically after refresh; acceptance score climbs
   from 0.68 to 0.74.

10. The author also defines Sindarin in LANG-1, derived from
    Proto-Eldarin. Cross-conlang translation works immediately via
    the English pivot (Quenya → English Tier 2 reverse → English →
    Sindarin Tier 1, since Sindarin has no trained model yet).

11. Months later, the novel is publication-ready. The author runs
    `inkhaven conlang translate model export qya --out qya-pack.itm`.
    The bundle is 22 MB (adapter + tokenizer + lexicon + runtime
    examples; base referenced by hash for download).

12. The author publishes the novel and includes a link to qya-pack.itm
    on the book's website with installation instructions. Readers can
    translate arbitrary English into the novel's invented language
    without owning Inkhaven.

13. A reader files a bug report on a translation that doesn't quite
    fit. The author imports the report as a correction, runs refresh,
    publishes adapter v6. Translation model lives on past the book.
```

This is the full arc: from existing LANG-1 language, through trained
model, through cross-conlang, through publication, through community
feedback. Each step is independently invocable and each artifact lives
in the project as a versioned, snapshot-tracked piece of work.

---

**End of RFC LANG-3.**
