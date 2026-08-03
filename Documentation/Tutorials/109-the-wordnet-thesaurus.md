# Tutorial 109 — The WordNet Thesaurus

*Inkhaven 1.8.3*

A thesaurus that offers "big → large, huge, enormous" without knowing which sense
you meant is a blunt instrument. Inkhaven's thesaurus is **sense-based**: it
looks a word up in WordNet, shows you its distinct senses, and offers the
synonyms / antonyms / hypernyms / hyponyms *for the sense you pick* — for your
manuscript prose, in your language.

## Install a dictionary

```sh
inkhaven wordnet list                 # what's available / installed
inkhaven wordnet fetch en             # download + index open WordNet (English)
```

English fetches openly today; French, German, and Spanish arrive via the OMW
sources. For data that can't be openly fetched (Russian), build the index from a
local WN-LMF file:

```sh
inkhaven wordnet import ru path/to/russian-wordnet.xml.gz
```

The index lives under your data directory (`<data_dir>/inkhaven/wordnet/`), shared
across projects.

## Look a word up

```sh
inkhaven wordnet lookup bank
```

```
bank  (noun)
  · sense 1 — a financial institution
      syn: depository, banking company   hyper: financial institution
  · sense 2 — sloping land beside a body of water
      syn: riverside, embankment         hyper: slope
```

Two senses, two different synonym sets — you choose the one that fits.

## In the editor — `Ctrl+V Shift+Y`

Put the cursor on a word (or select it) and press **`Ctrl+V Shift+Y`**. The
thesaurus opens on that word's senses; pick a sense, then a synonym / antonym /
related word, and it's **replaced in place** — no retyping, and the Typst markup
around it is preserved.

## Cross-lingual and AI fallback

WordNet's interlingual index means senses connect across languages: "what's the
German word for this Russian concept?" is a lookup on the shared sense. And for a
language with no installed WordNet and nothing to download (Russian, if you
haven't imported one), the chord falls back to the AI — so the workflow is the
same everywhere, keyed to your project language.

The thesaurus also feeds the knowledge graph: `inkhaven graph lexical` imports
the words your manuscript uses, their senses, and the local semantic net
(hypernym / hyponym / antonym) as graph edges (Tutorial 104).

---

**See also:** [KEYBINDING.md → `Ctrl+V Shift+Y`](../KEYBINDING.md) ·
Tutorial 104 (the knowledge graph) · `inkhaven wordnet --help`.
