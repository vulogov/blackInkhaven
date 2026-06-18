# BREADTH-1 — Cancel · Full multilingual · Production polish (1.3.13)

_RAG / the Whole-Book AI Editor is reserved for **1.4**. 1.3.13 is a breadth
cut that finishes loose ends and lifts Inkhaven to **true any-language**
support. Three threads:_

1. **Cancel-in-flight** — the deferred DEEP-1 follow-up: stop a running deep
   refresh.
2. **Full multilingual** — the centerpiece: make every language-sensitive
   feature correct for any of the stemmer's 18 languages, not just the 5
   with hand-curated lists.
3. **Production polish** — the deferred PDF / ePub output items.

Zero new dependencies (rust_stemmers, genai, lopdf all already in).

---

## Where multilingual stands (the gap to close)

| Layer | Today |
|---|---|
| **Stemming** (`parse_stemmer_language`) | ✅ 18 Snowball langs (en/ru/fr/de/es + it/pt/nl/da/fi/no/sv/ro/hu/el/ar/tr/ta) |
| **AI prompts** | ✅ language-agnostic — every prompt carries `cfg.language` |
| **Embeddings** | ✅ multilingual (`MultilingualE5Small`) |
| **Detector word-lists** (filter-words, show-don't-tell, stop-words, drift pronouns) | ⚠️ curated for **5** only; **unknown langs fall back to ENGLISH** — so Italian prose gets English filter-words flagged |
| **Anachronism lexicon** | English built-ins + user `terms` (culture-specific; stays config-driven) |

So a non-curated language is *half*-supported (stemming + prompts + embeddings)
yet **wrong** on the detectors. "Full multilingual" closes that.

---

## P0 — Cancel the background deep refresh

- An `Arc<AtomicBool>` cancel flag created in `start_bg_job` and handed to the
  worker; `App.bg_job` keeps a clone. A TUI key — **`Esc` while the `⟳` chip
  shows** (or re-pressing `Ctrl+V Shift+F`) — sets it.
- `deep_refresh_shared` checks the flag **between** scans; the `*_with` loops
  check it **per chapter / entity** so a long facts/continuity pass stops
  promptly. On cancel the worker returns early and sends `Done(Err("cancelled"))`
  (a clean outcome, not a failure tone).
- Already-written sidecars stay valid (atomic); a partial one is simply not
  saved. The status reports `deep refresh cancelled — N of M scans done`.

**Deliverable:** a runaway refresh is one keypress to stop.

---

## P1 — Multilingual: honest fallback + `lang status`

- **Fix the wrong-language fallback.** `built_in_filter_words` (and the
  show-don't-tell / stop-word / pronoun selectors) currently return the English
  list for any unrecognised language. Change to: a curated language → its list;
  **anything else → empty** (the detector is *off*, never flagging English
  words in foreign prose). English stays the default only when the language is
  actually English/unset.
- **`inkhaven lang status [--language <l>]`** — a coverage matrix for the
  active language: stemming (Snowball ✓/✗), filter-words (built-in N / config /
  none), show-don't-tell, stop-words, drift pronouns, anachronism, embeddings.
  One honest answer to "what works in my language?".
- **Force AI output into the manuscript's language.** Every AI scan already
  *passes* `cfg.language`, but the system prompts are English and don't require
  the *output* (the `why` / explanation text) to come back in that language —
  so a Russian project can get English explanations. Each scan prompt (facts
  check ✓ done, facts scan, drift, continuity) explicitly instructs the model
  to write its explanations in `cfg.language`. (The fact's own text is quoted
  verbatim, so it stays in-language regardless.)

**Deliverable:** non-curated languages are *correct* (detectors off, not
wrong), AI findings come back in the manuscript's language, and `lang status`
says exactly what's covered.

---

## P1b — Domain-aware, tunable fact-check prompt

The `facts check` already (1.3.13) reasons with real-world domain knowledge
(planetary science / physics / geology / hydrology / climate / ecology /
culture) — flagging facts that *cannot coexist*, not just textual
contradictions, while treating the author's invented rules as authoritative.
This phase makes that prompt **3-tier tunable** (the `plan` / `submission`
pattern: **Prompts book → `prompts.hjson` → built-in**), keyed by language:

- a `facts-check` prompt slug resolves through the tiers in `check_with`,
  falling back to the (domain-aware) built-in — so a hard-SF project can lean
  on orbital mechanics, a fantasy one on its own cosmology, and a Russian
  project can supply a Russian-language check prompt.
- Because the resolver is language-aware, this also gives per-language fact-
  check prompts for free (ties into P1's output-language work).

**Deliverable:** the world-consistency check is tunable per project + per
language, with the strong domain-reasoning default.

---

## P2 — Multilingual: bootstrap ANY language

Generalise the existing `show-dont-tell bootstrap` into one command:

- **`inkhaven lang bootstrap <language> [--provider]`** — a single LLM pass
  that generates the full per-language detector vocabulary (filter words;
  show-don't-tell linking-verbs / emotion-adjectives / manner-adverbs;
  repeated-phrase stop-words; drift pronouns) for any Snowball-supported
  language, written into `inkhaven.hjson` (idempotent; shows a diff before
  writing). The prompt is itself in the target language; stemming + the
  existing config plumbing do the rest.
- After a bootstrap, that language is **fully supported** — detectors,
  stemming, prompts, embeddings all keyed off `cfg.language`.

**Deliverable:** any of the 18 stemmer languages reaches full detector support
with one command — no hand-curated lists to ship.

---

## P3 — Multilingual: more built-in languages (out-of-box)

Ship curated built-in lists for the most-requested additional languages
(proposed **Italian, Portuguese, Dutch**) across filter-words /
show-don't-tell / stop-words / drift pronouns, so they work with zero setup.
Lower priority than P2 (bootstrap already covers them) — a convenience for
common languages. Scope by demand.

**Deliverable:** 8 first-class out-of-box languages instead of 5.

---

## P4 — Production polish: PDF

- **N-up / booklet quick-impose** — a `--booklet` preset (2-up saddle-stitch
  signatures) atop the existing `imposition.profiles`, so a booklet doesn't
  need a hand-written profile.
- **CMYK-JPEG grayscale** — extend the existing DCTDecode grayscale
  (`transform::grayscale_jpeg`) to CMYK JPEGs (currently only RGB/gray
  handled), so a grayscale interior export doesn't choke on CMYK art.

**Deliverable:** one-flag booklets + grayscale that handles CMYK source art.

---

## P5 — Production polish: ePub

- **Inline images** — embed referenced images into the EPUB 3 package
  (manifest + spine-adjacent resources) instead of dropping them.
- **Popup footnotes** — EPUB 3 `epub:type="footnote"` + `aside` popups so
  footnotes render as reader-native popups.

**Deliverable:** ePub exports that carry images and reader-native footnotes
(the 1.2.20 carryover).

---

## P6 — Docs + 1.3.13 release cut

- **Tutorial 73** — full multilingual: the coverage matrix, `lang status` /
  `lang bootstrap`, what's automatic (stemming / prompts / embeddings) vs
  bootstrappable.
- KEYBINDING / quick-help — `Esc`-to-cancel the deep refresh.
- CONFIGURATION — the bootstrapped per-language list blocks; PDF booklet /
  ePub knobs.
- RELEASE_NOTES/1.3.13.md + index; README; version bump; signed tag; publish;
  merge; open next cycle.

---

## Notes on scope & order

This is a **wide** cut — three independent threads. Suggested order: **P0
(cancel, small) → P1 → P2 (the multilingual core) → P4/P5 (production) → P3
(more built-ins, optional) → P6**. Any thread can be trimmed or split to a
follow-up without blocking the others; the multilingual thread (P1–P2) is the
headline the others orbit.

## Out of scope (carryovers)

- **RAG / the Whole-Book AI Editor** — the **1.4** headline.
- A multi-job queue for background jobs; per-language anachronism lexicons;
  GitHub Release backfill 1.2.11–1.2.19.
