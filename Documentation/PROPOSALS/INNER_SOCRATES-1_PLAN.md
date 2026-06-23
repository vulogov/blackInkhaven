# RFC INNER_SOCRATES-1 — Examined Authorship via Socratic Interrogation

| | |
|---|---|
| **RFC** | INNER_SOCRATES-1 |
| **Title** | Examined Authorship via Socratic Interrogation (Reader Personas, Intent Ledger, Fast/Slow Socratic Tracks) |
| **Status** | **In progress — building incrementally in the 1.3.x cycle** |
| **Created** | 2026-06-26 |
| **Author** | Vladimir Ulogov |
| **Target version** | author wrote 1.6.0; **pulled forward into the 1.3.x tree** (same cadence as WORLD-4) |
| **Depends on** | PANE-1 (Output pane) — **COMPLETE (1.3.24)** · WORLD-4 (Fast/Slow infra, multilingual baseline, ledger pattern) — **COMPLETE through 1.3.27** |
| **Soft-depends on** | LANG-1 (conlang gazetteer, future) |
| **External dependency** | the timeline feature (1.2.6+) — for the 3 timeline Slow categories |

---

> ## Status banner (1.3.x incremental build)
>
> Author targeted 1.6.0; per Vladimir's direction we build incrementally in the
> **1.3.x** tree, **one phase per signed increment** — exactly as WORLD-4 / LANG /
> PANE-1 were built. **ZERO new external deps** (the RFC itself commits to this in
> §11; everything is inherited from PANE-1 + WORLD-4 + `dirs`, already in tree).
>
> **The new module is `src/inner_socrates/`.** It is a *sibling* of `src/world/`:
> independent message kinds, independent storage tables, no direct seam with the
> fact-checker (both are read-only consumers of the shared language detection +
> gazetteer + cost machinery).
>
> ### What we reuse (no reinvention)
>
> | Need | Reused from |
> |---|---|
> | Output emission (`socratic_inquiry` kind, severities, actions, ask-AI bridge) | PANE-1 Output pane (`src/pane/output/`) |
> | Fast/Slow split, debounced trigger, background-job harness | WORLD-4 (`tick_fact_check`, `start_bg_job`) |
> | Language detection + confidence + graceful degradation | `world::fact_check_lang::detect_with_confidence` |
> | Whole-word / Unicode matching | `world::fact_check_lang::contains_word` |
> | The **ledger pattern** (Entry / Kind / Coverage / Scope / lazy Consultation / Suppression / Promotion) | WORLD-4 magic ledger (`world::types::magic`) |
> | Slow-track cost preflight + soft/daily caps + retry-backoff | `world::fact_check_slow` (`slow_preflight`, `backoff_delay`, `is_transient`) |
> | Cross-paragraph coherence pass shape | `world::fact_check_slow::build_coherence_prompt` |
> | Finding shape (`category`/`severity`/`body`/`body_en`/`suppressed_by`) | `world::fact_check::Finding` (extended) |
> | Per-project store on `StorageEngine` | `world::storage::WorldStore` template |
>
> ### The two ledgers, side by side
>
> The **magic ledger** (WORLD-4) declares physical exceptions a *world* allows; the
> **intent ledger** (this RFC) declares deliberate authorial choices about a
> *prose's* ambiguity / framing / echo / style / temporality. Same vocabulary
> (Ledger, Entry, Kind, Coverage, Scope, Consultation, Suppression, Promotion),
> same lazy-consult-after-candidate shape. A user fluent in one understands the
> other.
>
> ### Severity mapping onto PANE-1
>
> Notice → `info`, Inquiry → `warning`, Probe → `contradiction`. User-facing labels
> renamed; the underlying PANE-1 envelope is unchanged. Default visible threshold =
> Inquiry (quiet by default).
>
> ### Phasing (RFC §12 re-cut for 1.3.x; MVP = P0–P5)
>
> - **P0** — Foundation: types (severity / 7 Fast + 8 Slow categories / finding /
>   persona / intent), the **intent ledger** + lazy consultation (pure, mirrors the
>   magic ledger), 2–3 deterministic English Fast categories. *No storage / LLM /
>   TUI yet — the pure core first, fully tested.*
> - **P1** — Fast track complete: all 7 categories × EN, persona emphasis weighting,
>   `detect`-driven language gate, the `check_paragraph` entry point + a headless
>   CLI (`inkhaven inner-socrates check --fast-only`).
> - **P2** — Storage + Intent system book (13th): `InnerSocratesStore` on
>   `StorageEngine` (findings, ledger, personas, usage), the Intent book scaffold,
>   bundled-persona distribution, `socratic_inquiry` → Output.
> - **P3** — TUI: the `Ctrl+B I` chord family (overview, persona select, ledger
>   view), debounced Fast auto-check into Output (mirrors `tick_fact_check`).
> - **P4** — Slow track: 5 prose categories via the LLM, the persona/genre/intent
>   prompt builder, reused cost preflight + retry, `--slow` CLI + idle/close
>   triggers + coherence pass.
> - **P5** — Multilingual (RU/ES/FR/DE pattern tables + per-language prompt
>   templates + warning localization), inheriting WORLD-4's degradation chain.
> - **P6** — Timeline Slow categories (dramatization gap / implication tracing /
>   temporal density) consuming `ink.event.*`; `timeline_range` intent scope.
> - **P7** — Conversation + Suggestion F9 scopes, persona authoring wizard,
>   promotion mechanism, `make_note` primitive.
> - **P8** — Snapshot finding-history, `.isl` bundle export/import, user-level
>   config layer (`~/.config/inkhaven/`), genre templates, Bund `ink.inner_socrates.*`,
>   docs (WORLDBUILDING-style reference + tutorials).
>
> **Non-prescriptive discipline (the spine).** Every finding is a *question*, never
> a correction. If a surface would say "the prose should be X", it does not ship.

