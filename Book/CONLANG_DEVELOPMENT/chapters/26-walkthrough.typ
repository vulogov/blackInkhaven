#import "../design.typ": *

#chapter(number: 25, title: "A complete walkthrough")

This chapter builds a whole small language — *Avesha* — from nothing to finished
books, in one unbroken sequence. Follow along command by command and you will
practise every pillar of the book at once. Everything here uses what the earlier
chapters explained; refer back whenever a step needs more detail.

#callout(label: "There is a script for this")[
  Inkhaven ships a runnable script that performs this entire walkthrough
  automatically — `examples/conlang/build-sample-language.sh`, with a checked-in
  sample of its output in `examples/conlang/sample-output/`. Read this chapter to
  understand each step; run the script to watch it happen.
]

#section("1. Project and language")

```sh
inkhaven init ~/avesha-project
cd ~/avesha-project
inkhaven language init Avesha
```

#section("2. Phonology")

Create a file in Avesha's `04-phonology` folder with the sound system — eleven
sounds, syllable shapes, a couple of constraints, two allophony rules, and
penultimate stress:

```hjson
{
  phonemes: [
    { ipa: "p", kind: "consonant" } { ipa: "t", kind: "consonant" }
    { ipa: "k", kind: "consonant" } { ipa: "s", kind: "consonant" }
    { ipa: "m", kind: "consonant" } { ipa: "n", kind: "consonant" }
    { ipa: "l", kind: "consonant" } { ipa: "r", kind: "consonant" }
    { ipa: "a", kind: "vowel" } { ipa: "i", kind: "vowel" } { ipa: "u", kind: "vowel" }
  ]
  classes: { C: ["p","t","k","s","m","n","l","r"], V: ["a","i","u"] }
  templates: { root: [ { pattern: "C V" }, { pattern: "C V C" } ] }
  constraints: [ { kind: "no_geminate" }, { kind: "max_cluster_size", value: 2 } ]
  allophony: [ { rule: "t > s / _ i" }, { rule: "n > m / _ p" } ]
  stress: "penultimate"
}
```

Then adopt it and check:

```sh
inkhaven reindex --adopt
inkhaven language stats Avesha
```

#section("3. A vocabulary")

```sh
inkhaven language add-word Avesha pata --type noun --translation stone
inkhaven language add-word Avesha kira --type noun --translation bird
inkhaven language add-word Avesha suna --type noun --translation sun
inkhaven language add-word Avesha nami --type verb --translation see
inkhaven language add-word Avesha palu --type verb --translation run
inkhaven language add-word Avesha mira --type adjective --translation bright
inkhaven language audit Avesha
```

#section("4. Grammar")

Add a morphology block to the `03-grammar` folder — a dative and a plural suffix,
and an agent-noun derivation:

```hjson
{
  morphemes: [
    { id: "dat", gloss: "DAT", form: "ti", position: "suffix" }
    { id: "pl",  gloss: "PL",  form: "u",  position: "suffix" }
  ]
  derivations: [
    { name: "agent", form: "ar", position: "suffix",
      from_pos: "verb", to_pos: "noun", gloss_template: "one who {}s" }
  ]
}
```

Record the typological choices and reindex:

```sh
inkhaven reindex --adopt
inkhaven language grammar Avesha --set word_order=sov
inkhaven language grammar Avesha --set alignment=nominative_accusative
inkhaven language grammar Avesha --set case=yes
```

Watch the allophony fire at an affix boundary — *pata* plus the dative *-ti*:

```sh
inkhaven language paradigm Avesha --root pata --gloss stone
```

The dative form comes out *patasi*: the /t/ of the root softened to /s/ before
the /i/ of the suffix, by the rule you wrote in step 2.

#section("5. A writing system")

Draw or AI-draft one glyph per phoneme, bind each to its sound and a Private Use
codepoint, then compile the font. With AI drafting:

```sh
inkhaven language glyph-draft Avesha --describe "a bold vertical post" \
    --phoneme p --codepoint U+E000 --name p --provider deepseek --yes
# … one per phoneme …
inkhaven language font-config Avesha
inkhaven language font-build --language Avesha --format ttf --out Avesha
```

Type a word in the new script:

```sh
inkhaven language transliterate Avesha --text kira
```

#section("6. The books")

```sh
inkhaven language dictionary   Avesha --format typ --font Avesha --out dict.typ
inkhaven language grammar-book Avesha --format typ --font Avesha --study \
    --provider deepseek --out grammar.typ
inkhaven language tutorial     Avesha --format typ --font Avesha \
    --provider deepseek --out learn.typ
```

Compile any of them to a PDF that embeds your font:

```sh
typst compile --font-path . dict.typ dict.pdf
```

#section("You have built a language")

Look back over what you did. You chose a sound system and tuned it by ear. You
coined words and kept them consistent. You built grammar that inflects words and
coins new ones, with sound changes firing automatically at the seams. You set the
language's typological character. You designed an alphabet and compiled it into a
real font. And you produced a dictionary, a grammar, and a textbook — printable
books in your language's own script.

That is a complete constructed language, made from nothing. From here you can go
as deep as you like: hundreds more words, a proto-language and a whole family
(Part V), idioms and metaphors, a richer script. The tools are the same ones you
have now used; only the scale changes.

#recap((
  [The whole journey is six steps: project, phonology, vocabulary, grammar,
   writing system, books.],
  [Each step uses commands from earlier chapters; the allophony, dedup gate, and
   font compiler all work together automatically.],
  [The shipped `build-sample-language.sh` runs this end to end.],
  [You now have everything you need to build your own language from zero.],
))
