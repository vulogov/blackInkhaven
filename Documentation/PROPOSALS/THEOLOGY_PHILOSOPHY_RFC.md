# RFC (DRAFT) — Theology & Philosophy Toolkit: Primary-Source Engagement and Reasoning Tools

**Status:** DRAFT / pre-RFC — compiled from a design discussion for review. **Not a committed RFC.** Names, syntax, and shapes are provisional and open to change.
**Date:** 2026-07 · **Branch context:** 1.6.16-dev · **Proposed target:** 1.6.x, phased.

---

## 0. Purpose of this document

Collect and organize the ideas discussed for making inkhaven a serious tool for authors who *engage primary canonical texts and argue about them* — theologians and philosophers — into one coherent design sketch. It exists to be argued with, sliced, and turned into real per-feature plans; it is deliberately broad and non-binding.

---

## 1. Motivation

Today inkhaven serves a theology/philosophy author only by **tuning** existing machinery: the `genre: philosophy|theology` strings reweight Socratic categories and LLM prompt wording (`inner_socrates/slow.rs`, `inner_editor/prompt.rs`), and the Inner Theologian offers a moral/theological reading. There is **no capability built for the distinctive work** of these authors:

- engaging a **primary canonical text** — citing a locus within it, finding related and contradicting passages, comparing what different texts say;
- **arguing** — testing inferences, surfacing hidden premises, holding a term to one sense.

**Illustrative authors** (examples, not an exhaustive or privileged set):
- an author writing a **critique of Kant** (A/B + Akademie citation; the antinomies as arguments);
- a **Christian studying** scripture (verse references, cross-references, fidelity to tradition);
- a **Bible critique** (verse-level citation, original languages, exegetical argument).

**Hard requirement:** the design MUST generalize to *any* tradition — Book of Mormon, Quran, Ayurveda (Charaka Samhita), Talmud, the Vedas, Buddhist canon, a legal code — and be multilingual. **No canon is privileged**; built-ins are conveniences, not the mechanism.

---

## 2. Goals / Non-goals

**Goals**
- First-class engagement with primary texts: cite a **locus**; find **related** and **contradicting** material; **compare** sources; find **internal** contradictions.
- **Deep contradiction analysis over the *collected facts*** — the differentiating capability, and explicitly *beyond* pairwise factchecking: surface cross-source tensions, inference-level clashes (facts that jointly conflict without directly negating), and self-contradiction within a source, attributed to loci. See §7.1.
- **Tradition-agnostic** and **multilingual** (including non-Latin scripts).
- **Public-domain-only sourcing** — no legal risk; matches the `/gutenberg` ethos.
- **Grounded, cited, provenance-tracked** — never LLM *recall* of scripture; the LLM only *judges* retrieved text.
- **Reuse the existing engine** (RAG, research adapters, `facts_scan`, personas); minimize new infrastructure.

**Non-goals**
- Not bundling copyrighted texts, and not privileging any single canon.
- Not an arbiter of theological or philosophical *truth* — the tool surfaces, cites, and flags; the author judges.
- Not a from-scratch NLI model — contradiction is *retrieve-then-LLM-judge*.

---

## 3. Background — what already exists (reuse, don't reinvent)

| Capability | Where | Note |
|---|---|---|
| Multilingual RAG | `MultilingualE5Small` (config.rs:1508); `book_rag::retrieval::retrieve`, `research::rag::retrieve_sources` | Shared cross-lingual vector space; strong Russian |
| Keyless research adapters | `research/command.rs`; `/gutenberg`, `/openalex`, `/arxiv`, `/wikidata` | The clone template for new source adapters |
| Fact machinery | `/fact` (source→provenance-tagged fact); `facts_scan` consistency (contradicting *pairs*); `/triangulate` (stance judge SUPPORTS/CONTRADICTS/SILENT vs external APIs); `/synthesize` (grounded cited synthesis, Facts-scoped) | Contradiction detection is **LLM-based**, currently **Facts-scoped or external-API-scoped** |
| Readers | Inner Theologian (deterministic fast track + LLM); Inner Socrates (personas: `philosophical-reader`/"Dialectician", `theological-reader`, `prosecutor`/`defender`) | Inner Theologian's fast track is the template for a deterministic reader |
| Citation / structure | `BibEntry` (whole-work bibliographic; `pages` free string); Glossary (one-term-one-def); INDEX-1 back-of-book index | No canonical loci; no distinctions |
| Output surface | `kinds::*` paragraph-anchored findings in the Output pane | New reader/finding kinds slot in here |

