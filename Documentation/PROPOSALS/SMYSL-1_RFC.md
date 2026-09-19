# SMYSL-1 — a narrative-development substrate (SMYSL feature RFC)

Status: **SHIPPED & PUBLISHED in smysl 1.7.0** (on crates.io as of 2026-09-19).
Written 2026-09-18 as a proposal; updated to record what landed and its publication.
Scope: changes to the **smysl** crate (github.com/vulogov/smysl, format `smysl/1.0`) so inkhaven can
adopt it as a story-canon development ledger. This RFC lives in inkhaven's PROPOSALS because inkhaven is
the motivating consumer. Both concrete asks (§2, §3) were **implemented in the open 1.7.0 cycle**
(additive over the `v1.6.0` semver baseline, so no 1.8.0 or 2.0 was needed) and verified green — see §8.
The sections below record the **as-built** result and flag where it diverged from the original proposal.

---

## 0. Why — SMYSL as the story's *development history*, not a facts DB

inkhaven already models *current-state* epistemics: `EdgeKind` carries `SourcedFrom` (provenance),
`GradedAs` (trust ladder), `Contradicts`, `InTension`, `Qualifies`, `Agrees`, `Cites`
(`src/storage/edge_store.rs`); research emits confidence-scored, provenance-tagged, contradiction-gated
facts (`src/research/agentic.rs`). Measured against that, smysl looks *redundant*.

The fit appears on a **different axis**: smysl is suited to the **history of how a story's canon
developed** — what was decided, revised, retracted, and what each decision rests on — which nothing in
inkhaven captures today:

- **git** tracks *text* (bytes in `.typ`), not that a villain's motive shifted from revenge to grief.
- **Facts / SEMNET / KEN / SENTINEL** track the *current* canon and check it for consistency. The only
  thing inkhaven calls "canon" today is **series-shared _current_ facts** (`facts:` block, WORLD-1 —
  `src/facts_scan.rs`, `src/config.rs`): a fact directory, not a history.
- **CHRONICLE** tracks *history*, but of the **readers' findings** per milestone — `milestone_findings`
  fingerprints + a `MetricVector`, diffed as cleared-vs-introduced (`src/chronicle/`). That is the
  history of *problems detected*, not of *decisions made*: the symptom, not the decision.

Verified (§8): inkhaven has **no** decision/canon development-history ledger — the axis is genuinely
open, so this is not reinventing an existing subsystem.

