#set document(title: "Avesha — Dictionary")
#set page(paper: "a5", margin: 1.6cm, numbering: "1")
#set text(size: 10pt)
#set par(justify: true)
#let conscript(cp) = text(font: "Avesha", size: 1.4em)[#cp]

#align(center)[
  #text(size: 26pt, weight: "bold")[Avesha] \
  #text(size: 14pt, fill: gray)[Dictionary]
]
#v(1cm)

#heading(level: 1, numbering: none)[Overview]
#table(columns: 2, stroke: none,
  [Phonemes], [11 (8 C / 3 V)],
  [Entries], [10],
  [Word shape], [4.0 phonemes, 2.0 syllables avg],
)
#v(0.5cm)

#columns(2)[
#heading(level: 2, numbering: none)[K]
/ *kanu* #conscript("\u{E002}\u{E008}\u{E005}\u{E00A}") #text(fill: gray)[/ka.nu/] #emph[noun]: hand
/ *kira* #conscript("\u{E002}\u{E009}\u{E007}\u{E008}") #text(fill: gray)[/ki.ra/] #emph[noun]: bird
#heading(level: 2, numbering: none)[L]
/ *lasu* #conscript("\u{E006}\u{E008}\u{E003}\u{E00A}") #text(fill: gray)[/la.su/] #emph[adjective]: cold
#heading(level: 2, numbering: none)[M]
/ *mira* #conscript("\u{E004}\u{E009}\u{E007}\u{E008}") #text(fill: gray)[/mi.ra/] #emph[adjective]: bright
#heading(level: 2, numbering: none)[N]
/ *nami* #conscript("\u{E005}\u{E008}\u{E004}\u{E009}") #text(fill: gray)[/na.mi/] #emph[verb]: see
#heading(level: 2, numbering: none)[P]
/ *palu* #conscript("\u{E000}\u{E008}\u{E006}\u{E00A}") #text(fill: gray)[/pa.lu/] #emph[verb]: run
/ *pata* #conscript("\u{E000}\u{E008}\u{E001}\u{E008}") #text(fill: gray)[/pa.ta/] #emph[noun]: stone
#heading(level: 2, numbering: none)[S]
/ *suna* #conscript("\u{E003}\u{E00A}\u{E005}\u{E008}") #text(fill: gray)[/su.na/] #emph[noun]: sun
#heading(level: 2, numbering: none)[T]
/ *talu* #conscript("\u{E001}\u{E008}\u{E006}\u{E00A}") #text(fill: gray)[/ta.lu/] #emph[noun]: river
/ *tani* #conscript("\u{E001}\u{E008}\u{E005}\u{E009}") #text(fill: gray)[/ta.ni/] #emph[verb]: speak
]
