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

// Opening block — two columns. Portrait on the left at a modest 56mm width
// (≈ 1/3 of the text width); the opening paragraphs flow alongside on the right.
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

    Observability, in the end, is the art of *holding a system too large for one
    head* — of watching it without losing the thread, and never reporting a thing
    it cannot back up. It is not an accident that a tool he built for writers
    carries the same instinct to the desk: a book, past a certain size, is exactly
    such a system, and it too deserves an instrument that holds it for you.
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

#section("Why these intelligences exist")

Inkhaven is Vladimir's personal reflection on how a literary tool can help the
people who write books. The intelligences in this book are one answer to a specific
frustration: that a manuscript, past a certain length, outgrows the mind writing it
— and the tools writers use offer no help holding it. They format the words; they do
not *understand* them.

A novelist loses track of who knows what, whether the continuity holds, whether two
characters have started to sound alike — not from carelessness, but because no one
holds a whole book at once. A non-fiction author loses the thread between a claim and
its source, and never sees the two of their own findings that quietly disagree. These
features were built to close that gap: to let the tool that holds your words also
*know* them — watch the facts, the continuity, the knowledge, the shape, and the
voices — so the writer is free to do the one thing no tool can, which is write.

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
They cost money — which is fine for many writers, and a barrier for many more. Inkhaven exists for the second group: for the graduate student writing
a dissertation on a battered laptop, for the novelist who shouldn't have to pick
between rent and software, for the engineer drafting in the same terminal where
they already write code, for anyone who would benefit from a tool that respects
their work without asking for a credit-card number.

#section("A note on cooperation")

Vladimir believes firmly in the human capacity for mutual help — that we make
better work, and live better lives, when we share what we know and what we build.
Open source is one of the most concrete expressions of cooperation our era has
produced: code read, improved, and passed forward without payment, without
permission, by people who will never meet.

If Inkhaven helps you finish your book — that is enough. If it gives you a chord
pattern you adapt into your own tool — that's a gift back to the larger project of
making software writers can love. As a society, we achieve the greatest things
when we help each other rather than compete with each other.

#section("Where to find more")

#chord_table((
  chord_row("GitHub", "@vulogov — the source for Inkhaven, Bund, and the dozen-plus Rust crates that carry the infrastructure. Issues and PRs welcome."),
  chord_row("LinkedIn", "/in/vladimirulogov — posts on observability, the occasional long-form essay."),
  chord_row("YouTube", "@vulogov — talks and walkthroughs from the conference trail."),
  chord_row("X / Twitter", "@vladimir_ulogov"),
))

#v(8mm)

#text(font: body_family, style: "italic", size: 11pt, fill: ink_gray,
  "If a fact you grounded ever surprises you, or a command trips you up and you can't find the answer in this book — open an issue on GitHub. The author reads them."
)

#v(2cm)
#align(center, text(font: sans_family, size: 8pt, fill: ink_faint, tracking: 4pt,
  upper("end of the book · " + book_version)))