smysl is architecturally built for the decision-level history: an **append-only store**, **content-
addressed identity** (`Uid = blake3(det_cbor(UnitCore))`), a **logical clock** with `model:` / `human:`
/ `tool:` agent provenance, first-class **`Supersedes`** and **`Retracts`** relations, and
`diff` / `trace` / `retract`. The primitives map almost one-to-one onto narrative operations
(`grounds` = "this reveal depends on that setup"; `retract` blast-radius = "if I cut this, what
breaks"; `trace` = "why is this true in my world"; `diff` = "what changed in canon since draft 2").

Three gaps stood between smysl and this use. Verification (see §8) reduced them to **two real,
additive changes** plus one that **already exists** and one **larger track**.

---

## 1. Summary of asks

| # | Ask | Verdict | As built in smysl 1.7.0 |
|---|---|---|---|
| **A** | A `commitment` (canonicity) axis, parallel to epistemic `Status` | **✅ shipped** | `Commitment` enum + first-class `Commit` **record** (type 13) — *not* a `Unit` field (§2); check pass 11 `SMY-W057` (warning); render **deferred** |
| **C** | A host/location `SourceRef` variant | **✅ shipped** | `SourceKind::Node` (`inkhaven:<uuid>#loc` string convention) + `SourceRef.extra` forward-compat + reverse lookup |
| ~~B~~ | Seed `pack`/`salience` with caller-supplied relevance | **withdrawn — already existed** | `role_weights` / `seed` / `Unit.salience` |
| M | Multilingual retrieval + human-facing strings | **deferred (separate cycle)** | — |

A and C landed additive, **identity-preserving**, **determinism-preserving**, with **no new
dependency**. inkhaven depends on smysl as `default-features = false` (+ `render-typst`), whose entire
non-workspace footprint is `blake3` (pure, no C), `unicode-normalization`, `bm25`, `fxhash` — no async,
no HTTP, no arg parser, no C/C++ (MSRV 1.79, edition 2021, MPL-2.0).

---

## 2. Proposal A — a `commitment` axis parallel to `Status`

### Problem
`Status` (`Unfounded…Measured`) means *how true in the real world* — `Measured` = an instrument
recorded it. It is the **one kernel axis that is not extensible** (`SchemaId` and `RelKind` both have
`Extension`; `Status` does not), and its integer order **is** the rule-M order. Fiction canon is
authorial fiat: a plot point is *canonical* because the author committed to it, not because reality
confirmed it. The epistemic axis is the wrong quantity, and it cannot be re-vocabularied without a 2.0.

### Design — as built (a record, not a `Unit` field)
The proposal put `commitment` as an optional field on `Unit` (beside `salience`). **That was wrong, and
1.7.0 corrected it:** `salience` is *not persisted* — there is no codec key for it, so an authored value
is silently dropped at the first store write. A field-based commitment would have died on the first save,
fatal for a ledger whose whole job is to persist / merge / diff. 1.7.0 makes commitment a **first-class
record** (record type 13), which also answers *who* settled it and *when* — which a field cannot.

The enum (variants exactly as proposed) — `crates/smysl-core/src/types/lifecycle.rs`:

```rust
#[repr(u8)] #[non_exhaustive]
pub enum Commitment { Floated=0, Drafted=1, Committed=2, Canonical=3, Retconned=4 }
```

The record + API — `Commit { unit, level, agent, ts, note, extra }`:

```rust
Commit::new(unit: Uid, level: Commitment, agent: AgentId, ts: Hlc).with_note(..)   // Record::Commit
Store::commitment_of(&Uid) -> Option<Commitment>   // derived: LATEST wins by (ts, agent) — rule U
Store::commits_of(&Uid) -> &BTreeSet<Commit>
Store::units_at_commitment(Commitment) -> Vec<Uid>
```

Surface `@commit d/motive { level: canonical, agent: …, ts: … }`; CLI `smysl commit --level … --as … --note`.
"How settled is a unit" is the **latest** commitment by `(ts, agent)` (order-independent, rule U) — *not*
highest-ever-asserted (rejected because it would make `Retconned` impossible to enact).

### The commitment-support check — shipped
Check **pass 11**, `SMY-W057` (`crates/smysl-check/src/passes/commitment.rs`): a unit may not be more
committed than the weakest thing it `grounds`-on — rule-M's shape on the commitment axis, the
*canonical-scene-built-on-sand* detector. A **warning** by design (outrunning your foundations is a normal
draft state; `--strict` makes it fatal); units with no commitment are skipped (silence ≠ `Floated`), and
`Retconned` units are skipped (a retcon resting on something weaker is expected).

### Concurrent disagreement — CommitmentFork (SMY-W058)
When two agents — or the author across sessions — commit the **same** unit to **different** levels with
**concurrent** logical clocks, "latest wins" cannot order them: a *commitment fork*. This is the
commitment-axis analogue of smysl's content **contention**, detected at merge — `DetectionKind::CommitmentFork`
→ a `Contention` with diag `SMY-W058` (`crates/smysl-graph/src/merge/contention.rs::commitment_forks`). It
matters directly for inkhaven's harvest-on-save, where the author and an AI reader (or two writing sessions)
can disagree about how settled a decision is; the fork is **recorded**, not silently resolved to a winner.
Shipped in 1.7.0 and verified green (§8).

### Render — deliberately deferred
The proposal asked for profile-marker display; 1.7.0 **shipped the data + checks and deferred render**
("render when someone needs it — don't guess a marker vocabulary nobody asked for", PLAN step 10). Not a gap
for inkhaven, which renders via its own Typst pipeline, not smysl's markers.

### Compatibility — verified green (§8)
Identity-preserving (commitment and forks are records; `canonical_uid` still hashes `UnitCore` only),
deterministic, additive over the `v1.6.0` baseline → landed in 1.7.0. Enum variants match the proposal;
the divergences from it: (a) record vs `Unit` field [the proposal's design was unworkable], (b) latest-wins
derivation (rule U), (c) render deferred, (d) the support check is a warning.

---

## 3. Proposal C — a host/location `SourceRef`

### Problem
`SourceKind{Url,File,Metric,Tool,Doc}` + local nicknames get close, but there is no clean way to say
"this unit derives from **host node UUID X at chapter/scene Y**." That back-reference is what lets the
author jump ledger→manuscript and lets a host store (inkhaven's DuckDB, which stays system-of-record)
know which units a given paragraph produced.

### As built
`SourceKind::Node = 5` (`crates/smysl-core/src/types/epistemics.rs`), documented as `host:<id>` + optional
locator, e.g. `inkhaven:0f3a…#ch3/scene2`. It is a **fieldless** `#[repr(u8)]` variant — *not* the
`Node { host, id, locator }` struct the proposal sketched — deliberately, because a data-carrying variant
on a `#[non_exhaustive] #[repr(u8)]` enum would be a **2.0** break; host/id/locator live in the existing
`SourceRef` string by convention. Reverse lookup: `Store::units_with_source_prefix(prefix)` ("which units
came from this node").

Two things 1.7.0 got right that the proposal didn't foresee:
- **A forward-compat prerequisite landed first:** `SourceRef` now carries an `extra` field so unknown source
  sub-keys round-trip verbatim — without it, a newer unit re-encoded to a *different* `Uid` (silent, because
  `source` is inside identity). Pinned by a splice test.
- **`Node` is kept out of the model-facing ingest schema** (provenance is exactly what `SourcePolicy` keeps
  from the model); a host supplies it via `IngestOptions::with_source`.

**Reader-compat caveat (unfixable, worth stating):** a pre-1.7 reader *rejects* a record whose `SourceKind`
it doesn't know (unknown discriminant → whole-record decode fails). A producer needing pre-1.7 readers keeps
using `Doc` with the same `inkhaven:…` string (works everywhere; `units_with_source_prefix` has answered the
reverse query since 1.5).

---

## 4. ~~Proposal B~~ — already present (integration note, no smysl change)

An earlier draft asked for a hook to seed `pack`/`salience` with a host's semantic relevance.
**smysl already has this**, so it is an inkhaven integration pattern, not a feature:

- `salience()` reads a per-unit authored override — `Unit.salience` **fully overrides** the derived
  score (`crates/smysl-graph/src/salience.rs`).
- `SalienceRequest.role_weights: BTreeMap<Uid,f32>` and `SalienceRequest.seed` are the caller's
  relevance seams, blended into the score deterministically; that `SalienceReport` is exactly what
  `pack` consumes for its value term and density tie-break.

So inkhaven injects its fastembed/HNSW relevance via `role_weights` (blend) or `Unit.salience`
(override) — composing smysl's closure-complete, budget-bounded selection with inkhaven's semantic
retrieval, **with no smysl change and no embedding dependency** (inkhaven never enables the `semantic`
feature, avoiding `model2vec-rs`/`tokenizers`/a C++ toolchain). A new smysl input would be warranted
only for a relevance signal *independent* of the single salience scalar the packer reads — almost
certainly unnecessary.

---

## 5. Deferred track — multilingual (noted, not specced)

inkhaven has a hard rule: every NLP/AI feature must work in en/ru/fr/de/es with Unicode-aware
matching. smysl has NFC (good) but English-leaning BM25 tokenization and English diagnostics / ingest
messages / render registers. A pluggable retrieval tokenizer + localizable human-facing strings is
real and on-brand (*смысл*), but it is a cycle of its own, not a single additive hook. Out of scope
for the 1.7.0 asks; recorded for a later smysl cycle.

---

## 6. What this unlocks on the inkhaven side (context, not part of the smysl asks)

With A + C landed, inkhaven can build a **Canon Ledger** — the semantic development history of the
story — sitting beside CHRONICLE (verdict counts) and reusing the readers it already runs:

- **"What breaks if I cut this?"** — `retract` a decision's unit → every downstream reveal/setup/
  decision that now dangles. Dependency-aware revision impact; inkhaven has nothing like it.
- **Canon over time** — `trace` ("why is this true in my world"), `diff` ("what changed since draft
  2"), commitment ("how settled is this").
- **One editable review artifact** — the per-reader `Finding` types + `.inkhaven/*.json` sidecars
  (KEN / SENTINEL / LECTOR / BONDS / myth / stylist) could emit into one smysl document with
  provenance + trust + trace, turning the Editorial Pass into "open it, fix a line, save."

Decisions enter the ledger **harvested on save** by the existing readers (zero extra author effort),
carrying `human:` vs `model:` provenance — the save path already re-embeds a paragraph
(`Store::update_paragraph_content` → `DocumentStorage::reembed_document`, `src/store/mod.rs:2014` /
`src/storage/document.rs:223`), so it is the natural hook to also emit/refresh the paragraph's canon
units. And CHRONICLE's existing **milestone** trigger + cleared/introduced **finding-diff** machinery
(`src/chronicle/`) is the natural driver for the ledger's *canon*-diff — the two are complementary
(findings-history vs decision-history) and can share the milestone boundary. This is an inkhaven-side
design (a separate inkhaven PLAN); it is summarized here only to justify the smysl asks.

---

## 7. Scaling & budget

Two budgets: the **token/context** budget (`pack`) and the **resource/cost** budget. The feature
scales like inkhaven's existing per-project stores (the owned vector index, `chronicle.db`) because it
shares their shape — per-project store, incremental append on save, deterministic queries.

- **Data.** O(10²–10³) canon decisions for a novel, low 10⁴ for deep worldbuilding — tiny for a graph
  (cf. the ~4k-vector help corpus loads in 49 ms). Append-only growth is proportional to *distinct
  decisions + revisions*, not to save count: content-addressing dedups idempotent re-derivations, and a
  *changed* decision appends one unit + a `Supersedes`. Superseded / `Retconned` units are the history
  you want, filterable by commitment and compactable if ever heavy (same story as the vector store's
  orphans, see [[owned-vector-store]] / RELEASE_NOTES 3.10.0).
- **Compute — deterministic, ~$0.** Every author-facing query is a pure function, no model call:
  `salience` fixed 32 iterations (`O(32·E)`); `pack` greedy over the *scoped* subgraph (exact
  branch-and-bound gated behind a feature + `EXACT_THRESHOLD`); `retract` blast-radius =
  reverse-reachability over `grounds`; `trace` / `diff` = bounded walk / Uid-set compare.
  Sub-millisecond at book scale. "What breaks if I cut this," canon `diff`, and `trace` spend **zero
  tokens**.
- **On-save latency — the real constraint.** Deterministic harvest (SENTINEL continuity detectors,
  `world/fact_check`) appends units **backgrounded like the vector-index sync** (`sync_in_background`).
  **Model-based** harvest ("what decision does this paragraph *establish*") is **opt-in /
  milestone-triggered** (natural at a CHRONICLE mark) or batched — never on the save hot path, per the
  AI-advisory + cost-caps-inform principles. Automatic per-save LLM harvest is explicitly **out**.
- **Token/context budget.** Context cost is **constant in canon size**: `pack --budget b --reserve r`
  reserves prompt + question + answer room and closure-fills the rest (a decision drags in its grounds
  + rebuttals); `scope` + `focus` (HNSW-seeded via `role_weights`, §4) bound the input; selection is
  deterministic and free. A bigger canon means *better selection into the same window*, not bigger
  prompts — strictly tighter than today's best-hits truncation (`src/book_rag/retrieval.rs`).
- **$ / memory / deps.** Deterministic ops $0; LLM only at opt-in harvest + grounded chat (which
  tighter packing can make *cheaper* per query). Loaded store is single-digit MB (10³–10⁴ units × a few
  hundred bytes) beside the ~21 MB vector index. Pure-profile dependency (§1), no async/HTTP/C++, no
  crate-publish bloat.

**Scale-up (large project / series).** A single project's store stays in the ranges above; `scope`
keeps every query bounded to the relevant book/character subgraph, so per-operation cost tracks the
subgraph, not the whole store. A **series** is the one case with more than one store: SMYSL's
coordinator-free **merge** unions per-project ledgers into a series canon (O(total) once; contentions
= cross-book inconsistencies) — the SAGA path, deferred and gated on a real series user. Persistence is
**one store per project** (`<project>/canon.cbor` beside `chronicle.db`), never per-paragraph:
paragraph/chapter/character are *views* via the host `SourceRef` (§3) + `scope`, not separate files.

## 8. Verification record (against the smysl tree, `1.7.0`)

Read-only passes confirmed the compatibility claims (pre-implementation):

- **Identity / `Unit` fields.** `Unit` (`crates/smysl-core/src/types/unit.rs` L190-198,
  `#[non_exhaustive]`) carries `attestations` / `salience` / `labels` outside `core`; `canonical_uid`
  hashes `UnitCore` only (`hash.rs` L13-15; `cbor/envelope.rs` L44-57), pinned by tests
  `attestations_labels_and_salience_do_not_change_identity` (hash.rs L128-148) and
  `every_hashed_field_changes_the_uid` (hash.rs L76-124). ⇒ A's field on `Unit` is identity-safe;
  it must **not** go on `UnitCore`.
- **Enum extensibility.** `Status` (`epistemics.rs` L15-26) and `SourceKind` (L155-164) are both
  `#[repr(u8)] #[non_exhaustive]`; `Status` order is rule-M (used as `status > cap` in
  `check/src/passes/epistemics.rs` L45 and `trust.rs` L40). ⇒ C's new `SourceKind` variant is 1.x;
  A's parallel enum is independent of `Status`.
- **Relevance seam (B).** `salience.rs` L264-267 reads `Unit.salience` as an override; `role_weights`
  (L127/L249) and `seed` (L124) are per-unit seams feeding `pack` (`pack/src/solve.rs` L242/L279,
  `weigh` L697-698). ⇒ no new input needed.
- **Check framework.** Hand-written `Pass` enum + dispatch (`check/src/lib.rs` L29-52 / L221-251),
  `#[non_exhaustive]`; adding a pass is additive across ~6 sites. `runs()` (L168-170) gates on
  `is_implemented()`.
- **Render.** Rule V1 enforced at `Profile::load` (`render/src/profile.rs` L265 / `enforce_v1`
  L273-293) over a `Status`-specific `markers` map (L156); a commitment axis follows the V1/V2
  precedent as new code.

Existing kernel vocabulary already covering the narrative motion (no ask needed): unit kinds
`Decision` / `Observation` / `Hypothesis` / `Contention` (`KernelType`), relations `Supersedes` /
`Retracts` / `Rebuts` (`RelKind`), and `x.<domain>/…` extension schemas/relations for narrative kinds.

### inkhaven side (verified 2026-09-18, against `3.11.0-dev`)

The asks A+C are unchanged by this pass; it confirms the *motivation* is real and current:

- **No existing decision/canon development-history ledger** (verified negative). `canon` in-tree means
  series-shared *current* facts (`src/facts_scan.rs:152`, `src/config.rs:2071`, WORLD-1) or story-shape
  framework (`src/planning.rs`) — neither is a decision history. ⇒ the feature is genuinely new.
- **CHRONICLE = finding/verdict milestone history, not decisions.** `src/chronicle/mod.rs` ("persists
  the readers' collective verdict at each draft milestone") + `store.rs` (`milestone_findings` rows:
  fingerprint/category/severity/location/paragraph); types `Milestone` / `MetricVector` / `FindingRef`
  / `FindingDiff` / `Trend`. Complementary to, not overlapping, a canon ledger.
- **Current-state epistemics already rich** (why smysl is *not* for that): `EdgeKind` = `SourcedFrom` /
  `GradedAs` / `Contradicts` / `InTension` / `Qualifies` / `Agrees` / `Cites` (`src/storage/edge_store.rs`);
  confidence-scored, provenance-tagged, contradiction-gated fact emission (`src/research/agentic.rs`).
- **Fragmented findings** (what a unified smysl artifact would consolidate): per-reader `Finding` types
  in `src/inner_poet/`, `src/myth/`, `src/conlang/`, `src/world/`, `src/inner_stylist/`,
  `src/world/utopia/`, collected via `.inkhaven/*.json` sidecars (`src/editorial.rs`, `src/cli/editorial.rs`).
- **Readers present** (finding sources for harvest): `src/ken/`, `src/lector/`, `src/bonds/`, `src/myth/`,
  `src/inner_stylist/`, `src/conlang/`, `src/world/utopia/`, and SENTINEL = `src/continuity_intel/`.
- **Harvest hook present**: `Store::update_paragraph_content` (`src/store/mod.rs:2014`) →
  `DocumentStorage::reembed_document` (`src/storage/document.rs:223`).
- **RAG budgeting today** is best-hits truncation (`src/book_rag/retrieval.rs`: `max_context_tokens`,
  `estimate_tokens ≈ chars/4`) — the approximate version of what smysl `pack` does principally (context
  for the "use `pack`" note, not an ask).

### As-built + green gate (smysl 1.7.0, `dev/1.7.0`)

A third pass (the as-built read) plus a green gate confirmed the shipped result:

- **Commitment** — `Commitment` enum + `Commit` record (type 13, `crates/smysl-core/src/types/lifecycle.rs`);
  `Store::commitment_of` (latest-wins, rule U) / `commits_of` / `units_at_commitment`; check pass 11
  `SMY-W057` (`check/src/passes/commitment.rs`, warning); `@commit` surface + `smysl commit` CLI. `Uid`
  unchanged (commitment is a record, not in `UnitCore`).
- **Host source** — `SourceKind::Node` + `SourceRef.extra` forward-compat + `Store::units_with_source_prefix`;
  excluded from the model-facing ingest schema.
- **Bonus, same cycle** — extension-schema units are now retrievable (`Query::schemas` / `find --schema`), so
  `x.narrative/…` kinds work in `find`/`pack`; and the hybrid engine exposes a library `Retriever` seam, so
  inkhaven can drop its own fastembed/HNSW backend into `Hybrid::new(lexical, my_retriever)` with no
  `model2vec`/C++ path.
- **Green gate** — `cargo test --workspace --all-features` = **1880 passed, 0 failed** at HEAD `1af075f`
  (re-run after CommitmentFork landed; 1875 at the prior `1005cfe`); `cargo xtask check-purity` ✅ and
  `cargo xtask determinism` ✅ (pack / salience / merge / derive_thread / render bit-identical); build clean.
  (A plain `cargo test` had shown 3 `cmd_merge` failures — a feature-unification artifact where the exec'd
  binary lost its default `ingest`; all pass under `--all-features`, smysl's canonical gate.)
- **CommitmentFork (SMY-W058) landed and is green** (HEAD `1af075f`): the re-run includes its 8 tests
  (`two_agents_disagreeing_is_a_fork`, `one_agent_changing_their_mind_is_not_a_fork`,
  `the_fork_is_found_whatever_order_the_records_arrive_in`, and the not-a-fork chain/settled cases). No
  caveat remains.
- **PLAN doc:** `smysl:Documentation/PLAN_1.7_COMMITMENT.md`.

---

## 9. Status & next

- **smysl asks:** A (commitment axis) and C (host `SourceRef`) — **shipped in smysl 1.7.0** (unreleased;
  additive over `v1.6.0`, so no 1.8.0 or 2.0). Commitment render display deliberately deferred.
  **CommitmentFork (SMY-W058)** adds concurrent-disagreement detection on the axis (shipped + green, §8).
- **Verified green at HEAD `1af075f`** (incl. CommitmentFork): `cargo test --workspace --all-features` =
  **1880 passed, 0 failed**; `xtask check-purity` + `xtask determinism` pass (§8).
- **inkhaven side — now unblocked:** smysl 1.7.0 is **published on crates.io**, so inkhaven can depend on
  it directly (`smysl = "1.7"`, `default-features = false` + `render-typst`) and plug its own fastembed/HNSW
  retriever into smysl's `Retriever` seam. Next is a separate Canon-Ledger PLAN (harvest-on-save, `retract`
  blast-radius, and CommitmentFork-surfaced author↔reader disagreement), still gated on the ~1-user test
  ("would the author keep a canon ledger while writing") — not yet greenlit.
