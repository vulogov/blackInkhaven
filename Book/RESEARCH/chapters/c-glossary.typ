#import "../design.typ": *

#appendix(letter: "C", title: "Glossary")

Every term this book defined, gathered in one place. Each was introduced in a
marked box the first time it was needed; here they are, alphabetical, for looking
up.

// The bold term keeps a real gap above its definition (Typst 0.14 honours the
// `below`/`above` literally — a tiny value collapses the term onto its body) and
// `sticky` so a term is never orphaned from its definition at a page break.
#let gloss(name, body) = {
  block(above: 4.5mm, below: 1.6mm, sticky: true, breakable: false,
    text(font: body_family, weight: "bold", size: 10.5pt, fill: ink_term, name))
  block(above: 0mm, text(font: body_family, size: 10pt, body))
}

#gloss("Audit")[A pass over your whole Facts book at once (`/factcheck`), rather than a check of a single claim. It catches old facts never re-examined, and contradictions between facts that are each fine alone.]

#gloss("Batch research")[Answering a whole list of questions in one unattended run (`--batch`), writing a report of what was found and how confident it was. It trades the interactive gate for a confidence threshold you set.]

#gloss("Bibliography")[The list of sources a work draws on — its references. Yours is never hand-maintained: it accumulates in the Sources book as a by-product of grounding, and `/bibliography` collects it.]

#gloss("BibTeX")[A plain-text format for bibliographic entries — author, title, year, identifier — read by nearly every citation tool. What `/bibliography` produces.]

#gloss("Confirmation gate")[The mandatory pause between the Assistant proposing a fact and the fact entering your book. You edit, accept, or discard; nothing reaches your Facts without it. The Assistant proposes; you dispose.]

#gloss("Corpus")[The whole body of research material gathered for a project — Facts, Notes, Sources, and imported documents. It grows as you work, and you can search and compose from it.]

#gloss("CSL-JSON")[The JSON citation format of the Citation Style Language ecosystem — what Zotero and most modern reference managers read and write. Where BibTeX serves LaTeX, CSL-JSON is the interchange format between citation tools; `inkhaven sources export --format csl-json` produces it.]

#gloss("Deterministic")[Following from inputs by a fixed rule, so anyone who runs the same computation gets the same answer. Deterministic facts (computed, simulated) are the firmest rung — you trust arithmetic, not a source.]

#gloss("DOI")[A permanent code pointing to a specific scholarly work — the scholarly equivalent of a Q-id. A claim cited by DOI can be resolved to the exact paper by any reader.]

#gloss("Fact")[A specific, checkable claim your writing rests on — a date, number, name, cause — the opposite of a thing you invented. Grounding the checkable kind is the craft of this book.]

#gloss("Fact-check gate")[For a fact drawn from the web or the model, a single-claim accuracy check the gate runs before the fact commits. ACCURATE passes; DUBIOUS / INACCURATE shows its reasoning and asks you to confirm again. It informs; it never blocks.]

#gloss("Gazetteer")[A geographical dictionary: a database of place names with their locations, regions, feature types, and populations. `/geonames` reads one.]

#gloss("Grounding")[Connecting a claim to something outside your own head that supports it — a source, a computation, a cross-checked agreement. A grounded claim can answer "how do you know?"]

#gloss("Grounding on your corpus")[Before answering a question, the Assistant retrieves the most relevant facts you have already kept and hands them to the model as context — so answers stay consistent with what you established, and improve as your corpus grows.]

#gloss("Headless")[Working without the interactive interface — driven from the command line, unattended, results written to a file. `inkhaven research …` with flags.]

#gloss("Ingesting a source")[Bringing a source's full text into your corpus, split into searchable passages, so the Assistant can retrieve the relevant ones and quote the actual text rather than a summary.]

#gloss("Internal coherence")[Consistency within an invented frame, judged without reference to the real world. `/undisputed` checks authorial facts for self-contradiction; it never rewrites them.]

#gloss("Preprint")[A scholarly paper shared publicly before formal peer review — fast and current, but less settled than a published article. arXiv is the largest preprint server.]

#gloss("Provenance")[The recorded origin of a fact — its rung on the trust ladder and the specific source. It travels with the fact silently and answers "how do you know?" any time.]

#gloss("Q-id")[A stable Wikidata code (like `Q220` for Rome) that never changes, so a fact cited by Q-id can be re-checked years later.]

#gloss("Refutation")[A check that actively tries to disprove a claim — the mirror of triangulation. A claim that survives a genuine attempt to break it has earned more trust than one merely unchallenged.]

#gloss("Research-to-writing bridge")[The plank across the gap between having done the reading and facing the blank page. An `/outline` turns your verified corpus into a shape you can write into, each point backed by a cited fact.]

#gloss("Staleness")[When enough time has passed that a fact's grounding may no longer hold. `/stale` surfaces the soft, aging facts (model / web) for a second look.]

#gloss("Structured data")[Information stored as discrete, labelled facts — subject, property, value — each with a stable identifier, rather than free-flowing prose. Nothing to misread, everything to cite.]

#gloss("Synthesis")[Drawing many separate facts into one coherent account of a topic. Here it is grounded: built only from your verified facts, each claim cited, honest about the gaps.]

#gloss("Thread")[One research conversation, saved. Keep one per topic so lines of inquiry don't tangle; threads persist between sessions.]

#gloss("Tier upgrade")[Re-grounding an existing fact on a firmer source and raising its provenance to match, without changing the fact's wording. A guess becomes a cited fact over time.]

#gloss("Triangulation")[Testing a claim against several independent sources at once and judging whether they agree. The verdict is not "a source says so" but "the sources concur."]

#gloss("Undisputed fact")[A fact you marked as your own creative invention (`u`, ※) — true within your work by decree, with no external truth to check it against. It sits outside the trust ladder as an axiom.]
