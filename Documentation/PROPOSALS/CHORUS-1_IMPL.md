# CHORUS-1 — Implementation Plan (grounded, file-by-file)

*Companion to [`CHORUS-1_PLAN.md`](CHORUS-1_PLAN.md). Every anchor below was
verified against the tree at 2.1.0-dev. Nothing built.*

## Grounded anchors (the reuse map)

**NARR-1 metric core (`src/prose/`)**
- `VoiceProfile` struct — `src/prose/profile.rs:51` (percentiles p10..p90, `cv`,
  `burstiness`, `mattr`, optional `modal_density`/`interiority_ratio`/
  `de_erlebte_rede_particle_density`, optional `tier2: VoiceTier2` = sensory[5] +
  active/passive, `text_hash`).
- **The reusable compute core**: `compute_profile_with(text: &str, scope:
  VoiceScope, lang: &ProseLanguage, lx: &CompiledLexicon, deep: bool,
  mattr_window: usize) -> VoiceProfile` — `src/prose/profile.rs:103`. Takes
  **arbitrary text**; the hierarchy coupling is only in `pipeline.rs`
  (`refresh_book` :44, `chapter_prose_text` :21). **Blocker to clear (CH-P0):**
  it is `pub(crate)` inside the *private* `mod profile`; `src/prose/mod.rs:22`
  re-exports only `VoiceProfile`/`VoiceScope`. `CompiledLexicon` (`mod.rs:153`,
  `for_language_with` :183), `mattr`, `modal_unigrams` are already crate-visible.
- `VoiceScope { Book, Chapter(u32) }` — `profile.rs:12`, with `as_str` :18 /
  `parse` :30 (round-trips `"book"`/`"chapter:N"`). Storage PK is
  `(book_slug, scope)`.
- `ProseStore` — `src/prose/store.rs:65`: `open` :71, `upsert(book_slug,
  &VoiceProfile, computed_at)` :79, `get_all` :119, `stored_hash` :136. Table
  `prose_profiles` :16.
- `prose drift` — `src/cli/prose.rs:122`; `Delta{cv,mattr,modal,interiority}`
  :211, `delta()` :218; `violations::violations(&profiles, baseline_ord,
  &thresholds) -> Vec<Violation>` — `src/prose/violations.rs:38`,
  `Violation{chapter,metric,baseline,value,delta}` :13, `emit_violation` :88.
- Language: `ProseLanguage{En,Ru,De,Fr,Es,Other}` — `src/prose/mod.rs:45`;
  `resolve_prose_language(explicit, project_lang)` :92; `is_supported()` :84.

**DIALOG-1 attribution (`src/dialogue/`)**
- `DialogueSpan{para_id, speech_text, word_count, attribution_name:
  Option<String>, attribution_conf, tag_verb_class, ends_question,
  ends_exclamation, …}` — `src/dialogue/mod.rs:125`; `AttributionConfidence
  {Certain,Inferred,None}` :73.
- **The character-line corpus**: `DialogueStore::certain_spans(book_slug) ->
  Vec<(u32 chapter_ord, DialogueSpan)>` — `src/dialogue/store.rs:220` (⚠ filters
  to `attribution_conf='certain'`; **CH-P0 adds `attributed_spans`**, the
  unfiltered book-wide getter). `refresh_book` — `src/dialogue/pipeline.rs:92`;
  roster via `character_names(h)` :39.
- **The precedent for NARR-1↔dialogue reuse**: `CharacterDialogueFingerprint`
  (`mod.rs:199`) is built in `src/dialogue/fingerprint.rs:46`, which already
  imports NARR-1's `mattr`/`modal_unigrams` (`fingerprint.rs:16`). CHORUS extends
  this from the lightweight fingerprint to the full `VoiceProfile`.
- CLI `DialogueCommand` — `src/cli/mod.rs:2271`, dispatch `src/cli/dialogue.rs`;
  TUI `Ctrl+B Shift+Q` → `open_dialogue_view` `src/tui/app.rs:13996`.

**POV / interiority / discipline substrate**
- `compute_pov_chip(lex, lines) -> Option<PovChip>` — `src/tui/pov_tracker.rs:54`
  (mention-count heuristic, per-paragraph; no per-scene/declared POV; first
  person is a blind spot).
- Interiority markers per language — `src/prose/lexicon.rs` (`Lexicon.interiority`
  :24; EN :42, RU :97, DE :146, FR :195, ES :235; German `erlebte_particles`
  :152; selector `lexicon(lang)` :264); `sentence_has_interiority`. **No
  marker→name linkage exists — CH-P4 builds it.**
