#set document(title: "Learn Avesha")
#set page(paper: "iso-b5", margin: (x: 2.2cm, y: 2.4cm), numbering: "1")
#set text(size: 11pt, font: ("Libertinus Serif", "New Computer Modern"))
#set par(justify: true, leading: 0.7em, first-line-indent: 1em)
#set heading(numbering: none)
#show heading.where(level: 1): it => block(below: 1em)[
  #set text(size: 18pt, weight: "bold")
  #it.body #v(-0.3em) #line(length: 100%, stroke: 0.5pt + luma(180))
]
#show heading.where(level: 2): set text(size: 12pt, weight: "bold")
#let practice(body) = block(width: 100%, fill: luma(244), stroke: (left: 2pt + rgb("#7a4a2f")), inset: 8pt, radius: 2pt)[
  #text(size: 8pt, weight: "bold", fill: rgb("#7a4a2f"), tracking: 1pt)[PRACTICE] #parbreak() #body
]
#let term(name, body) = block(width: 100%, fill: rgb("#f2f6f9"), stroke: (left: 2pt + rgb("#2f5d7a")), inset: 8pt, radius: 2pt)[
  #text(weight: "bold", fill: rgb("#2f5d7a"))[#name] #parbreak() #body
]
#let native(cp) = text(font: "Avesha", size: 1.3em)[#cp]

#align(center + horizon)[
  #text(size: 32pt, weight: "bold")[Learn Avesha] \
  #v(4mm) #text(size: 13pt, style: "italic", fill: luma(90))[A first course] \
  #v(12mm) #native("\u{E002}\u{E009}\u{E007}\u{E008}\u{20}\u{E003}\u{E00A}\u{E005}\u{E008}\u{20}\u{E005}\u{E008}\u{E004}\u{E009}\u{2E}\u{20}\u{E001}\u{E008}\u{E005}\u{E009}\u{20}\u{E000}\u{E008}\u{E006}\u{E00A}\u{2E}")
]
#pagebreak()

#outline(title: "Contents", depth: 2)
#pagebreak()


Welcome to Avesha! This is a language of stones, birds, suns, and rivers. You will learn it step by step, using only the sounds, words, and rules you are given. By the end of this book, you will be able to read the sample texts and build your own simple sentences.

== Pronunciation Guide

=== Consonants
Avesha has these consonant sounds: #strong[p, t, k, s, m, n, l, r].

- #strong[p] as in English "spit" (not the strong puff of "pit")
- #strong[t] as in "stop"
- #strong[k] as in "sky"
- #strong[s] as in "sun"
- #strong[m] as in "moon"
- #strong[n] as in "noon"
- #strong[l] as in "lamp"
- #strong[r] as in Spanish "pero" (a quick tap, like the "tt" in American English "butter")

=== Vowels
Avesha has three vowels: #strong[a, i, u]. Each is pronounced clearly and purely.

- #strong[a] as in "father" (short)
- #strong[i] as in "machine"
- #strong[u] as in "rude"

=== Stress
Stress always falls on the #strong[second-to-last syllable] (penultimate). For example:
- `pata` = PA.ta (stress on "pa")
- `kiru` (not in your list, but an example) = KI.ru

=== Sound Changes
Two important changes happen when certain sounds meet. Learn them now, because they affect how words are pronounced in sentences.

+ #strong[t becomes s before i]:
   If a word ending in `t` is followed by a word starting with `i`, the `t` changes to `s`.
   Example: The verb `nami` (see) has no `t`, but `pata` (stone) + `i`? Not given. Instead, look at a word like `tani` (speak). The `t` in `tani` is before `i` always, so it sounds like `sani`. But note: this is a rule that applies #strong[across words] too. So if you say `pata` + `ira` (not a word yet), the `t` would change to `s` only if followed directly by `i`.

+ #strong[n becomes m before p]:
   The sound `n` changes to `m` when it comes right before `p`.
   Example: If you have the word `kanu` (hand) and a word starting with `p`, like `palu` (run), and you say `kanu palu`, the `n` of `kanu` is not directly before `p` because there is a `u` in between. But if combined into a compound or a connected speech sequence, the `n` would change. For now, remember this rule for when words meet: `n` before a `p` sounds like `m`.

