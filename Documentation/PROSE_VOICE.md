# Narrative Voice Profiling (NARR-1)

`inkhaven prose` measures **voice as a statistical property of the whole book
over time** — sentence rhythm, lexical diversity, epistemic hedging, interiority,
sensory balance, passive ratio — per chapter, **deterministically, with no LLM,
no parser, and no external dependency**. It answers a question nothing else in
Inkhaven does: *does chapter 40 read like the person who wrote chapter 1?*

It **measures; it never prescribes.** Every finding is informational — it tells
you *where* the voice moved, not that it's wrong.

Five languages are first-class: **English, Russian, German, French, Spanish.**
Each language-sensitive metric has a complete curated word list for all five; an
unsupported-language book still gets the full language-agnostic rhythm tier.

---

## The metrics

### Tier 1 — always computed (language-agnostic)

- **Sentence-length distribution** — p10/p25/p50/p75/p90 of per-sentence word
  counts. The median is the typical sentence weight; the spread is the range.
- **Coefficient of variation (CV)** — σ/μ of sentence lengths. Below ~0.35 is
  metronomic; above ~0.5 is rhythmically varied. A falling CV across chapters is
  the primary signal of a voice narrowing.
- **Burstiness B** — the Goh-Barabási index `(σ − μ)/(σ + μ)`, bounded `[−1, +1]`.
  *(Honest caveat: with σ/μ over the length distribution, B is a monotone
  transform of CV — `B = (CV−1)/(CV+1)` — so it carries the same information,
  just bounded. A genuine order/clustering metric is future work.)*
- **MATTR** — Moving-Average Type-Token Ratio (sliding 100-token window),
  length-corrected lexical diversity. A drop late in a draft can signal
  vocabulary fatigue.

### Tier 1 — language-sensitive

- **Modal density** — proportion of *epistemic-hedging* tokens (not just modal
  auxiliaries: German also uses Konjunktiv II + adverbs; French/Spanish lean on
  the conditional/subjunctive + adverbs, so the lists are lexical-hedging-broad).
  A rise = a more uncertain, distanced narrator.
- **Interiority ratio** — share of sentences accessing inner life via a
  free-indirect-discourse marker (`she thought`, `ей казалось`, `elle pensait`).
  German *erlebte Rede* additionally counts modal particles (`ja`/`doch`/`wohl`)
  in declarative sentences at half weight, reported separately as **particle
  density**.

### Tier 2 — deep pass only (`--deep`)

- **Sensory channel balance** — the proportion of tokens in each of five
  vocabularies (visual / auditory / olfactory / tactile / kinesthetic).
- **Active/passive ratio** — per-language passive detection (EN be-aux +
  participle; RU reflexive `-ся/-сь`; DE Vorgangs/Zustandspassiv; FR `être` +
  participle; ES *perifrástica*). Trend-grade (~75–90%).

---

## Using it

### CLI — `inkhaven prose`

```sh
inkhaven prose profile [--deep] [--json] [--language de]   # compute + print
inkhaven prose refresh                                     # recompute, summary only
inkhaven prose drift  [--mode baseline|rolling] [--reference <project>] [--json]
inkhaven prose suggest                                     # how to read the metrics
```

`profile` prints the book aggregate with the language in the header (`[de]`), the
German particle row, and an FR/ES disclaimer (lexical hedging only —
conditionnel/subjuntivo need a parser). `drift` shows per-chapter ΔCV / ΔMATTR /
Δmodal / Δinteriority plus the threshold crossings; `--reference` compares the
book aggregate against another project's `prose.duckdb` (with a language-mismatch
warning — rhythm metrics are unaffected).

### TUI — `Ctrl+V V`

**`Ctrl+V V`** ("Voice") runs the check in the **background** (deterministic,
content-hash lazy — only edited chapters recompute, so no cost). Any chapter
metric that drifted past its threshold vs the baseline chapter is emitted to the
**Output pane** as an informational `prose` finding that navigates to the
chapter. **`Ctrl+V Shift+V`** toggles **ambient** mode (off by default): re-run
after an editing pause, gated by a cooldown floor.

### Scripting — `ink.prose.*`

`ink.prose.{profile, drift, violations}` read the stored profiles;
`ink.prose.refresh` recomputes (returns the count). Language-sensitive metrics
return **`null`** (not `0.0`) on an unsupported-language book.

---

## Configuration

```hjson
prose: {
  deep_metrics: false        // include Tier-2 (sensory + active/passive)
  mattr_window: 100
  baseline_chapter: 1        // drift / violations measured against this chapter
  language: null             // override; null → project language → EN (with a note)
  ambient: false             // Ctrl+V Shift+V default; re-run on editing pause
  ambient_cooldown_secs: 90  // floor between ambient runs (whole-book scan)
  thresholds: {              // a crossing emits an informational `prose` finding
    sent_len_cv: 0.15, burstiness_b: 0.15, mattr: 0.05,
    modal_density: 0.020, interiority_ratio: 0.10,
    de_erlebte_rede_particle_density: 0.05,
    sensory_channel_max: 0.15, active_passive_ratio: 1.5,
  }
  extra_modal_tokens: []         // appended to the active language's modal list
  extra_interiority_phrases: []  // appended to the active language's FID list
}
```

French/Spanish authors can add subjunctive collocations they know signal hedging
via `extra_modal_tokens` without waiting for a new release.

---

## What it is *not*

- **Not AI advice.** NARR-1 never calls a model and never rewrites prose. It's a
  measurement instrument.
- **Not readability scoring** (Flesch/Lix) — those are comprehension proxies, not
  voice.
- **Not a parser.** Interiority and passive use curated phrase lists + regex
  heuristics, not syntax trees (so FR/ES morphological mood isn't detected).

Profiles live in `.inkhaven/prose.duckdb`, invalidated by a content hash; a
language change recomputes only the language-sensitive metrics (rhythm is
deterministic and preserved). Structural paragraphs (STRUCT-2) are reduced to
prose content; Jinja templates (STRUCT-1) are excluded.

---

**See also:** [Tutorial 95 — Narrative voice](Tutorials/95-narrative-voice.md) ·
[KEYBINDING.md → `Ctrl+V V`](KEYBINDING.md) · `inkhaven prose --help`.