- `AnachronismDetector` — `src/tui/style_warnings.rs:667` (`new(cfg)` :674,
  `detect(line) -> Vec<StyleHit>` :698; config-year driven, `BUILTIN_ANACHRONISMS`
  :626). **The shape CH-P6 clones for register word-lists.**
- **No POS/tense/morphology anywhere** — `src/grammar/` is Typst tree-sitter
  markup only (`language()` `src/grammar/mod.rs:37`). Tense is heuristic-only.
  Russian aspect: only `src/conlang/grammar.rs:95` (constructed-language
  authoring metadata, not applicable). → CH-P5 is English-gated, RU excluded.

**Review-pass rails + inner-family template**
- Output kinds are **string consts** — `src/pane/output/types.rs` `mod kinds`
  :17 (add `pub const STYLIST: &str = "stylist";`). `Message::new` :263;
  `Severity{Info,Warning,Contradiction,Progress}` :100; `crate::pane::output::
  {active,emit}`.
- Unified review pass `run_unified_check` — `src/tui/app.rs:15394`; deterministic
  checkers each `run_*_check(store,cfg,layout)->Result<usize>` (model:
  `run_theologian_check` :2859, `run_rigor_check` :2931); summed into `total`
  :15462, named in status :15481. **CLI mirror diverges** — `src/cli/check.rs:32`
  runs only fact/socrates/timeline; a new checker must be wired **both** places.
- Inner-reader template = `src/inner_poet/` (`mod.rs`, `fast.rs`
  [`Severity{Praise,Note,Concern}` :17, `Finding` :25, `scan_stanza` :35],
  `slow.rs` [`POET_SYSTEM` :12, `poet_llm_call` :48], `storage.rs`
  [`InnerPoetStore` wrapping `StorageEngine`, `replace_findings`/`findings_for`/
  suppressions with `finding_key` :66]).
- LLM idiom (copied per reader): `AiClient::from_config` + `resolve_provider` +
  `crate::ai::stream::collect_blocking` + `world::fact_check_slow::{is_transient,
  backoff_delay}` retry.
- `Ctrl+B J` hub → `inner_socrates_overview_handle_key` — `src/tui/app.rs:15778`
  (sub-keys F/E/T/R/P/A; hint string :15729). Poet's sub-overview modal pattern:
  `open_inner_poet_overview` :16487, `inner_poet_overview_handle_key` :15829,
  render guard :25585. `BgJobKind` :7256 (add `StylistSlow`); bg-completion
  handler :4184; `start_bg_job`/`BgMsg::Done`.
- Config: top-level `Config` `src/config.rs:9`; section pattern `ProseConfig`
  :4278 (`#[serde(default)]` + hand-written `Default`); permissive cap pattern
  `TheologianConfig.session_budget: f32` :4595 ("caps inform, never block").
- StorageEngine — `src/storage/engine.rs:31` (`new(path, init_sql, pool)`,
  `transaction`, `execute_with`, `select_all_with`). One `.db` per reader.

---

## Phase map

### CH-P0 — Substrate seams (pure, no behaviour change)
Three tiny enabling edits + tests; nothing user-visible.
- `src/prose/mod.rs`: `pub(crate) use profile::compute_profile_with;` (unblock
  the metric core for a sibling module).
- `src/prose/profile.rs`: extend `VoiceScope` with `Character(String)`; update
  `as_str` (`"character:<name>"`) + `parse` round-trip. (Storage PK already
  `(book_slug, scope)` — character profiles coexist with book/chapter rows.)
- `src/dialogue/store.rs`: add `attributed_spans(book_slug) -> Vec<(u32,
  DialogueSpan)>` — `certain_spans` without the `certain` filter (all attributed
  lines, confidence carried on the span).
- Tests: scope round-trip incl. a name with a colon; `attributed_spans` returns
  Inferred+Certain. **Gate:** `prose`/`dialogue` suites still green.

### CH-P1 — Character voice fingerprints
Profile each character's dialogue with the narrator's engine.
- New `src/chorus/` module (`mod.rs`, `voices.rs`): `character_corpora(spans) ->
  HashMap<String, (String /*all*/, BTreeMap<u32, String> /*per-chapter*/)>` —
  group `attributed_spans` by `attribution_name`, join `speech_text`.
- `character_profiles(store, layout, h, cfg, book) -> Vec<CharacterVoice>` where
  `CharacterVoice { name, profile: VoiceProfile, confidence: Confidence,
  per_chapter: Vec<(u32, VoiceProfile)> }`; each profile via
  `compute_profile_with(blob, VoiceScope::Character(name), lang, lexicon, deep,
  window)`. `Confidence` from utterance/word count (sparse → `Low`, refuse to
  flag).
