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
    for writers carries the same instinct into a work of theology, where the whole
    art is to advance nothing the argument cannot answer for.
  ],
)

#v(4mm)

What makes him slightly unusual in his corner of the industry is a tendency to
write his own tools — not in the sense of small utilities, but in the sense of
programming languages. The Bund language (its compiler, its VM, its document
store, its parser) lives in a long series of Rust crates on crates.io.
`rust_dynamic`, `rust_multistack`, `rust_multistackvm`, `bundcore`,
`zbus_universal_data_gateway` — each is a building block that exists because the
off-the-shelf options didn't fit the shape of the work. Inkhaven's research desk,
with its provenance ledger and its refusal to let a claim rest on memory, is that
same instinct pointed at the humanities.

#section("Why a tool for the unmeasurable")

Inkhaven was built to serve every kind of book, and this one — the theological
essay, the philosophical argument — is the hardest case for a tool that began in a
world of metrics and telemetry. There is no dataset here, no fact to check against
the world, no ground truth a monitor could alarm on. And yet the underlying
discipline turned out to be the same: hold a large structure of commitments without
letting it quietly contradict itself, and never assert what you cannot trace.
Observability calls that coherence; philosophy has called it the same thing for
rather longer. Inkhaven's theology-and-philosophy track is the attempt to bring an
engineer's instinct for traceability to a kind of writing that measurement cannot
touch — not to settle its questions, which no tool can, but to make its arguments
_checkable_ in the only sense they admit.

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
Inkhaven exists for the second group: for the graduate student writing a thesis in
divinity on a battered laptop, for the independent scholar working outside any
institution's subscriptions, for the pastor drafting in the same terminal where they
keep their notes — for anyone who would benefit from a tool that respects their work
without asking for a credit-card number.

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
  "Whatever you are arguing — if the tool ever gets in the way of the argument instead of out of it, open an issue on GitHub. The author reads them."
)

#v(2cm)
#align(center, text(font: sans_family, size: 8pt, fill: ink_faint, tracking: 4pt,
  upper("end of the book · " + book_version)))
