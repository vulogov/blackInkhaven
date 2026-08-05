# KEN-1 — implementation plan (grounded, file-by-file)

*Companion to `KEN-1_PLAN.md`. Every anchor verified against the tree on 2026-08-04.
Phases KEN-P0→P8; value core P1+P2+P3. KEN watches the epistemic axis — who knows what,
when — reusing SENTINEL's mention-matching, DIALOG-1's attribution, CHORUS's scenes/POV,
and the timeline's event-participant lists.*

---

## Grounded substrate (KEN is an assembly of parts that already exist)

**The forward-walk spine + whose-head** — `src/chorus/scenes.rs:15` `Scene { chapter_ord,
scene_index, first_para: Uuid, text, declared_pov: Option<String> }`; `book_scenes(...)`
(`:26`) returns **every scene in reading order** (the walk). POV per scene:
`src/chorus/pov.rs:70` `scene_pov(text, roster, declared) -> ScenePov{Single|First|
Omniscient|Unknown}` (`:26`); `head_hops` / `is_leak` (`:127,:152`). LECTOR already walks
this spine (`src/lector/walk.rs`).

**Grants from presence — the timeline** — `src/world/timeline_context.rs:19` `TlEvent {
id, title, start_ticks, end_ticks: Option<i64>, linked_paragraphs: Vec<Uuid>, characters:
Vec<Uuid>, places: Vec<Uuid> }`; `gather_events` / `events_for_character` / `events_near`
(`:89+`). Persisted on nodes: `src/store/node.rs:211` `EventData { start_ticks, end_ticks,
precision, characters, places }`. So **`event.characters` at `start_ticks` = who could
know that event, and when.** Richer projection `CritiqueEvent` (`src/timeline/critique/
types.rs:43`). SEMNET `EventInvolves` edges are derived from `event.characters` (`src/
store/graph.rs`).

**Use detection — mention matching + attribution** — SENTINEL's Unicode-aware entity
matcher: `src/continuity_intel/introduce.rs` (`referenced_before_introduced`, `EntityIntro
{ entity, first-scene }`, `:37,:70`) is the model for "topic named in this text." Speaker
attribution: `src/dialogue/attribute.rs` cascade (Certain[name+verb] / Inferred / None) →
**who is speaking a line** (DIALOG-1). Character roster from the Characters system book.

**Declared grants — the tag pattern** — `src/tension.rs:50` `TensionKind{Introduce,Resolve}`,
`TensionTag{kind, topic, chapter_index}` (`:75`), `detect_unresolved` (`:194`),
`obligation_spans` (`:240`). KEN's `secret:` / `know:` tags mirror this declare-then-check
shape exactly (a topic + an owner + a chapter position).

**Worklist bridge + promotion** — `collect` (`src/cli/editorial.rs:22`) accumulates
`raw: Vec<EditorialFinding>` via `from_X_finding` converters (`:108-153`);
`from_knowledge_finding` slots at `src/editorial.rs:~565`. `response_kind` (`:174`) routes
a `knowledge`/`leaked_secret` category to **Decision** (author chooses: fix the leak or
move the reveal). Review-pass line pattern: `run_*_check` in the unified pass.

**Optional LLM pass** — the SENTINEL coherence / LECTOR synthetic shape:
`start_bg_job(BgJobKind::…)` + `collect_blocking` + `is_transient` retry, cost-capped,
`--deep` / a dashboard `k`.

**Free chord** — only `Ctrl+B Shift+Z` remains unbound in the `meta_sub` group
(`src/tui/keybind.rs`; resolve_in first-match; guard-test the binding).

**Bund policy** — `every_registered_word_is_classified` (`src/scripting/policy.rs:757`)
forces each `ink.knowledge.*` word into `WORD_CATEGORIES` as `STORE_READ`.

---

## KEN-P0 — substrate (pure types)

New `src/ken/mod.rs` (+ `mod ken;` in `main.rs`).
- `ScenePos { chapter_ord: u32, scene_index: u32 }` (reading-order key; `Ord`).
- `KnowledgeItem { topic: String, secret: bool }` — a knowable thing (an event subject, a
  named entity, or a declared secret).
- `Grant { character: String, topic: String, at: ScenePos, source: Presence | Declared |
  Told }` — the earliest a character could know a topic.
- `Use { character: String, topic: String, at: ScenePos, via: Dialogue | Pov }`.
- `KnowledgeFinding { kind: &'static str /*premature_knowledge|leaked_secret|dropped_reveal
  |implied_irony*/, severity, chapter: u32, anchor: Option<Uuid>, character, topic,
  message }` (mirrors `ReaderFinding` / `ContinuityFinding`).
- Pure helpers + tests (ScenePos ordering; a `earliest_grant(grants, char, topic)` lookup).
- `#![allow(dead_code)]` until P1 consumes (allow-until-consumer idiom).

## KEN-P1 — declared knowledge model + grants (value: the spine)

