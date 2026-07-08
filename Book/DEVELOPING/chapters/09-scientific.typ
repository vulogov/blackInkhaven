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

Set the genre so the readers hold you to evidence and to a hostile standard:

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

#section("Produce")

`export pdf|docx` renders the paper or the book with its reference list assembled
from the Sources you gathered. Because every citation was filed from a real source
with a real identifier, the bibliography is not a hand-typed hope that a reference
looks right — it is the actual, resolvable record of what grounded the work.

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
))
