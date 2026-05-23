#import "../design.typ": *

#pagebreak(weak: true, to: "odd")

#hide(heading(
  level: 1,
  numbering: none,
  outlined: true,
  bookmarked: true,
  "About the Author",
))

#v(2cm)
#align(left)[
  #text(font: sans_family, size: 9pt, tracking: 2pt, fill: ink_gray,
    upper("Afterword"))
  #v(4mm)
  #text(font: body_family, size: 36pt, weight: "regular", fill: ink_black,
    "About the author")
]
#v(1cm)
#line(length: 100%, stroke: 0.5pt + ink_rule)
#v(12mm)

// Opening block — two columns. Portrait on the left at a
// modest 56mm width (≈ 1/3 of the text width), opening
// paragraphs flow alongside on the right. Once the grid
// closes, the remainder of the chapter returns to full
// page width.
#grid(
  columns: (56mm, 1fr),
  gutter: 7mm,
  // ── Left: portrait + caption ─────────────────────────
  [
    #image("../images/author-portrait.png", width: 100%)
    #v(2mm)
    #align(center,
      text(
        font: body_family,
        style: "italic",
        size: 9pt,
        fill: ink_gray,
        "Vladimir Ulogov.",
      )
    )
  ],
  // ── Right: opening paragraphs ───────────────────────
  [
    #dropcap("V")ladimir Ulogov has spent
    decades building infrastructure for distributed
    systems — the kind of software that watches other
    software. Early in his career he worked on monitoring
    and telemetry platforms; later years took him into
    federated observability, telemetry buses, and the
    architecture of systems that have to make sense of
    millions of data points without losing the thread.

    He is also a familiar face on the conference circuit —
    talks on observability design, on the discipline of
    knowing what a system is doing without drowning in
    metrics, on building software that can be read as well
    as run. The same questions keep coming back: what
    deserves to be measured, what deserves to be ignored,
    and how does the tool stay out of its own way.
  ],
)

#v(4mm)

What makes him slightly unusual in his corner of the
industry is a tendency to write his own tools — not in the
sense of small utilities, but in the sense of programming
languages. The Bund language (its compiler, its VM, its
document store, its parser) lives in a long series of Rust
crates on crates.io. `rust_dynamic`, `rust_multistack`,
`rust_multistackvm`, `bundcore`,
`zbus_universal_data_gateway` — each is a building block
that exists because the off-the-shelf options didn't fit
the shape of the work.

#section("Why Inkhaven exists")

Inkhaven is Vladimir's personal reflection on how a
literary tool can help the people who write books. It grew
out of two things, neither of them grand.

The first was his own experiments in literature — fiction
drafts, essays, the long slow project of figuring out how
his prose wants to behave on the page. A working editor is
a piece of furniture you sit in for hours, and the editor
he wanted didn't exist in the shape he wanted it.

The second was dissatisfaction. Existing tools — the
commercial ones in particular — kept getting the small
things wrong. The tree pane didn't model the way
manuscripts actually nest. The search was either too
literal or too loose. Snapshots were an afterthought.
Tags were a checkbox. AI was either everywhere and intrusive
or entirely absent. None of these are world-shaking
problems on their own, but added together they make the
hours stack up against the writing instead of with it.

Inkhaven was built to make those hours stack up in
the other direction. The tree is structural. The search is
semantic and exact in parallel. Snapshots get annotations.
Tags are a project-wide metadata layer. AI sits behind a
chord and a scope flag; it never gets in the way unless
you ask. Every feature in this book is one author's answer
to one small, persistent thing he wanted out of the work.

#section("A work of love")

Inkhaven is open source. The licence is permissive — you
can read it, fork it, study it, modify it, and pass it on.
Strictly speaking, the licence also lets you sell it. The
author would, gently but firmly, disagree with you doing
that. Inkhaven was not designed as a #emph[for sale]
project. It is a work of love made for the authors who can
least afford to pay for software, and turning it into a
commercial product would betray the reason it exists. So
please — don't.

It carries no analytics, no telemetry, no upsell. The
binary will never phone home; the project will never have
an "Enterprise" tier you have to escape from.

This was a deliberate choice. There are excellent
commercial editors for writing books. They cost money —
which is fine for many writers, and a barrier for many
more. Inkhaven exists for the second group: for the
graduate student writing a dissertation on a battered
laptop, for the novelist who shouldn't have to pick
between rent and software, for the engineer drafting in
the same terminal where they already write code, for
anyone who would benefit from a tool that respects their
work without asking for a credit-card number.

It is not built to compete with those other editors. It is
not a Scrivener-killer, a Vellum-killer, a Word-killer.
It's a quieter project that says: here is another way; if
it fits your hands, use it.

The author calls inkhaven a #emph[work of love] —
which is a phrase that sometimes embarrasses people in
software circles, but which means precisely what it says.
The hours spent on it weren't pulled from a balance sheet.
The features weren't road-mapped. The choices that shaped
it were the choices of someone who hoped, in writing each
feature, that the next writer who opened it would feel
slightly less alone in front of an empty paragraph.

#section("A note on cooperation")

Vladimir believes firmly in the human capacity for mutual
help — that we make better work, and live better lives,
when we share what we know and what we build. Software
written in that spirit is a small contribution to that
larger pattern. Open source is one of the most concrete
expressions of cooperation our era has produced: code
read, improved, and passed forward without payment,
without permission, by people who will never meet.

If inkhaven helps you finish your book — that is enough.
If it gives you a chord pattern you adapt into your own
tool — that's a gift back to the larger project of making
software writers can love. As a society, we achieve the
greatest things when we help each other rather than
compete with each other.

The book in your hands is part of that hope. Read it,
disagree with it, fork it, send a pull request, file an
issue — whatever shape your contribution takes, the
project is large enough to hold it.

#section("Where to find more")

#chord_table((
  chord_row("GitHub", "@vulogov — the source for inkhaven, Bund, and the dozen-plus Rust crates that carry the infrastructure. Issues and PRs welcome."),
  chord_row("LinkedIn", "/in/vladimirulogov — posts on observability, the occasional long-form essay."),
  chord_row("YouTube", "@vulogov — talks and walkthroughs from the conference trail."),
  chord_row("X / Twitter", "@vladimir_ulogov"),
))

#v(8mm)

#text(font: body_family, style: "italic", size: 11pt, fill: ink_gray,
  "If you build something on top of Inkhaven, or if a chord trips you up and you can't find the answer in this book — open an issue on GitHub. The author reads them."
)

#v(2cm)
#align(center,
  text(font: sans_family, size: 8pt, fill: ink_faint, tracking: 4pt,
    upper("end of the book of inkhaven · " + book_version)
  )
)
