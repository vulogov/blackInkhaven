#import "../design.typ": *

#chapter(number: 13, title: "Typology: the shape of your grammar")

Beyond individual words lies the architecture of a language: does the verb come
before or after the object? Does it mark the subject of a sentence specially? Does
it have grammatical gender? These big design choices are the subject of
*typology*, and making them is how you give your grammar a coherent character.
Inkhaven gives you a questionnaire of the major choices, drawn from how real
languages actually vary.

#term("Typology")[
  The classification of languages by their structural features — word order,
  how they mark who-does-what, whether they have case or gender, and so on.
  *Typology* is, in effect, the menu of high-level design decisions every
  language makes. Linguists catalogue these in surveys like WALS, the *World
  Atlas of Language Structures*, which Inkhaven's questionnaire follows.
]

#section("The questionnaire")

Run the grammar command with no options to see the catalogue of sixteen features,
your language's current answers, and how much you have filled in:

```sh
inkhaven language grammar Eldar
```

The sixteen are word order, adjective order, genitive order, adpositions
(prepositions versus postpositions), alignment, case, gender, number,
definiteness, tense, aspect, mood, evidentiality, negation, question formation,
and relative clauses. A few of these may be unfamiliar: *aspect* is how an action
unfolds in time (ongoing versus completed), *mood* is the speaker's stance
(fact, command, wish), *definiteness* is the "a" versus "the" distinction, and
*evidentiality* is marking how you know something (witnessed, reported,
inferred) — each has a glossary entry. You do not have to answer all sixteen;
answer the ones that matter to your language and leave the rest.

#section("Three choices worth understanding")

Three of the features shape a grammar more than any others. Let us define them,
since they are the ones a newcomer most often meets.

#term("Word order")[
  The default order of the *subject* (S, the doer), *verb* (V, the action), and
  *object* (O, the thing acted on) in a basic sentence. English is *SVO*: "the
  cat (S) sees (V) the bird (O)". Japanese and Latin are *SOV*: "the cat the bird
  sees". The six possible orders (SOV, SVO, VSO, …) are unevenly common across
  the world; SOV and SVO are by far the most frequent.
]

#term("Alignment")[
  How a language marks the *subject* of a sentence. In a *nominative–accusative*
  language (like English), the doer is treated the same whether or not there is
  an object ("*she* runs", "*she* sees it"), and the object is marked differently.
  In an *ergative–absolutive* language, the lone subject of "she runs" is instead
  grouped with the *object* of "she sees it". This sounds exotic but is common —
  Basque and many others work this way. It is one of the most character-defining
  choices you can make.
]

#term("Grammatical case")[
  Marking a noun's *role* in the sentence by changing its form (usually with an
  ending), rather than relying on word order. Latin *puella* "girl" becomes
  *puellam* as a direct object, *puellae* as a possessor. Common cases include
  *nominative* (subject), *accusative* (object), *dative* (recipient,
  "to/for"), and *genitive* (possessor, "of"). You built a dative suffix back in
  Chapter 11; declaring `case: yes` here records that your language has a case
  system.
]

#section("Answering a feature")

You set an answer with `--set feature=value`. The answer is checked against the
catalogue, so you cannot set an invalid one:

```sh
inkhaven language grammar Eldar --set word_order=sov
inkhaven language grammar Eldar --set alignment=nominative_accusative
inkhaven language grammar Eldar --set case=yes
```

Each answer is stored in the *Grammar* chapter. They are not just notes:
Inkhaven's grammar book and study guide (Part VII) read these answers and explain
them, and the AI translator uses them to put words in the right order.

#callout(label: "Let your choices reinforce each other")[
  Typological features tend to cluster in real languages — SOV languages, for
  instance, usually put adjectives and possessors *before* their nouns and use
  *postpositions* ("the house behind" rather than "behind the house"). The
  questionnaire shows the typical consequences of each answer, nudging you toward
  a grammar that hangs together naturally. You are free to break the patterns —
  but knowing them helps you break them on purpose.
]

== Putting words into a sentence

Word order, case, and agreement only come alive when you string words together.
Inkhaven can do exactly that. Give it a subject, a verb, and an object — each a
word from your lexicon, written `root` or `root:gloss` — and it builds the clause
for you:

```
inkhaven language sentence Eldar --subject kira:bird --verb nami:see \
    --object pata:stone --object-adj mira:bright
```

Behind that one command the engine does four things in sequence. It *orders* the
three constituents by your `word_order` (so an SOV language prints subject, then
object, then verb). It *assigns case* by your `alignment` (nominative–accusative
makes the subject nominative and the object accusative). It *inflects* each noun
through your `noun` paradigm to reach that case. And it *runs agreement*, so the
adjective copies its noun's case and the verb agrees with its subject. The result
is printed three ways — the surface clause, an interlinear gloss, and a literal
back-translation:

#term("Interlinear gloss")[
  A word-by-word translation lined up *under* the original, the standard way
  linguists display a sentence in an unfamiliar language. Each native word sits
  above its meaning and grammatical tags (`stone-ACC`), so a reader can see
  exactly how the grammar assembles meaning.
]

This is the same machinery the grammar book uses to print a worked example
sentence from your own vocabulary — proof that your phonology, lexicon,
paradigms, and typology have come together into something you can actually
*say*.

