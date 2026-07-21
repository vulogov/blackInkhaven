#import "../design.typ": *

#pagebreak(weak: true, to: "odd")

#hide(heading(
  level: 1, numbering: none, outlined: true, bookmarked: true,
  "About the Author",
))

#v(2cm)
#align(left)[
  #text(font: body_family, size: 9pt, tracking: 2pt, fill: ink_gray, upper("Afterword"))
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
    Vladimir Ulogov has spent decades building infrastructure for distributed
    systems — the kind of software that watches other software. Early in his career
    he worked on monitoring and telemetry platforms; later years took him into
    federated observability, telemetry buses, and the architecture of systems that
    have to make sense of millions of data points without losing the thread.

    Observability, in the end, is a discipline of *coherence* — of never reporting a
    state the system cannot account for, of insisting that every signal follow from
    something real. It is not an accident that a tool he built for writers carries
    the same instinct into every kind of book it helps make, and into the linguistic
    analysis this one describes: never a result the model cannot account for.
  ],
)

#v(4mm)

What makes him slightly unusual in his corner of the industry is a tendency to write
his own tools — not small utilities, but programming languages. The Bund language
(its compiler, its VM, its document store, its parser) lives in a long series of Rust
crates on crates.io. `rust_dynamic`, `rust_multistackvm`, `bundcore` — each is a
building block that exists because the off-the-shelf options didn't fit the shape of
the work. Inkhaven's linguistics grew the same way: an engine for inventing languages
that turned out, with no extra machinery, to be an engine for studying them.

#section("A work of love")

Inkhaven is open source, under a permissive licence — you can read it, fork it, study
it, modify it, and pass it on. Strictly speaking the licence also lets you sell it;
the author would, gently but firmly, disagree with your doing that. Inkhaven was not
designed as a #emph[for sale] project. It is a work of love made for the people who
can least afford to pay for software — the novelist on a battered laptop, the
graduate student writing a thesis, the field linguist documenting a language with no
budget at all — and turning it into a commercial product would betray the reason it
exists. It carries no analytics, no telemetry, no upsell; the binary will never phone
home.

#section("A note on cooperation")

Vladimir believes firmly in the human capacity for mutual help — that we make better
work, and live better lives, when we share what we know and what we build. Open
source is one of the most concrete expressions of cooperation our era has produced:
code read, improved, and passed forward without payment, without permission, by
people who will never meet. If this book helps you understand a language a little
better — your own, or one you are documenting for the first time — that is enough.

#section("Where to find more")

/ *GitHub*: `@vulogov` — the source for Inkhaven, Bund, and the dozen-plus Rust crates that carry the infrastructure. Issues and pull requests welcome.
/ *LinkedIn*: `/in/vladimirulogov` — posts on observability, the occasional long-form essay.
/ *YouTube*: `@vulogov` — talks and walkthroughs from the conference trail.

#v(8mm)

#text(font: body_family, style: "italic", size: 11pt, fill: ink_gray,
  "If the tool ever gets in the way of the language instead of out of it, open an issue on GitHub. The author reads them."
)

#v(2cm)
#align(center, text(font: body_family, size: 8pt, fill: ink_faint, tracking: 4pt,
  upper("end of the book")))
