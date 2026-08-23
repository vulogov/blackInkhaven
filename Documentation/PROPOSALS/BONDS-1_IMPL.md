# BONDS-1 — implementation plan (grounded, file-by-file)

*Companion to `BONDS-1_PLAN.md`. Every anchor verified against the tree on
2026-08-22. Phases BD-P0→BD-P7; value core = P1+P2+P3. BONDS is a near-1:1
mirror of KEN (`src/ken/`) — this plan is written as "copy KEN's X at `file:line`,
change Y."*

---

## Grounded substrate (what BONDS builds on — almost all of it exists)

**The KEN module is the whole template** — `src/ken/` (5 files, declared `mod ken;`
in `src/main.rs:66`). BONDS = `src/bonds/` + `mod bonds;`. Shared types in
`src/ken/mod.rs`; the run entry is `ken::check::run`, NOT in mod.rs.

- **Reading-order key** — `ScenePos { chapter_ord: u32, scene_index: u32 }`
  (`ken/mod.rs:26`, derives `Ord` → reading order for free). **BONDS reuses it**
  (re-export from `ken` or lift to a shared `scene` mod).
- **The paragraph/scene walk** — `ken::walk::book_paras(layout, h, book) ->
  Vec<ParaRef>` (`ken/walk.rs:29`); `ParaRef { id, at: ScenePos, tags: Vec<String>,
  text: String, declared_pov: Option<String> }` (`walk.rs:16`). Scenes split on
  `manuscript::is_scene_break`; `declared_pov` is the scene's `pov:` tag stamped on
  every para (`walk.rs:70`). Skips `content_type=="jinja"`. **BONDS reuses this
  verbatim** — it is already the generic manuscript walk.
- **Tags are free-form** — `Node.tags: Vec<String>` (`store/node.rs:179`, "free-form
  strings, case preserved"). Prefixed tags parse via `str::strip_prefix` (KEN:
  `tag.strip_prefix("know:")` / `"secret:"` `grants.rs:37,42`; `"pov:"` `walk.rs:71`).
  **`rel:` needs ZERO schema change** — no tag enum exists.
- **Co-presence evidence** — two overlapping sources KEN already wires:
  1. *Scene-derived cast* (primary, no timeline needed): the characters "present"
     in a `ScenePos` = its `declared_pov` + dialogue-attributed speakers
     (`dialogue::detect_spans`+`attribute_spans`, used by `ken::check::detect_uses`
     `check.rs:43`) + roster-name mentions via `drift::mentions` (whole-word,
     multilingual). This is exactly KEN's use-detection machinery, repurposed to
     "who shares this scene."
  2. *Timeline participants* (precise reinforcement): `TlEvent.characters:
     Vec<Uuid>` from `timeline_context::gather_events(h)` (`timeline_context.rs:91`),
     mapped to `ScenePos` via `linked_paragraphs` (KEN does this in
     `grants::grants_from_events` `grants.rs:88`). This is the same signal the 3.0.2
     participant-list fix made populatable.
  → BONDS's per-scene cast = union of (1) and (2). It works on `rel:` tags + prose
  alone; a populated timeline sharpens it.
- **Character roster (name↔UUID)** — `continuity_intel::introduce::roster(h,
  SYSTEM_TAG_CHARACTERS) -> Vec<(Uuid, String)>` (`introduce.rs:140`;
  `SYSTEM_TAG_CHARACTERS` = `store/mod.rs:93`). Names-only:
  `dialogue::pipeline::character_names(h)` (`pipeline.rs:39`). **Limitation: no alias
  field** — a character is one roster node, name = title. `rel:<A>:<B>` names must
  match roster titles (case-insensitive, whitespace-normalized via
  `grants::normalize_topic` `grants.rs:24`); alias/nickname resolution is a
  non-goal for v1 (note in help).
- **The KEN driver to mirror** — `ken::grants::build_grants(layout, h, book) ->
  (Vec<Grant>, Vec<KnowledgeItem>, Vec<ParaRef>)` (`grants.rs:131`): one walk + tag
  parse + roster + `gather_events`. BONDS's `build_bonds` is the same four calls.

**The worklist bridge** — `cli/editorial.rs::collect(project, book_name, only,
include_deferred) -> Result<EditorialReport>` (`:22`). Readers run inside the
`if let Ok(cfg)…store…h` guard (`:64`); KEN's block is `:154-160`
(`resolve_user_book` → `ken::check::run` → `editorial::from_knowledge_finding`).
`EditorialFinding { category: String, severity, location: Location{chapter:
Option<String>, paragraph: Option<Uuid>, char_range, path}, message, hint, source:
&'static str, autofixable }` (`editorial.rs:97`). Converter template
`from_knowledge_finding` (`editorial.rs:621`). Promotion `response_kind(category)`
(`editorial.rs:174`): Decision arm `:183-185`, default `_ => Brief` `:189`.

