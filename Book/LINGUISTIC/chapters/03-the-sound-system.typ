#import "../design.typ": *

#chapter(number: 3, title: "The sound system")

With an inventory declared and a lexicon growing, we can ask the first empirical
questions. How large and how balanced is Russian's sound system? What contrasts does
it lean on? And where does a tool built around the IPA meet a language modelled in
Cyrillic? This chapter is the workbench doing phonology.

#section("The shape of the inventory")

`stats` already told us the size — thirty-three segments, twenty-three consonants to
ten vowels. That ratio is itself a finding: Russian is *consonant-heavy*, and the
reason is the hard/soft contrast. Most Russian consonants come in pairs, one *plain*
and one *palatalized* (pronounced with the tongue raised toward the palate), and that
doubling roughly halves the work the vowels have to do. In the orthography the
contrast is carried partly by the consonant and partly by the *following vowel
letter*: the pairs `а`/`я`, `о`/`ё`, `у`/`ю`, `э`/`е` and the sign `ь` are how Russian
spelling writes "the consonant before me is soft."

#section("Counting the sounds")

`metrics` reads the lexicon and reports the sound system quantitatively:

```sh
inkhaven language metrics Russian
```

```
  entropy   · 4.31 bits (max 5.04) · evenness 85% · perplexity 19.8
  zipf      · slope -0.91 (≈−1 is Zipfian) · fit R² 0.94
  syllables · 41 attested / 210 possible · saturation 20%
  prosody   · 1.9 moras/word · 62% heavy syllables
```

*Entropy* measures how evenly the language spreads its words across its sounds — high
evenness means no handful of letters carries everything. The *Zipf* slope near −1
says the letter frequencies follow the same power law natural language always does
(a reassuring sign the model behaves like a real language). *Saturation* asks how
much of the possible syllable space the language actually uses — a low figure is
normal; no language uses more than a fraction of what its phonotactics permit.

#term("Phoneme entropy")[
  A measure, in bits, of how unpredictable the next sound is — highest when every
  sound is equally likely, lower when a few dominate. It captures the *balance* of an
  inventory: a language that leans hard on a handful of sounds has low entropy, one
  that uses its whole inventory evenly has high entropy.
]

#section("The contrasts Russian leans on: minimal pairs")

The sharpest tool for a sound system is the *minimal pair* — two different words that
differ in exactly one sound. Each minimal pair proves that the one sound they differ
in is *contrastive*: swapping it changes the word.

```sh
inkhaven language pairs Russian
```

Russian is famously full of them, and the richest source is exactly the hard/soft
contrast:

```
  examples:
      мать / мять    а~я    (mother / to crumple)
      был / бил      ы~и    (was / he beat)
      дом / том      д~т    (house / volume)
```

`мать`/`мять` is the classic case: the words differ only in the vowel letter, `а`
versus `я`, which in Russian spelling signals a hard `т` versus a soft `тʲ`. That one
orthographic difference is a genuine phonemic contrast, and the language is built on
hundreds like it.

#term("Minimal pair")[
  Two words identical but for a single sound in the same position (*pat* / *bat*,
  *мать* / *мять*). Minimal pairs are the primary evidence that two sounds are
  separate phonemes in a language rather than variants of one — if swapping them can
  change a word, the language must distinguish them.
]

#section("Where the IPA shows through")

Ask the workbench to judge the inventory's *naturalness*, and the seam appears:

```sh
inkhaven language naturalness Russian
```

```
  outside · а е ё и о у … (not in the feature matrix)
  score   · 0.35 (0–1; higher = more typologically ordinary)
```

The low score is not a verdict on Russian — Russian is as natural as a language gets.
It is the tool telling the truth about its own limits. Naturalness, and the
functional-load half of the minimal-pairs report, are computed from a table of
*distinctive features* (voicing, place, manner) that is keyed to IPA symbols. Our
segments are Cyrillic *letters*, which that table has never heard of — so it reports
them "outside the feature matrix" and cannot grade them.