- Persist through `ProseStore::upsert` under `VoiceScope::Character`. Keep the
  cheap `CharacterDialogueFingerprint` as the fast summary; the full profile is
  the new authority.
- Surface: extend `inkhaven dialogue profile` (or new `inkhaven style voices`) to
  print a **signature card** per character (rhythm/diversity/hedging + tics +
  Δ-from-cast-mean) with the confidence badge.
- Tests: pure `character_corpora` grouping; a synthetic two-character corpus
  profiles distinctly.

### CH-P2 — Distinctiveness matrix (the headline)
- `src/chorus/distinct.rs`: `feature_vector(&VoiceProfile) -> Vec<f32>`
  (normalized, z-scored across the cast); `distance(a,b)` (Euclidean/cosine over
  the normalized space — decide + document; genre-relative); `matrix(&[Character
  Voice]) -> DistinctMatrix` with pairwise distances, the most/least distinct,
  and **indistinguishable pairs** below `cfg.chorus.distinct_threshold` *and*
  both `Confidence >= Medium`.
- Author override: `chorus.distinct_ignore_pairs` (deliberate twins/chorus).
- Surface: the matrix in the signature-card output + fed to the Inner Stylist.
- Tests: identical corpora → distance 0 → flagged; distinct corpora → not
  flagged; low-confidence pair → never flagged.

### CH-P3 — Per-character voice drift
- `src/chorus/drift.rs`: reuse the `Delta`/`violations` shape (`src/prose/
  violations.rs`) over a character's `per_chapter` profiles — does Mara's Act-I
  voice match Act-III. `character_drift(&CharacterVoice, &thresholds) ->
  Vec<Violation>`.
- Surface: drift rows on the signature card + a Stylist finding.
- Tests: a corpus whose late chapters shift CV crosses the threshold.

### CH-P4 — POV & head-hop discipline
- **Per-scene POV.** `src/chorus/pov.rs`: a scene's POV = declared
  (`pov:<name>` / `pov:first` scene tag — parse from the existing tag surface) or
  inferred (per-scene extension of `compute_pov_chip` over the scene's
  paragraphs). `scene_pov(scene_paras, lex, declared) -> ScenePov`.
- **Head-hop.** For each interiority marker hit (`lexicon(lang).interiority`),
  resolve its **subject → name** (nearest character name in the marker's clause;
  confidence). If the resolved name ≠ scene POV and both are known → a `head-hop`
  finding. Deterministic fast track; optional AI adjudication for
  low-confidence clauses (the `src/drift.rs` retrieve-then-judge pattern).
- Emit: `kinds::STYLIST` Output findings, on the review-pass rails; jump-anchored.
- Tests: pure subject-resolution on crafted clauses (EN + RU markers); a
  single-POV scene with a non-POV `thought` flags; a declared multi-POV scene
  does not.

### CH-P5 — Tense discipline (English-gated)
- `src/chorus/tense.rs`: heuristic finite-verb-surface classifier (past/present)
  from suffix + auxiliary word-lists (the passive-heuristic style). Establish the
  scene's dominant tense; flag sentences that break it.
