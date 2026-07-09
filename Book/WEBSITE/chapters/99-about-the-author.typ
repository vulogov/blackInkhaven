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

#dropcap("V")ladimir Ulogov has spent decades building infrastructure for distributed
systems — the kind of software that watches other software. His career runs through
monitoring and telemetry platforms and into federated observability: systems that must
make sense of millions of data points without losing the thread. Observability, in the
end, is a discipline of _coherence_ — of never reporting a state the system cannot
account for. It is not an accident that a tool he built for writers carries the same
instinct.

What makes him slightly unusual is a tendency to write his own tools — not small
utilities, but programming languages. The Bund language — its compiler, its virtual
machine, its document store — lives in a long series of Rust crates on crates.io
(`rust_dynamic`, `rust_multistack`, `bundcore`, and more), each a building block that
exists because the off-the-shelf options did not fit the shape of the work. Inkhaven
is cut from the same cloth.

#section("Why a book just about publishing")

Inkhaven can already turn a manuscript into a PDF, an e-book, a printed volume. Adding
the web was not about chasing another format for its own sake. It was about reach: a
website is the one form of a book that anyone, on any device, can open without owning,
buying, or installing a thing. For an author who cannot afford a publisher — and
Inkhaven is built first for those authors — a self-contained website is the shortest
honest path between finishing a book and being read.

That is also why this short guide exists. Publishing to the web is surrounded by
jargon and by tools that assume you are an engineer. None of that should stand between
a writer and their readers. So the feature was built to be simple, and this book was
written to explain it in plain words — so that _one command_ and _one folder_ is
genuinely all it takes, and everything past that is yours to shape if you wish.

#section("A work of love")

Inkhaven is open source, under a permissive licence: you can read it, fork it, study
it, modify it, and pass it on. Strictly, the licence also lets you sell it. The author
would, gently but firmly, ask that you do not. Inkhaven was not made as a product. It
is a work of love for the authors who can least afford to pay for software — the
novelist on a battered laptop, the game-master choosing between rent and tools, the
graduate student, the engineer drafting in the same terminal where they write code.

It carries no analytics, no telemetry, no upsell. The binary will never phone home;
the websites it builds never will either — that is the whole point of _self-contained_.
Nothing you make with it reports back to anyone, including its author.

#section("A note on cooperation")

Vladimir believes firmly in the human capacity for mutual help — that we make better
work, and live better lives, when we share what we know and what we build. Open source
is one of the most concrete expressions of cooperation our era has produced: code
read, improved, and passed forward without payment, without permission, by people who
will never meet. If this book helps you put your own book in front of readers, that is
enough.

#section("Where to find more")

#chord_table((
  chord_row("GitHub", "@vulogov — the source for Inkhaven, Bund, and the dozen-plus Rust crates that carry the infrastructure. Issues and PRs welcome."),
  chord_row("LinkedIn", "/in/vladimirulogov — posts on observability, the occasional long-form essay."),
  chord_row("YouTube", "@vulogov — talks and walkthroughs from the conference trail."),
  chord_row("X / Twitter", "@vladimir_ulogov"),
))

#v(8mm)

#text(font: body_family, style: "italic", size: 11pt, fill: ink_gray,
  "If the export ever gets in the way of publishing instead of out of it, open an issue on GitHub. The author reads them."
)

#v(2cm)
#align(center, text(font: sans_family, size: 8pt, fill: ink_faint, tracking: 4pt,
  upper("end of the book · " + book_version)))
