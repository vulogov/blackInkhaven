#import "../design.typ": *

#chapter(number: 9, title: "The scientific track")

Scientific and scholarly writing is nonfiction at its most exacting. Every
load-bearing claim must trace to evidence; the argument must survive a reader who
_wants_ to break it; and every citation must resolve to a source that actually says
what you claim it says. Your ground is the literature and the data; your risks are
the unsupported assertion and the citation that doesn't hold. This chapter adds to
the nonfiction loop the two disciplines that define the track: sourcing everything,
and reading adversarially.

#section("Frame — the academic genre")

The scientific template scaffolds the whole track in one step — an IMRaD manuscript
(Introduction, Methods, Results, Discussion) with the academic genre already set and
the paper's front-matter block (below) waiting to be filled:

```
inkhaven init --template scientific my-paper
```

If you are already in a project, set the genre by hand so the readers hold you to
evidence and to a hostile standard:

#config("inkhaven.hjson", [```hjson
genre: "science"
```])

`academic`, `scholarly`, and `research` share this frame; `science` and
`popular_science` lean the same way with a lighter touch for a general audience.
Everything in the nonfiction chapter applies — plan the argument, build a corpus,
write from it — so read that chapter first. What follows is what this track adds.

#section("Gather — the literature, with its papers named")

The Research Assistant (`inkhaven research`) is the whole of this track's gathering,
and here you use its scholarly reach in earnest. It can pull the top work on a
question from the scholarly indexes — a paper by its DOI, a preprint from arXiv —
and, crucially, _auto-file its citation_ to the Sources book as it does. It can
triangulate a claim across independent sources and report whether they agree,
refute a claim by trying to break it, and upgrade a tentative note into a cited
fact once a source is found. The companion volume, _The Research Assistant with
Inkhaven_, is essentially this track's deep manual.

#term("Triangulation")[
  Testing a claim against several independent sources at once and asking whether
  they agree — so the verdict is not "a source says so" but "the sources concur." On
  the scientific track it is the difference between a claim you can defend and one
  you found stated confidently in a single place. A claim that survives triangulation
  _and_ a genuine attempt to refute it has earned its place in the argument.
]

#section("Cite — the Sources book and the bibliography")

Where fiction has a World book, the scientific track has a *Sources* book — and it
is the one you never leave. Every grounded answer files a citation there
automatically, and you manage the whole from the shell for interchange with the
tools you already use:

```
inkhaven sources import zotero-export.bib
inkhaven sources export --format csl-json --out sources.json
inkhaven sources check
```

BibTeX comes in from a reference manager; BibTeX or CSL-JSON goes back out, closing
the round-trip with Zotero and its kin. `sources check` validates every entry and
exits non-zero on a problem, so it fits a continuous-integration step. Inside the
editor, the cite picker (`Ctrl+V @`) drops a citation into your prose where the
cursor sits, and the accumulated Sources render into a formatted reference list.

#note[
  A citation that no longer resolves is worse than no citation — it looks like
  authority and delivers nothing. The Research Assistant's dead-source check
  (`/deadsources`) scans your kept web sources for link-rot and flags the ones that
  have quietly died, so a reference does not fail under a referee's click.
]

#section("Read — adversarially")

Every other track's readers ask in good faith. The scientific track keeps two that
do not. The *verdict* personas — the `prosecutor` and the `defender` — argue your
claims rather than question them: the prosecutor tries to break a claim before a
reviewer does, the defender answers. And the `expert-reviewer` audience persona
reads a finished section looking for exactly the hole a referee will find. Run them
on your argument _before_ you submit it, when the hole is still cheap to close.

The deterministic checks matter here too. `/factcheck` sweeps the Facts book for
per-claim accuracy and for contradictions between claims that are each fine alone;
`/undisputed` checks the axioms you have declared for internal coherence. And for
anything computed — a rate, a distance, a growth over time — the assistant's `/calc`
produces a fact whose provenance is _computed_, un-fabricatable, needing no source
because arithmetic is its own authority.

#insight[
  The scientific track's whole discipline is _make the referee's job easy and then
  do it yourself first_. Source every claim so the trail is there; triangulate and
  refute so the claim is strong; run the adversarial readers so the weak point is
  found in your office, not in review. Nothing here makes the argument for you. It
  makes the argument _checkable_ — which, on this track, is the same as making it
  credible.
]

#section("Produce — the paper, formatted for its venue")

A paper is not only its prose. Declare its _front matter_ — the title block a
journal expects — in one config block, and it renders into the PDF and the LaTeX
alike:

#config("inkhaven.hjson", [```hjson
frontmatter: {
  abstract: "One paragraph — the problem, the method, the finding."
  keywords: ["specificity", "CRISPR", "off-target"]
  authors: [
    { name: "Ada Lovelace", affiliation: "Analytical Engine Co.", orcid: "0000-0002-1825-0097", email: "ada@example.org", corresponding: true }
    { name: "Charles Babbage", affiliation: "Analytical Engine Co." }
  ]
  funding: "Supported by grant No. 42."
}
```])

Authors sharing an affiliation share one superscript; the corresponding author is
marked and their address noted. The labels — _Abstract_, _Keywords_, _Corresponding
author_, _Funding_ — follow the book's language, so a Russian or French paper reads
in its own tongue. Leave the block empty and nothing renders, so a book that is not
a paper is untouched.

Then render. `export pdf` produces the paper with its reference list assembled from
the Sources you gathered — every citation filed from a real source with a real
identifier, so the bibliography is the resolvable record of what grounded the work,
not a hand-typed hope. `export tex` hands you LaTeX for a journal's submission
system, and it targets the journal's document class when you name one:

#config("inkhaven.hjson", [```hjson
tex_export: {
  document_class: "IEEEtran"
  class_options: "conference"
  extra_packages: ["amsmath"]
}
```])

Left unset, the export keeps the sensible `article` class the converter emits. Name
a class — `IEEEtran`, `elsarticle`, or `article` with `twocolumn` in the options —
and Inkhaven rewrites the document class and folds in whatever extra packages the
venue needs.

#note[
  A cross-reference that does not resolve — `@fig:flux` with no figure labelled
  `<fig:flux>` — is a hard error in Typst: the paper will not compile, and the
  reason is easy to lose in a wall of compiler output. When a build fails on one,
  Inkhaven lifts it out as a cross-reference finding in the Output pane, naming the
  label and the line, so a dangling reference is as findable as an uncited claim.
  To stop writing one in the first place, `Ctrl+V &` opens a cross-reference
  picker — every label defined across the manuscript, fuzzy-searchable — and drops
  the matching `@label` where the cursor sits.
]

#section("Submit — double-blind and the availability statements")

A submission is not the desk copy. For a double-blind venue, `export tex --blind`
(or `--blind` on the `pdf`/`typst` export) renders the title block with the
identifying front matter withheld — authors, affiliations, ORCID, funding — while
keeping the title, abstract, keywords, and the availability statements the reviewer
still needs. And those statements are front matter of their own:

#config("inkhaven.hjson", [```hjson
frontmatter: {
  data_availability: "The dataset is archived at the repository DOI on acceptance."
  code_availability: "Analysis code is released under an open licence at the same DOI."
}
```])

They render as their own labelled blocks — in the book's language — and, unlike the
author line, survive `--blind` (you anonymise the links yourself for review).

#section("Reach for a package")

Scientific typesetting often wants a package the community has already written — a
diagram library, a units formatter, a journal template. `Ctrl+V #` opens the Typst
Universe picker: fuzzy-find a package by name, and Inkhaven inserts its
`#import` line where the cursor sits. The catalogue is fetched once and cached under
`.inkhaven/`; `Ctrl+R` inside the picker forces a fresh pull when you want the
latest, and `typst_universe.url` points it at a different index if you keep one.