**The six gaps** a theology/philosophy author hits: (1) no formal-argument/validity engine; (2) no canonical primary-source locators; (3) no distinctions/equivocation; (4) no deterministic philosophy reader; (5) coherence is fact-level not argument-level; (6) dialectic is prompt-only.

---

## 4. Design overview — five components

```
  A. Public-domain source adapters   ── the data in
  B. Primary-source loci + index      ── cite a locus
  C. SCHOLAR source interrogation     ── related / contradicting / cross-source
  D. Cross-lingual handling           ── foreign sources, native book
  E. Reasoning-rigor tooling          ── argument (later phase)
```

They compose: A feeds C; A's reference metadata feeds B; D wraps A+C; E is orthogonal and later.

---

## 5. Component A — Public-domain source adapters

**Pattern:** keyless fetch → chunk → embed → `research_source` with provenance — cloning `/gutenberg`.

**Catalog-driven, tradition-agnostic.** A table (config + bundled defaults) of public-domain canons; each entry:

```
{ command: "bible",  source: <PD API/data>, versions: [kjv, web, luther1912, segond1910, rvr1909, synodal, ...], ref_scheme: "book chapter:verse" }
{ command: "quran",  source: tanzil/alquran, versions: [uthmani, yusufali, pickthall, sablukov, ...],           ref_scheme: "surah:ayah" }
{ command: "bookofmormon", source: <PD json>, versions: [en],                                                    ref_scheme: "book chapter:verse" }
```

inkhaven registers `/bible`, `/quran`, `/bookofmormon`, … from the catalog. Adding a tradition later (`/tanakh`, `/dhammapada`) is a **catalog entry, not code** — the same principle as the Component B `ref_schemes`.

**Reference-carrying metadata (key decision).** A scripture ingest is richer than a generic `/import`: each chunk stores its canonical locus — `{ text, origin: public-domain, version: KJV, ref: "John 3:16" }`. This makes retrieval reference-aware, feeds B (loci/index), and lets deterministic cross-reference data (below) link verse→verse.

**PD-scoped by construction** — each adapter offers *only* its curated public-domain versions; no copyrighted text can be pulled. Provenance `origin=public-domain` + edition; cited `[Bible (KJV) John 3:16]`.

**Concrete adapters proposed**
- **`/archive`** — Internet Archive (`advancedsearch` + `metadata` + `_djvu.txt`), keyless; huge PD holdings incl. Sanskrit translations and Russian PD (`language:` facet). *[In progress.]*
- **`/wikisource <lang> <query>`** — MediaWiki API, keyless, **multilingual**; one adapter yields Russian literature + the Synodal Bible + philosophy + every other language's public domain.
- **Scripture commands** (`/bible`, `/quran`, `/bookofmormon`, …) via the catalog.
- **`/gutenberg` language filter** — Gutendex supports `?languages=ru`; expose it to reach the Russian PD subset (near-free).
- **Optional deterministic cross-references** — the public-domain **Treasury of Scripture Knowledge** as a bundled dataset → zero-LLM "related verses" for the Bible, complementing embeddings.

**Caveats:** free PD APIs vary in uptime → cache ingested text in the store (persistent); optionally bundle one or two core PD texts (KJV/WEB, ~1 MB compressed) for offline. A whole-corpus ingest (a Bible ≈ 31k verses) is a real one-time job → progress splash.

---

## 6. Component B — Primary-source loci + *index locorum*

**Declared reference schemes (tradition-agnostic).** `sources.ref_schemes` in config; each scheme = ordered segments + a format template + optional validation pattern:

```hjson
sources: { ref_schemes: {
  bible:          { segments: ["book","chapter","verse"], format: "{book} {chapter}:{verse}" }
  quran:          { segments: ["surah","ayah"],            format: "{surah}:{ayah}" }
  charaka:        { segments: ["sthana","chapter","verse"], format: "{sthana} {chapter}.{verse}" }
  talmud:         { segments: ["tractate","folio"],        format: "{tractate} {folio}" }
  stephanus:      { segments: ["locus"], pattern: "^[0-9]+[a-e]$" }
  akademie:       { segments: ["volume","page"],           format: "Ak. {volume}:{page}" }
} }
```

Ship presets + a generic custom escape hatch.

