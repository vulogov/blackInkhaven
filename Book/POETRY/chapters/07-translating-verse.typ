#import "../design.typ": *

#chapter(number: 7, title: "Translating verse")

To translate a poem is to face an impossibility and choose how to fail. A line of verse
carries three things at once — its *form* (the metre and rhyme), its *meaning* (what the
words say), and its *sound* (how they ring, alliterate, and chime) — and no translation
into another language keeps all three intact. Preserve the metre and you bend the sense;
chase the exact meaning and the music is gone; reproduce the sound and you have left the
dictionary behind. This is the translator's *trilemma*, and Inkhaven's contribution is not
to solve it — nothing can — but to *measure the trade* you actually made.

#section("The verse-translation paragraph")

Translation is where the sixth member of the verse family, `para:verse-translation` (⇄),
earns its place. It holds a translated stanza *paired with its source* — the original and
its rendering kept together as one structural unit, so the two never drift apart in the
manuscript and every tool that reads the paragraph can see both sides at once. A book of
translations built this way is a parallel text by construction: the outline's ⇄ marks show
you, at a glance, which stanzas are original and which carry a source across.

#section("The trilemma, scored")

`poetry trilemma` takes a source line or stanza and its translation and reports how the
translation fares on each axis of the impossibility:

```
$ inkhaven poetry trilemma --form sonnet --language ru --to-language en \
    --source "Я вас люби́л: любо́вь ещё́, быть мо́жет" \
    --translation "I lóved you ónce, and stíll perháps I dó"
Translation trilemma (ru → en):

  Form     █████░░░░░   50%   0/1 lines keep the foot · the source does not rhyme…
  Meaning  ░░░░░░░░░░       (the AI axis — engage the Inner Poet in the editor)
  Sound    ████████░░   83%   alliteration density 0.17 → 0.00
```

Three bars, one axis apiece. Two of them the workbench fills in mechanically; the third it
pointedly leaves empty, and that emptiness is the most honest thing on the readout.

#subsection("Form — measured")

The *form* score asks how well the translation keeps the original's shape: does it hold the
declared metre, foot for foot, and does it rhyme where the scheme says it should? It is
computed from the same scansion and rhyme engines you already know, run on both sides and
compared. A translation that renders a Russian iambic pentameter as an English iambic
pentameter, rhyming where the sonnet rhymes, scores high; one that keeps the words and
drops the metre scores low. Give the tool two clean, marked pentameter lines and the foot
axis reads 100%:

```
  Form     ██████████  100%   1/1 lines keep the foot · …
```

The note beside the bar shows its working — how many lines kept the foot, and what it found
of the rhyme — so a bare percentage never has to be taken on trust.

#subsection("Sound — measured, roughly")

The *sound* score asks whether the translation carries the original's *texture* across —
its density of alliteration and repeated sound, the sonic weave beneath the words. Inkhaven
measures this by comparing sound-repetition profiles on the two sides: the readout above
reports the source's alliteration density falling from 0.17 to 0.00 in the translation, a
concrete sign that the English has smoothed out music the Russian carried. This is the
roughest of the three measures — sound similarity across languages with different phoneme
inventories is genuinely hard, and the tool measures a proxy, not the thing itself — but a
proxy you can see beats an impression you cannot.

#subsection("Meaning — left to judgement")

The *meaning* axis is blank, always, and deliberately. Whether a translation is *faithful
to the sense* — whether \"still perhaps I do\" carries what Pushkin's line means, with its
weight and its reticence — is not a thing a ruler measures. It requires reading, and taste,
and knowledge of both languages' connotations. So the trilemma refuses to fake a number
here and instead points you to the one part of the workbench equipped for the question: the
*Inner Poet's slow track* (Chapter 5). Engage it on a verse-translation paragraph and it
reads both sides and offers observations on the meaning carried and the meaning lost — as
prose to weigh, never a score to trust.

#callout(label: "Two axes a machine can hold, one it must not")[
  The trilemma readout is a small manifesto for the whole poetry layer. Form and sound are
  *countable*, so Inkhaven counts them and shows its working. Meaning is a matter of
  *judgement*, so Inkhaven leaves it to a reader — you, or the Inner Poet reading for you —
  and marks the axis empty rather than inventing a figure. A tool that scored \"meaning
  fidelity: 71%\" would be lying with a decimal point. The honest readout has a blank where
  the judgement goes.
]

#section("Reading a translation as a critic")

The trilemma is as much the critic's tool as the translator's. Point it at an existing
translation — Nabokov's deliberately literal English #emph[Eugene Onegin] against a rhyming
verse translation of the same Pushkin, or two Englishings of Rilke — and it quantifies, in a
form you can put in an argument, the choice each translator made: this one kept the form and
paid in sound; that one preserved the music and loosened the metre. The numbers do not settle which is *better* — that judgement is yours, as
always — but they make the trade-off visible and citable, which is exactly what a close
reading of a translation needs and rarely has.

#recap((
  [Verse translation faces a *trilemma*: form, meaning, and sound cannot all survive the
   crossing. Inkhaven does not solve it — it measures the trade you made.],
  [`para:verse-translation` (⇄) pairs a translated stanza with its source as one unit, so a
   book of translations is a parallel text by construction.],
  [`poetry trilemma` scores *form* (metre + rhyme fidelity, from the familiar engines) and
   *sound* (alliteration/repetition texture carried across — the roughest measure), each
   with its working shown.],
  [The *meaning* axis is left deliberately blank: semantic fidelity is judgement, not
   measurement, so the tool points to the Inner Poet's slow track rather than faking a
   number.],
  [The same tool serves the critic: it makes a translator's trade-offs visible and citable,
   without presuming to say which trade was right.],
))
