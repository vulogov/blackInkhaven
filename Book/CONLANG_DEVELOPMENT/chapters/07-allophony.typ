#import "../design.typ": *

#chapter(number: 7, title: "Allophony: sounds that shift")

Here is a subtle, lovely fact about real languages: a single phoneme is often
pronounced differently depending on its neighbours, without speakers even
noticing. The English /t/ in *top* is a sharp, puffed sound; the /t/ in *stop* is
softer; the /t/ in *butter* (in American speech) is a quick tap. Same phoneme,
three pronunciations. These context-dependent variants are *allophones*, and the
phenomenon is *allophony*. Adding a little of it is one of the easiest ways to
make a conlang feel alive.

#term("Allophone and allophony")[
  An *allophone* is one of the several ways a single phoneme is actually
  pronounced, chosen automatically by its surroundings. *Allophony* is the system
  of such variations. Speakers treat the allophones as "the same sound", but the
  surface pronunciation differs. Example: a /k/ that becomes a *ch*-sound before
  /i/.
]

#section("Underlying and surface forms")

To talk about allophony we need two ideas. The *underlying form* of a word is
how it is built from phonemes — its blueprint. The *surface form* is how it is
actually pronounced after the allophony rules have applied. A rule turns one into
the other.

#term("Underlying form and surface form")[
  The *underlying form* is the abstract phoneme sequence a word is made of. The
  *surface form* is the real pronunciation after allophony rules run. If /k/
  becomes *ch* before /i/, then the word with underlying /kira/ has the surface
  form *chira* — but it is still "the /k/ word" underneath.
]

#section("Writing a rule")

Allophony rules use a compact notation borrowed from linguistics, sometimes
called *SPE notation*. A rule has four parts:

```text
WHAT  >  BECOMES  /  LEFT _ RIGHT
```

Read it as: "*WHAT* becomes *BECOMES* when it sits between *LEFT* and *RIGHT*".
The underscore `_` marks the spot where the changing sound sits. So:

```text
k > tʃ / _ i
```

means "/k/ becomes /tʃ/ (the *ch* sound) when an /i/ follows it" — the left side
of the context is empty, the right side is /i/. A few more building blocks:

#set enum(numbering: "1.")
+ A `#` stands for a *word boundary* — the start or end of a word. `p > f / _ #`
  means "/p/ becomes /f/ at the end of a word".
+ An empty side means "no condition there". `_ i` cares only about what follows.
+ A context token may be a *class name* (like `V` for any vowel) rather than a
  single sound: `k > h / V _ V` means "/k/ becomes /h/ between two vowels".

#term("SPE notation")[
  A standard shorthand for sound rules, named after a famous 1968 linguistics
  book (*The Sound Pattern of English*). The form is `target > result / left _
  right`, where `_` is the target's position, `#` is a word edge, and a class
  name matches any sound in that class. Inkhaven uses it for both allophony and,
  later, historical sound change.
]

#section("Adding allophony to Eldar")

You declare rules under an `allophony` field in the phonology block. Each rule
has an optional `name` (for your own reference) and the `rule` itself:

```hjson
allophony: [
  { name: "palatalization", rule: "k > tʃ / _ i" }
  { name: "lenition",       rule: "t > s / _ i" }
]
```

Add these, reindex, and check the effect with the *ipa* inspector, which shows a
word's *surface* form:

```sh
inkhaven language ipa Eldar --word kira
```

The underlying *kira* comes back pronounced *tʃira* — the /k/ has palatalized
before the /i/. From now on, every word Inkhaven generates or inflects will have
these rules applied automatically, so the variation is consistent everywhere.

#callout(label: "Allophony rules apply in order")[
  The rules run top to bottom, and an earlier rule can feed a later one. Order
  them thoughtfully, and use the `ipa` inspector to confirm a tricky word comes
  out the way you intend. A little allophony goes a long way — two or three
  well-chosen rules are plenty for a natural feel.
]

#callout(label: "Why this matters later")[
  Allophony is not just decoration. The same rules fire when you add grammatical
  endings to words (Part IV) — so a suffix can trigger a sound change at the
  join, exactly as in real languages. And the *historical* sound changes of
  Part V use this very same notation. Learn it once; use it three times.
]

#recap((
  [An *allophone* is a context-dependent pronunciation of a phoneme; the system
   is *allophony*.],
  [The *underlying form* is the blueprint; the *surface form* is the real
   pronunciation after rules apply.],
  [Rules use *SPE notation*: `target > result / left _ right`, with `_` the
   target, `#` a word edge, and class names matching families.],
  [Declare them under `allophony`; check results with `ipa --word`.],
  [The same notation reappears for grammar boundaries and for historical change.],
))