**The surfaces** — CLI `Command::Knowledge { book_name, json, deep, max_cost }`
(`cli/mod.rs:1423`, dispatch `:6893`); dashboard `Action::OpenKnowledge`
(`keybind.rs:150`) → `Modal::Knowledge { rows, anchors, cursor }` (`modal.rs:954`)
→ `open_knowledge`/`build_knowledge_rows`/`knowledge_handle_key`
(`app.rs:16379/16392/16445`) → `draw_knowledge_modal` (`render/modals.rs:6628`);
Bund `src/scripting/stdlib/knowledge.rs` (`ink.knowledge.{grants,findings,check}`,
policy rows `policy.rs:245`); config — KEN has **none** (reads only `cfg.language`),
so `bonds:` is net-new, modeled on `ContinuityConfig` (`config.rs:4398`).

---

## Phase map

### BD-P0 — model + `rel:` grammar  ·  `src/bonds/mod.rs`
Mirror `ken/mod.rs`. Types:
- Re-export `ScenePos`, `Severity` (Info/Notice/Break) from `ken` (or lift both to a
  shared `src/scene.rs`; re-export is lighter for v1).
- `enum BondSource { CoPresence, Declared }` (mirror `GrantSource`).
- `struct Declared { a: String, b: String, kind: String, at: ScenePos, anchor: Uuid }`
  — one `rel:` tag occurrence (`kind` = state at that point). Pair stored
  **canonicalized** (sort `(a,b)`) so `rel:ally:mara:kell` == `rel:ally:kell:mara`.
- `struct CoScene { a: String, b: String, at: ScenePos, anchor: Uuid }` — a scene
  both share (derived).
- `struct BondFinding { kind: &'static str, severity: Severity, chapter: u32, anchor:
  Option<Uuid>, a: String, b: String, message: String }` (mirror `KnowledgeFinding`
  `mod.rs:114`; pair instead of single character/topic).
- helper `pair_key(a,b) -> (String,String)` (canonical, normalized via
  `ken::grants::normalize_topic`).

### BD-P1 — gather (declared + derived)  ·  `src/bonds/gather.rs`  *(value core)*
Mirror `ken/grants.rs`.
- `bonds_from_tags(paras: &[ParaRef]) -> Vec<Declared>` — for each para tag,
  `strip_prefix("rel:")` then split `kind:a:b` on `':'` (mirror the `know:…@…`
  split `grants.rs:44`); resolve `a`/`b` against the roster (drop unresolved with a
  soft skip, like bare `know:` `grants.rs:51`); `at = p.at`, `anchor = p.id`.
- `coscenes_from_paras(paras, roster, events) -> Vec<CoScene>` — build the per-
  `ScenePos` cast set (scene-derived POV + dialogue speakers + `drift::mentions` of
  roster names) UNION timeline `TlEvent.characters` mapped via `linked_paragraphs`
  (mirror `grants_from_events` `grants.rs:88`); emit a `CoScene` for every unordered
  pair co-present in a scene.
- `build_bonds(layout, h, book) -> (Vec<Declared>, Vec<CoScene>, Vec<ParaRef>)` —
  mirror `build_grants` `grants.rs:131` (walk once, roster, `gather_events`).

### BD-P2 — check (THE deterministic core)  ·  `src/bonds/check.rs`  *(value core)*
Mirror `ken/check.rs`. Pure fns over `(declared, coscenes, cfg.bonds)`:
- **`unwritten_bond`** (Notice) — a declared pair whose count of shared `CoScene`s
  `< cfg.bonds.min_co_presence`. (told-not-shown.) Mirrors `dropped_reveals`
  `check.rs:180` (declared-with-no-derived).
- **`unearned_shift`** (Break) — order a pair's `Declared` states by `ScenePos`; for
  each adjacent pair of states with **different `kind`**, require ≥1 `CoScene` for
  that pair with `state[i].at < coscene.at <= state[i+1].at`; else emit, anchored at
  `state[i+1].anchor`, message naming both states + chapters. **The flagship.**
  Mirrors the premature invariant (`check.rs:112`: a transition with no grounding
  scene between).
