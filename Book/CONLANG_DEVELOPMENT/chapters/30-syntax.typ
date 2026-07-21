#import "../design.typ": *

#chapter(number: 30, title: "Syntax: parsing and structure")

Everything up to now has run one way: you declared sounds, morphemes and rules,
and Inkhaven *generated* forms from them — a root plus a paradigm became an
inflected word, a proto-form became a daughter. This chapter runs the other way.
Given a word or a sentence your language could produce, it takes the thing *apart*:
peels a surface word back to its root, works out who did what to whom, and draws the
tree the sentence hangs on. These are the tools of a working syntactician, pointed
at the language you built.

None of them change your language. They read it and report, so you can check that
the grammar you declared actually produces the structures you intended.

#section("Reading a word backwards: the parser")

Paradigm generation goes root → surface. The *parser* goes surface → root: given a
word, it strips the affixes your morphology declares until what remains is an entry
in the lexicon.

```sh
inkhaven language parse Eldar --word katai
```

It reports every analysis it finds, simplest first — here `kata` ‘stone’ plus the
plural suffix. The parser is not a lookup; it genuinely reverses the generator, so
it undoes stacked affixes (`nakatas` → `DEF` + `kata` + `DAT`) and the
non-concatenative processes too: full reduplication (`katakata` → `REDUP` + `kata`),
partial reduplication where only the first syllable copies (`kakata` → `ka~` +
`kata`), and ablaut, the internal vowel change of *sing/sang* (a past tense that
rewrites `a → i` turns `kat` into `kit`, which the parser reads back as `kat` +
`PST`).

#term("Lemma")[
  The dictionary form of a word — its root headword, the form you would look up.
  Parsing an inflected surface form recovers its lemma; the corpus tools of the next
  chapters use the lemma to gather every inflected form of a word together.
]

Ablaut can't be reversed by running the rule backwards — a rule that turns every
`a` into `i` can't be told which `i`s were once `a`s. So the parser tests forward
instead: it runs each root through the ablaut rules and remembers the results, and a
surface word that matches one of them is analysed as that root plus the ablaut
morpheme. This is why the parser needs no special cleverness for irregularity — it
only ever asks "could the generator have produced this?"

#section("Who did what to whom: argument linking")

A verb has a *valence* — the number of participants it takes — and each participant
plays a role. `link` works out those roles for a clause from the verb's valence:

```sh
inkhaven language link Eldar --verb see --args "she, bird"
```

For each argument it reports three things at once: its *thematic role* (is it the
agent, the patient, the theme, the recipient?), its *macrorole* in the
Role-and-Reference-Grammar sense (the actor or the undergoer), and its *grammatical
relation* (subject, object, indirect object). A transitive clause draws the default
linking — the higher-ranking argument is the actor and subject, the lower the
undergoer and object — and where the argument count doesn't match the declared
valence, it says so.

#term("Thematic role")[
  The semantic part a participant plays in an event: *agent* (the doer), *patient*
  (the affected), *theme* (the thing moved or located), *recipient*, and so on.
  Distinct from the grammatical relation (subject/object), which is about form —
  languages map the two differently, which is much of what makes their syntax vary.
]

#section("The shape of a sentence: the X-bar tree")

Sentences are not flat strings of words; they are nested. `tree` draws that nesting
as an *X-bar* phrase-structure tree — the `CP → TP → VP` scaffold generative syntax
assumes — from a clause's verb and arguments:

```sh
inkhaven language tree Eldar --verb sees --args "she, bird"
```

```
CP
├─ C ∅
└─ TP
   ├─ NP
   │  └─ N she
   └─ T'
      ├─ T ∅
      └─ VP
         └─ V'
            ├─ V sees
            └─ NP
               └─ N bird
```

Specifiers sit on the left — the subject is the specifier of `TP` — and the one
parameter that varies is the order of a head and its complement, which Inkhaven
reads from your language's declared word order. A head-final (SOV) language reverses
the tree correctly: the object precedes the verb inside `VP`, `VP` precedes `T`, and
`TP` precedes `C`. Ask for the tree of a clause in a language whose word order you
have set, and you can see at a glance whether the grammar you declared builds the
constituents you meant.

#section("Moving a piece: fronting and traces")

Questions and topicalisation *move* a constituent to the front of the clause.
`movement` performs that operation over the tree, leaving a coindexed *trace* where
the moved piece came from:

```sh
inkhaven language movement Eldar --verb sees --args "she, bird" --move object
```

The object lifts to the specifier of `CP` as `NP₁`, and a trace `t₁` marks the gap
it left in the verb phrase — the derivation made visible. This is the machinery
behind *"the bird, she sees ⟨t⟩"* and behind wh-questions.

#term("Trace")[
  The silent placeholder a moved constituent leaves behind, coindexed with it
  (`NP₁ … t₁`). Traces are how generative syntax keeps track of where a fronted or
  questioned phrase is interpreted, even though it is pronounced somewhere else.
]

#section("Who can refer to whom: binding")

When can two noun phrases in a clause refer to the same person? *"She sees herself"*
works; *"herself sees her"* does not. `binding` decides such questions from the tree
and the three binding principles:

```sh
inkhaven language binding Eldar --verb sees --args "she, herself" --type reflexive
```

The engine is *c-command* — a node c-commands its sibling and everything the sibling
contains — plus the principles: an anaphor (a reflexive) must be bound by a
c-commanding antecedent in its clause (Principle A); a pronoun must *not* be
(Principle B); a name must be free everywhere (Principle C). So a reflexive object
*may* refer to the subject that c-commands it, a pronoun in the same slot *may not*,
and a name there is a Principle-C violation. The relation is correctly asymmetric —
the subject c-commands the object, but not the reverse — which is exactly why
*"herself sees her"* fails.

#term("C-command")[
  A structural relation: a node c-commands its sibling and everything beneath that
  sibling. It is the backbone of binding, of how far a fronted phrase can move, and
  of much else in syntax — a surprising amount of grammar turns out to depend not on
  linear order but on this "is it my sibling's descendant?" question.
]

#section("At your fingertips")

Three of these live in the Linguistic companion's chat as well, for when you want to
check a structure while you think: `/parse <word>`, `/tree <verb> <subject>
[object]`, and `/clause <verb> <subject> …` (the clause check of the next chapter).
Each runs over the language your cursor sits in and prints inline.

#recap((
  [`parse` reverses generation: it strips affixes — concatenative, reduplication,
   ablaut — to recover a word's root and its morphemes, the inverse of the paradigm
   engine.],
  [`link` works out a clause's argument structure: each argument's thematic role,
   its actor/undergoer macrorole, and its grammatical relation, from the verb's
   valence.],
  [`tree` draws the X-bar phrase-structure tree (`CP → TP → VP`), placing heads and
   complements by your language's word order; `movement` fronts a constituent
   leaving a trace; `binding` decides coreference by c-command and Principles A/B/C.],
  [All are read-only: they analyse the language you declared and never change it.],
))
