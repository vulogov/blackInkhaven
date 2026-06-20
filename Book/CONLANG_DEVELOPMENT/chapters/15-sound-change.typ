#import "../design.typ": *

#chapter(number: 15, title: "Sound change and proto-languages")

Real languages have a past. Latin slowly became French, Spanish, and Italian; the
single sound /k/ in Latin *centum* became the /s/ of French *cent* and stayed a
/k/ in others. Languages change, and the most regular kind of change is in their
sounds. Giving your language a history — descending it from an older
*proto-language* by a chain of sound changes — is what turns a flat invention
into something with depth. This part is optional, but it is where conlanging
becomes most rewarding.

#term("Proto-language")[
  An older, ancestral language that later languages descend from. Latin is the
  proto-language of the *Romance* family (French, Spanish, Italian, …).
  Proto-languages are sometimes real and recorded (like Latin) and sometimes
  *reconstructed* — worked out backward from their descendants, like
  Proto-Indo-European, which no text records.
]

#term("Sound change")[
  A regular shift in pronunciation that spreads through a language over time,
  affecting every word that meets its conditions — for example, every /p/ at the
  end of a word becoming /f/. Because sound changes are *regular*, they are
  predictable, and they are the engine that turns one language into several.
]

#section("Sound change is just allophony over time")

Here is a piece of good news: a historical sound change is written *exactly* like
an allophony rule (Chapter 7). The same `target > result / left _ right` notation
— *SPE notation* — applies. The only difference is in the telling — allophony is variation happening
*now*, within one language; a sound change is a shift that happened *over
generations*, turning a parent language into a child.

#section("Declaring a descent")

You give a language a parent by adding a `diachronics` block to its *Phonology*
chapter. It names the `proto` (the parent language) and an ordered list of the
sound-change `rules` that turned the parent into this language:

```hjson
{ diachronics: {
    proto: "ProtoEldarin"
    rules: [
      { rule: "p > f / _ #" }
      { rule: "k > h / V _ V" }
    ]
} }
```

This says Eldar descends from a language called *ProtoEldarin*, and two changes
happened on the way: every /p/ at the end of a word became /f/, and every /k/
between two vowels became /h/. (You would, of course, first create *ProtoEldarin*
as its own language with `language init`, and give it a vocabulary.)

#callout(label: "The rules are defined on the parent's sounds")[
  The changes describe what happened to the *proto-language's* sounds, so
  Inkhaven uses the proto's inventory to read the words before applying the
  chain. Build the proto first — its phonology and at least some words — then
  describe how the child diverged from it.
]

#section("Evolving a word")

Watch a single proto-word travel down the chain to its modern form:

```sh
inkhaven language sound-change Eldar --form tap
```

Inkhaven applies the rules in order and prints the result — `tap > taf` (the
final /p/ became /f/). Try it with several words to feel how the chain reshapes
the language.

#section("Evolving the whole vocabulary")

The real payoff: take the proto-language's *entire* dictionary and evolve it,
producing the daughter's vocabulary in one step — each word transformed by the
sound changes, with its meaning carried forward and its origin recorded:

```sh
inkhaven language derive-lexicon Eldar --yes
```

Without `--yes` it only proposes; with it, the words are added to Eldar's
dictionary, each remembering the proto-word it came from. In moments you have a
vocabulary that is *systematically related* to its ancestor — the hallmark of a
real language family, which the next chapter explores.

#recap((
  [A *proto-language* is an ancestor; *sound changes* are the regular shifts that
   turn it into descendants.],
  [A sound change uses the same SPE notation as allophony — change over
   generations rather than variation now.],
  [Declare descent with a `diachronics` block: a `proto` and an ordered list of
   `rules`.],
  [`sound-change --form` evolves one word; `derive-lexicon` evolves the proto's
   whole dictionary into the daughter's.],
))
