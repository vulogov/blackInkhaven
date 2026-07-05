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

#section("Where worldbuilding sits in Inkhaven")

Inkhaven is a whole writing environment, and the World Simulation is one
subsystem inside it. It is worth seeing, from the start, how the world connects
to everything else — because a world that sits in its own corner is just a
prettier binder. Inkhaven's world does not sit in a corner.

#term("System book")[
  Inkhaven keeps certain kinds of project-wide material in dedicated *system
  books* — Places, Characters, the Timeline, and (for us) a World book — that
  live alongside your manuscript and can be read, searched, and cited from it.
  Your compiled world is written into the World book; its settlements can become
  entries in the Places book; its calendar can drive the Timeline.
]

So the world you build here is not a separate program. It feeds the same Places
you tag in your prose, the same Timeline your scenes are dated on, and — as you
will see in Part VI — the very paragraph you are typing. Worldbuilding in
Inkhaven is a source that pours into the rest of your writing.

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
  [The world is not a separate binder: it feeds Inkhaven's Places, Timeline, and
   the page you are writing. A built world is only worth it if it *touches the
   page*.],
))
