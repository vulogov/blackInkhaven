#import "../design.typ": *

#chapter(number: 31, title: "The Oracle: judging well-formedness")

The parser of the last chapter tells you how a word is *built*. The Oracle tells you
whether it *could exist* — whether a candidate word or clause is well-formed by your
language's own rules, and if not, why not. Where `audit` scans the finished lexicon,
the Oracle judges arbitrary input: a word you are about to coin, a sentence you just
wrote, a form you found in the prose and can't place. It answers in levels, from the
sounds up to the syntax.

#section("Is this a possible word?")

`check` runs the Oracle over a single candidate word:

```sh
inkhaven language check Eldar --word ktazil
```

It works upward through two levels. First *phonotactics*: are all the segments in
your inventory, and does the word obey your syllable-structure constraints? A stray
foreign sound or an illegal cluster is caught here. Then *morphology*: does the word
analyse — as a listed root, or as a root plus affixes the parser can strip to reach
one? A word that is phonotactically fine but reaches no root is flagged, so you can
decide whether to coin it or reject it. A clean word gets a single line saying so.

#term("Well-formedness")[
  Whether a form obeys the rules of its language — not whether it *means* anything,
  but whether it *could be* a word or sentence of the language at all. Native
  speakers judge well-formedness instantly and unconsciously; the Oracle makes your
  invented language's judgements explicit, so you can hold it to its own standards.
]

#section("Is this a possible clause?")

Words are levels one and two; a clause adds two more. `check-clause` judges a whole
sentence:

```sh
inkhaven language check-clause Eldar --verb katai --args "she, bird" \
  --verb-root kata --subject-features "number=pl"
```

*Agreement* (level three): given the subject's grammatical features and the verb's
root, the Oracle regenerates the verb form your agreement rule *should* produce and
compares it to the one you wrote. A plural subject with a singular verb is flagged,
and the expected form is named. *Argument structure* (level four): reusing the
argument linker, it checks that the number of arguments matches the verb's valence —
an intransitive verb handed two arguments is caught, a transitive one handed two is
not.

Agreement is not only subject–verb. `check-agreement` generalises the check to *any*
head–dependent pair — an adjective with its noun, a determiner with its noun:

```sh
inkhaven language check-agreement Eldar --dependent adjective \
  --form mira --root mira --head-features "number=pl"
```

It looks up the agreement rule you declared for that dependent, regenerates the form
it should take, and flags the one you gave if it differs. A dependent your language
declares no agreement rule for is silently passed — there is nothing to enforce.

#section("The Oracle watching your prose")

You met the Oracle already, in the healthy-lexicon chapter: when you save a
paragraph, the *phonotactic guardian* checks the conlang words in it and flags any
that break your language's rules, as an advisory finding in the Output pane. That is
the same level-one judgement, run automatically over your manuscript rather than on
a word you typed. The whole-book sweep in the review pass (`Ctrl+B J`) runs it across
every chapter at once. So the Oracle works three ways: on demand over a word or
clause, as you save, and across the finished manuscript — the same standard applied
wherever ill-formed conlang words might hide.

It is always advisory. The Oracle reports; it never edits your prose or rejects your
coinage. A language is yours to break when you mean to — the Oracle only makes sure
you meant to.

#section("At your fingertips")

In the companion, `/check <word>` runs the word Oracle inline and `/clause <verb>
<subject> …` runs the clause Oracle's argument-structure check (agreement needs the
subject's features, so that half stays on the command line).

#recap((
  [`check <word>` judges a candidate word by level: phonotactics (legal segments and
   clusters) then morphology (does it analyse to a root?).],
  [`check-clause` adds two levels over a sentence: subject–verb agreement (does the
   verb inflect for its subject?) and argument structure (does the argument count fit
   the valence?); `check-agreement` opens the agreement check to any head–dependent
   pair.],
  [The same phonotactic judgement runs automatically on save and across the whole
   manuscript in the review pass, flagging ill-formed conlang words in your prose.],
  [Every Oracle finding is advisory — it reports and never changes your writing.],
))
