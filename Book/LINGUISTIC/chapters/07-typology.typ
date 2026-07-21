#import "../design.typ": *

#chapter(number: 7, title: "Grammar and typology")

A grammar is the set of rules by which a language builds its sentences, and describing
one is much of what a linguist does. Inkhaven lets you write that description down
formally — the word order, how grammatical roles are marked, what each verb demands of
its arguments — and then two things become possible: the analysis tools of the earlier
chapters can *read* the description, and it can be *checked*, against itself and against
the strong cross-linguistic generalizations of typology. This chapter is the workbench
describing a grammar, with Russian as the case.

#section("The typological profile")

The broad shape of a grammar is a handful of answers, and you record them with
`grammar --set`:

```sh
inkhaven language grammar Russian --set word_order=SVO
inkhaven language grammar Russian --set alignment=nominative_accusative
inkhaven language grammar Russian --set adposition=preposition
inkhaven language grammar Russian --set adjective_order=adjective_noun
inkhaven language grammar Russian --set genitive_order=noun_genitive
```

Each answer places Russian on one axis of variation the world's languages spread
across. Russian's profile is that of a typical European language — subject–verb–object,
adjectives before nouns, prepositions rather than postpositions — with one system that
carries most of the grammatical load: *case*.

#term("Grammatical alignment")[
  How a language marks the core arguments of a verb — in particular, whether the single
  argument of an intransitive verb (*she sleeps*) is treated like the subject of a
  transitive verb (*she sees it*) or like its object. *Nominative–accusative* alignment,
  Russian's, groups the two subjects together and marks the object differently;
  *ergative–absolutive* alignment groups the intransitive subject with the object. It
  is one of the deepest divisions among the world's grammars.
]

In Russian the alignment is carried by the case endings of Chapter 4: the nominative
marks subjects, the accusative marks direct objects, and four more cases mark
everything else. A language with such rich case can afford the free word order of
Chapter 5 — because the *endings* say who is subject and who is object, the *order*
doesn't have to.

#section("What each verb demands: valence")

Verbs differ in how many participants they take, and the syntax tools need to know.
Declare each verb's *valence* in the grammar's `verb_classes` block:

```hjson
{ verb_classes: [
    { name: "спать",  valence: "intransitive" }   // sleep — one argument
    { name: "видеть", valence: "transitive" }      // see — two
    { name: "дать",   valence: "ditransitive" }    // give — three
  ] }
```

Now `link`, `tree` and `check-clause` read a verb's valence rather than guessing it from
the words present, so the argument-structure check of Chapter 5 can tell a genuine
missing object from an intentional omission.

#term("Valence")[
  The number of core arguments a verb requires — one for *sleep*, two for *see*, three
  for *give*. Valence is the backbone of a clause's structure: it determines how many
  noun phrases the sentence needs and what roles they play, and mismatches between a
  verb's valence and the arguments present are among the first things a grammar check
  looks for.
]

Russian layers *aspect* on top of valence — nearly every verb comes in an imperfective
and a perfective pair (`писать`/`написать`, "to write" / "to write-to-completion") — a
grammatical category English lacks. You would record such pairs in the lexicon and note
the distinction in the grammar; the point is that the description is yours to make as
fine as the language demands.

#section("Deeper parameters")

Beyond the surface profile sit the abstract switches generative grammar calls
*parameters*. Declare them in `ug_parameters`:

```hjson
{ ug_parameters: {
    head_final:  false     // Russian is head-initial
    pro_drop:    true       // subject pronouns may be dropped
    wh_movement: true       // question words front
  } }
```

Russian earns each of these. It is *head-initial* (verbs before objects, prepositions
before nouns). It is a *pro-drop* language, at least partially — *Иду домой* "(I'm)
going home" needs no pronoun, because the verb ending already says who. And its question
words move to the front — *Что ты видишь?* "What do you see?" — the wh-movement of
Chapter 5.

#term("Pro-drop")[
  The property of a language that lets it omit a subject pronoun when the verb's
  agreement already identifies the subject — Italian *parlo* "(I) speak", Russian *иду*
  "(I) go". Pro-drop correlates with rich verb agreement: a language whose verb endings
  spell out the subject can afford to leave the pronoun unsaid. English, with almost no
  agreement, cannot.
]

#section("Checking the description")

A formal description can be *inconsistent* — you might declare a head-final parameter
while setting an SVO (head-initial) word order. `grammar-check` catches such conflicts:

```sh
inkhaven language grammar-check Russian
```

It validates the typed blocks and cross-checks them against each other and against the
typological answers — flagging, for instance, a `head_final` parameter that contradicts
your declared word order. Run against a language you know, it is a proof-reader for your
model; the errors it finds are errors in the *description*, caught before they mislead
you.

#section("Testing against the universals")

Typology's central discovery is that grammatical features do not vary independently.
They cluster, and the clusters are stated as *implicational universals*: "if a language
has X, it (tends to) have Y". `universals` checks a profile against the classic ones:

```sh
inkhaven language universals Russian
```

It reports the language's *head-directionality harmony* — whether it consistently puts
heads before their complements — and judges the Greenberg and Dryer implicational
universals satisfied, violated, or not applicable. Russian's profile lines up: a
verb–object language *tends* to be prepositional (Russian is), to put genitives after
the noun (*крыша дома*, "roof of-the-house"), and to place relative clauses after their
head — and Russian obliges on each. That harmony is not a coincidence; it is the
pressure typology measures.

#term("Implicational universal")[
  A conditional generalization over the world's languages: *if* a language has one
  feature, *then* it almost always has another — if the object follows the verb, the
  language tends to use prepositions rather than postpositions. Such universals are the
  strongest empirical results typology has produced, and checking a language against
  them tests whether your description of it is internally coherent.
]

#callout(label: "A violation is a bug in the model")[
  If `universals` flagged a violation for Russian, that would not be a discovery about
  Russian — it would be an answer set wrong in your model. That is exactly the check's
  value: run against a language you know conforms, it tells you whether your *formal
  description* is consistent before you trust it on a language you don't. A model that
  breaks the universals a real language obeys is a model to fix.
]

#recap((
  [`grammar --set` records a language's typological profile (word order, alignment,
   adpositions, adjective and genitive order); Russian is a nominative–accusative,
   head-initial, prepositional language whose case system carries the grammatical load.],
  [`verb_classes` declares each verb's *valence* (intransitive / transitive /
   ditransitive) and `ug_parameters` its deeper switches (head-final, pro-drop,
   wh-movement) — both read by the syntax tools of Chapter 5; `grammar-check` validates
   the description for internal consistency.],
  [`universals` tests the profile against head-directionality harmony and the classic
   implicational universals; run against a language you know, it validates the model —
   a violation where the real language conforms means the description is wrong.],
))