**In prose:** `@key[locus]` cites a locus within a source that declares a scheme. The locus is **validated** against the scheme (a malformed `327z` / out-of-range verse flagged, like `sources check` / the XREF finding), **rendered canonically** ("Kant, *Groundwork*, Ak. 4:421"; "John 3:16"), and **rolled into an *index locorum*** — the index-of-passages-cited appendix every scholarly work carries — reusing the INDEX-1 machinery. **Any-script** (Sanskrit / Arabic / Hebrew / CJK).

---

## 7. Component C — SCHOLAR: source interrogation

**The four author asks:** related material to a paragraph · contradicting material · related ideas across sources · contradictions within/between sources.

**The model — Facts are the workbench.** Raw corpora are too large to compare directly (a Bible is 31k verses; you cannot pairwise-judge 31k²). So both source passages *and* the manuscript's own assertions are distilled into **provenance-tagged claims** (Facts), and contradiction/comparison runs over claims.

```
  sources (/bible, /quran, /kant…) ─┐
                                      ├─►  Facts book (provenance-tagged claims)  ─►  analysis
  the manuscript's assertions ──────┘
```

**Pipeline — Collect → Confront → Reconcile:**
1. **Collect** — ingest via Component A; distill claims with `/fact` / `/synthesize` (each cites its locus).
2. **Confront** — run over the claim pool: `facts_scan` consistency already finds contradicting *pairs* (= "between sources", once each fact carries provenance); filter by `origin` for within-source; for a paragraph, retrieve nearest facts/source-chunks and **stance-judge** each (SUPPORTS/CONTRADICTS/SILENT).
3. **Reconcile** — surface each as a **cited finding anchored to the paragraph** in the Output pane (the NF-CITE / ARGUMENT / Theologian channel).

**Mapping to engine status:**

| Ask | Mechanism | Status |
|---|---|---|
| Related material | retrieve nearest facts/source-chunks | **present** (data missing) |
| Contradicting material | retrieve nearest → stance-judge CONTRADICTS | **new wiring** (re-point `/triangulate` at the ingested corpus) |
| Cross-source ("Quran vs Kant on X") | facts each source contributes on T, compared | `facts_scan` + **provenance grouping** |
| Within/between sources | `facts_scan` consistency, filtered/grouped by source | mostly present; **source-aware framing** |

**The genuinely new pieces (small):**
1. **Source-aware `facts_scan`** — carry and display *which source / locus* each side of a contradiction came from ("Quran 5:32 ⇄ Book of Mormon Alma …").
2. **A paragraph "confront against sources" action** (a chord) — retrieve → stance-judge → anchored cited findings; new `kinds::*` (e.g. `contradiction`).
3. **A cross-source contrast view** — "what does each source say on T, and where do they clash."

> **Scope note (relation engine).** SCHOLAR reports **both contradiction and confirmation**, over both bodies — the manuscript **Text** and the collected **Research**. Text↔Research: your claim *opposed* or *backed* by a source. Within Research: sources *disagree* or *converge*. Within Text: self-contradiction is **Inner Socrates'** existing job (SCHOLAR defers). Open track: pointing Inner Socrates at the Facts/research corpus (questioning the collected facts) — complementary to the *detection* below. See `SCHOLAR_PLAN.md` §0.1.

### 7.1 Contradiction & tension analysis over the collected facts — PRIORITY (beyond factchecking)

The existing `facts_scan` "consistency" pass is **simple factchecking**: it looks for directly contradicting *pairs* (`A ⇄ B`) within the Facts book, and a truth pass against the model's general knowledge. For a theology/philosophy author that is necessary but not sufficient — the *valuable* work is finding where a whole **body of collected sources genuinely clashes on an idea**. This component is elevated to a first-class goal, and it goes beyond pairwise checking in five ways:

1. **Cross-source, topic-clustered tension** — cluster the collected facts by claim/topic and, within each cluster, surface where *sources disagree* — "on the basis of moral duty: **Kant (Ak. 4:421)** vs **Quran 5:32** vs **Book of Mormon Alma …**." A source-attributed *dialectical map*, not a flat pair list.
2. **Inference-level contradiction** — facts that don't *directly negate* but **jointly conflict** (A implies X; B implies not-X; a scope or qualification shift makes two apparently-agreeing claims incompatible). This needs the LLM to reason about entailment, not string-match negation — this is the core "beyond factchecking" jump.
3. **Within-source self-contradiction** — a *single* source that says both X and not-X (filter facts to one `origin`, run the deeper pass). Central to a *critique*.
4. **Manuscript-vs-corpus** — the author's own thesis against the collected facts: does your argument contradict a source you've gathered, or is a source you rely on internally inconsistent?
5. **Graded tension, not binary** — report *degree*: flat contradiction vs. qualification vs. scope-difference vs. mere emphasis — so a source that *qualifies* isn't mislabeled as one that *refutes*.

