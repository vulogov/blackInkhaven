#import "../design.typ": *

#chapter(number: 6, title: "Choosing what goes public")

A finished Inkhaven project holds far more than the manuscript. It holds your sources,
a glossary, a cast of characters, a map of places, the world behind the story, perhaps
an invented language — a whole workshop. When you publish, you decide how much of that
workshop joins the book on the website. This chapter is the switchboard.

Everything here produces _one_ website. Turning a part on does not make a separate
site; it adds a page to the same site, listed in the same contents down the side. The
book and its apparatus are published together, as one thing.

#section("What is on the site by default")

A plain `inkhaven export html` publishes your manuscript and the two things that
normally belong _in_ a finished book: its *sources* (a bibliography) and its
*glossary*. Everything else in the workshop stays private until you ask for it.

#insight[
  The default is the safe, sensible one: what a reader would expect between the covers
  of the book — the chapters, the references, the glossary — goes public; your working
  material — rough notes, character sketches, the world bible — does not. You never
  accidentally publish your workshop; you _choose_ to.
]

#section("The switchboard")

Which parts are published is controlled by a block in your settings file. Each line is
a part of the workshop and a plain `true` or `false`:

#config("inkhaven.hjson", [```hjson
docs: {
  html: {
    include: {
      sources:    true    # the bibliography (on by default)
      glossary:   true    # your defined terms (on by default)
      characters: true    # the cast of characters
      places:     true    # the places / gazetteer
      world:      true    # the world's description
      language:   true    # the invented language
      mythology:  false   # myths and symbols
      notes:      false   # your private notes — usually leave off
    }
  }
}
```])

Set a part to `true` to publish it, `false` to hold it back. To publish a book with no
bibliography, set `sources: false`. To add your cast and your gazetteer, set
`characters: true` and `places: true`. There is no command-line flag for these — they
live in the settings file, because they are a decision about the book, not about one
export.

Each part you switch on becomes its own page at the end of the site, and appears in the
contents list beside your chapters. Here is what each one turns into on the page:

#gloss("`sources`")[A *bibliography* — your citations, sorted by author, each formatted as a reference with its link.]
#gloss("`glossary`")[Your *defined terms*, laid out for reading.]
#gloss("`characters`")[The *cast* — a page of your characters and what you have recorded about each.]
#gloss("`places`")[A *gazetteer* — your places and their details.]
#gloss("`world`")[Your *world*, written up as a *narrative guide* — its sky, its lands, its waters, its peoples, their livelihood, the laws of magic, and its history, in readable prose drawn from your `world.hjson`.]
#gloss("`language`")[Your *invented language* — a *dictionary* presented as a table your readers can *sort by any column and filter as they type*, alongside your grammar.]
#gloss("`mythology`")[Your *myths and symbols*.]
#gloss("`notes`")[Your *notes*. Almost always left `false` — this is your private desk.]

#note[
  A part you switch on but never filled in adds nothing — if your Places book is empty,
  `places: true` produces no page and no clutter. So you can turn on the parts you
  intend to grow into, and they appear only once they have something to show.
]

#tryit[
  Turn on one part you have actually written — say your glossary or your list of places.
  Set it to `true` in the settings block above, run `inkhaven export html -o site`, and
  open the site. A new entry has appeared in the contents, and clicking it shows that
  part of your workshop, formatted to match the rest of the book. Turn it back to
  `false` and export again to confirm it vanishes. That is the whole switchboard.
]

#pitfall[
  `notes` is off by default for a reason: your notes are where unfinished thoughts,
  spoilers, and reminders-to-self live. Turn it on only if you genuinely mean to
  publish that material. When in doubt, leave it off — you can always switch it on
  later.
]

#section("Which book")

If your project holds several books, choose the one that becomes the site with
`--book-name`, as Chapter 2 showed. Each book makes its own site; to publish two,
export each into its own folder.

#run[```
inkhaven export html -o site --book-name "The Drowned Atlas"
```]

#section("Which edition")

The finest control is over _which paragraphs_ of the manuscript go out — because one
book can quietly contain several editions. You mark a paragraph for an edition in the
editor with the tagging chord (`Ctrl+B ]`), giving it a tag of the form
`profile:edition:full` or `profile:audience:expert`. A paragraph everyone should see
gets no such tag.

#term("A profile")[
  A label on a paragraph saying which edition it belongs to, written
  `dimension:value` — like `edition:full`. At export you ask for an edition, and only
  the matching paragraphs (plus the unlabelled ones) go out.
]

Then publish one slice by naming the edition:

#run[```
inkhaven export html -o site --profile edition=full
```]

The rule is designed to be safe: for each edition you ask for, a paragraph is published
only if it _matches_ that edition or carries _no label for it at all_. Unlabelled
writing appears in every edition; asking for no profile publishes everything. So labels
only ever _narrow_ — they never expose something you did not mark.

#pitfall[
  Because asking for no profile publishes every paragraph, do not rely on omitting a
  profile to hide a draft. If a paragraph must never go public, keep it out of the book
  or below its finished status — profiles choose between editions, they are not a lock
  on secrets.
]

#recap((
  [The site is *one* website; switching a part on adds a page to it, in the same contents.],
  [By default you publish the manuscript, the *sources*, and the *glossary*; the rest of the workshop is private.],
  [`docs: { html: { include: { … } } }` in the settings file turns parts on or off — characters, places, world, language, mythology, notes.],
  [An empty part adds nothing; `--book-name` picks the book; `--profile` publishes one edition and only ever narrows.],
))