- **`dropped_bond`** (Notice) — a pair with an early declared/derived bond and no
  `CoScene` for `> cfg.bonds.dormancy_window` chapters before it resurfaces (a later
  `CoScene` or `Declared`). Threads-dormancy shape.
- `run(layout, h, cfg, book) -> Vec<BondFinding>` — mirror `check::run`
  (`check.rs:211`): `build_bonds` → **self-gate: no `Declared` → `Vec::new()`** →
  run the three checks → sort by `(severity desc, chapter, a, b)`. Zero-AI, ≈$0.
- Unit-testable in isolation (the KEN pattern): construct `Declared`/`CoScene`
  literals, assert findings. Aim for the KEN test density.

### BD-P3 — promote into REDLINE  ·  `src/editorial.rs` + `src/cli/editorial.rs`  *(value core)*
- `from_bonds_finding(f: &bonds::BondFinding) -> EditorialFinding` — mirror
  `from_knowledge_finding` (`editorial.rs:621`): `category = f.kind.to_string()`,
  severity three-arm map (Break→Error/Notice→Warn/Info→Info), `location = Location {
  chapter: chapter_label(f.chapter), paragraph: f.anchor, ..default }`, `source:
  "bonds"`, `autofixable: false`.
- `collect` block — after the KEN block (`cli/editorial.rs:160`), inside the same
  guard: `if let Ok(book) = resolve_user_book(&h, book_name, "editorial") { for f in
  bonds::check::run(&layout, &h, &cfg, book) { raw.push(from_bonds_finding(&f)); } }`.
- `response_kind` (`editorial.rs:174`): add `"unearned_shift"` to the **Decision**
  arm (`:183-185`); `unwritten_bond` / `dropped_bond` / `implied_cooling` fall to the
  default `_ => Brief` (`:189`). **No `fix_spec` entry** — BONDS findings have no
  localized char-range rewrite, so keeping them out of `fix_spec` (`:218`) leaves
  `rewritable()` false and preserves the RD-P7 reversibility invariant (tested
  `:930`). Update the two `response_kind` tests (`:786-800`).

### BD-P4 — surfaces: CLI + dashboard  ·  `src/cli/mod.rs`, `src/cli/bonds.rs`, `src/tui/*`
- **CLI** (flat, exact KEN mirror — recommended over a `check` subcommand):
  `pub mod bonds;` (`cli/mod.rs:31`); `Command::Bonds { book_name, json, deep,
  #[arg(long, default_value_t=8000)] max_cost, #[arg(long)] strict }` after the
  `Knowledge` variant (`:1438`); dispatch after `:6895` → `bonds::run(...)`; new
  `src/cli/bonds.rs` mirroring `src/cli/knowledge.rs` (add `--strict` → non-zero exit
  on any finding, for a CI gate, like `inkhaven rigor --strict`).
- **Dashboard** on `Ctrl+V Shift+O` (**verified free** in the view_sub vec
  `keybind.rs:1823-1990`; `Ctrl+V o` = `OpenInnerEditorOverview` `:1940`, distinct):
  `Action::OpenBonds` (`#[serde(rename="view.open_bonds")]`, near `keybind.rs:150`);
  `entry("Shift+o", Action::OpenBonds, Scope::Any)` in the **view_sub** vec (~after
  `:1940`); short label (`:1013`) + long help (`:1246`); `Modal::Bonds { rows,
  anchors, cursor }` (`modal.rs:954`); `A::OpenBonds => self.open_bonds()`
  (`app.rs:12532`); `open_bonds` + `build_bonds_rows` (findings from
  `bonds::check::run`, grouped by `f.kind`, `f.anchor` parallel to rows) +
  `bonds_handle_key` (Enter → `open_paragraph_by_uuid`) (mirror `app.rs:16379/16392/
  16445`); handler routing (`app.rs:26899`) + render dispatch (`render.rs:334`) +
  `draw_bonds_modal` (`render/modals.rs:6628`, title `" Bonds "`).
- **Guard**: the `no_default_binding_is_fully_shadowed` test (added 3.0.8,
  `keybind.rs:~2413`) automatically covers the new binding; add a positive
  `resolve_view_sub(Shift+O) == OpenBonds` test near the KEN chord tests
  (`keybind.rs:2952`).

