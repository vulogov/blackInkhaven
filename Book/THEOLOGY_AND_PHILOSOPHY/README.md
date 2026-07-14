# Theology and Philosophy with Inkhaven

*Researching, writing, and composing a work of argument — a worked example, and
the essay it produced.*

This directory holds two books that belong together. The first is a **process
manual**; the second is the **essay that process produced**. Read them side by
side and you can trace every claim in the finished essay back to the stage of the
research loop that made it.

They demonstrate Inkhaven's theology-and-philosophy toolchain on one real
question — *does Kant's transcendental idealism rhyme with the Latter-day Saint
doctrine of eternal progression?* — and, in doing so, exercise the features built
for this kind of work: the public-domain **scripture adapters** (`/bible`,
`/quran`, `/bookofmormon`), the **SCHOLAR** relation engine (`/relate`,
`/contradict`, `/converge`, `/socrates`, `/report`), the manuscript **confront**
chord (`Ctrl+V ?`), and **primary-source loci** with their **Index Locorum**.

## The two books

### `MANUAL/` — *Theology and Philosophy with Inkhaven*

A process book that takes one question from a blank project to a finished essay,
showing every stage on that single example:

1. **Framing** — `genre: "philosophy"`, and sharpening the thesis with the Inner
   Socrates Dialectician (`/socrates`).
2. **Gathering** — the primary sources from the public domain: Kant via
   `/gutenberg`, the Bible and Book of Mormon via the verse-structured scripture
   adapters, each auto-cited under a stable key.
3. **The corpus** — turning sources into provenance-tagged facts and declared
   axioms.
4. **SCHOLAR** — interrogating the corpus for contradiction, convergence, and
   graded relation, and keeping a persistent, staleness-aware report.
5. **Reading the draft** — confronting each paragraph against the corpus
   (`Ctrl+V ?`) and weighing it with the **Inner Theologian** (`Ctrl+B J → T`),
   the reader this track exists for.
6. **Loci** — citing passages with `@key[locus]` and generating the Index
   Locorum.
7. **Revising the prose** — the **Inner Editor** (`Ctrl+V O`), the **grammar
   check** (`F7`), and **snapshots** (`F5` / `F6`) beneath a bold rewrite.
8. **Producing** — the front matter, the export, the bibliography.

A conclusion and an *About the Author* close the book. Modelled on *Developing a
Story with Inkhaven* for style and voice.

Compile: `typst compile MANUAL/MANUAL.typ`

### `ARTICLE/` — *Kant's Transcendental Idealism and Eternal Progression*

The scholarly essay the manual's process produced — a genuine comparative study
in philosophical theology. Every citation is a locus (`@kant-cpracr[Ak. 5:122]`,
`@bible[Matthew 5:48]`, `@book-of-mormon[Moroni 10:32]`) resolving against
`sources.bib`; the footnotes are Chicago notes; and the reference list and the
**Index Locorum** at the back are the apparatus Inkhaven assembles from the loci
the prose actually cites.

Compile: `typst compile ARTICLE/ARTICLE.typ`

## Author

Vladimir Ulogov · 2026 · examples assume Inkhaven 1.6.18 or newer.
