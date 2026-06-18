# Custom language configurations

1.3.13 lets you enable **any** of the stemmer's languages (Arabic, Hungarian,
Italian, Portuguese, Dutch, …) for the style + drift detectors by supplying
per-language word lists in `inkhaven.hjson` — not just the five with built-in
lists (English / Russian / French / German / Spanish).

The files here are **worked examples** you can paste into your project's
`inkhaven.hjson` (merging under the matching keys). They populate the
1.3.13 per-language **maps**:

- `editor.style_warnings.filter_words.languages.<lang>` — filter words
- `editor.style_warnings.show_dont_tell.languages.<lang>` — the four
  show-don't-tell lists
- `editor.style_warnings.repeated_phrases.languages.<lang>` — stop words
- `drift.pronouns.<lang>` — coref pronoun sets (character / place / artefact)

The language key is the lowercased name (`arabic`, `hungarian`) and must match
your project's top-level `language` field. Stemming, AI prompts, and embeddings
already work for these languages out of the box — these lists add the
detector vocabulary on top.

You don't have to write these by hand: **`inkhaven lang bootstrap <language>`**
generates the same structure with an LLM pass. Check coverage any time with
**`inkhaven lang status`**.

Examples:

- [`arabic.hjson`](arabic.hjson)
- [`hungarian.hjson`](hungarian.hjson)
