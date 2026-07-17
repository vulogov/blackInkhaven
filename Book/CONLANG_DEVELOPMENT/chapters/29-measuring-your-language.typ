#import "../design.typ": *

#chapter(number: 29, title: "Measuring, testing, and tracing")

You have built a language. How do you know whether it is any good — whether its
sounds hang together the way real languages' do, whether its grammar is
internally consistent, whether a change you are considering will do what you
hope? Inkhaven answers these with a set of read-only *analyses*. None of them
touches your language; each holds it up against how the world's languages
actually behave and tells you where yours is ordinary and where it is unusual.
A flag is never an error — real languages break every tendency — it is simply a
place to look.

#section("The shape of the sound system")

`stats` (Chapter 10) counts your phonemes and syllables. `metrics` measures them.

```sh
inkhaven language metrics Eldar
```

#term("Phoneme entropy")[
  A number, in *bits*, for how evenly your phonemes are used across the lexicon.
  A language that leans hard on a handful of sounds has low entropy; one that
  spreads its inventory evenly has high entropy. Alongside it, *evenness* rescales
  this to 0–100%, and *perplexity* reports the effective inventory size — how many
  equally-frequent phonemes would give the same spread.
]

The report also fits your phoneme frequencies to *Zipf's law* — the tendency, seen
across natural languages, for the #super[n]th most common item to appear about
#raw("1/n") as often as the most common; a slope near #raw("−1") is Zipfian.
*Phonotactic saturation* asks how much of the syllable space your phonotactics
allow you actually use (a low number means many possible syllables go unfilled),
and *mora weight* reports the average heaviness of your syllables. All of it is
computed from the dictionary words that parse cleanly as your own phonemes; loans
and names are skipped.

#section("Is the inventory natural?")

`naturalness` judges the phoneme inventory itself against cross-linguistic
tendencies, using a table of distinctive features — the place, manner and voicing
that distinguish one sound from another.

```sh
inkhaven language naturalness Eldar
```

It checks four things. *Voicing symmetry*: does every voiceless obstruent have a
voiced partner, or is there a `/k/`-without-`/ɡ/` gap? *Place coverage*: are the
major places — labial, coronal, dorsal — all represented? *Near-universals*: does
the inventory include the segments almost every language has (`/m n p t k s a i
u/`…)? And *size*: is the inventory in the ordinary range, or notably small or
large? These fold into a single 0–1 score. A gap here is a design signal, not a
mistake — plenty of natural languages lack `/p/` or `/ɡ/` — but if the tool
reports you are missing `/a/`, that is worth a second look.

#term("Distinctive feature")[
  A single dimension along which two sounds can differ — voicing, place of
  articulation, nasality, vowel height, and so on. `/p/` and `/b/` differ in one
  feature (voicing); `/p/` and `/t/` in place. Describing sounds by their
  features is how a computer — or a linguist — reasons about whether two of them
  contrast, and whether a class of them behaves alike.
]

#section("Which contrasts carry the weight?")

Two words that differ in a single sound — *pat* and *bat* — are a *minimal pair*,
and they are the classic proof that the two sounds are distinct phonemes in your
language rather than variants of one. `pairs` finds every minimal pair in your
lexicon and, through the feature table, reports the one feature each turns on:

```sh
inkhaven language pairs Eldar
```

The result is a picture of *functional load* — which distinctions your language
actually leans on. If a hundred pairs turn on voicing and only two on aspiration,
you have learned something about where your language's contrasts do their work,
and perhaps where a distinction you thought important is barely paying its way.

#section("Does the grammar obey the universals?")

Chapter 13 had you answer a typological questionnaire. `universals` checks those
answers against how real languages combine them.

```sh
inkhaven language universals Eldar
```

#term("Implicational universal")[
  A statement of the form "if a language has X, it (almost always) has Y" —
  Greenberg's classic findings. Verb-final languages tend to have postpositions,
  not prepositions; prepositional languages tend to put the possessor after the
  noun. These are strong cross-linguistic *tendencies*, not laws, and a
  natural-seeming language mostly follows them.
]

The report does two things. It measures *head-directionality harmony* — whether
your adpositions, genitives and relative clauses all "branch" the same way as
your verb phrase, which harmonious languages do — and it judges a handful of the
classic implicational universals (Greenberg's 2, 3 and 4, and the correlations
between object–verb order and genitive and relative-clause order), marking each
*satisfied*, *violated*, or *not applicable*. A violation is a flag: your language
has made a combination that is rare in the world. That may be exactly the
character you want — but now you have chosen it on purpose.

#section("Tracing a change before you make it")

The most consequential thing you can do to a language is change its sounds. A
single rule — palatalize `/s/` before `/i/`, drop final vowels — ripples through
every word at once, and some of those ripples you will not have foreseen. The
*Consequence Tracer* lets you look before you leap.

```sh
inkhaven language trace Eldar --rule "s > ʃ / _ i"
```

#term("Consequence Tracer")[
  A preview of a pending sound change. It applies the rule to a copy of your
  whole lexicon, re-derives every word exactly as a real sound change would, and
  reports what would happen — without changing anything. Nothing is written; it
  is a rehearsal.
]

It tells you two things. First, *which words the change touches*, old form beside
new. Second, and more valuable, whether the change would create *new homophones*
— words that were distinct but which the rule collapses onto the same form. A
merger like that is sometimes what you want (languages merge sounds all the time)
and sometimes a quiet disaster you would rather catch now than after you have
written a chapter of prose in the language. Try a rule, read the consequences,
and keep it or discard it with nothing lost.

You can run the same trace inside the Linguistic companion — type `/trace s > ʃ /
_ i` into the chat — so the rehearsal is always a keystroke away while you work.

#recap((
  [The analyses are *read-only*: they describe your language against the
   cross-linguistic baseline; a flag is a place to look, never an error.],
  [`metrics` measures the sound system — phoneme *entropy* (and evenness,
   perplexity), the *Zipf* fit, *phonotactic saturation*, and *mora weight*.],
  [`naturalness` judges the inventory — voicing symmetry, place coverage,
   near-universal segments, and size — into a 0–1 score.],
  [`pairs` finds *minimal pairs* and, via the distinctive-feature table, reports
   the *functional load* of each contrast.],
  [`universals` checks your grammar's *head-directionality harmony* and the
   classic *implicational universals* (Greenberg/Dryer), flagging marked
   combinations.],
  [`trace` (the *Consequence Tracer*) previews a sound change across the whole
   lexicon — which words shift, and which new *homophones* it would create —
   without committing it; `/trace <rule>` does the same inside the companion.],
))