=== Quick Check
Say these aloud:
- `palu` = PA.lu (run)
- `suna` = SU.na (sun)
- `nami` = NA.mi (see)
- `tani` (remember, `t` before `i` becomes `s`) = SA.ni (speak)

#line(length: 100%, stroke: 0.5pt + luma(200))


== Lesson 1: Basic Words and SOV Word Order

=== Vocabulary
#table(columns: 4,
  table.header([Avesha], [Part of Speech], [Meaning], [Pronunciation]),
  [pata], [noun], [stone], [PA.ta], 
  [kira], [noun], [bird], [KI.ra], 
  [suna], [noun], [sun], [SU.na], 
  [nami], [verb], [see], [NA.mi], 
)


=== Grammar: Word Order
Avesha is a #strong[Subject-Object-Verb (SOV)] language. This means that in a normal sentence, the #strong[subject] comes first, then the #strong[object], then the #strong[verb].

- English: "The bird sees the sun."
- Avesha: `kira suna nami` = bird sun sees

Notice: No words for "the" or "a" – Avesha does not use articles. The noun alone is enough.

=== Worked Examples
+ `kira pata nami` → "The bird sees the stone."
   (subject = bird, object = stone, verb = sees)

+ `suna kira nami` → "The sun sees the bird."
   (subject = sun, object = bird, verb = sees)

Word order is strict: subject comes before object.

=== Practice Exercise
#practice[Translate these sentences into Avesha using the words you have learned: 1. The stone sees the bird. 2. The bird sees the sun. 3. The sun sees the stone.]


Check your answers: 1) `pata kira nami` 2) `kira suna nami` 3) `suna pata nami`.

#line(length: 100%, stroke: 0.5pt + luma(200))


== Lesson 2: More Nouns and Adjectives

=== Vocabulary
#table(columns: 4,
  table.header([Avesha], [Part of Speech], [Meaning], [Pronunciation]),
  [kanu], [noun], [hand], [KA.nu], 
  [talu], [noun], [river], [TA.lu], 
  [mira], [adjective], [bright], [MI.ra], 
  [lasu], [adjective], [cold], [LA.su], 
)


=== Adjective Position
Adjectives come #strong[before] the noun they describe. So "bright sun" is `mira suna`, and "cold river" is `lasu talu`.

=== Building Simple Sentences with Adjectives
You can now make longer subject phrases.
- `mira kira talu nami` → "The bright bird sees the river."
  (subject = bright bird, object = river, verb = sees)

- `lasu talu pata nami` → "The cold river sees the stone."
  (subject = cold river, object = stone, verb = sees)

=== Worked Example
`mira suna lasu talu nami` → "The bright sun sees the cold river."
Breakdown: `mira` (bright) `suna` (sun) = subject, `lasu` (cold) `talu` (river) = object, `nami` (sees).

=== Practice Exercise
#practice[Translate these into Avesha: 1. The cold stone sees the bright bird. 2. The bright river sees the hand. 3. The sun sees the cold hand.]


Answers: 1) `lasu pata mira kira nami` 2) `mira talu kanu nami` 3) `suna lasu kanu nami`.

#line(length: 100%, stroke: 0.5pt + luma(200))


== Lesson 3: Verbs and More Actions

=== Vocabulary
#table(columns: 4,
  table.header([Avesha], [Part of Speech], [Meaning], [Pronunciation]),
  [palu], [verb], [run], [PA.lu], 
  [tani], [verb], [speak], [TA.ni], 
)


=== Sound Change Reminder
Remember: `t` before `i` becomes `s`. So when you see `tani`, pronounce it as `sani`. This is part of the language's natural sound.

=== Using Verbs in SOV
You already know how to do this. Now add two new actions.

- `kira palu` → "The bird runs."
  (no object needed – some verbs can be intransitive)
- `talu palu` → "The river runs." (like "flows")

- `pata tani` → "The stone speaks."
  (pronounce: `pata sani` because the `t` of `tani` becomes `s`)

=== Verb with Object
`kira pata nami` = bird stone sees.
But what about "The bird sees the stone and runs"? Avesha can link actions simply: `kira pata nami palu` = "bird stone sees (and) runs". No "and" is needed – just put verbs in order.

