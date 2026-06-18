#import "../design.typ": *

#chapter(number: 13, part: "Part IV — World Building",
  title: "Places and characters")

#dropcap("T")hree system books — Places, Characters, and
Artefacts — together form what inkhaven calls the
#emph[lexicon]. Their titles power an inline highlight
overlay in the editor: when you type a name that's in
the lexicon, it lights up.

#section("Building entries")

A character card is just a paragraph in the Characters book:

```typst
= Aerin Sandreaver

Twenty-three years old when the storm comes. Daughter of
the Highkeep gatekeeper. Carries a wooden flute that
belonged to her grandfather. Travels light; reads
old maps in candlelight.
```

Same shape for Places and Artefacts. The title is what gets
matched against your manuscript prose.

#section("The highlight overlay")

Open any paragraph. Type "Aerin walked into Highkeep" —
both names light up:

#figure_slot(
  id: "lexicon-highlight",
  caption: "Lexicon overlay — character names in cyan, place names in yellow, artefacts in mauve. Subtle but always-visible.",
  height: 35mm,
)

Default colour scheme (Catppuccin Mocha):

#chord_table((
  chord_row("Cyan", "Character match."),
  chord_row("Yellow", "Place match."),
  chord_row("Mauve", "Artefact match."),
))

#section("Stemming")

The lexicon scanner uses a Snowball stemmer keyed to your
`language:` setting. So "Aerin" matches "Aerin's";
"Highkeep" matches "Highkeep's". For Russian, "Москва"
matches "Москве", "Москвою", etc. — declension-aware.

Stemmers available: english, russian, french, german,
spanish, italian, portuguese, dutch, danish, swedish,
norwegian, finnish, hungarian, romanian, arabic, basque,
catalan, turkish, irish, tamil. Add `editor.stemming.languages`
in HJSON for multiple at once.

#section("RAG against an entry — `Ctrl+B P / C / A`")

When the cursor is on a lexicon match, three chords ask
the AI about the matched entry:

#chord_table((
  chord_row("Ctrl+B P", "Place RAG — sends the matched entry's body + nearby paragraphs to the AI."),
  chord_row("Ctrl+B C", "Character RAG."),
  chord_row("Ctrl+B A", "Artefact RAG."),
))

The matched entry lands as RAG context, and the AI pane
prompt slot is focused so you can ask a follow-up question.
Useful when you want to check whether your prose about
Aerin matches her established card.

#section("Lookup chords (no RAG)")

#chord_table((
  chord_row("Ctrl+B P (no match)", "Open the Places book listing."),
  chord_row("Ctrl+B C (no match)", "Open the Characters book listing."),
  chord_row("Ctrl+B A (no match)", "Open the Artefacts book listing."),
  chord_row("Ctrl+B N", "Open the Notes book listing — same shape."),
  chord_row("Ctrl+B U", "Open the Research book listing."),
))

When the cursor isn't on a match, the same chords open a
list of every entry in the relevant book — pick one with
Enter.

#section("Image references in entries")

Character cards can include images:

```typst
= Aerin Sandreaver

#image("aerin-portrait.png", width: 60%)

Twenty-three years old when the storm comes …
```

Drop the PNG into `books/characters/`. The image renders
in `Ctrl+V R` preview + the final PDF. Inkhaven's image
node kind (Chapter 3) is a separate tree node you can
reorder; the `#image()` call is independent of the node.

#recap((
  [Lexicon = Places + Characters + Artefacts books. Titles auto-highlight in the editor.],
  [Snowball stemmer matches inflections; multilingual via `editor.stemming.languages`.],
  [`Ctrl+B P/C/A` — when on a match, RAG against the entry; when not, open the book listing.],
  [`Ctrl+B N/U` — Notes / Research book listings.],
  [Character cards can include images via `#image()` + co-located PNGs.],
))
