#set document(title: "Avesha — Dictionary")
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
  #text(size: 32pt, weight: "bold")[Avesha Dictionary] \
  #v(4mm) #text(size: 13pt, style: "italic", fill: luma(90))[A lexicon] \
  #v(12mm) #native("\u{E000}\u{E008}\u{E001}\u{E008}")
]
#pagebreak()

#outline(title: "Contents", depth: 2)
#pagebreak()

#let conscript(cp) = text(font: "Avesha", size: 1.5em)[#cp]

= Overview
#table(columns: 2, stroke: none, inset: (x: 0pt, y: 3pt),
  [Phonemes], [11 (8 consonants / 3 vowels)],
  [Entries], [10],
  [Average word], [4.0 phonemes, 2.0 syllables],
)
#pagebreak()

= The Lexicon
#columns(2, gutter: 1.2em)[
== K
/ *kanu* #conscript("\u{E002}\u{E008}\u{E005}\u{E00A}") #text(fill: luma(110))[/ka.nu/] #text(style: "italic", fill: luma(110))[noun]: hand
/ *kira* #conscript("\u{E002}\u{E009}\u{E007}\u{E008}") #text(fill: luma(110))[/ki.ra/] #text(style: "italic", fill: luma(110))[noun]: bird
== L
/ *lasu* #conscript("\u{E006}\u{E008}\u{E003}\u{E00A}") #text(fill: luma(110))[/la.su/] #text(style: "italic", fill: luma(110))[adjective]: cold
== M
/ *mira* #conscript("\u{E004}\u{E009}\u{E007}\u{E008}") #text(fill: luma(110))[/mi.ra/] #text(style: "italic", fill: luma(110))[adjective]: bright
== N
/ *nami* #conscript("\u{E005}\u{E008}\u{E004}\u{E009}") #text(fill: luma(110))[/na.mi/] #text(style: "italic", fill: luma(110))[verb]: see
== P
/ *palu* #conscript("\u{E000}\u{E008}\u{E006}\u{E00A}") #text(fill: luma(110))[/pa.lu/] #text(style: "italic", fill: luma(110))[verb]: run
/ *pata* #conscript("\u{E000}\u{E008}\u{E001}\u{E008}") #text(fill: luma(110))[/pa.ta/] #text(style: "italic", fill: luma(110))[noun]: stone
== S
/ *suna* #conscript("\u{E003}\u{E00A}\u{E005}\u{E008}") #text(fill: luma(110))[/su.na/] #text(style: "italic", fill: luma(110))[noun]: sun
== T
/ *talu* #conscript("\u{E001}\u{E008}\u{E006}\u{E00A}") #text(fill: luma(110))[/ta.lu/] #text(style: "italic", fill: luma(110))[noun]: river
/ *tani* #conscript("\u{E001}\u{E008}\u{E005}\u{E009}") #text(fill: luma(110))[/ta.ni/] #text(style: "italic", fill: luma(110))[verb]: speak
]
