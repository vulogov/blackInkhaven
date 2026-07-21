#import "../design.typ": *

#chapter(number: 7, title: "Typology")

No language is an island. Every language sits somewhere on a map of the world's
languages, sharing features with some and not others, and — remarkably — the features
do not vary independently. Some combinations are common, some vanishingly rare, and a
few are ruled out altogether. *Typology* is the study of that map, and this chapter
places Russian on it.

#section("A profile in features")

Inkhaven records a language's typological profile as a set of answers — its word
order, how it marks grammatical relations, whether adjectives precede or follow their
nouns, whether it uses prepositions or postpositions, and so on. You set them in the
grammar:

```sh
inkhaven language grammar Russian --set word_order=SVO
inkhaven language grammar Russian --set alignment=nominative_accusative
inkhaven language grammar Russian --set adposition=preposition
inkhaven language grammar Russian --set adjective_order=adjective_noun
```

Russian's profile is that of a fairly typical European language: subject–verb–object,
*nominative–accusative* alignment (the subject of a verb is marked the same whether the
verb is transitive or not), prepositions rather than postpositions, adjectives before
their nouns. Its one showy departure is the rich case system of Chapter 4 — but rich
case morphology sits comfortably with everything else in the profile.

#term("Linguistic typology")[
  The classification of languages by their structural features, and the study of how
  those features cluster. Its central discovery is that the features are not
  independent: knowing one often predicts others. Typology is what lets a linguist look
  at an unfamiliar language and know, from a single trait, what else to expect.
]

#section("Testing the universals")

The clusters typology finds are stated as *implicational universals* — "if a language
has X, it (tends to) have Y". `universals` checks a language's declared profile against
the classic ones:

```sh
inkhaven language universals Russian
```

It reports the language's *head-directionality harmony* — whether it consistently puts
heads before or after their complements, which correlates with word order — and judges
the Greenberg and Dryer implicational universals as satisfied, violated, or not
applicable. For Russian's SVO, prepositional, verb-before-object profile, the classic
correlations line up: a verb–object language tends to be prepositional and to put
genitives and relative clauses after the noun, and Russian obliges.

#term("Implicational universal")[
  A conditional generalization over the world's languages: *if* a language has one
  feature, *then* it (almost always) has another — for instance, if a language puts the
  object after the verb, it tends to use prepositions rather than postpositions. Such
  universals are the strongest empirical results typology has, and testing a language
  against them is a quick check that your model of it is coherent.
]

#section("What a violation would mean")

If `universals` flagged a violation for Russian, that would not be a discovery about
Russian — it would be a bug in your *model* of Russian, an answer set wrong. That is
exactly the value of the check: run against a language you know, it tells you whether
your formal description is internally consistent before you trust it on a language you
don't. A model that violates the universals a real language obeys is a model to fix.

#recap((
  [Typology maps languages by structural features, whose central finding is that the
   features *cluster* — one predicts others; you record Russian's profile with
   `grammar --set` (SVO, nominative–accusative, prepositional, adjective–noun).],
  [`universals` checks the profile against head-directionality harmony and the classic
   Greenberg/Dryer implicational universals; Russian's European profile satisfies them.],
  [Run against a language you know, the check validates your *model*: a violation where
   the real language conforms means the description, not the language, is wrong.],
))
