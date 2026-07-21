#import "../design.typ": *

#chapter(number: 8, title: "Historical linguistics")

Languages are the visible present of a long past. Russian did not always sound as it
does now; the modern forms are the outcome of centuries of regular sound change, and
the comparative method — reasoning backward from the daughters to their common
ancestor — is one of the great achievements of the human sciences. This final chapter
turns the workbench on the past, and treats the study of it as what it is: a science
of *hypotheses*.

#section("A famous change")

Old East Slavic had two short vowels, written `ъ` and `ь` and called the *yers*. Around
the twelfth century the weak yers were lost, and the strong ones became full vowels —
the single most consequential sound change in Russian's history. Old `сънъ` became
modern `сон` "dream"; `дьнь` became `день` "day". Whole classes of words were reshaped
by one regular rule.

That rule is a *claim* about Russian's history, and a claim is something to record and
test, not merely assert. Register it:

```sh
inkhaven language hypothesize Russian --kind sound-change \
  --claim "ъ > ∅ / _ #" --note "loss of the weak final yer" --id yer-loss
```

It enters the language's register of hypotheses, marked *proposed*.

#term("Sound change")[
  A regular alteration in pronunciation that spreads through a language's whole
  vocabulary — not a one-off, but a rule: *every* word in the right environment
  changes. The regularity is what makes historical linguistics possible; because sound
  change is systematic, its effects can be predicted, reversed, and used to prove that
  two languages descend from one.
]

#section("Testing the consequences")

A hypothesis earns its keep by making predictions. `hypothesis-check` runs your
sound-change claim through the Consequence Tracer — applying it across the lexicon and
reporting exactly which words it changes and which distinct words it *merges* into
homophones:

```sh
inkhaven language hypothesis-check Russian --id yer-loss
```

Now the claim is falsifiable. If the words it predicts to change are the ones that did,
and the mergers it predicts are ones Russian actually shows, the evidence supports it;
if it collapses distinctions the language in fact keeps, that is evidence against.
Record the verdict:

```sh
inkhaven language hypothesis-status Russian --id yer-loss --status supported
```

`inkhaven language hypotheses Russian` then shows the register at a glance, each claim
marked with its standing — proposed, supported, refuted, retired. A refuted hypothesis
kept on the books is as valuable as a supported one: it stops you proposing it again.

#term("The comparative method")[
  The technique of reconstructing an unattested ancestor by systematically comparing
  its descendants: line up *cognates* — words descended from one original — find the
  regular sound correspondences between them, and reason back to the form that would
  yield them all. It is how Proto-Indo-European was reconstructed from Sanskrit, Greek,
  Latin and the rest, without a single written record of it surviving.
]

#section("Russian among the Slavic languages")

Russian is one daughter of several. Its sisters — Ukrainian, Belarusian, Polish, Czech,
the South Slavic languages — descend with it from Proto-Slavic, and the regular
differences between them are the raw material of reconstruction. Where Russian has
`город` "city", Polish has `gród` and Old Church Slavonic `градъ`: one cognate set,
three reflexes of a single ancestral form, and the *pleophony* (the Russian `-оро-`
against South Slavic `-ра-`) is itself a regular, datable change.

Model the sisters as a language family and Inkhaven's diachronic tools apply directly:
`cognates` traces a proto-form's reflex in each daughter, `reconstruct` proposes an
ancestor from a cognate set, and each proposal is another hypothesis for the register.
The comparative method, systematized: propose a change or a cognacy, trace its
consequences, check it against the forms, and write down whether it held.

#recap((
  [Russian is the outcome of regular sound change — the fall of the yers turned `сънъ`
   into `сон`; such changes are rules over the whole vocabulary, which is what makes
   them recoverable.],
  [`hypothesize` records a sound-change (or cognacy, or borrowing) claim; `hypothesis-check`
   runs it through the Consequence Tracer to make it falsifiable; `hypothesis-status`
   records the verdict as the evidence comes in.],
  [Russian's Slavic sisters supply the cognate sets of the comparative method;
   `cognates` and `reconstruct` reason across a modelled family, each result another
   hypothesis to test — historical linguistics as an explicit, testable loop.],
))
