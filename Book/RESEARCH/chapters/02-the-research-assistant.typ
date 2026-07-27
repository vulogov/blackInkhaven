#import "../design.typ": *

#chapter(number: 2, title: "The Research Assistant")

The Research Assistant is a separate screen inside Inkhaven — a room you step
into when you want to research, and step out of to write. It is deliberately
quiet: two panes, a place to type, and a status line. Everything you do here
either *gathers* a fact, *checks* one, or *composes* from the ones you have kept.

This chapter is a tour of the room and the way you move through it. There is no
command to memorise yet; the point is to understand the shape of the work so the
commands in the next chapter feel like the natural thing to type.

#section("Two panes, one conversation")

Picture the screen split in two — the Facts tree on the left, the conversation on
the right, a status line beneath:

#screen(caption: "inkhaven research — the two-pane Assistant")[```
┌─ Facts ─────────────┬─ thread: 1920s Vienna ────┐
│ ▾ Geography         │ > Vienna's population     │
│   ✓ Vienna on the   │   in 1925?                │
│     Danube…         │                           │
│ ▾ Society           │ About 1.87 million —      │
│   ✓ The Ringstrasse…│ just past its peak.       │
│   ? Coffeehouse…    │ ── src: Wikidata Q1741    │
│   ※ (novel: café on │                           │
│      Kärntnerstraße)│ /fact → keep as a Fact?   │
├─────────────────────┴───────────────────────────┤
│ default · 4 facts · $0.02 · ✓ ? ※ · ? help      │
└─────────────────────────────────────────────────┘
```]

On the left is your *Facts tree* — the growing outline of everything you have
kept for this project: your Facts, organised the way a book is, in chapters and
sections you can fold and unfold. When you are new to a project this pane is
nearly empty. It fills as you work, and by the end of a book it is the map of
everything you learned.

On the right is the *conversation* — where you ask questions and read answers.
You type at the bottom; the exchange scrolls above. This is where the language
model lives, where search results arrive, where a fact-check verdict appears. It
is a chat, but a chat with a purpose: everything said here is a candidate for the
tree on the left.

The whole workflow is the traffic between these two panes: a question on the
right becomes, after checking and your confirmation, a fact on the left.

#term("Thread")[
  A *thread* is one research conversation, saved. You might keep one thread per
  topic — "the aqueduct," "1920s Vienna," "cell biology for chapter 9" — so each
  line of inquiry has its own history you can return to. Threads keep unrelated
  research from tangling together, and they persist between sessions.
]

#section("Two ways to speak")

You say things to the Assistant in one of two registers, and learning the
difference is most of learning the tool.

The first is *plain language*. You type a question the way you would ask a
knowledgeable friend — "how far could a Roman legion march in a day?" — and you
get an answer, grounded on whatever you have already gathered. This is for
*thinking*: exploring, orienting, working out what you actually need to know.

The second is a *command*. A command starts with a slash — `/fact`, `/geonames`,
`/triangulate` — and it does something precise and repeatable: keep this claim as
a fact, look this place up in a gazetteer, cross-check this against three sources.
Commands are for *acting*: turning a loose exchange into something durable.

#callout(label: "You don't have to memorise the commands")[
  Type a single `/` and a *command palette* opens — a searchable list of every
  command with a one-line description. You can browse, filter by typing, and pick
  one without ever having learned its name. Throughout this book, when a command
  is introduced, that palette is where you would rediscover it later. Learn the
  workflow; let the palette remember the spelling.
]

#section("The arc of a project")

Zoom out from a single question to a whole book, and the work has a natural shape.
It is a loop, and the Research Assistant is built to carry you around it:

#research_arc()

You *acquire* — ask, search, import — pulling raw material in. You *cross-check*
— hold a claim up against independent sources, even argue against it — so that
only what survives is kept. You *maintain* — because a knowledge base decays;
guesses can be re-grounded on real sources later, and old facts can be flagged
for a second look. And you *compose* — turning the corpus back into output: a
synthesis, an outline, a bibliography that feeds the very book you are writing.

Most of this book is a slow walk around that loop. Part I gets you onto it — one
question, one checked fact. Later parts deepen each quarter of the circle.

#two_track(
  [A novelist tends to live in *acquire* and *compose*: gather enough real
   texture to make the world convincing, then let it quietly inform the prose.
   Cross-checking matters most for the load-bearing facts a sharp reader would
   catch.],
  [A non-fiction author lives in *cross-check* and *maintain*: every claim must
   survive scrutiny and stay current, and the bibliography that *compose*
   produces is often part of the deliverable itself.],
)

#section("Nothing happens to your manuscript by accident")

One promise, made early because it shapes how freely you can experiment: the
Research Assistant never touches your manuscript on its own, and never edits your
prose. It writes only to your research books — Facts, Notes, Sources — and only
the deliberate acts you take (keeping a fact, recording a source) change anything
on disk. You can ask anything, explore any tangent, take any command back out for
a test drive, and know that your book on the other screen is exactly as you left
it.

That safety is what lets the next chapter be hands-on. You are about to gather
your first fact — and there is nothing you can do in the process that you cannot
undo.

#recap((
  [The Research Assistant is a separate, quiet screen: a *Facts tree* on the left,
   a *conversation* on the right, and the traffic between them is the workflow.],
  [*Threads* keep separate lines of inquiry apart and persist between sessions.],
  [You speak in *plain language* to think and explore, or in *slash commands* to
   act; a `/` palette means you never have to memorise a command.],
  [A project moves around a loop — *acquire → cross-check → maintain → compose* —
   and the tool is built to carry you around it.],
  [The Assistant writes only to your research books and never edits your prose, so
   you can explore without risk.],
))
