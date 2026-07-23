#import "../design.typ": *

#v(1cm)
#align(center)[
  #text(font: body_family, size: 20pt, weight: "bold", fill: ink_black, "Before we begin")
]
#v(6mm)

There is a thing this book will never do, and it is worth saying on the first page so
that no one waits for it: *Inkhaven does not write poems.* No command produces a line
of verse. No key summons a stanza. If you came hoping to type a topic and receive a
sonnet, close the book gently and go elsewhere; you will not find that here, by design.

What Inkhaven does — the only thing every poetry feature does — is *observe, measure,
and report.* It counts your syllables. It scans your line and names the metre it hears.
It tells you whether two of your endings truly rhyme, and in which tradition, and how.
It checks a villanelle for its refrains and a sonnet for its turn. It weighs a
translation against its original on form and sound. And then it stops, and hands the
verdict back to you, and says nothing about what you should do next. The poem is yours.
The judgement is yours. Inkhaven is the ruler, the tuning fork, and the honest second
reader — never the author.

#section("Two readers, one workbench")

The title of this book drops a word on purpose. It is not *Writing Poetry with
Inkhaven*, because the workbench serves two people who are often the same person on
different days.

The first is the *poet* — someone making verse, who wants a second reader that never
flatters. You declare that this is a sonnet in iambic pentameter; Inkhaven holds you to
it, line by line, and marks where the foot slips or the rhyme goes slack — not to
correct you (a dropped foot is often the best thing in a poem) but so that when you
break a rule you are *choosing* to, with your eyes open.

The second is the *critic* — the scholar, the student, the translator, the teacher —
someone writing *about* verse that already exists. You paste in a stanza of Pushkin or
Shakespeare and ask the workbench to scan it, classify its rhymes, name its form, or
measure how faithfully a translation carried the sound across. Here Inkhaven is an
instrument of close reading: it makes the mechanics of a poem explicit and countable, so
your argument about the poem rests on evidence rather than impression.

The same commands serve both. `poetry metre` does not care whether the line it scans is
yours or Milton's. That neutrality is the whole point — and it is why this book teaches
the measuring first and leaves the making, always, to you.

#callout(label: "A note on the examples")[
  The worked examples lean on poems in the public domain — Shakespeare's sonnets,
  Pushkin's tetrameter, a little Rilke and Verlaine — because they are famous, fixed,
  and free to quote. Every feature is also shown across Inkhaven's five prose languages
  (English, Russian, French, German, Spanish); where a technique behaves differently in
  Russian than in English, we stop and look, because those seams are where the prosody
  actually lives.
]

#section("What you need")

You do not need to be a metrist, and every term is defined where it first appears — a
#emph[foot], a #emph[volta], a #emph[feminine ending] are all explained on the page that
first needs them. You do not need to read Russian; every non-English example is
translated. What you do need is a working copy of Inkhaven (the examples assume 1.8.17
or newer) and a willingness to let a poem be measured — to hear a machine count what you
felt, and to keep your own counsel about what the count means.

A word on honesty, carried over from this book's companions. Measuring verse from plain
text has hard edges — English rhyme and metre, read from spelling alone with no
pronouncing dictionary, are rougher than Russian's, where the orthography wears its
sounds on its sleeve. This book does not hide those edges. Where the tool guesses, it
says so, and so will we.

#v(4mm)
#line(length: 100%, stroke: 0.5pt + ink_rule)
