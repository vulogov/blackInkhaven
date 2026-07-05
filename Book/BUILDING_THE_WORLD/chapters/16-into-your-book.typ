#import "../design.typ": *

#chapter(number: 16, title: "Into Your Book")

You have a world that comes to your cursor and a world that checks your prose.
The last step of the whole journey is the quietest and, in a way, the most
important: letting the world flow *into the manuscript itself* — not as a
separate reference you keep beside your book, but as the very Places you tag, the
Timeline you date scenes on, the page you type. A world that only ever answers
questions is still a world apart. The aim of this chapter is a world that is a
*source*: material that pours into the rest of your writing rather than sitting in
a silo next to it.

#insight[
  The world is not a separate document. Its settlements become the same *Places*
  you tag in prose; its calendar and history become the same *Timeline* your
  scenes are dated on; its facts reach the page you are typing. A setting kept in
  its own file is a binder; a setting that flows into the tools you already write
  with is a source. The difference is the whole reason to have built it here.
]

Here are the bridges, drawn as one picture — every path the world takes into your
book:

#prose_bridges()

#section("Settlements into the Places book")

The cities and towns your demographics layer grew are not much use to a novelist
as a list in a world file. They become useful when they are *Places* — entries
you can tag in prose, search, and cite.

#term("Places book")[
  Inkhaven's system book of settings — the towns, cities, regions, and landmarks
  your story uses. Tag a place in your prose and Inkhaven can gather every scene
  set there; a Place entry is the shared, canonical record of a spot your story
  keeps returning to.
]

The bridge here is `realworld propose`. The world does not write itself into your
Places book — it *proposes* its settlements as Place entries, and you accept or
reject each one. Accept Rivenmouth and it becomes a real Place you can tag; leave
the hundred villages you will never name unaccepted and they stay in the world
file, out of your way. The authority runs one direction only: the world offers,
you choose.

#note[
  Run `realworld propose` to have the world put its settlements forward as Place
  candidates, and `realworld proposals` to review what is waiting. Nothing enters
  the Places book until you accept it — the world never writes on its own.
]

#section("The calendar and history into the Timeline")

The second bridge carries time. Your world computed a calendar from its sky and a
chronology from its founding, and both belong in the one place your story keeps
time.

#term("Timeline")[
  Inkhaven's system book of *when* — the calendar your story runs on and the
  dated events along it. Scenes are dated against the Timeline; it is what lets
  Inkhaven know that one scene comes before another, and what season a given date
  falls in.
]

The astronomy's calendar adopts into `timeline.calendar`: run `realworld calendar` and the world hands you the months, weekdays, and new-year alignment it
derived from the sky, as lines you adopt so that your scene dates and your world's
seasons share one root. The history adopts the same way — `realworld history`
emits its epochs and events as ready-made `inkhaven event add` lines, one per
founding and migration and the rise or fall of a realm, dated in years before the
present. You paste in the ones your story reaches back to and leave the rest; the
world's whole past is available, and only the part you use enters your Timeline.

#insight[
  Notice the same discipline on every bridge: the world *proposes*, in a form you
  can adopt, and you decide what crosses over. Places come as proposals you
  accept; the calendar and history come as command lines you choose to run. The
  world's reach into your manuscript is real but never unilateral — the author
  always has the last word.
]

#section("The world as a manuscript appendix")

The last bridge is the simplest. Sometimes you do not want the world threaded
through Places and Timeline — you want it *whole*, one readable reference bound in
the back of the book, for yourself or your reader.

```
realworld gazetteer --output world-reference.md
```

The gazetteer gathers the entire compiled world — its calendar and sky, its
regions and landmarks, its waters, its settlements, its economy and magic — into
a single consolidated Markdown document. Written out with `--output`, it becomes
a file you can keep as a series bible or fold into the manuscript as an appendix:
the gazetteer at the back of the fantasy novel, made not by hand but from the
same world every other bridge draws on, and so guaranteed to agree with them.

#tryit[
  Run `realworld gazetteer --output world-reference.md` and open the file. Read it
  as a stranger to your own world would — this is the reference your reader might
  hold. Where it reads thin, you have found the part of the world worth deepening
  next; where it reads rich, you have found what your prose can safely lean on.
]

#section("The world touches the page")

Step back and look at what the bridges add up to. A settlement your climate and
rivers decided becomes a Place you tag in a sentence. A calendar your sky computed
becomes the date at the top of a scene. A founding from your world's deep past
becomes an event on the Timeline your story is measured against. And while you
write the scene itself, the season and the people of that place sit in the corner
of the editor, and the fact-checker reads the finished paragraph back against all
of it. There is no seam left between the world and the book. That was the promise
made in the introduction — that a world is only worth the trouble if it *touches
the page* — and here, on every bridge at once, it is kept.

#recap((
  [The world is a *source*, not a silo: it flows into the same Places, Timeline,
   and page you already write with.],
  [Settlements cross into the *Places book* by `realworld propose` and your
   acceptance — the world proposes, you decide.],
  [The astronomy's calendar adopts into `timeline.calendar` via `realworld calendar`, and history's epochs and events adopt as `inkhaven event add` lines
   from `realworld history`.],
  [`realworld gazetteer --output <path>` writes the whole world as one Markdown
   reference — a series bible or manuscript appendix that agrees with every other
   bridge.],
  [Every bridge keeps the same discipline — the world offers, the author chooses —
   and together they close the seam between the world and the book.],
))
