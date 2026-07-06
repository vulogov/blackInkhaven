#import "../design.typ": *

#v(1cm)
#text(font: body_family, size: 22pt, weight: "bold")[Before You Begin]
#v(6mm)
#line(length: 100%, stroke: 0.5pt + ink_rule)
#v(8mm)

This is a book about building a world you can believe in — and then writing
inside it. A place with a sky that makes sense, land that the weather actually
shapes, rivers that run where rivers would run, cities that stand where people
would build them. A world with a past that predates page one, peoples who did
not spring into being the moment your hero met them, and a calendar that says
what season it is when your character looks out the window.

You will build it inside *Inkhaven* — a writing tool with a *World Simulation*
built in. You will not draw a map by hand or fill a binder with notes that
quietly contradict each other by chapter twelve. Instead you will set a few
starting conditions — a star, a planet, a seed — and let a small, patient engine
grow the rest, consistently, every time. Then you will deepen that world with a
history and a people, and finally carry it to your desk, so the world is *present
while you write* rather than filed away in a drawer.

#section("Who this book is for")

This guide assumes *no prior knowledge* — of worldbuilding, or of Inkhaven.

You do not need to have built a world before. You do not need to know what a
biome is, or how a river system forms, or why a coastline ends up where it does.
Every idea is introduced the first time it is needed, in a marked box like this
one:

#term("Worldbuilding")[
  The craft of inventing a setting — its geography, history, peoples, and rules —
  thoroughly and consistently enough that a reader (or a player, or you, six
  months later) never catches it contradicting itself. Good worldbuilding is
  less about how much you invent than about how well the pieces hold together.
]

You do not need to be a novelist. A world serves anyone who tells stories in a
setting — a novelist, a game-master preparing a campaign, a screenwriter, a
designer. Where this book says "your story," read it as "your project"; the
craft is the same.

And you do not need to be a programmer. You will type a few short commands and
edit one small text file, and the book walks you through every character of it.

#section("How this book talks to you")

Because you are learning a craft and not just a tool, the asides in this book
come in five clearly-marked kinds. You will see each of them often, so meet them
now.

#term("Term")[
  A boxed definition, like the two above. Every piece of jargon is defined in one
  of these the first time it appears — so you can always read straight through
  without looking anything up.
]

#note[
  A *Note* is a practical remark about how Inkhaven behaves — a command, a
  default, a place a thing is saved. When you are actually at the keyboard, the
  Notes are what you act on.
]

#insight[
  An *Insight* is a worldbuilding principle — the "why" beneath the "how", the
  thing worth remembering long after you have forgotten which command did what.
  The Insights are the real subject of this book.
]

#question[
  An *Ask Yourself* box poses a worldbuilding question for you to answer about
  *your own* world before you build it. Worldbuilding is mostly the art of asking
  good questions in the right order; these boxes are that order, made explicit.
]

#pitfall[
  A *Pitfall* is a common mistake and how to step around it — usually a way that
  worlds quietly become inconsistent, which is the one thing good worldbuilding
  exists to prevent.
]

#tryit[
  A *Try It* box is a short exercise at the keyboard: a command to run, a value
  to change, a result to look at. You will learn this far faster by doing it than
  by reading about it, so do them.
]

#section("What you will be able to do")

By the last page you will be able to:

#list(
  [grow a complete physical world — sky, land, climate, rivers, and settlements —
   from a handful of choices, and understand *why* each part came out the way it
   did;],
  [give that world a past: an age of founding, migrations, the rise and fall of
   realms, dated on the world's own calendar;],
  [give it a people: nations, cultures, beliefs, and the sketch of a language
   each one speaks;],
  [decide, deliberately, what you let the world invent and what you declare by
   your own hand — and keep the two from ever contradicting each other;],
  [and write *against* that world: with the season and weather of a scene at your
   cursor, a check that your riders cannot cross a continent in a day, and the
   world's places and calendar flowing straight into your manuscript.],
)

#section("Not a set of tools — one environment")

It is easy to meet Inkhaven as a *toolbox*: a world compiler here, a map renderer
there, a place to keep character notes, a checker for your prose. Each of those is
real, and this book will teach you to use them. But if a toolbox is all you see,
you will use these the way you used the binder and the spreadsheet before them —
side by side, and quietly drifting apart by chapter twelve. Inkhaven is built on
the opposite premise. Its parts are not a set of instruments laid out on a bench;
they are one *environment*, and every part knows about the others.

#term("System book")[
  Inkhaven keeps certain kinds of project-wide material in dedicated *system
  books* that live alongside your manuscript and can be read, searched, and cited
  from it. Some you fill *by hand* — *Characters* for your cast, *Places* for your
  settings, *Artefacts* for the objects that matter to your story — and some the
  tools maintain for you, like the *Timeline* of your story's events and the
  *World* book that holds your compiled world. They are all the same kind of
  thing, and they all speak to the same manuscript.
]