=== Worked Example
`suna talu nami tani` → "The sun sees the river and speaks."
Breakdown: `suna` (sun) `talu` (river) `nami` (sees) `tani` (speaks). Remember: `tani` is pronounced `sani`.

=== Practice Exercise
#practice[Translate these into Avesha: 1. The bird runs and sees the river. 2. The stone speaks and runs. 3. The bright sun sees the cold river and speaks.]


Answers: 1) `kira talu nami palu` 2) `pata tani palu` (pronounce as `pata sani palu`) 3) `mira suna lasu talu nami tani` (pronounce last word as `sani`).

#line(length: 100%, stroke: 0.5pt + luma(200))


== Lesson 4: Plural and Dative Case

=== Affixes
#table(columns: 4,
  table.header([Gloss], [Form], [Position], [Meaning]),
  [PL], [u], [suffix], [plural (more than one)], 
  [DAT], [ti], [suffix], [dative (to, for)], 
)


=== Plural
Add #strong[-u] to a noun to make it plural.
- `kira` (bird) → `kiru` (birds)
- `pata` (stone) → `patu` (stones)
- `suna` (sun) → no example given, but `sunau`? Actually, `suna + u = sunau` (suns). Be careful: if the noun ends in a vowel, just add `u`. So `suna` → `sunau`. But note: `talu` (river) → `taluu`? Wait, rule says suffix -u. `talu + u = taluu`. That's fine – say it as two syllables: `ta.lu.u`.

=== Dative Case
Add #strong[-ti] to a noun to mean "to" or "for" that noun.
- `pata` (stone) → `patati` (to the stone)
- `kira` (bird) → `kirati` (to the bird)

The dative noun often goes before the verb, after the direct object (if any) in SOV order. Typically: Subject Object Dative Verb.

=== Word Order with Dative
`suna kira tani` = "The sun speaks to the bird."
But with dative, it's clearer: `suna kirati tani` = "The sun to-the-bird speaks." Remember: pronounce `tani` as `sani`.

- `kira patu nami` = "The bird sees stones."
- `kira patu nami palu` = "The bird sees stones and runs."

=== Worked Examples
+ `kiru suna tani` = "Birds see the sun (and) speak."
   Wait, `sunati` needed for object? No, `suna` is object, `kiru` is subject. So: `kiru suna nami tani` = "Birds see the sun and speak."

+ `kanu patati nami` = "The hand sees the stone." No – `patati` is "to the stone", so `kanu patati nami` means "The hand sees to the stone"? Actually, `nami` takes a direct object. So `kanu pata nami` = "hand sees stone." Dative is for indirect objects:
   `kira patu nami patati` = "The bird sees stones to the stone." (Weird but possible.)

Better example: `suna kiru tani` = "The sun speaks birds" doesn't make sense. Use dative: `suna kiruti tani` = "The sun speaks to birds."

=== Sound Change Reminder
`n` before `p` changes to `m`. So if you say `kanu pata` (hand stone) in a sentence like `kanu pata nami` = "hand sees stone," the `n` of `kanu` is not directly before `p` because there's a `u`. But if you say `kan pata` without the final vowel? No – don't drop vowels. The rule only applies when they meet. In connected speech, `kanu pata` – the `n` is followed by `u`, no change.

=== Practice Exercise
#practice[Translate these into Avesha: 1. The stones see the birds. 2. The sun speaks to the cold river. 3. The bright birds run to the stone.]


Answers: 1) `patu kiru nami` (but careful: `kiru` = birds, `patu` = stones – subject first = stones see birds? Yes, `patu kiru nami` = stones see birds) 2) `suna lasu taluti tani` (pronounce `taluti` = TA.lu.ti, `tani` = SA.ni) 3) `mira kiru patati palu`.

#line(length: 100%, stroke: 0.5pt + luma(200))


== Lesson 5: Word Building – Agent Nouns

=== Rule
#table(columns: 5,
  table.header([Name], [From], [To], [Suffix], [Meaning]),
  [agent], [verb], [noun], [-ar], [one who verbs], 
)


