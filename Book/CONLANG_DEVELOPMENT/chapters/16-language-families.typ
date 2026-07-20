#import "../design.typ": *

#chapter(number: 16, title: "Language families")

Give one proto-language *two* sets of sound changes and you have two sister
languages — a *family*. French and Spanish are sisters, both children of Latin,
recognisably related yet distinct. This chapter shows how to grow and explore a
family: how related words line up across the daughters, how to draw the family
tree, and how Inkhaven can even reconstruct a lost ancestor or judge whether your
history is believable.

#term("Language family")[
  A group of languages descended from a common proto-language. Members are
  *related*; a word that survives from the ancestor into several daughters
  appears in each as a *cognate*. Examples: the Romance family (from Latin), the
  Germanic family (English, German, Dutch, …).
]

#term("Daughter language")[
  A language that descends from a given proto-language. French and Spanish are
  daughters of Latin. You make a daughter by creating a language whose
  `diachronics` block names the proto and gives the sound changes that produced
  it (Chapter 15).
]

#section("Cognates: the same word, evolved")

When a proto-word survives into several daughters, its descendants are
*cognates* — the "same word", reshaped differently by each daughter's sound
changes. Latin *centum* gives French *cent*, Spanish *ciento*, Italian *cento*:
cognates all. Inkhaven traces a proto-form's reflex in every daughter at once:

```sh
inkhaven language cognates ProtoEldarin --form takap
```

For the proto-form *takap* it applies each daughter's chain and prints the result
in each — perhaps `Eldar takaf` versus `Sindarin tahaf`. Lining up cognates this
way is exactly how historical linguists study real families, and it is deeply
satisfying to watch your own languages diverge from a shared root.

#term("Cognate")[
  A word in one language that descends from the same ancestral word as a word in
  a related language. English *father*, German *Vater*, and Latin *pater* are
  cognates — all from one Proto-Indo-European word. Cognates are how relatedness
  is proven, and how the shape of the proto is recovered.
]

#section("The family tree")

To see the whole family laid out — every language under its declared ancestor —
draw the tree:

```sh
inkhaven language family-tree
```

It prints a genealogical diagram of your languages, each nested under its
`proto`, like this:

```
ProtoEldarin
├─ Eldar
│  └─ LowEldar
└─ Sindarin
```

Here *Eldar* and *Sindarin* both descend from *ProtoEldarin*, and *LowEldar* is a
daughter of *Eldar* — a granddaughter of the proto. As you add daughters and
granddaughters, the tree grows to show the full shape of your invented family.

#section("Looking backward: reconstruction")

Historical linguists often work in the other direction: given several cognates,
they *reconstruct* the ancestral form that must have produced them. Inkhaven can
do this with AI — propose the proto-form from a set of daughter forms:

```sh
inkhaven language reconstruct --forms "tava taba" --gloss water
```

Given the cognate forms *tava* and *taba* (both meaning "water"), it proposes the
most plausible ancestor they could both come from, and explains the reasoning.
This is AI-assisted and advisory — a creative aid for designing a deep history.

#term("Reconstruction")[
  Working out the form of an unrecorded ancestral word from its surviving
  descendants, by reversing the sound changes that produced them. The starred
  forms in linguistics books (*\*pater*) are reconstructions. Inkhaven's
  `reconstruct` proposes one for you from cognate forms.
]

#section("Is your history believable?")

Finally, you can ask the AI whether the chain of sound changes you invented is
*typologically plausible* — whether each change is a natural kind that real
languages actually undergo, and whether the order makes sense:

```sh
inkhaven language realism-check Eldar
```

It assesses your sound-change chain and flags anything unnatural, so you can make
a history that feels real. Like reconstruction, it is advisory: a knowledgeable
second opinion, not a verdict.

#section("Keeping a register of hypotheses")

Serious historical work is an accumulation of *claims*: "\*k became tʃ before front
vowels in the Northern branch"; "these two daughter forms are cognate"; "this word
is a loan". Each has evidence, and each is either holding up or falling apart as you
find more of it. Inkhaven can keep that reasoning rather than leaving it in your
head. `inkhaven language hypothesize <lang> --kind sound-change --claim "k > tʃ / _
i" --evidence "kina → tʃina"` records a hypothesis; it starts *proposed*, and as the
evidence comes in you move it along with `hypothesis-status ... --status supported`
(or `refuted`). `inkhaven language hypotheses <lang>` shows the register at a glance,
each line marked with its status, and `/hypotheses` lists it in the companion.

#term("Hypothesis (in historical linguistics)")[
  A proposed regular sound change, cognacy relation, or borrowing, held provisionally
  and tested against the data. The comparative method is precisely the business of
  proposing such hypotheses and keeping the ones that survive every relevant form —
  which is why recording them, with their evidence and their fate, is how the work is
  actually done.
]

The register turns the one-shot tools of this chapter into a *method*: propose a
change, trace its consequences (Chapter 15), check the cognates it predicts, and
write down whether it held. A refuted hypothesis kept on the books is as valuable as
a supported one — it stops you proposing it again.

#callout(label: "Where the suite shines")[
  Diachronics is where Inkhaven's design pays off most. Because sound change
  reuses the allophony engine, because daughters reuse the lexicon, and because
  cognates reuse the change-chains, a few rules give you a whole evolving family
  for almost no extra effort. A proto plus two daughters is a weekend's work and
  an enormous boost to realism.
]

#recap((
  [A *language family* is a set of daughters from one proto; related words are
   *cognates*.],
  [`cognates <proto> --form` shows a proto-word's reflex in every daughter;
   `family-tree` draws the whole family.],
  [`reconstruct --forms` (AI) proposes a lost ancestor from cognates;
   `realism-check` (AI) judges whether your sound history is plausible.],
  [Two daughters of one proto is a small effort for a large gain in depth.],
))