- `secret:<topic>` and `know:<topic>` inline tags (parsed like tension tags / the `pov:`
  tag) → `KnowledgeItem{secret}` + `Grant{source: Declared}` at the tagged scene. Roster +
  topic strings are Unicode-normalised (multilingual, like SENTINEL's matcher).
- **Event-presence grants (auto, free):** for each `TlEvent`, every `character ∈
  event.characters` gets `Grant{topic = event subject, at = ScenePos of the event's first
  linked paragraph, source: Presence}`. Pure `grants_from_events(events, scene_index)`.
- `build_grant_table(store, h, cfg) -> Vec<Grant>` merges declared + presence, keeping the
  **earliest** per (character, topic). Tests over a fixture.

## KEN-P2 — use detection + the epistemic break (THE value core)

- Forward-walk `book_scenes`; per scene, gather `Use`s: for each attributed-dialogue line
  (`dialogue::attribute` → speaker) and the POV character (`scene_pov`), match declared/
  known topics present in the text via the SENTINEL Unicode mention matcher → `Use{via}`.
- `check(grants, uses) -> Vec<KnowledgeFinding>` (pure, unit-tested): for each `Use(char,
  topic, at)`, look up `earliest_grant(char, topic)`; if none or `grant.at > use.at` →
  `premature_knowledge` (anchored to the use's paragraph). A `secret` topic used by an
  ungranted character → `leaked_secret` (higher severity). Silent where it can't ground.
- This is the flagship deterministic engine — the "could they know this yet?" check.

## KEN-P3 — dropped reveals + presence enrichment (value)

- `dropped_reveal`: a `know:`/declared grant whose topic never appears in any later `Use`
  (the epistemic `unpaid_setup`) — reuse the `obligation_spans` idea over grants→uses.
- Sharpen presence grants: an event's *subject* (topic to match on) is fuzzy from a terse
  `TlEvent.title`; prefer the event's linked entities / an explicit `reveals:<topic>` on
  the event tag when present, else fall back to the title token. Document the precision
  ladder honestly (declared > event-with-reveals-tag > bare-title).

## KEN-P4 — CLI + the worklist bridge

- `inkhaven knowledge check [--book-name] [--json]` — build grants → detect uses → check →
  print (grouped by kind) / JSON; **non-zero exit** on a `premature_knowledge`/`leaked_
  secret` (a CI gate, like `continuity check`). New `src/cli/knowledge.rs` + `Command::
  Knowledge`.
- `from_knowledge_finding(f) -> EditorialFinding` (`src/editorial.rs`) `source:"knowledge"`;
  wire a `knowledge` source block into `collect` (self-gating: no tags + no events → nothing
  added). A `knowledge` line on the `Ctrl+B Shift+C` review pass.

## KEN-P5 — the dashboard

- `Ctrl+B Shift+Z` → `Action::OpenKnowledge` → `Modal::Knowledge{rows,anchors,cursor}`
  (rows+anchors like the CHRONICLE/ledger dashboards): the findings grouped by kind, Enter
  jumps to the offending paragraph. + a `resolve_in` guard test (the SENTINEL/LECTOR/
  CHRONICLE shadow lesson — Shift+Z is the last free chord).

## KEN-P6 — the optional LLM pass (implied irony)

- `knowledge check --deep` / dashboard `k`: a cost-capped, per-scene LLM pass that reads a
  scene against the reader's known-state summary and flags **`implied_irony`** — a character
  acting informed/ignorant without a named topic. Reuses the SENTINEL-coherence machinery
  (`collect_blocking`, retry, daily cap, findings → Output `knowledge`). Explicit, never
  automatic, never whole-book.

## KEN-P7 — Bund + policy + config

- `src/scripting/stdlib/knowledge.rs` (mirror `stdlib/chronicle.rs`): `ink.knowledge.grants`
  ( -- list ), `ink.knowledge.findings` ( -- list ), `ink.knowledge.check` ( -- dict )
  `{premature, leaked, dropped, clean}` (clean = no premature/leaked). Classify STORE_READ
  ×3 in `policy.rs`; the classification guard enforces it.
- `knowledge:` config block (`enabled` for the review-pass line; `deep_cost_warn`).

## KEN-P8 — capstone (docs + e2e)

- `Documentation/KNOWLEDGE.md` (mirror SENTINEL/CHRONICLE); `Tutorials/115-*.md` + index;
  `KEYBINDING.md` (Ctrl+B Shift+Z); top-level `README.md` "Latest release"; `RELEASE_NOTES/
  2.6.0.md` + index; DEVELOPING-book audit (a "who knows what" beat in the fiction chapter).
- e2e: a fixture with a declared `secret:` referenced by an ungranted character → `knowledge
  check` flags `leaked_secret`, promotes into `Ctrl+V Shift+R`; `ink.knowledge.check` gate;
  the dashboard jump. Suite green + warning-free.

---

## Open decisions (resolve as we build)

- **Topic identity** — declared string topics vs binding to Facts/SEMNET entities. Lean:
  declared strings + entity-name matching (like SENTINEL), Facts binding a later refinement.
- **Auto (un-tagged) reach** — how far to push event-presence without any `secret:`/`know:`
  tags. Lean: presence grants always on (free); the sharp leak/premature checks need at
  least a `secret:`/`reveals:` tag, and KEN says so rather than guessing. Decide at P3.
- **`told` grants from dialogue** — deriving "A told B" from attributed dialogue + B's
  presence is powerful but fuzzy; keep it out of the P1 core, revisit as an enrichment.
- **Codename / verb** — KEN (codename) + `inkhaven knowledge` (verb), or `inkhaven ken`.
  Settle before P4, as REDLINE settled REDLINE-vs-REVISE.