You can make a noun from a verb by adding #strong[-ar]. This means "one who does the action."

- `nami` (see) → `namar` (seer, one who sees)
- `palu` (run) → `palar` (runner)
- `tani` (speak) → `tanir` (speaker) – but remember `t` before `i` becomes `s`, so pronounce as `sanir`.

=== Using Agent Nouns
Agent nouns are regular nouns. They can be subjects, objects, take plural and dative.

- `palar talu nami` = "The runner sees the river."
- `tanir suna palu` = "The speaker sees the sun (and) runs." – wait, no: `tanir suna nami palu` = "speaker sees sun and runs."

=== Plural of Agent Nouns
`palar` + `u` = `palaru` (runners)
`tanir` + `u` = `taniru` (speakers)

=== Worked Example
`namaru lasu pata nami` = "Seers see the cold stone."
Breakdown: `namaru` (seers, subject), `lasu pata` (cold stone, object), `nami` (see).

=== Practice Exercise
#practice[Translate these into Avesha: 1. The runner speaks to the birds. 2. The seers see the bright sun. 3. The cold river runs.]


Answers: 1) `palar kiruti tani` (pronounce `tani` as `sani`) 2) `namaru mira suna nami` 3) `lasu talu palu`.

#line(length: 100%, stroke: 0.5pt + luma(200))


== Lesson 6: Reading and Translating Sample Text

=== Sample Text
Here is a sample text from the language brief:

#practice[\[02\] kira suna nami. tani palu.]


=== Word-by-Word Gloss
#table(columns: 2,
  table.header([Avesha], [Gloss]),
  [kira], [bird], 
  [suna], [sun], 
  [nami], [see(s)], 
  [tani], [speak(s)], 
  [palu], [run(s)], 
)


=== Interlinear Translation
Line by line:

`kira suna nami` = "Bird sun sees."
In natural English: "The bird sees the sun."

`tani palu` = "Speaks runs."
This could be parsed as a second sentence: "It speaks and runs." The subject from the first sentence (`kira`) carries over.

=== Full Translation
"The bird sees the sun. It speaks and runs."

=== Your Turn
Now translate this variation on the sample text:

`mira kira lasu talu nami palu`
Break it down: subject = `mira kira` (bright bird), object = `lasu talu` (cold river), verbs = `nami palu` (sees and runs).
Translation: "The bright bird sees the cold river and runs."

=== Practice Exercise
#practice[Translate the following into English: `suna patu nami tani. kiru palu.` (Hint: `patu` = stones, `kiru` = birds)]


Answer: "The sun sees the stones and speaks. The birds run."

#line(length: 100%, stroke: 0.5pt + luma(200))


== Lesson 7: Review and Extended Practice

=== Vocabulary Review
Here are all the words you have learned:

#table(columns: 2,
  table.header([Avesha], [Meaning]),
  [pata], [stone], 
  [palu], [run], 
  [kira], [bird], 
  [kanu], [hand], 
  [nami], [see], 
  [tani], [speak], 
  [talu], [river], 
  [mira], [bright], 
  [suna], [sun], 
  [lasu], [cold], 
)


=== Grammar Review
- Word order: Subject Object Verb (SOV)
- Adjectives before nouns
- Plural: add -u
- Dative: add -ti (means "to" or "for")
- Agent noun: add -ar to a verb
- Sound changes: `t` becomes `s` before `i`; `n` becomes `m` before `p`

=== Practice Exercise
#practice[Combine everything you know. Translate these sentences into Avesha: 1. The cold stones see the bright birds. 2. The runner speaks to the river. 3. The seers run and see the suns. 4. The hand sees the stone to the speaker.]


Answers:
+ `lasu patu mira kiru nami`
+ `palar taluti tani` (pronounce `tani` as `sani`)
+ `namaru palu sunau nami`
+ `kanu pata nami tanirti` (here `tanirti` = to the speaker; subject = hand, object = stone, verb = sees, dative = to-speaker)

=== Final Note
You now have the tools to read and write basic Avesha. Keep practicing with the words and rules you have, and you will become comfortable with this beautiful, simple language. Good luck, and remember: `mira suna lasu talu nami` – the bright sun sees the cold river.

