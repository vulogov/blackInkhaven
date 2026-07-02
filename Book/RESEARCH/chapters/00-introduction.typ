#import "../design.typ": *

#v(1cm)
#text(font: body_family, size: 22pt, weight: "bold")[Before You Begin]
#v(6mm)
#line(length: 100%, stroke: 0.5pt + ink_rule)
#v(8mm)

This is a book about getting the facts right. Not the invented facts — the ones
you make up freely, which are yours to command — but the *borrowed* ones: the
year a bridge was built, the population of a city, the distance a rider could
cover in a day, the claim in your argument that a reader could look up and check.
Every book leans on facts it did not invent, and a book that gets them wrong
loses the reader's trust in the ones it did.

You will do this inside *Inkhaven* — a writing tool with a built-in *Research
Assistant*: a quiet workspace where you ask questions, gather answers from
trustworthy places, check them against each other, and keep only what survives.
By the end of this book you will be able to take a manuscript from "I think that's
about right" to a knowledge base where every fact you kept remembers *where it
came from* — and to compose from that knowledge base directly into your writing.

#section("Who this book is for")

This guide assumes *no prior knowledge* — of research, or of Inkhaven.

You do not need to be a researcher. You do not need to know what a citation
manager is, or how a database works, or the difference between a primary and a
secondary source. Every idea is introduced the first time it is needed, in a
marked box like this one:

#term("Fact")[
  In this book, a *fact* is a specific, checkable claim you want your writing to
  rest on — a date, a number, a name, a cause. It is the opposite of a thing you
  invented. The whole craft here is telling the two apart, and grounding the
  first kind.
]

You do not need to be a novelist, either. The Research Assistant serves two
kinds of author equally, and this book keeps both in view the whole way through.
Where a task looks different depending on what you write, you will see it split
into two tracks:

#two_track(
  [You are writing a novel set during the building of a Roman aqueduct. You need
   the engineering to feel *true* — real distances, plausible dates — without
   stopping the story to fact-hunt.],
  [You are writing a history of Roman water supply. You need every claim
   *sourced* — a real citation behind each figure — so a reviewer can follow your
   evidence.],
)

Same tool, same workflow, two destinations: the novelist wants the world to feel
solid; the non-fiction author wants the argument to be defensible. Both are the
same act — grounding a claim — and both are what the Research Assistant is for.

#section("What you will be able to do")

By the last page you will be able to:

#list(
  [ask a question and get an answer *grounded on facts you already trust*, not a
   confident guess;],
  [pull facts from authoritative places — a structured knowledge base, real
   places, scholarly papers, public-domain books, the open web — each one
   remembering its source;],
  [cross-check a shaky claim against several independent sources before you commit
   it, and let the tool argue *against* a fact to see if it survives;],
  [keep your knowledge base honest over time — re-grounding old guesses, flagging
   facts that may have gone stale;],
  [and compose *out* of the corpus you built: a cited synthesis, an outline, a
   real bibliography — straight into the book you are writing.],
)

#section("What you will need")

Inkhaven, installed and open on a project — a folder with a manuscript in it.
That is all. Nothing in this book requires an account, a subscription, or an
internet connection you have to pay for. A few features reach out to free public
services (a maps database, a scholarly index, a library of public-domain books);
each one is optional, degrades gracefully when you are offline, and is clearly
marked where it appears.

#callout(label: "On the AI")[
  The Research Assistant uses a language model to help you *find* and *phrase*
  answers — but it never decides what is true for you, and it never edits your
  prose. Every fact crosses a confirmation step before it is kept, and every
  fact records where it came from. The model is a research partner with a short
  leash, not an oracle. If you have no model configured, most of the workflow
  still works; the parts that need one say so.
]

#section("The one idea to carry with you")

If you remember nothing else from this introduction, remember this picture. It
is the spine of the whole book:

#trust_ladder()

Not all facts are equally trustworthy, and the Research Assistant never pretends
otherwise. A number you *computed* is firmer than one a structured database
gives you; that is firmer than a scholarly paper; that is firmer than a web page;
and all of them are firmer than a model's unaided guess. Every fact you keep is
stamped with its rung on this ladder — its *provenance* — so that later, you (or
a reader, or a reviewer) can see at a glance how much weight it can bear.

Everything else in this book is a way of climbing that ladder: getting a claim
from a guess at the bottom to something cited and checked near the top.

#section("How to read this book")

Read it front to back the first time. Part I builds the foundation — why
grounding matters, the workspace you will live in, and your very first grounded
fact — and everything after it assumes those three chapters. Later parts are more
independent: once you have the basics, you can jump to the source you care about
(real places, scholarly papers, the web) or the technique you need (cross-checking,
composing a bibliography).

Commands you type look like `/fact` or `/geonames`. Keys you press look like
`Ctrl+S`. When a chord opens something on screen, the book tells you what you
would see and why it matters — but it shows you the *shape* of the work with
diagrams rather than screenshots, because screenshots age badly and the ideas
underneath them do not.

Turn the page, and let us talk about why any of this matters.

#recap((
  [A *fact* here is a borrowed, checkable claim — the opposite of what you
   invented — and grounding it is the whole craft.],
  [The book serves *fiction and non-fiction* authors equally, and assumes no
   prior knowledge of research or Inkhaven.],
  [Facts differ in trustworthiness; the *trust ladder* ranks them, and every kept
   fact records its rung (its *provenance*).],
  [The AI helps you find and phrase, but never decides truth and never edits your
   prose — every fact crosses a confirmation step.],
))
