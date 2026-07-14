// Kant's Transcendental Idealism and Eternal Progression — the essay.
//
// Compile with:
//   typst compile Book/THEOLOGY_AND_PHILOSOPHY/ARTICLE/ARTICLE.typ
//
// This is the *outcome* volume: the essay the companion manual's process
// produced. Every citation is a locus — @key[passage] — resolving against
// sources.bib; the footnotes are Chicago full notes; the reference list and the
// Index Locorum at the end are the apparatus Inkhaven assembles from the loci the
// prose actually cites.

#import "design.typ": *

#show: article.with(
  abstract: [
    Kant's transcendental idealism and the Latter-day Saint doctrine of eternal
    progression both refuse a single, instantaneous, static salvation, making
    perfection a trajectory rather than a state. This essay locates two joints
    where the resemblance genuinely holds — the asymptotic moral self of the
    second _Critique_ and the uncreated self that grounds freedom — and three
    where it breaks: epistemic posture (a postulate held in moral faith versus a
    revealed metaphysic), asymptote versus arrival (an endless approach that never
    closes the gap versus a real deification that does), and the kind of God at
    the end of the process (a non-anthropomorphic moral postulate versus an
    embodied being continuous with humanity). The family resemblance is real; the
    traditions diverge less on whether perfection is a process than on how
    confidently one may say so, and on whether the gap between creature and God
    ever actually closes.
  ],
  keywords: ("Kant", "transcendental idealism", "eternal progression", "postulate", "deification", "moral faith"),
)

#include "sections/01-introduction.typ"
#include "sections/02-where-they-rhyme.typ"
#include "sections/03-where-they-diverge.typ"
#include "sections/04-a-compact-framing.typ"
#include "sections/05-conclusion.typ"

#section("Works Cited")
#bibliography("sources.bib", title: none, style: "chicago-notes")

#include "sections/99-index-locorum.typ"
