#set document(title: "Avesha — A Grammar")
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
  #text(size: 32pt, weight: "bold")[A Grammar of Avesha] \
  #v(4mm) #text(size: 13pt, style: "italic", fill: luma(90))[Phonology · Morphology · Syntax] \
]
#pagebreak()

#outline(title: "Contents", depth: 2)
#pagebreak()

= Study Guide
== Avesha Study Guide

Welcome to the Avesha language! This guide walks you through the basic structure of the language. We'll define every linguistic term we use, so no prior training is needed. Let's begin.

=== Phoneme

A #strong[phoneme] is the smallest unit of sound in a language that can change the meaning of a word. Avesha has a small sound system: only #strong[8 consonants] and #strong[3 vowels]. That makes it easy to learn to pronounce.

=== Consonant and Vowel

A #strong[consonant] is a sound made by partly or fully blocking airflow through the mouth (like #emph[t], #emph[n], or #emph[s]). A #strong[vowel] is a sound made with no such blockage (like #emph[a], #emph[e], or #emph[i]). In Avesha, the 3 vowels are likely a basic set (like #emph[a], #emph[i], #emph[u]), and the 8 consonants form a minimal set.

=== Syllable

A #strong[syllable] is a single, unbroken sound of a word, usually containing one vowel. For example, the word "ti" has one syllable, while "kati" has two (ka-ti). Avesha's small phoneme inventory means most syllables are simple (consonant + vowel).

=== Stress and Penultimate Stress

#strong[Stress] is the emphasis placed on a particular syllable in a word. Avesha uses #strong[penultimate stress]: the second-to-last syllable of a word is always stressed. For example, in a word like #emph[kati], the stress falls on #emph[ka] (because it's the second syllable from the end). This rule is consistent and predictable.

=== Allophony / Conditioned Sound Change

#strong[Allophony] (or #strong[conditioned sound change]) happens when a phoneme is pronounced slightly differently depending on the sounds around it. In Avesha, two rules apply:

- #strong[`t > s / _ i`]: The consonant #emph[t] becomes #emph[s] when it comes #emph[before] an #emph[i] vowel. So a word like #emph[ti] is actually pronounced #emph[si].
- #strong[`n > m / _ p`]: The consonant #emph[n] becomes #emph[m] when it comes #emph[before] a #emph[p] consonant. So a sequence like #emph[npa] is pronounced #emph[mpa].

These changes are automatic; you don't need to think about them—just follow the rule when speaking or writing.

=== Affix

An #strong[affix] is a small piece added to a word to change its meaning or grammatical function. In Avesha, all affixes are #strong[suffixes] (they come after the root). The brief gives two:

- #strong[`ti`] (dative case) adds the meaning of "to" or "for" something.
- #strong[`u`] (plural) makes a noun refer to more than one.

For example, if your noun root is #emph[kata] (cat), #emph[kata-u] means "cats," and #emph[kata-ti] means "to/for the cat."

=== Inflection vs. Derivation

#strong[Inflection] makes a word fit a grammatical role (like case or number) without changing its core meaning. #strong[Derivation] creates a whole new word with a different meaning, often changing its part of speech. Avesha uses both:

- The suffix #strong[`u` (plural)] and #strong[`ti` (dative)] are examples of #strong[inflection]: they modify a noun for number or case.
- The agent-noun rule is a #strong[derivation] (see below).

=== Grammatical Case

A #strong[grammatical case] is a way of marking a noun’s role in a sentence (e.g., who is doing the action, who is receiving it). Avesha has #strong[case] as a feature, with at least one specific case:

=== Dative Case

The #strong[dative case] marks the indirect object—the recipient or beneficiary of an action. In Avesha, it is formed by adding the suffix #strong[-ti] to a noun. For example, in "I give the book to the teacher," "teacher" would take the dative case. So if "teacher" is #emph[sapu], then #emph[sapu-ti] means "to/for the teacher."

=== Word Order: SOV

#strong[Word order] is the usual arrangement of subject, object, and verb in a sentence. Avesha uses #strong[SOV] order: #strong[Subject] comes first, then #strong[Object], then #strong[Verb]. For example:

- #emph[Kata nana pitu] (Cat mouse sees) = "The cat sees the mouse."

=== Morphosyntactic Alignment: Nominative-Accusative

#strong[Morphosyntactic alignment] tells us how a language treats the subject of an intransitive verb (like "run") and the subject/object of a transitive verb (like "see"). In a #strong[nominative-accusative] system, the subject of both intransitive and transitive verbs is in the same case (the #strong[nominative]), while the object of a transitive verb takes a special case (the #strong[accusative]). In Avesha, since case marking exists, nouns likely have a basic form for the subject and a different form for the direct object (though only the dative suffix is given in the brief—the accusative may be unmarked or marked by context).

=== Adposition and Postposition

An #strong[adposition] is a word that shows a relationship between two things (like "in," "on," "to"). Avesha uses #strong[postpositions], which come #emph[after] the noun they relate to. For example, instead of "to the cat," you'd say #emph[kata] \[postposition\]. Since no specific postposition is given, you'll know they always follow the noun.

=== Agent Noun

An #strong[agent noun] is a noun that means "one who does" an action. In Avesha, you can derive an agent noun from a verb. For example, from the verb #emph[pitu] (to see), you could form a noun meaning "seer" (the one who sees). The exact suffix isn't listed, but the rule "agent | verb | noun" means you take a verb and apply a process to get a noun meaning "the doer of that action."

Remember: always pronounce #emph[ti] as #emph[si], and #emph[n] before #emph[p] as #emph[m]. Stress the second-to-last syllable. Practice with simple sentences like #emph[Kata sapu-ti pitu] ("The cat sees to the teacher"), and you're on your way.

#pagebreak()

= Phonology
*Consonants.* p · t · k · s · m · n · l · r

*Vowels.* a · i · u

*Phonotactics.*
- no geminate (doubled) consonants

*Allophony.*
- `t > s / _ i`
- `n > m / _ p`

*Stress.* penultimate — the second-to-last syllable

= Morphology
*Affixes.*
/ *DAT*: `ti` #emph[Suffix]
/ *PL*: `u` #emph[Suffix]

*Derivation.*
- *agent*: verb → noun via `ar`

= Grammar
#table(columns: 2, stroke: none,
  [adposition], [postposition #text(fill: gray)[— after the noun (the house to)]],
  [alignment], [nominative\_accusative #text(fill: gray)[— subject vs object (most European languages)]],
  [case], [yes],
  [word order], [sov #text(fill: gray)[— subject–object–verb (Japanese, Turkish, Latin)]],
)

= Sample texts
== 02
kira suna nami. tani palu.