#section("Building a phonemic model in the IPA")

To make the feature tools work, give Russian a second inventory written in the
International Phonetic Alphabet. The move is simple: each phoneme's `ipa` field holds
its IPA symbol, and its `romanize` field holds the Cyrillic letter you want it *shown*
as. Declare it in the Phonology chapter of a parallel language (call it `RussianIPA`)
just as before, but phonemically:

```hjson
{ phonemes: [
    { ipa: "a", kind: "vowel",     romanize: "а" }
    { ipa: "ɨ", kind: "vowel",     romanize: "ы" }
    { ipa: "p", kind: "consonant", romanize: "п" }
    { ipa: "b", kind: "consonant", romanize: "б" }
    { ipa: "s", kind: "consonant", romanize: "с" }
    { ipa: "z", kind: "consonant", romanize: "з" }
    { ipa: "ɡ", kind: "consonant", romanize: "г" }
    // …the other vowels, consonants and the palatalized pairs…
] }
```

The `romanize` field is the bridge: the model reasons over the IPA symbol `p`, but any
form it prints shows the Cyrillic `п`. You would transcribe the lexicon phonemically
too — `дом` becomes `dom`, `был` becomes `bɨl` — so that words and inventory share the
IPA level.

#section("Before and after")

Now ask the same question of the phonemic model:

```sh
inkhaven language naturalness RussianIPA
```

```
  inventory  · 24 phonemes (18 C / 6 V) — typical
  voicing    · p/b  t/d  s/z  ʃ/ʒ
  places     · labial coronal dorsal
  missing    · w (near-universal)
  score      · 0.94 (0–1; higher = more typologically ordinary)
```

The score jumps from 0.35 to *0.94*, and — more to the point — the analysis becomes
real. Where the Cyrillic model could say nothing, the phonemic one recognizes that
Russian's obstruents come in neat voicing pairs (`p`/`b`, `t`/`d`, `s`/`z`, `ʃ`/`ʒ`),
that its consonants cover the major places of articulation, and that it lacks only the
near-universal `w` — a fair and accurate portrait of a thoroughly ordinary, thoroughly
natural sound system. The minimal-pairs report changes too: `мать`/`мять` is no longer
"outside the feature matrix" but a contrast in *palatalization*, and the functional
load of each feature can finally be weighed.

#callout(label: "Two models, two jobs")[
  This is the pattern for studying any language whose script is not the IPA: keep an
  *orthographic* model for the work that lives in the writing system — glossing, the
  corpus, syntax over real text — and a *phonemic* model in the IPA for the work that
  lives in the sounds — naturalness, distinctive features, functional load. Each is
  right for its half of the subject; the mistake is expecting one model to do both.
]

#section("Where each sound lives")

`distribution` reports where in the word each segment tends to appear — onset,
nucleus, coda, word edges — and flags any with a restricted distribution:

```sh
inkhaven language distribution Russian
```

For Russian this surfaces real facts of the orthography: `ъ` never begins a word and
`ы` never does either, while the soft sign `ь` clusters at word end. Restricted
distributions like these are the fingerprints of a sound system, and reading them off
your own model is how you learn to see them.

#recap((
  [Russian's inventory is consonant-heavy (23 C / 10 V) because most consonants come
   in a plain/palatalized pair — the hard/soft contrast, written partly by the
   following vowel letter.],
  [`metrics` quantifies the system (entropy, the Zipf fit, syllable saturation,
   mora weight); `pairs` finds the minimal pairs that prove which sounds contrast —
   `мать`/`мять` chief among them.],
  [`naturalness` and functional load read an IPA-keyed feature table, so a Cyrillic
   *orthographic* model falls "outside the feature matrix": for feature analysis you
   would build a parallel IPA model; for everything else the orthographic one serves.],
))
