#import "../design.typ": *

#chapter(number: 8, title: "Producing the essay")

The argument has been framed, grounded, interrogated, confronted, and cited. What
remains is to turn the tree of paragraphs into something a reader can hold — and to
do it so that the finished essay carries, on its own pages, the evidence that it was
built the way this book describes: a reference list of the works, an index of the
passages, and front matter fit for a journal or an anthology.

#section("The front matter of an essay")

An essay is more than its body. Declare its front matter — the abstract, the author,
the keywords — in one config block, and it renders into the title page in the book's
language:

#config("inkhaven.hjson", [```hjson
frontmatter: {
  abstract: "Kant's transcendental idealism and Latter-day Saint eternal
    progression both refuse instantaneous salvation, making perfection a
    trajectory. This essay locates two joints where the resemblance holds and
    three where it breaks, and argues the family resemblance is real but the
    traditions diverge on how confidently one may claim it."
  keywords: ["Kant", "eternal progression", "postulate", "deification"]
  authors: [
    { name: "Vladimir Ulogov", affiliation: "Independent", corresponding: true }
  ]
}
```])

Leave the block empty and nothing renders, so a book that is not an essay is
untouched; fill it, and the labels — _Abstract_, _Keywords_, _Corresponding author_
— follow `language`, so a Russian study reads in Russian without a second setting.

#section("Render")

Then render. `export pdf` produces the essay with its reference list assembled from
the Sources you gathered — every citation filed from a real public-domain text — and,
with the switch from Chapter 6 set, its Index Locorum after that:

```
inkhaven export pdf --output kant-and-eternal-progression.pdf
inkhaven export epub --output kant-and-eternal-progression.epub
inkhaven export docx --output kant-and-eternal-progression.docx
```

The same tree becomes a PDF for a reader, an EPUB for a shelf, or a DOCX for an
editor who redlines in a word processor. Nothing about the argument changes across
them; the press does.

#note[
  Two of the earlier tools reappear at production time. The bibliography assembles
  from the Sources book, so the works you auto-filed while gathering become the
  reference list with no re-typing; and because every locus in the prose is a real
  citation to one of those works, a passage cited on a source you never filed would
  fail the build — the essay cannot ship claiming a source it does not carry.
]

#section("The essay this produced")

This is not a hypothetical pipeline. The companion volume in this directory —
_Kant's Transcendental Idealism and Eternal Progression_ — is the essay this exact
process produced. Its two opening movements are the convergence and the relation
`/relate` surfaced in Chapter 4; its third is the cross-source contradiction
`/contradict` found; its hardest paragraph is the question `/socrates` asked; and its
apparatus is the bibliography and the Index Locorum this chapter renders. Read it
beside this one and you can trace every claim back to the station of the loop that
produced it.

#insight[
  The measure of this track is not that Inkhaven wrote the essay — it did not, and
  must not. The measure is that every claim in the finished essay can be walked back
  to a passage in a public-domain source, that the tensions were found in your study
  rather than in a reviewer's, and that the apparatus at the back is generated from
  what the prose actually cites and not from what you hoped it did. The essay is
  yours; its _checkability_ is what the desk gave you.
]

#recap((
  [Declare the essay's *front matter* (abstract, author, keywords) in one config block — it renders into the title page in the book's `language`, and an empty block renders nothing.],
  [`export pdf | epub | docx` renders the same tree for a reader, a shelf, or an editor — the *bibliography* assembles from the Sources you auto-filed while gathering, with no re-typing.],
  [Because every locus is a real citation, the build *cannot ship* a passage on a source you never filed — production is the last check, not just the last step.],
  [The companion essay in this directory is the literal output of this loop — every movement traces to the SCHOLAR pass that produced it, and the back matter to the bibliography and Index Locorum this chapter renders.],
))