### BD-P5 — Bund + config  ·  `src/scripting/stdlib/bonds.rs`, `src/config.rs`
- **Bund** — new `src/scripting/stdlib/bonds.rs` mirroring `knowledge.rs`:
  `ink.bonds.findings` ( -- list, `{kind, severity, chapter, a, b, message}` ) +
  `ink.bonds.check` ( -- dict, `{unwritten, unearned, dropped, clean}` ) + optional
  `ink.bonds.ties` ( -- list, the declared bonds ) mirroring `ink.knowledge.grants`.
  `mod bonds;` + `bonds::register(vm)?;` (`stdlib/mod.rs:37,104`). **The `--deep`
  LLM pass is NOT exposed to Bund** (it costs — KEN precedent). Mandatory:
  - policy rows `("ink.bonds.findings", category::STORE_READ)` etc.
    (`policy.rs:245` area) — else `every_registered_word_is_classified` fails.
  - a `**ink.bonds.*** — BONDS` section in `Documentation/Bund/WORD_REFERENCE.md`
    (`:396` area) — else `word_reference_doc_matches_the_policy_table` fails.
- **Config** — `BondsConfig { enabled: bool, min_co_presence: u32 (~2),
  dormancy_window: u32 (~6), deep_cost_warn: f64 }` modeled on `ContinuityConfig`
  (`config.rs:4398`); `#[serde(default)] pub bonds: BondsConfig` on `Config`
  (`:81`); `bonds: BondsConfig::default()` (`:279`); permissive `enabled: true`.

### BD-P6 — `--deep` implied_cooling  ·  `src/bonds/deep.rs`
Mirror `ken/deep.rs`. `run(project, book_name, max_cost, force) -> Result<Vec<
BondFinding>, String>`: self-gate on empty declared bonds; build a per-pair ledger
(declared states + the scenes they share) via a `build_ledger`/`group_scenes` pair
(`deep.rs:69,86`); one `implied_cooling` system prompt (JSON array, `[]` if clean);
ride `cli::realworld::slow_llm_call(project, "bonds-cooling", SYSTEM, prompt,
max_cost, force)` (`realworld.rs:287`, daily-cap + soft-cap enforced); `map_finding`
→ `BondFinding { kind: "implied_cooling", severity: Notice|Info (never Break),
chapter: 0, anchor: None }`. **Invariant: per-pair only, never a whole-book pass.**

### BD-P7 — capstone
`Documentation/BONDS.md`; a tutorial (next free number); KEYBINDING (Ctrl+V Shift+O),
CONFIGURATION (`bonds:`), FEATURE_INDEX (a "Character relationships" row → CLI
`bonds`, chord `Ctrl+V Shift+O`, `ink.bonds`), README latest-release, `Documentation/
RELEASE_NOTES/3.1.0.md`, DEVELOPING (the reader-family table). An e2e test (build a
tiny tagged project, assert `unearned_shift` fires). Multilingual: `rel:` kinds +
roster names are language-agnostic; the `--deep` prompt keys off `cfg.language`;
`drift::mentions` is whole-word Unicode. Add BONDS to the WORD_REFERENCE + confirm
both doc guards + the keymap guard stay green.

---

## Open design decisions (resolve at build; defaults chosen)
1. **Co-presence source** — scene-derived cast (POV + dialogue + mentions) UNION
   timeline participants. *Default: both* (works without a populated timeline;
   sharper with one). Simpler v1 fallback: timeline-only (exact KEN parity) if the
   scene-derived cast proves noisy in testing.
2. **CLI shape** — flat `inkhaven bonds --json --deep --strict` (KEN mirror).
   *Default: flat* (drop the `check` subcommand from the PLAN's prose; it added
   surface with no benefit — KEN has none).
3. **`rel:` tag placement** — manuscript paragraph tags, scene-scoped like `pov:`
   (a `rel:` in a scene declares that pair's state at that ScenePos). *Default: yes.*
   A one-shot baseline in the Characters bible is a possible P1 add if wanted.
4. **Alias resolution** — none (roster title match only). *Default: exact-ish
   (normalized, case-insensitive); document the limitation.* Fuzzy-name via
   `drift::mentions` is a later enhancement.
5. **`unearned_shift` severity/response** — Break → Decision. Others → Notice →
   Brief. *Default as stated.*

## Effort
Value core (P0-P3) is the bulk and is ~1:1 with KEN's core, which is a few hundred
lines + tests. P4-P5 are mechanical wiring (each anchor above is a small edit).
P6 is the smallest phase (one prompt + the shared `slow_llm_call`). Total surface is
smaller than KEN's original build because the walk, roster, timeline, collect bridge,
dashboard-modal pattern, and Bund pattern all already exist — BONDS mostly *composes*
them.

---
*See `BONDS-1_PLAN.md` for the value case. Template throughout: KEN [[ken-1-rfc]].*