---

## Design anchors (from the RFC, condensed)

- **Severity:** `Notice` (info, hidden by default) · `Inquiry` (warning, default
  visible) · `Probe` (contradiction, always visible).
- **Fast categories (7, deterministic):** modal_claims, hedged_uncertainty,
  structural_patterns, unattributed_dialogue, pronoun_ambiguity, tense_voice_shifts,
  sentence_length_anomalies.
- **Slow prose categories (5, LLM):** assumption_surfacing, tension_detection,
  framing_interrogation, significance_probing, implicit_comparison.
- **Slow timeline categories (3, LLM + timeline):** dramatization_gap,
  implication_tracing, temporal_density.
- **Bundled personas (5):** inner-socrates (default), careful-editor,
  skeptical-reader, first-time-reader, slow-reader — each with category emphasis
  weights + voice notes.
- **Intent kinds:** deliberate_ambiguity, framing_choice, structural_echo,
  stylistic_choice, deliberate_temporal_ambiguity (+ reserved CustomBund, never in
  MVP). **Scopes:** chapter, paragraph_range, character, scene, timeline_range,
  project. **scope_level:** project | series (for `.isl`).
- **Chord family:** `Ctrl+B I` (overview), `I F`/`F W`/`F P`/`F R` (slow scopes),
  `I S` select persona, `I N` new persona, `I C` conversation, `I L` ledger,
  `I X` promotion candidates, `I U` usage.
- **Message kinds:** `socratic_inquiry` (+ `intent_ledger_promotion_suggestion`,
  `inner_socrates_conversation_preflight`, `inner_socrates_conversation_complete`).
- **Cost:** separate `inner_socrates_llm_usage` table, sub-budgets (slow_track /
  conversation / wizard); reuse WORLD-4's preflight + caps + backoff.

See the full RFC text (filed with this commit's PR description / the source RFC)
for §8 detailed design, Appendix A (schema), B (config), E (overlays).

---

## Increment log

- **P0.1 — foundation core (UNRELEASED, 1.3.28-dev).** New `src/inner_socrates/`:
  `types` (`Severity`, `FastCategory`×7, `SlowCategory`×8, `Category`,
  `SocraticFinding`, `Persona` with emphasis maps), `intent` (`IntentEntry`,
  `IntentKind`, `IntentScope`, `IntentLedger` + lazy `consult` → `Emit`/`Suppress`,
  mirroring the magic ledger), and `fast` (deterministic EN detectors for
  `modal_claims` + `hedged_uncertainty`, reusing `contains_word`). All pure +
  tested; non-prescriptive (every finding renders as a question). No storage / LLM
  / TUI yet. +17 tests, 1675.
- **P1.1 — three more deterministic Fast categories (UNRELEASED, 1.3.28-dev).**
  A dependency-free `text` util (sentence split, word count, opening word,
  dialogue-segment count) + three EN detectors wired into `check_paragraph`:
  `structural_patterns` (a run of ≥3 sentences sharing an opening word, or ≥4 of
  the same length), `unattributed_dialogue` (≥4 spoken segments with no
  attribution verb anywhere), `sentence_length_anomalies` (a sentence over 45
  words). Five of the seven Fast categories now ship; `pronoun_ambiguity` +
  `tense_voice_shifts` await the UD parser (P5). +8 tests, 1683.
- **P2 — storage + Intent system book + Output (UNRELEASED, 1.3.28-dev).**
  `InnerSocratesStore` on `StorageEngine` (`<project>/inner_socrates.db`) persists
  emitted findings (clear-per-paragraph for re-checks) and the **intent ledger**
  (scope + coverage serialized, `load_ledger` for consultation). The **Intent
  system book** is registered (`SYSTEM_TAG_INTENT` + `SYSTEM_BOOKS`) and seeds on
  project init. `output::emit_finding` bridges a finding to the PANE-1 Output pane
  as a `socratic_inquiry` message (persona / category / track / question + EN
  fallback). A headless CLI (`inkhaven inner-socrates check --text|--paragraph`,
  `… ledger`) runs the Fast track → consult ledger → persist → emit → print. +3
  tests, 1686. *Next: P3 — the `Ctrl+B I` chord family + debounced auto-check into
  the Output pane.*