#section("Hands-on: two procedures")

#subsection("Cite a paper, end to end")

+ In the assistant (`inkhaven research`), pull the work: `/openalex CRISPR off-target effects`. The citation is filed to your Sources book automatically.
+ Bring in a library you already curate: `inkhaven sources import zotero-export.bib`.
+ Drop a citation into your prose where the cursor sits: `Ctrl+V @` opens the cite picker.
+ Validate every entry before you rely on them: `inkhaven sources check` (it exits non-zero on a problem, so it fits a CI step).
+ Send the bibliography back out for your reference manager or your typesetter: `inkhaven sources export --format csl-json --out sources.json` (or `--format bibtex`).

#subsection("Stress a claim before a reviewer does")

+ Test a claim against independent sources: `/triangulate the treatment reduced mortality by a third`. The verdict is whether the sources _concur_, not whether one asserts it.
+ Turn the adversary on your argument: `Ctrl+B J`, then the `prosecutor` persona, which tries to break the claim; the `defender` answers it.
+ Audit the whole Facts book for accuracy and for contradictions between claims: `/factcheck`. Check your declared axioms for internal coherence: `/undisputed`.
+ Catch a reference that has quietly died before a referee's click does: `/deadsources`.

#recap((
  [Scientific writing is *nonfiction held to a hostile standard*: every load-bearing claim sourced, the argument built to survive a reviewer. Set `genre: "science"` (or `academic`) — and read the nonfiction chapter first.],
  [*Gather* from the literature with the Research Assistant's scholarly reach (DOI, arXiv), which auto-files citations; *triangulate* and *refute* so a claim is concurred and stress-tested, not merely stated.],
  [*Cite* through the *Sources* book — `sources import/export` (BibTeX, CSL-JSON) round-trips with Zotero, `sources check` fits CI, the cite picker (`Ctrl+V @`) drops references inline, and `/deadsources` catches link-rot.],
  [*Read adversarially* with the `prosecutor`/`defender` verdict personas and the `expert-reviewer`; verify with `/factcheck`, `/undisputed`, and computed `/calc` facts — find the hole yourself before the referee does.],
  [*Produce* for the venue: `inkhaven init --template scientific` scaffolds IMRaD; a `frontmatter` block renders the title page (authors, affiliations, ORCID, abstract, keywords, data/code availability — in the book's language); `export tex` targets a journal `document_class`; `Ctrl+V #` pulls in a Typst Universe package; `Ctrl+V &` inserts a cross-reference, and a dangling one surfaces as an Output finding.],
  [*Submit* clean: `export tex --blind` withholds the identifying front matter (authors, affiliations, ORCID, funding) for double-blind review while keeping the abstract, keywords, and availability statements.],
))