== Saying no, and asking: negation and questions

A language needs more than plain statements. Two clause-level operations come
straight from typology answers you already gave through the questionnaire above.
To *negate* a clause, add `--negate` and, if your language has a negative word,
name it with `--negator`:

```
inkhaven language sentence Eldar --subject kira:bird --verb nami:see \
    --object pata:stone --negate --negator na:not
```

How the negation appears follows your `negation` feature: a *particle* or
*auxiliary* puts the negator as its own word before the verb; an *affix* fuses it
onto the verb form. If you have not coined a negator yet, Inkhaven marks only the
gloss — it never invents a word for you.

To make a *polar* (yes/no) question, add `--question` (and `--q-particle` if your
language uses one):

#term("Polar question")[
  A yes/no question — "does the bird see the stone?" — as opposed to a *content*
  question that asks who, what, or where. Languages mark them by a particle, by
  inverting the word order, by special verb morphology, or by intonation alone.
]

```
inkhaven language sentence Eldar --subject kira:bird --verb nami:see \
    --object pata:stone --question --q-particle ka:Q
```

Your `question` feature decides the realization: a *particle* is appended at the
clause edge (glossed `Q`, so the example above yields `kira patan nami ka?`),
*word_order* fronts the verb (English-style inversion), *morphology* tags the
verb, and every strategy adds a surface "?".

== Building bigger sentences: relative clauses and coordination

Real sentences nest and join. A *relative clause* lets one clause modify a noun —
"the bird #emph[that sees the stone]":

#term("Relative clause")[
  A clause that modifies a noun, the way "that sees the stone" narrows down
  "the bird". The modified noun (the *head*) plays a role inside the embedded
  clause — here it is the one doing the seeing (the subject); the empty slot it
  leaves behind is called the *gap*.
]

```
inkhaven language relative Eldar --head kira:bird --role subject \
    --verb nami:see --with pata:stone --relativizer ya:that
```

You tell Inkhaven which role the head plays inside the clause — `subject` ("the
bird that sees…") or `object` ("the stone that … sees") — and supply the other
argument with `--with`. The embedded clause runs through the very same engine, so
it still case-marks and agrees (the object stays accusative). Whether the clause
sits *before* or *after* the head follows your `relative_clause` feature
(prenominal, as in Japanese, versus postnominal, as in English).

*Coordination* joins two things of the same kind with a conjunction — two nouns,
or two whole clauses:

```
inkhaven language coordinate Eldar --np kira:bird --np pata:stone --conjunction na:and
inkhaven language coordinate Eldar --conjunction na:and \
    --clause "kira:bird nami:see pata:stone" --clause "muru:river tasa:fall"
```

Give two or more `--np` nouns, or two or more `--clause` clauses (each written as
space-separated `root:gloss` words), and Inkhaven threads your conjunction
between them — assembling each clause in full, so "bird sees stone and river
falls" keeps its case marking throughout.

Finally, a *complement clause* lets one whole clause become the object of another
— "I know #emph[that the bird sees the stone]":

#term("Complement clause")[
  A subordinate clause that serves as an argument — usually the object — of a main
  (#emph[matrix]) verb of speech or thought, such as *know*, *say*, or *think*. It
  is often introduced by a *complementizer* like "that". The matrix clause ("I
  know…") and the embedded clause ("the bird sees the stone") each have their own
  subject and verb.
]

```
inkhaven language complement Eldar --subject mi:I --verb tira:know \
    --complementizer ya:that \
    --comp-subject kira:bird --comp-verb nami:see --comp-object pata:stone
```

The matrix subject and verb wrap the embedded clause (`--comp-subject` /
`--comp-verb` / `--comp-object`), introduced by an optional complementizer
(glossed `COMP`). Because the complement fills the matrix *object* slot, your word
order places it correctly on its own — an SVO language prints it after the matrix
verb, a verb-final language before it — and the embedded clause still case-marks
its own object.

#callout(label: "Grammar you can hear")[
  Negation, questions, relative clauses, and coordination all read from the same
  typology answers and the same paradigms as a plain sentence. Once your phonology,
  lexicon, and grammar are in place, these richer sentences cost you nothing extra
  — they fall out of choices you already made.
]

#recap((
  [*Typology* is the set of high-level structural choices a grammar makes;
   `grammar` lists sixteen, WALS-aligned.],
  [*Word order* (SOV, SVO, …) sets the default subject–verb–object sequence.],
  [*Alignment* (nominative–accusative vs ergative–absolutive) decides how the
   subject is marked.],
  [*Case* marks a noun's role by its form (nominative, accusative, dative,
   genitive).],
  [`grammar --set feature=value` records an answer, validated against the
   catalogue.],
  [`sentence` assembles a clause — ordering, case, and agreement together — with
   an interlinear gloss.],
  [`--negate` and `--question` add negation and yes/no questions, realized by
   your `negation` and `question` features.],
  [`relative` builds a noun modified by a *relative clause* (gap + relativizer,
   pre- or postnominal); `coordinate` joins nouns or clauses with a conjunction.],
  [`complement` makes one clause the object of another ("I know *that* …"), placed
   by word order around the matrix verb.],
))
