# SMYSL-1 — a narrative-development substrate (SMYSL feature RFC)

Status: **proposal**, written 2026-09-18.
Scope: changes requested in the **smysl** crate (github.com/vulogov/smysl, currently `1.7.0`,
format `smysl/1.0`) so inkhaven can adopt it. This RFC lives in inkhaven's PROPOSALS because inkhaven
is the motivating consumer; the two concrete asks (§2, §3) are additive `1.x` changes to smysl and, by
smysl's own `cargo-semver-checks` discipline (new public items → minor), land as **smysl 1.8.0** — not
a 2.0. Every compatibility claim below was verified against the smysl tree; see §7.

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

Verified (§7): inkhaven has **no** decision/canon development-history ledger — the axis is genuinely
open, so this is not reinventing an existing subsystem.

smysl is architecturally built for the decision-level history: an **append-only store**, **content-
addressed identity** (`Uid = blake3(det_cbor(UnitCore))`), a **logical clock** with `model:` / `human:`
/ `tool:` agent provenance, first-class **`Supersedes`** and **`Retracts`** relations, and
`diff` / `trace` / `retract`. The primitives map almost one-to-one onto narrative operations
(`grounds` = "this reveal depends on that setup"; `retract` blast-radius = "if I cut this, what
breaks"; `trace` = "why is this true in my world"; `diff` = "what changed in canon since draft 2").

Three gaps stood between smysl and this use. Verification (see §7) reduced them to **two real,
additive changes** plus one that **already exists** and one **larger track**.

---

## 1. Summary of asks

| # | Ask | Verdict | smysl surface touched |
|---|---|---|---|
| **A** | A `commitment` (canonicity) axis, parallel to epistemic `Status` | **needed, 1.x** | new `Unit` field + enum + optional check pass + render axis |
| **C** | A host/location `SourceRef` variant | **needed, 1.x** | one `SourceKind` variant + adapter convention |
| ~~B~~ | Seed `pack`/`salience` with caller-supplied relevance | **already exists — no change** | use `role_weights` / `Unit.salience` |
| M | Multilingual retrieval + human-facing strings | **separate cycle** | pluggable tokenizer + localizable strings |

All of A and C are additive, **identity-preserving**, **determinism-preserving**, and add **no new
dependency**. inkhaven would depend on smysl as `default-features = false` (+ `render-typst`), whose
entire non-workspace footprint is `blake3` (pure, no C), `unicode-normalization`, `bm25`, `fxhash` —
no async, no HTTP, no arg parser, no C/C++ (MSRV 1.79, edition 2021, MPL-2.0).

---

## 2. Proposal A — a `commitment` axis parallel to `Status`

### Problem
`Status` (`Unfounded…Measured`) means *how true in the real world* — `Measured` = an instrument
recorded it. It is the **one kernel axis that is not extensible** (`SchemaId` and `RelKind` both have
`Extension`; `Status` does not), and its integer order **is** the rule-M order. Fiction canon is
authorial fiat: a plot point is *canonical* because the author committed to it, not because reality
confirmed it. The epistemic axis is the wrong quantity, and it cannot be re-vocabularied without a 2.0.

### Design
Add an **optional `commitment` annotation on `Unit`, outside the hashed `UnitCore`** — the same place
`attestations` / `salience` / `labels` already live. Because it rides outside identity, a unit's `Uid`
is unchanged whether or not it is committed — which is exactly right: *"the decision's content did not
change; my commitment to it did"* is a real development event, and `diff` over two snapshots reports it
as a commitment change on the **same** `Uid`, not a new unit.

A small ordered enum, its own type (independent of `Status`'s rule-M ordering):

```rust
// crates/smysl-core/src/types/epistemics.rs  (new, sibling to Status)
#[repr(u8)]
#[non_exhaustive]
pub enum Commitment {          // higher = more settled
    Floated   = 0,             // an idea on the table
    Drafted   = 1,             // written, not load-bearing
    Committed = 2,             // other decisions may depend on it
    Canonical = 3,             // settled truth of the work
    Retconned = 4,             // explicitly overridden; kept as history (pairs with Supersedes)
}
```

```rust
// crates/smysl-core/src/types/unit.rs  — sibling of with_salience / with_label
Unit::with_commitment(Commitment) -> Unit
Unit::commitment(&self) -> Option<Commitment>
```

### The commitment-support check (optional pass)
Mirror rule-M on the new axis: **a unit may not be more *committed* than the weakest thing it
`grounds`-on.** "A `Canonical` scene resting on a `Floated` premise" is the fiction analogue of "a
measured claim resting on a guess" — the *canonical-scene-built-on-sand* detector, surfaced not hoped
for. Same shape as the existing rule M (`status > cap`), a new axis (`commitment > min(grounds)`).

### Compatibility (verified — §7)
- **1.x, identity-preserving.** The field lives on `Unit` (`#[non_exhaustive]`), never `UnitCore`.
  `canonical_uid` hashes `UnitCore` only; a test already pins that `.with_salience(…)` etc. do not
  change the `Uid`. A new independent enum is unaffected by `Status`'s order.
- **Deterministic.** Stored data; no I/O, no async.
- **Costs, stated honestly.** (a) The check pass touches ~6 hand-maintained sites (the `Pass` enum +
  `ALL` / `IMPLEMENTED` / `number()` / `as_str()` + one dispatch arm + a new `passes/<name>.rs`); it is
  additive but not one edit, and a *strictly* opt-in pass needs `CheckOptions.only` to name it (an
  implemented pass otherwise runs whenever `only` is empty). (b) Rendering commitment is **new code**,
  not config — the rule-V1 marker map is `Status`-specific — but there is a clean two-axis precedent
  (V1 = status, V2 = contentions) to follow.
- The cleaner long-term design (make `Status` itself schema-interpretable) is the **2.0** alternative;
  this annotation is the 1.x path that unblocks the use now.

---

## 3. Proposal C — a host/location `SourceRef`

### Problem
`SourceKind{Url,File,Metric,Tool,Doc}` + local nicknames get close, but there is no clean way to say
"this unit derives from **host node UUID X at chapter/scene Y**." That back-reference is what lets the
author jump ledger→manuscript and lets a host store (inkhaven's DuckDB, which stays system-of-record)
know which units a given paragraph produced.

### Design
Add a location-bearing source variant, plus a documented adapter convention (host store is authority;
smysl is the content-addressed ledger over it, cross-linked by this `SourceRef`):

```rust
// crates/smysl-core/src/types/epistemics.rs  — SourceKind is #[non_exhaustive]
SourceKind::Node    // host-defined id + optional locator, e.g. { host: "inkhaven", id, path }
```

### Compatibility (verified — §7)
**1.x.** `SourceKind` is `#[repr(u8)] #[non_exhaustive]`, so a new variant is non-breaking. No deps,
deterministic.

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
for the 1.8.0 asks; recorded for a later smysl cycle.

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

## 7. Verification record (against the smysl tree, `1.7.0`)

Two read-only passes confirmed the compatibility claims:

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

---

## 8. Status & next

- **smysl asks:** A (commitment axis) and C (host `SourceRef`) — additive, land as **smysl 1.8.0**.
- **No 2.0** required; no new dependency; identity + determinism preserved.
- **inkhaven side:** a separate Canon-Ledger PLAN (harvest-on-save, `retract` blast-radius) — to be
  written if this is greenlit; gated on the ~1-user test ("would the author keep a canon ledger while
  writing").