**How it's built** (mostly re-pointing + deepening existing machinery): make `facts_scan` **source/provenance-aware**; add a **topical clustering** step over the fact pool (embeddings already available); replace the pairwise-negation prompt with an **entailment/tension judge** (the `/triangulate` stance judge, generalized to `CONTRADICTS | TENSION | QUALIFIES | AGREES | SILENT` with a cited rationale); output a **contradiction/tension report** grouped by topic and attributed to loci, surfaced both in the research view (whole-corpus) and anchored to the paragraph (Output pane).

**Honest constraints:** retrieval finds *similar*, not *opposing*, so a contradicting passage surfaces as a candidate that must be judged; the deeper (inference-level, clustered) analysis is more LLM-intensive, so it runs bounded by top-K, topical batching, and the existing cost caps, and is author-invoked (not ambient). Every reported clash traces to real, cited passages — no LLM assertion.

---

## 8. Component D — Cross-lingual handling (foreign sources, native book)

E.g. a **Russian** manuscript against **English-only** public-domain sources (Book of Mormon has no PD Russian; Kant's Russian translations are copyrighted). This is a **first-class case, not an edge case**:

- **Works today** — `MultilingualE5Small` is a *shared* space, so a Russian paragraph retrieves relevant English passages, and the LLM judge reads both languages.
- **Refinements** for quality:
  1. **Query translation** — translate the book-language query → the source language before retrieval (recall booster; per-query, cheap).
  2. **Distill facts in the *book's* language** — an English source becomes a **Russian** claim citing the **English** locus; the Facts layer is monolingual, comparison is clean, the boundary lives only at the citation.
  3. **Localized findings** — framing in the book language (`Labels::for_language`, EN/RU/ES/FR/DE); the *quoted* passage stays in the source's language (you cite the real PD text) with an **on-demand translation** shown as a reading aid, never baked into the citation.
  4. **Prefer a native PD edition when one exists** — the catalog surfaces it: Bible → **Russian Synodal (1876)**; Quran → **Sablukov (1878)**. Book of Mormon Russian is copyrighted → English + Russian Facts layer.

**Rule the catalog encodes:** *native PD source if it exists; otherwise foreign source + book-language Facts layer + localized findings.*

---

## 9. Component E — Reasoning-rigor tooling (later phase)

- **Rigor fast-track** — a deterministic, zero-AI, multilingual reasoning-red-flag reader (question-begging, false dichotomy, unsupported inference, equivocation, straw man), folded into **Inner Socrates** (per the "build on Socrates" steer) as its fast complement — mirroring the Inner Theologian's fast track. New `kinds::*` (e.g. `⊢`).
- **Scholarly lexicon / distinctions** — the Glossary holds a technical term **with its original-language form** (German for Kant; Greek/Hebrew for the Bible) and **two precise senses**, flagging undisambiguated use (gap 3).
- **Objection–reply / steelman check** — for each central thesis (ARG-1), is the strongest objection stated *and* answered? Driven by `prosecutor`/`defender` (gap 6).
- **AI validity pass** — extract premises→conclusion, assess validity, name the missing premise or fallacy (gaps 1, 5). Deepest; last, after the deterministic reader exists.

---

## 10. Public-domain constraint (cross-cutting)

Only public-domain / open-license (PD, CC0, CC-BY) sources. Adapters offer *only* PD versions. Provenance `origin=public-domain` + edition. **Avoid:** modern copyrighted translations (NIV/ESV/Sahih International), non-English LDS, and license-gated APIs (e.g. API.Bible's copyrighted set).

---

## 11. Engine reuse vs. new-build

The bulk is **extension of the Research Assistant + Sources subsystems**, not new infrastructure:

- **Reuse:** multilingual embeddings + retrieval; keyless-adapter pattern; `/fact` provenance; `facts_scan`; the `/triangulate` stance judge; `/synthesize`; personas; INDEX-1; the Output pane; the Glossary.
- **New (small):** the PD catalog + `/archive` + `/wikisource` + scripture adapters; reference-carrying chunk metadata; re-pointing contradiction/synthesis at the *ingested* corpus; source-aware `facts_scan`; the paragraph "confront" action; the `ref_schemes` + `@key[locus]` parser/validator/renderer + index locorum; the deterministic rigor reader.

---

## 12. Proposed phasing (provisional; order flexible)

- **Phase 1 — data in.** `/archive`, `/wikisource`, `/gutenberg` language filter, the PD catalog, reference-carrying metadata. Immediately useful on its own.
- **Phase 2 — the analysis (the differentiator).** SCHOLAR contradiction & tension analysis over the collected facts (§7.1) — source-aware `facts_scan`, topical clustering, the entailment/tension judge, the contradiction/tension report — plus the paragraph "confront against sources" action and the cross-source contrast view. This is the capability that makes the toolkit worth building; the adapters (Phase 1) exist to feed it.
- **Phase 3 — citation.** Primary-source loci (`ref_schemes`, `@key[locus]`, validation, canonical render) + index locorum.
- **Phase 4 — argument.** The reasoning-rigor reader; then distinctions, objection–reply, validity.

Rationale for adapters first: concrete, low-risk, valuable standalone, and they produce the corpus every later phase needs.

---

## 13. Open questions

1. **Adapter granularity** — per-canon commands (`/bible`, `/quran`) vs one `/scripture <canon>`? (Lean: catalog → per-canon commands over a shared ingest core.)
2. **Offline** — bundle one or two core PD texts, or always fetch-and-cache?
3. **Ingest scope** — whole corpus up front vs on-demand passages/books?
4. **Contradiction cost/latency** — top-K and per-run cost ceilings; the deep (§7.1) analysis is author-invoked and topically batched.
5. **Tension taxonomy** — is `CONTRADICTS | TENSION | QUALIFIES | AGREES | SILENT` the right set, or finer (scope-shift, equivocation, category error)?
6. **Topical clustering** — cluster facts by embedding similarity, by declared topic/tag, or by an LLM topic pass? Granularity of a "cluster."
7. **Loci syntax** — `@key[locus]` vs a dedicated token; how the locus binds to the scheme (per-source vs per-scheme).
8. **Rigor reader home** — new "Inner Philosopher" vs a fast-track on Inner Socrates. (Lean: extend Socrates.)
9. **Catalog currency** — how the PD-source catalog stays current and stays community-extensible without shipping updates.
10. **Cross-reference data** — bundle TSK, or fetch? Licensing of other traditions' cross-reference sets.

---

## 14. Appendix — public-domain data availability per tradition

- **Bible — rich.** Translations: KJV, ASV, Darby, YLT, Douay-Rheims, Webster, **World English Bible** (modern PD). Multilingual PD: **Luther 1912** (de), **Louis Segond 1910** (fr), **Reina-Valera 1909** (es), **Russian Synodal 1876**, Diodati (it), Vulgate (la). Originals: **Westminster Leningrad Codex** (Hebrew), **Nestle 1904 / Byzantine** (Greek). Cross-refs: **Treasury of Scripture Knowledge** (PD), OpenBible.info (CC-BY).
- **Quran — good.** Uthmani Arabic (Tanzil, freely distributable); classic PD translations **Yusuf Ali, Pickthall, Sale, Rodwell, Palmer**; Russian **Sablukov (1878)**.
- **LDS — English only.** Book of Mormon / D&C / Pearl of Great Price English text is PD; **non-English is copyrighted (Intellectual Reserve) → excluded**.
- **Philosophy — full.** Pre-1929 translations of Kant (Meiklejohn, Abbott), Plato (Jowett), Aristotle — abundant on **Project Gutenberg** (already ingestable); **Perseus** for Greek/Latin with Stephanus/Bekker loci; Kant's Akademie online (korpora.org). German/Greek originals PD.
- **Ayurveda / Sanskrit — available, less turnkey.** Ancient texts PD; PD English translations (Kaviratna's Charaka, Bhishagratna's Sushruta) on archive.org; **GRETIL / SARIT** freely-licensed TEI Sanskrit corpora.
- **Russian PD literary sources.** **ru.wikisource** (classics + Synodal Bible + philosophy, MediaWiki API — the strongest add); `/gutenberg` Russian subset; archive.org `language:Russian`. Non-API bulk collections (import recipes, not adapters): **Lib.ru**, **RVB**, **FEB-web**, **imwerden.de**. Skip: Russian National Corpus (restricted redistribution).
- **Where to fetch, all PD/open:** Project Gutenberg (wired), Wikisource, Sacred-Texts.com, CCEL, Tanzil, TSK/OpenBible, WLC/Nestle repos. **Avoid:** copyrighted modern translations, non-English LDS, license-gated version APIs.

---

*End of draft. This is a discussion artifact, not a committed plan — expect the component boundaries, phasing, and syntax to move as it's reviewed.*
