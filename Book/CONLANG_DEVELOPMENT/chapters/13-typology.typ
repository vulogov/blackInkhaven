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

The features include word order, alignment, case, gender, number, definiteness,
tense, aspect, mood, evidentiality, negation, question formation, and relative
clauses. You do not have to answer all of them; answer the ones that matter to
your language and leave the rest.

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
))
