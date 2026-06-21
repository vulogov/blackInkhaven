#import "../design.typ": *

#chapter(number: 8, title: "Stress, spelling, and tone")

Three finishing touches complete your sound system: where the emphasis falls in a
word (*stress*), how you write the language with ordinary letters
(*romanization*), and — for some languages — meaningful pitch (*tone*). The first
two you will almost certainly want; the third is optional.

#section("Stress")

In most languages, one syllable of a word is said a little louder or longer than
the others. That is *stress*. English stress is unpredictable (*REcord* the noun
versus *reCORD* the verb), but many languages place it by a simple rule — always
the first syllable, or always the next-to-last.

#term("Stress")[
  The relative emphasis given to one syllable of a word, by loudness, length, or
  pitch. A *fixed* stress rule places it predictably; common rules are *initial*
  (first syllable), *final* (last), *penultimate* (second-to-last), and
  *antepenultimate* (third-to-last).
]

You set a stress rule with the `stress` field in the phonology block. The value
is one of `initial`, `final`, `penultimate`, `antepenultimate`, or `latin`. That
last one is *weight-sensitive*: it stresses the second-to-last syllable when that
syllable is *heavy* (it has a long vowel or ends in a consonant) and otherwise
moves the stress one syllable earlier — the rule classical Latin used. (See
*syllable weight* in the glossary.) Eldar will simply stress the second-to-last
syllable:

```hjson
stress: "penultimate"
```

Check it with the *stress* inspector, which marks the stressed syllable:

```sh
inkhaven language stress Eldar --word takina
```

It returns something like `ta.ˈki.na` — the mark `ˈ` sits before the stressed
syllable.

#section("Romanization: writing it down")

Your phonemes may include symbols like /ʃ/ that you do not want to type
constantly. *Romanization* is the system for writing the language with ordinary
Latin letters — the *romanize* spellings you already gave each phoneme are the
simple version of this. For anything trickier, you can define a named
*romanization scheme*.

#term("Romanization")[
  A way of writing a language's sounds using the Latin alphabet (the letters
  a–z). The simplest romanization gives each phoneme a spelling; a fuller scheme
  can spell one sound several ways depending on context, the way Italian writes
  /k/ as *c* before *a* but *ch* before *e*.
]

A scheme maps phonemes to letters, and can include *contextual* rules — a
spelling that depends on the surrounding sounds. Suppose you want /k/ and /s/
both written `c`, with `c` read as /s/ before a front vowel (as in Italian or
English *city*):

```hjson
romanizations: [
  { name: "default"
    mappings: [ { ipa: "k", roman: "c" }, { ipa: "s", roman: "c" } ]
    contextual: [ { roman: "c", ipa: "s", before: "FrontV" } ] }
]
default_romanization: "default"
```

You can then convert text either way with the *romanize* inspector:

```sh
inkhaven language romanize Eldar --text cace
inkhaven language romanize Eldar --text /kase/ --reverse
```

The first reads spelled text into sounds; the second (`--reverse`) writes sounds
back out as spelling — note the slashes around `/kase/`, the usual way of marking
that the input is a string of sounds (IPA) rather than ordinary spelling. For
most languages the simple per-phoneme `romanize` fields are all you need, and you
can skip schemes entirely.

#section("Tone (optional)")

Some languages — Mandarin, Yoruba, Thai — use *pitch* to distinguish words: the
same syllable said with a rising versus a falling pitch means different things.
That is *tone*. If your language has no tone, skip this section; most conlangs
do.

#term("Tone")[
  The use of pitch (the rise and fall of the voice) to distinguish word
  meanings. In a *tonal* language, *ma* on a high pitch and *ma* on a falling
  pitch are different words. Tones can also interact: a *tone sandhi* rule
  changes one tone next to another.
]

To add tone, declare a `tone` block with the tones you use and any *sandhi* rules
(written in the same SPE notation as allophony):

```hjson
tone: {
  kind: "contour"
  tones: ["1", "2", "3", "4"]
  sandhi: [ { rule: "3 > 2 / _ 3" } ]
}
```

The `kind` says what sort of tones these are. A *contour* tone glides during the
syllable (rising, falling, dipping) — the Mandarin style, where the numbers name
shapes; the alternative, `register`, is for tones held level at a fixed pitch
(high, mid, low). This example gives four contour tones (numbered) and one
*sandhi* rule — "a third tone becomes a second tone before another third tone",
the famous Mandarin pattern. Apply it to a sequence of tone numbers with:

```sh
inkhaven language tone Eldar --tones "3 3 3"
# → 2 2 3   (each 3 before another 3 lowers to 2; the last one is unchanged)
```

#callout(label: "Your phonology is done")[
  With sounds, syllable shapes, phonotactics, allophony, stress, and (optionally)
  romanization and tone, your sound system is complete. Everything from here on —
  words, grammar, history, writing — is spoken with these sounds and obeys these
  rules automatically. This is the foundation the whole language stands on.
]

#recap((
  [*Stress* is the emphasised syllable; set a fixed rule with the `stress` field
   (`initial`, `final`, `penultimate`, …).],
  [*Romanization* writes the language in Latin letters; per-phoneme `romanize`
   spellings suffice for most languages, with `romanizations` schemes for harder
   cases.],
  [*Tone* (optional) uses pitch to distinguish words; declare a `tone` block with
   tones and `sandhi` rules.],
  [Inspectors `stress`, `romanize`, and `tone` let you check each at a word.],
))