The characters, places, and artefacts of your world are *yours to author*, first
and always. You open the Characters book and write your protagonist; you open
Places and set down the city your story opens in; you open Artefacts and record
the sword, the letter, the stolen crown. These entries are not outputs of the
simulation — they are your own worldbuilding, kept where the rest of Inkhaven can
see them. The World Simulation is one *contributor* to that shared world, never
its author: it can *propose* a settlement into Places, or a realm's ruler into
Characters, for you to accept or refuse — but the books are yours, the entries are
yours, and most of what fills them you will have written by hand. The world never
touches the Artefacts book at all; the objects of your story are wholly your own.

Because these books are shared, the connections cost you nothing. A place you tag
in your prose is the same place the world gave a climate and a position on the
map. A character you wrote is the same character an event on the Timeline is dated
around. The season at your cursor is read from the same astronomy that raised your
mountains. Nothing has to be kept in step by hand, because nothing lives in its
own corner to begin with.

#insight[
  A toolbox asks you to remember which tool holds which truth, and to reconcile
  them yourself. An environment holds *one* truth and lets every tool read it.
  That is the difference this book is really teaching: not a set of commands, but a
  place where your world, your cast, your map, and your manuscript are all the same
  world, seen from different windows.
]

So the world you build here is not a separate program bolted onto your writing. It
feeds the same Places you tag in your prose, the same Timeline your scenes are
dated on, the same cast and objects you invented — and, as you will see in Part
VI, the very paragraph you are typing.

#insight[
  A setting is only worth the trouble if it *touches the page*. The measure of a
  built world is not how thick the appendix is — it is how often the world saves
  you from a contradiction, hands you a detail, or tells you what the weather is
  while you write. Keep that test in mind through every chapter.
]

#section("The one idea to carry with you")

If you remember nothing else from this introduction, remember this picture. It
is the spine of the whole book:

#world_from_seed()

You do not build a world the way you draw a map — placing every mountain by hand
and hoping the rivers you sketch would really flow downhill. You set the starting
conditions, and the world *follows* from them. Give the engine a star and a
planet and a number to seed its choices, and it works out the seasons, raises the
mountains, runs the rain down them into rivers, and settles people where the land
would support them — the same way, every time you ask.

#term("Seed")[
  A single number that fixes all the "random" choices a world involves — where
  exactly the coastlines fall, which valley grows the first city. The same seed
  always produces the same world, so your world is *reproducible*: you can share
  the short definition, and anyone gets your world back, cities and all.
]

Everything in this book is a way of working with that idea: choosing the starting
conditions well, reading what grew from them, deepening it with time and people,
and finally writing against it.

#section("What you will need")

Inkhaven, installed and open on a project — a folder with a manuscript in it.
That is all. Nothing here requires an account, a subscription, or a paid
connection. The world compiler is fully deterministic and runs offline; the few
places where the world reaches out to the rest of Inkhaven (a language model to
help name things, an external map tool to render a picture) are optional and
clearly marked where they appear.

#section("How to read this book")

Read it front to back the first time. Part I builds the foundation — why a world
is worth building, the idea of a world as a system, and your very first compiled
world — and everything after it assumes those three chapters. Part II grows the
physical world one layer at a time; Parts III and IV give it a past and a people;
Part V is about the line between what you invent and what you declare; and Part
VI brings the whole world to your writing desk. Part VII builds one world end to
end, so you can see the whole process in motion.

Commands you type look like `realworld compile`. Every one of them is a
subcommand of Inkhaven: the full command is `inkhaven realworld compile`, and the
book abbreviates it to `realworld …` once Chapter 3 has made that plain. Keys you
press look like `Ctrl+B W`. When a command shows you something, the book tells you
what you would see and why it matters — but it draws the *shape* of the work with
diagrams rather than screenshots, because screenshots age and the ideas beneath
them do not.

Turn the page, and let us talk about why you would build a world at all.

#recap((
  [This book teaches a *process* for building a believable, consistent world and
   writing inside it — assuming no prior knowledge of worldbuilding or Inkhaven.],
  [Its asides come in five marked kinds — *Note, Insight, Ask Yourself, Pitfall,
   Try It* — plus boxed *Term* definitions for every piece of jargon.],
  [You do not draw a world; you set its starting conditions and a *seed*, and a
   deterministic compiler grows the rest — the same way every time.],
  [Inkhaven is not a toolbox but one *environment*: your *Characters*, *Places*,
   and *Artefacts* books are yours to author by hand, and the World Simulation is
   one contributor that *proposes* into them — never their author. A built world is
   only worth it if it *touches the page*.],
))
