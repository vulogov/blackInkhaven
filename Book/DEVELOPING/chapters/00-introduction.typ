#import "../design.typ": *

#pagebreak(weak: true, to: "odd")

#hide(heading(level: 1, numbering: none, outlined: true, bookmarked: true, "Introduction"))

#v(2cm)
#align(left)[
  #text(font: sans_family, size: 9pt, tracking: 2pt, fill: ink_gray, upper("Introduction"))
  #v(4mm)
  #text(font: body_family, size: 34pt, weight: "regular", fill: ink_black, "What kind of book are you making?")
]
#v(1cm)
#line(length: 100%, stroke: 0.5pt + ink_rule)
#v(10mm)

#dropcap("I")nkhaven is one program, but it is not one tool. It holds a
structured editor, a world simulation, a constructed-language workshop, a
research assistant, a small family of AI readers, a citation manager, and a
press that turns any of it into a finished PDF, EPUB, or manuscript. That is a
great deal of machinery. The natural question — the one this book exists to
answer — is not _what does each part do_ (the other companion books answer that,
part by part) but _which parts do I need, and in what order, for the book I am
actually writing?_

Because the honest answer is: it depends on the book. A novelist and a
research scientist both sit at the same desk, in the same terminal, typing into
the same tree of chapters — and almost nothing else about their process is the
same. The novelist grows a world and reads her prose for what it presupposes;
the scientist grounds every claim in a source and checks that his citations
resolve. The tools are shared. The _work_ is not.

This book is about the work. It sorts the writing Inkhaven supports into a set
of *tracks* — fiction, utopia, science fiction, nonfiction, scenarios, technical
documentation, scientific writing, theology or philosophy, and poetry — and gives each
its own full guide: what is specific to it, which of Inkhaven's tools earn their
place in it, and how those tools tie together into one working process rather
than a drawer of gadgets.

#term("Track")[
  A kind of literary work, and the working process suited to it. A track is not a
  rigid category you must belong to — it is a starting point: a recommended way to
  set up your project, a subset of Inkhaven's tools that pays off for that kind of
  book, and a rhythm of drafting and checking that fits its demands. Most real
  books lean on one track and borrow from a second; the guides are written so you
  can do exactly that.
]

#section("The four faculties")

However different the tracks look, each draws on the same four faculties. Every
guide in this book is, underneath, an account of how one kind of book balances
them.

#four_faculties()

*Structure* is the tree your work lives in — books, chapters, scenes, and
paragraphs, each a real node you can move, split, and search. *Grounding* is
whatever your book must answer to so it stays consistent: a compiled world for
fiction, a corpus of verified facts for nonfiction, a specification for technical
writing. *Reading* is the set of second pairs of eyes Inkhaven can turn on a
draft — the questioning inner readers, the fact-checker, the continuity checks.
And *production* is the press: the same tree rendered into something a reader can
hold.

A track is, in the end, a recipe for these four. Fiction is grounding-heavy and
reading-heavy; technical documentation is structure-heavy and light on
invention; scientific writing lives on grounding and citation. Learn the four,
and every track is a variation you can already half-guess.

#section("The work is a loop")

None of the tracks is a straight line from blank page to finished book. Each is a
loop, and the same loop:

#work_arc()

You *frame* the book — pick a track, set the genre, lay out the first chapters.
You *gather* what will ground it — grow a world, research the facts, collect the
sources. You *draft* against that ground rather than against a void. Then you
*read and revise* — turn the inner readers and the checks on what you wrote — and
return to gather more where the draft exposed a gap. The tracks differ in what
they gather and how hard they check, never in the shape of the loop.

#section("How to read this book")

The first two chapters are for everyone. Chapter 1 lays the tracks side by side —
what makes each distinct, and how to tell which one your book belongs to. Chapter
2 is a full tour of the desk: how to navigate and edit in Inkhaven, the companion
books, snapshots, split-edit, and export — the ground every track stands on.

After that, go to your track's chapter and read it end to end; it assumes you have
read the first two and nothing else. If your book straddles two tracks — a
hard-SF novel is fiction and science fiction; a philosophical essay grounded in
scholarship is theology and nonfiction — read both guides. They are written to
combine.

#note[
  This is a process book, not a reference. When a chapter names a command or a
  keystroke, it shows enough to act; for the exhaustive treatment of any one
  subsystem, the sibling books go deep — *Building the World with Inkhaven* on the
  world simulation, *Constructed Language Development* on the ConLang suite, and
  *The Research Assistant* on grounding and citation. This book's job is to tell
  you which of them to reach for, and when.
]

#insight[
  There is no wrong track, and no track you are locked into. The genre setting is
  one line in a config file; the tools are all present whatever you choose. Picking
  a track is not a commitment — it is a way of deciding, on day one, which of
  Inkhaven's many hands you actually want to hold.
]

Turn the page, and let us lay the tracks out.
