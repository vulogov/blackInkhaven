# Tutorial 73 — Writing in any language

*Inkhaven 1.3.13+*

Inkhaven's craft tooling — the inline style overlays, the world-consistency
scans, the drift coreference — is **language-aware**. It keys off your project
`language` field, and it works far beyond English. This tutorial shows what's
automatic, what's curated, and how to switch on a language Inkhaven doesn't yet
ship word lists for.

## Set your language

One top-level field in `inkhaven.hjson`:

```
language: russian
```

That single setting steers stemming, the style detectors, the drift pronoun
sets, and the **language the AI world-checks write their findings in**.

## Find out where you stand: `lang status`

```
inkhaven lang status
```

prints an honest coverage matrix for your project language (or pass
`--language <lang>` to check another):

```
inkhaven lang status · language: russian

  stemming                     ✓ Snowball (russian)
  filter words                 built-in 22
  show-don't-tell              built-in 71
  repeated-phrase stop-words   built-in 31
  drift pronouns (coref)       ✓ available
  anachronism lexicon          English built-ins + your `terms` (language-neutral)
  embeddings                   multilingual · MultilingualE5Small
  AI world-check output        forced in russian
  AI world-check prompts       ✓ localized (facts check / scan · drift · continuity)
```

Five languages ship fully curated: **English, Russian, French, German,
Spanish**. For those, every row is green out of the box.

## Honest fallback — no wrong-language flags

If you set a language Inkhaven *doesn't* curate (say `polish`), the detector
word lists come back **empty** — not English. That's deliberate: an empty list
gives you *no* false flags, where the old behaviour would have underlined
English filter-words in your Polish prose. Stemming (Snowball covers 18
languages), the multilingual embedding model, and the AI output language still
work; only the curated word lists are blank, and `lang status` says so:

```
  ▶ no curated detector lists for polish — run `inkhaven lang bootstrap polish`
    or add lists to inkhaven.hjson (stemming, prompts, and embeddings already work).
```

## Curate a new language: `lang bootstrap`

Let the model do the lexicography:

```
inkhaven lang bootstrap polish
```

It asks your LLM — as a careful lexicographer, answering **in Polish**, in
lemma (dictionary) form so Snowball stemming catches every inflection — for the
full detector vocabulary: filter words, the four show-don't-tell categories
(linking verbs, emotion adjectives, manner adverbs, cognition verbs),
repeated-phrase stop-words, and the three pronoun sets (character / place /
artefact). It prints a ready-to-paste HJSON snippet.

Happy with it? Patch the config automatically:

```
inkhaven lang bootstrap polish --yes
```

`--yes` writes the lists into `inkhaven.hjson` in place — with a versioned
backup first and your comments preserved. Re-run `inkhaven lang status polish`
to confirm everything's green, then tune the lists by hand whenever you like.

## The config shape

Bootstrap writes into per-language **maps** — you can also hand-author them:

```
editor: { style_warnings: {
  filter_words:    { languages: { polish: [ "tylko", "naprawdę", "bardzo", … ] } }
  show_dont_tell:  { languages: { polish: {
    linking_verbs:      [ … ]
    emotion_adjectives: [ … ]
    manner_adverbs:     [ … ]
    cognition_verbs:    [ … ]
  } } }
  repeated_phrases:{ languages: { polish: [ "i", "w", "na", "że", … ] } }
} }

drift: { pronouns: { polish: {
  character: [ "on", "ona", "oni", … ]
  place:     [ "tam", "tu", … ]
  artefact:  [ "to", "ten", … ]
} } }
```

Two complete worked examples ship in
[`custom_languages/`](../../custom_languages/): **Arabic** (right-to-left,
pro-drop, suffix pronouns) and **Hungarian** (agglutinative, no gendered third
person). Copy one as a starting point.

## The AI world-checks speak your language too

When you run the world-consistency scans (`inkhaven world --deep`, or
`Ctrl+V Shift+F` in the editor — see [Tutorial 72](72-deep-refresh.md)), the
system prompts for all four scans are **localized** to your language for the
five curated ones, and every finding's explanation is **written in your
language**. An uncurated language falls back to the English prompt with a
warning — never silently.

Want to override a prompt? There's a 3-tier cascade: a `Prompts`-book entry
(`facts-check-ru`, then `facts-check`) → `prompts.hjson` → the localized
built-in. See [`../PROMPTS.md`](../PROMPTS.md).

## Recap

- `language: <lang>` is the one switch that steers everything.
- `inkhaven lang status` tells you, honestly, what's on.
- Uncurated languages get **no** false flags, and stemming / embeddings /
  AI-output / localized-or-fallback prompts already work.
- `inkhaven lang bootstrap <lang>` curates the word lists for any language in
  one command; `--yes` patches the config safely.
- "Does it work in Russian?" — yes, fully; and now in Polish, Arabic,
  Hungarian, or anything you bootstrap.
