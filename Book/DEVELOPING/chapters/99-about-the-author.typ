#import "../design.typ": *

#pagebreak(weak: true, to: "odd")

#hide(heading(
  level: 1, numbering: none, outlined: true, bookmarked: true,
  "About the Author",
))

#v(2cm)
#align(left)[
  #text(font: sans_family, size: 9pt, tracking: 2pt, fill: ink_gray, upper("Afterword"))
  #v(4mm)
  #text(font: body_family, size: 36pt, weight: "regular", fill: ink_black, "About the author")
]
#v(1cm)
#line(length: 100%, stroke: 0.5pt + ink_rule)
#v(12mm)

#grid(
  columns: (56mm, 1fr),
  gutter: 7mm,
  [
    #image("../images/author-portrait.png", width: 100%)
    #v(2mm)
    #align(center, text(font: body_family, style: "italic", size: 9pt, fill: ink_gray, "Vladimir Ulogov."))
  ],
  [
    #dropcap("V")ladimir Ulogov has spent decades building infrastructure for
    distributed systems — the kind of software that watches other software. Early
    in his career he worked on monitoring and telemetry platforms; later years
    took him into federated observability, telemetry buses, and the architecture
    of systems that have to make sense of millions of data points without losing
    the thread.

    Observability, in the end, is a discipline of *coherence* — of never
    reporting a state the system cannot account for, of insisting that every
    signal follow from something real. It is not an accident that a tool he built
    for writers carries the same instinct into every kind of book it helps make.
  ],
)

#v(4mm)

What makes him slightly unusual in his corner of the industry is a tendency to
write his own tools — not in the sense of small utilities, but in the sense of
programming languages. The Bund language (its compiler, its VM, its document
store, its parser) lives in a long series of Rust crates on crates.io.
`rust_dynamic`, `rust_multistack`, `rust_multistackvm`, `bundcore`,
`zbus_universal_data_gateway` — each is a building block that exists because the
off-the-shelf options didn't fit the shape of the work.

#section("Why one tool for so many kinds of book")

Inkhaven is Vladimir's personal reflection on how a literary tool can help the
people who write books — _all_ of them, not one favoured kind. A novelist, a
technical writer, a research scientist, and a philosopher share more than they
usually admit: each is trying to hold a large structure in their head without it
quietly contradicting itself, and each is doing it in scattered tools that don't
talk to one another. The map is in one program, the citations in another, the
outline in a third, and the manuscript nowhere near any of them.

Inkhaven was built to close that gap for every one of them at once — to make the
structure, the grounding, the reading, and the press a single room. This book
exists because that generality has a cost: with so many tools present, a writer
new to Inkhaven can't always see which ones are _theirs_. The tracks are the
answer — a way of saying, plainly, here is the handful of tools your kind of book
will actually use, and here is how they fit together.

#section("A work of love")

Inkhaven is open source. The licence is permissive — you can read it, fork it,
study it, modify it, and pass it on. Strictly speaking, the licence also lets you
sell it. The author would, gently but firmly, disagree with you doing that.
Inkhaven was not designed as a #emph[for sale] project. It is a work of love made
for the authors who can least afford to pay for software, and turning it into a
commercial product would betray the reason it exists. So please — don't.

It carries no analytics, no telemetry, no upsell. The binary will never phone
home; the project will never have an "Enterprise" tier you have to escape from.

This was a deliberate choice. There are excellent commercial tools for writing.
They cost money — which is fine for many writers, and a barrier for many more.
Inkhaven exists for the second group: for the novelist building a trilogy on a
battered laptop, for the game-master who shouldn't have to pick between rent and
software, for the graduate student writing a thesis, for the engineer drafting in
the same terminal where they already write code — for anyone who would benefit
from a tool that respects their work without asking for a credit-card number.

#section("A note on cooperation")

Vladimir believes firmly in the human capacity for mutual help — that we make
better work, and live better lives, when we share what we know and what we build.
Open source is one of the most concrete expressions of cooperation our era has
produced: code read, improved, and passed forward without payment, without
permission, by people who will never meet.

If Inkhaven helps you finish your book — whatever kind of book it is — that is
enough. If it gives you a chord pattern you adapt into your own tool, that's a
gift back to the larger project of making software writers can love. As a
society, we achieve the greatest things when we help each other rather than
compete with each other.

#section("Where to find more")

#chord_table((
  chord_row("GitHub", "@vulogov — the source for Inkhaven, Bund, and the dozen-plus Rust crates that carry the infrastructure. Issues and PRs welcome."),
  chord_row("LinkedIn", "/in/vladimirulogov — posts on observability, the occasional long-form essay."),
  chord_row("YouTube", "@vulogov — talks and walkthroughs from the conference trail."),
  chord_row("X / Twitter", "@vladimir_ulogov"),
))

#v(8mm)

#text(font: body_family, style: "italic", size: 11pt, fill: ink_gray,
  "Whatever track you are on — if the tool ever gets in the way of the book instead of out of it, open an issue on GitHub. The author reads them."
)

#v(2cm)
#align(center, text(font: sans_family, size: 8pt, fill: ink_faint, tracking: 4pt,
  upper("end of the book · " + book_version)))
