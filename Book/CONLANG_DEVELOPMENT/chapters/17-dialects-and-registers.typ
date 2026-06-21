#import "../design.typ": *

#chapter(number: 17, title: "Dialects and registers")

No real language is spoken just one way. A farmer in the hills and a courtier in
the capital share a language, yet a listener can place each of them in a sentence
or two — by a vowel here, a different word there. The same speaker, too, talks
differently to a magistrate than to a child. These systematic ways one language
varies — across regions, across classes, across formality — are what make a
tongue feel inhabited rather than printed. This chapter shows how to give your
language varieties, and the single idea that makes them almost free to build.

#term("Variety")[
  A consistent, recognisable *way of speaking* one language — a dialect, a
  register, a class accent. A variety is not a separate language; it is the base
  language with a set of differences. Standard English, Scots English, and
  legalese are three varieties of one language.
]

#term("Dialect")[
  A variety tied to a *place* or community: the speech of a region, distinguished
  by its sounds, words, and sometimes grammar. Mutually intelligible with the
  rest of the language — that is what separates a dialect from a sister language.
]

#term("Register")[
  A variety tied to the *situation* rather than the speaker: formal versus casual,
  ceremonial versus intimate. The same person commands several registers and
  switches between them by setting.
]

#section("The one idea: a variety is a delta")

A variety is stored not as a whole second language but as a *delta* — a short
list of the differences from the base. And here is the idea that makes the whole
pillar cheap:

#callout(label: "A dialect is sound change, happening now")[
  The sound differences that mark a dialect are *exactly* an ordered list of
  sound changes — the same `target > result / left _ right` notation, the same
  engine, as the historical changes of Chapter 15. The only difference is in the
  telling. A *daughter language* applies its changes *diachronically*, across
  generations, and becomes a new language. A *dialect* applies its changes
  *synchronously*, here and now, and stays the same language. One engine, two
  framings.
]

So everything you learned about sound change you already know about dialects. A
lowland dialect in which /t/ softens to /d/ between vowels is one rule:
`t > d / V _ V`. That is the whole dialect, as far as its sounds go.

#section("Declaring varieties")

Varieties live in a `varieties` block in the language's *Grammar* chapter. Each
has an `id`, a `kind` (`dialect`, `register`, `sociolect`, or `idiolect`), an
`axis` saying what it varies along, an optional `prestige`, its `sound_changes`,
and optional word `lexicon` overrides:

```hjson
{ varieties: [
  { id: "lowland", kind: "dialect", axis: "region", prestige: "low",
    sound_changes: [ { rule: "t > d / V _ V" } ],   // SPE, as in diachronics
    lexicon: { "water": "móru" } }                  // a suppletive override
  { id: "high", kind: "register", axis: "formality", prestige: "high",
    sound_changes: [ { rule: "k > q / # _" } ] }
] }
```

The first is a low-prestige regional dialect with one sound change and one word
swapped outright; the second is a high register that hardens word-initial /k/ to
/q/. The two `kind`s you have not met yet name finer cuts:

#term("Sociolect")[
  A variety tied to a *social group* — class, trade, generation — rather than a
  region. The clipped speech of an aristocracy and the slang of an apprentices'
  guild are sociolects.
]

#term("Idiolect")[
  The way *one individual* speaks: their personal blend of dialect, register, and
  habit. Every character in your world has one; Chapter 19 builds idiolects from
  a character's background.
]

#section("Two ways a variety differs")

A variety changes a word in one of two ways. Most differences are *regular* —
produced by the sound changes, applying to every word that meets their
conditions. But a variety may also simply *use a different word* for a meaning,
unrelated to any sound rule. That is a `lexicon` override:

#term("Suppletion (override)")[
  A form that does not follow from the regular rules but is supplied outright — as
  English uses *went* for the past of *go*, an unrelated word. A variety's
  `lexicon` overrides are suppletive: the lowland word for "water" is *móru* not
  because a sound change produced it, but because the lowlanders simply say
  *móru*.
]

#section("Rendering speech in a variety")

With varieties declared, you can render any word or line of text *as a given
variety would say it*:

```sh
inkhaven language varieties Eldar                       # list them
inkhaven language lect Eldar lowland --word kata        # → kada  (t > d / V _ V)
inkhaven language lect Eldar lowland --text "kata tira" # word by word
```

`varieties` lists each variety with its axis, prestige, and the size of its
delta. `lect` (from *dialect*) renders a form or a whole line in a chosen
variety: the base word *kata* comes out *kada* in the lowland dialect, because
the /t/ sits between two vowels. `lect` also reports the base-to-variety
difference, so you can see exactly what changed.

#section("The dialectology table")

The classic way linguists display variation is a table: each headword down the
side, each variety across the top, so the eye can run along a row and watch a
word shift from dialect to dialect. Inkhaven prints exactly that:

```sh
inkhaven language dialects Eldar [--count N]
```

Each row is a base word and its form in every variety; a trailing `*` marks a
word that was *overridden* outright rather than derived by sound change. This is
the single most satisfying view of the pillar — your one language, fanned out
into a living spread of ways to say the same thing.

#section("Letting the AI propose a dialect")

Inventing a coherent set of sound changes for a dialect — ones that hang together
and suit a culture — is creative work the AI can seed. Describe the flavour you
want and let it propose:

```sh
inkhaven language propose-dialect Eldar --describe "a harsh mountain dialect" [--yes]
```

The model suggests a set of sound changes and a few lexical swaps that fit the
description. Crucially, *the AI only proposes*: each rule it offers is validated
against your actual phoneme inventory and previewed through the variety engine
above, so nothing illegal can slip in. Without `--yes` it only shows you the
preview; with `--yes` it writes the variety into your Grammar chapter, ready for
`lect` and `dialects`. This is the advisory pattern you saw with `reconstruct`
and `realism-check`: the AI brings ideas, the deterministic engine keeps them
legal, and nothing is written without your word.

#recap((
  [A *variety* is a consistent way of speaking one language — a *dialect*
   (region), *register* (situation), *sociolect* (group), or *idiolect*
   (person) — stored as a small *delta* on the base.],
  [The headline: a dialect's sound differences are an ordered list of sound
   changes (Chapter 15's engine) applied *synchronously* rather than across
   generations.],
  [Declare varieties in a `varieties` block; a variety differs by regular
   `sound_changes` and by suppletive `lexicon` overrides.],
  [`lect` renders a form or text in a variety; `dialects` prints the comparison
   table (a `*` marks an override).],
  [`propose-dialect` (AI) suggests a coherent dialect; each rule is validated and
   previewed, and nothing is saved without `--yes`.],
))