- **The language gate (the RFC's critical decision):** `tense_check` runs only
  for `ProseLanguage::En` (and, behind a cautious flag, De/Fr/Es); for `Ru` it
  returns "not available (Russian tense is aspect)" — surfaced in the UI, never a
  silent skip, mirroring `interiority()`'s `is_supported()` gate. Nothing models
  Russian aspect; do not pretend to.
- Emit: `kinds::STYLIST`. Tests: EN past-narrative with a present slip flags; the
  RU path yields the "not available" notice, never a false flag.

### CH-P6 — Register & diction axis
- Extend NARR-1: `src/prose/lang_metrics.rs` (or `src/chorus/register.rs`) adds a
  `register` metric bundle (contraction rate, formal/informal lexical ratio,
  latinate/germanic balance, archaism density) — language-keyed word-lists in
  `src/prose/lexicon.rs`; fold into `VoiceProfile` (new optional field) so
  `prose refresh` + `prose drift` pick it up for free.
- Period diction: clone `AnachronismDetector` (`src/tui/style_warnings.rs`) into a
  `RegisterDetector` driven by `chorus.register` word-lists (archaic/modern) — a
  config-list detector, not Facts-derived (documented).
- Tests: contraction-rate + formal-ratio on fixtures; a register drift crosses
  the threshold.

### CH-P7 — The Inner Stylist reader
The seventh inner-family member, cloning `src/inner_poet/`.
- New `src/inner_stylist/{mod,fast,slow,storage,vocab}.rs`; `mod inner_stylist;`
  in `src/main.rs`.
- `fast.rs`: `Severity{Praise,Note,Concern}` + `Finding`; `synthesize(matrix,
  drift, discipline, register) -> Vec<Finding>` — turns the measured numbers into
  structured observations (no LLM).
- `slow.rs`: `STYLIST_SYSTEM` (observe voice/style, **never rewrite** — the
  poet/theologian discipline) + `build_observation_prompt(measurements, lang)` +
  `stylist_llm_call` (the shared `collect_blocking`+retry idiom).
- `storage.rs`: `InnerStylistStore` over `StorageEngine` → `inner_stylist.db`
  (findings + suppressions; `finding_key`). Held on `App` (`inner_stylist_store`),
  opened at startup.
- Output: add `kinds::STYLIST` (`src/pane/output/types.rs`). Fast → Praise/Note/
  Concern to Output; slow → Thoughts pane (glyph e.g. `❝`).
- App wiring (`src/tui/app.rs`): `BgJobKind::StylistSlow` (+ completion arm);
  `run_stylist_check(store,cfg,layout)->Result<usize>` folded into
  `run_unified_check` **and** `src/cli/check.rs`; hub sub-key `Y` in
  `inner_socrates_overview_handle_key` (+ hint string) → `open_inner_stylist_
  overview` → `Modal::InnerStylistOverview` + `inner_stylist_overview_handle_key`
  (F fast / E engage / A ambient / S suppress) + render guard.
- Ambient: `stylist_ambient` runtime toggle + optional `chorus.ambient` config
  (poet pattern).
- Tests: `synthesize` maps an indistinguishable pair → a Concern; suppression
  round-trips.

### CH-P8 — Book-scale Style Report
- `src/cli/style.rs` + `StyleCommand{Voices, Distinct, Report, Scan, Suppress}`
  (`src/cli/mod.rs`): `inkhaven style report` prints the unified dashboard
  (narrator `VoiceProfile` + distinctiveness matrix + POV/tense/register
  findings). `--json` for tooling.
- TUI: a `Modal::StyleReport` dashboard (reachable from the Stylist overview),
  reusing the neighbourhood-modal scaffolding (scroll/close).
- Tests: report assembly over a fixture project is deterministic.

### CH-P9 — Capstone
- Bund: `ink.style.*` (`src/scripting/stdlib/`) — read the matrix / findings from
  a script (mirror `ink.prose.*`).
- Config: `StyleConfig`/`ChorusConfig` (`src/config.rs`, `ProseConfig` pattern) —
  `enabled`, `fast_track`, `ambient`, `distinct_threshold`,
  `distinct_ignore_pairs`, register lists, `session_budget` (informative), per-
  signal toggles, `language` override. `+serde(default)+Default`; round-trip test.
- Cost: Stylist slow track tagged `inner_stylist` in the usage dashboard.
- Multilingual gate verified: character voice + head-hop in Russian; tense "not
  available (RU)" surfaced; a Russian fixture test.
- Docs: new `Documentation/CHORUS.md`; update `PROSE_VOICE.md` (narrator vs
  cast), `KEYBINDING.md` (`Ctrl+B J → Y`), `CONFIGURATION.md` (`chorus` block),
  `Documentation/README.md` index; a companion-book chapter.

---

## Cross-cutting

- **Advisory / never-edit** (Thoughts pane for LLM, Output for findings);
  **cost informs, never blocks**; **no new crates**; **warning-free / 1.2.15**.
- **Confidence everywhere** — sparse speakers, ambiguous subject resolution, and
  register are all reported with confidence and refuse to false-flag.
- **The CLI/TUI review-pass divergence** (`run_unified_check` vs `check.rs`) is
  wired in both for `run_stylist_check`.
- **Value core = CH-P1 + P2 + P7** (character voice + distinctiveness +
  Stylist); if the cycle runs long, P6 folds into the Stylist and P3/P5 trail as
  2.1.x — but the scope is all of it.

## Open decisions (resolve during CH-P0/P1)

1. **Distance metric** (Euclidean z-scored vs cosine) + the default
   `distinct_threshold` — needs calibration on a real multi-voice manuscript.
2. **Character-profile storage** — extend `VoiceScope::Character` in
   `prose.duckdb` (chosen) vs a separate `character_voice_profiles` table. The
   enum route reuses `ProseStore` wholesale; revisit if the PK proves awkward.
3. **`pov:` scene-tag syntax** — align with the existing scene-tag/profile-tag
   surface rather than inventing a new namespace.
4. **DE/FR/ES tense** — ship English-only first; add others only with per-language
   validation (the RU exclusion is permanent by design).
