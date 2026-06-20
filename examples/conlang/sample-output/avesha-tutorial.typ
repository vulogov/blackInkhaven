#set document(title: "Learn Avesha")
#set page(paper: "a5", margin: 1.6cm, numbering: "1")
#set text(size: 11pt)
#set par(justify: true)
#set heading(numbering: "1.")
#let native(cp) = text(font: "Avesha", size: 1.3em)[#cp]

#align(center)[
  #text(size: 28pt, weight: "bold")[Learn Avesha]
]
#v(1cm)

= The sounds
*Consonants.* p · t · k · s · m · n · l · r

*Vowels.* a · i · u

= Your first words
#table(columns: 3, align: (left, left, left),
  table.header([*Word*], [*Script*], [*Meaning*]),
  [*pata*], [#native("\u{E000}\u{E008}\u{E001}\u{E008}")], [stone],
  [*palu*], [#native("\u{E000}\u{E008}\u{E006}\u{E00A}")], [run],
  [*talu*], [#native("\u{E001}\u{E008}\u{E006}\u{E00A}")], [river],
  [*tani*], [#native("\u{E001}\u{E008}\u{E005}\u{E009}")], [speak],
  [*kira*], [#native("\u{E002}\u{E009}\u{E007}\u{E008}")], [bird],
  [*kanu*], [#native("\u{E002}\u{E008}\u{E005}\u{E00A}")], [hand],
  [*suna*], [#native("\u{E003}\u{E00A}\u{E005}\u{E008}")], [sun],
  [*nami*], [#native("\u{E005}\u{E008}\u{E004}\u{E009}")], [see],
)

= Putting words together
Avesha is a *SOV (subject–object–verb)* language.

Words inflect — *pata* in its forms:
/ *pata*: stone
/ *patasi*: stone-DAT
/ *patau*: stone-PL

= A first text
#quote(block: true)[kira suna nami. tani palu.]

#raw("kira       bird
suna       sun
nami       see
tani       speak
palu       run")
