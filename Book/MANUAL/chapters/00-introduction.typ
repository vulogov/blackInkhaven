#import "../design.typ": *

#v(1cm)
#text(font: body_family, size: 22pt, weight: "bold")[About This Manual]
#v(6mm)
#line(length: 100%, stroke: 0.5pt + ink_rule)
#v(8mm)

Most writing tools ask you to leave the page. You open a browser to research, a
second app to track your characters, a third to check continuity, a spreadsheet
to count words, a design program to typeset. Inkhaven's premise is the opposite:
*the whole of writing a book lives in one terminal window* — the prose, the
world it is set in, the facts it must not contradict, the assistant you steer,
and the finished PDF — and you never leave the keyboard to reach any of it.

This manual is the complete tour of that window. It assumes *no prior
knowledge* — not of Inkhaven, not of the terminal beyond opening one — and it
runs in reading order from installing the binary to publishing a book. Read it
front to back once for the shape of the thing; return to any chapter when you
reach the workflow it covers.

#section("What Inkhaven is")

Inkhaven is a single command-line program for writing books and long-form
documentation. It pairs a full-screen editor for #link("https://typst.app")[Typst]
— a modern typesetting language — with a local semantic index, an AI writing
assistant you fully control, versioned snapshots, a backup pipeline, and an
unusually deep set of *reading* intelligences that watch a manuscript for the
things a long book gets wrong. Your prose lives as plain `.typ` files on disk;
a local database tracks the hierarchy and the search index, but the words are
always text you can read, diff, and version-control yourself.

#term("The manuscript is plain files")[
  A paragraph is a `.typ` file on disk, inside a tree of folders for books,
  chapters, and subchapters. Inkhaven manages that tree and indexes it, but it
  never locks your words inside a proprietary format. You can open any paragraph
  in any editor, at any time, and Inkhaven will notice.
]

#section("How this manual is organised")

The book is in nine parts and a set of reference appendices:

#screen(caption: "The shape of the manual")[```
  I    Getting Started ....... install, first project, the panes
  II   Writing .............. the editor, the tree, snapshots, search
  III  The AI Assistant ..... scopes, chat-with-your-book, prompts, cost
  IV   The World & Facts .... places, characters, world, graph, timeline
  V    The Intelligences .... continuity, knowledge, read-through, voices,
                              revision, the inner readers
  VI   Language & Verse ..... conlang, poetry, research, scholarship
  VII  Producing the Book ... PDF, EPUB, web, technical docs
  VIII Scripting ............ the embedded Bund language
  IX   Keeping It Healthy ... backup, doctor, configuration
  A–D  Reference ............ keys, commands, config, the feature index
```]

Parts I through III are the ground floor — everything you need to write and get
help. Parts IV and V are Inkhaven's distinctive depth: the machinery for a book
with a world, a cast, and secrets that must stay kept. Parts VI through IX are
the specialist tracks and the plumbing.

#section("This manual, and the companions")

Inkhaven ships a small library of *companion books*, each a deep dive on one
domain — *Know Your Book* on the reading intelligences, *Building the World* on
worldbuilding, *Poetry with Inkhaven*, *Grounding Your Book in Fact* on research,
and others. This manual is deliberately *breadth-first*: it tells you what each
feature is, how to reach it, and how to run it, then points you to the companion
when you want the full treatment.

#two_track(
  [If you write *fiction*, the heart of the book is Parts IV and V — a world
  and cast Inkhaven can hold in mind, and the readers that watch continuity,
  knowledge, voice, and pacing across a whole manuscript.],
  [If you write *non-fiction*, your centre of gravity is the facts and the
  graph (Part IV), research and sources (Part VI), and the technical-doc and
  index tools (Part VII) — the machinery of claims that must be right.],
)

#section("A word on the pictures")

Inkhaven is a terminal application, so this manual illustrates it with *terminal
frames* rather than photographs — a faithful rendering of what you will see on
screen, set in a monospace type. A frame is truer than a screenshot for a text
interface, and it keeps the book self-contained. When you see one, read it as
"this is what the screen shows here."

#recap((
  [Inkhaven is one terminal program for the *whole* of writing a book — prose,
  world, facts, an AI you steer, and the finished document.],
  [Your manuscript is always *plain `.typ` files* on disk; nothing is locked
  in a proprietary format.],
  [This manual is the *breadth-first tour*, install to publish; the companion
  books hold the deep dives.],
))
