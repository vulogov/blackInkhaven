#import "../design.typ": *

#chapter(number: 13, title: "The Bibliography That Built Itself")

Somewhere back in Chapter 5, a small promise was made and then left to run quietly
in the background. Every time you kept a fact grounded on a scholarly paper — every
`/openalex` and `/arxiv` result you turned into a `/fact` — a proper citation was
filed into your *Sources* book without you doing anything. Every reference you
imported went there too. All that time, a bibliography has been assembling itself
out of the provenance you accumulated. This chapter is where you collect on it.

#section("Where the citations went")

Recall the third of your research books, introduced back in Part I. *Facts* holds
what you trust; *Notes* holds what you're unsure of; *Sources* holds your
bibliography — the citable works behind your facts. You have barely had to touch it,
and yet it has been filling the whole time.

#term("Bibliography")[
  A *bibliography* is the list of sources a work draws on — its references. In a
  non-fiction book it is often part of the deliverable itself; in fiction it may be
  a private record or a "further reading" note. Either way, a hand-maintained
  bibliography is tedious and error-prone. The point of this chapter is that yours
  was never hand-maintained: it was a by-product of researching honestly.
]

#section("Collecting it: `/bibliography`")

One command turns the accumulated Sources book into a formatted reference list:

```
/bibliography
```

It walks the citations you gathered and emits them as *BibTeX* — the standard,
portable citation format — ready to copy into a references section, or into the
tools that typeset one.

#term("BibTeX")[
  *BibTeX* is a plain-text format for bibliographic entries — author, title, year,
  identifier, one block per source. It is the lingua franca of citations: nearly
  every academic tool reads it, and Inkhaven's own book-assembly can render it into
  a formatted reference list. It is what `/bibliography` produces.
]

Because each entry was filed from a real source with a real identifier — a DOI, a
catalogue number — the list is not a guess at how a citation *should* look; it is
the actual, resolvable reference. You copy it with a keystroke, or, if you would
rather not open the tool at all:

```
inkhaven research --bibliography --out references.bib
```

That writes the whole bibliography to a file from the command line — the same
citations, without the interface. Later parts return to this *headless* way of
working; here it is enough to know that your reference list is one command away
whether you are inside Inkhaven or scripting around it.

#two_track(
  [Fiction rarely ships a bibliography, but the research behind a serious novel
   deserves a record. `/bibliography` gives you a "sources consulted" list for an
   author's note, an acknowledgements page, or simply your own archive of what
   grounded the world — assembled for free as you worked.],
  [This is often part of what you are *delivering*. The references section that a
   reviewer or an editor expects builds itself from your provenance: every claim
   you grounded on a paper is already a citation, and `/bibliography` collects them
   into the list your manuscript needs. No separate citation manager, no re-typing,
   no drift between what you cited and what you listed.],
)

#callout(label: "The quiet reward of honesty")[
  There is a lesson hiding in this chapter. The bibliography assembled itself only
  because, all along, every fact recorded *where it came from*. Provenance — the
  small, almost invisible habit from Chapter 1 — is what makes a free bibliography
  possible. The discipline of grounding was never just about being right; it was
  about building a corpus that could give this much back.
]

#section("Managing the Sources book from the shell")

`/bibliography` is the quick path; the Sources book is also a first-class thing you
can manage directly, with a dedicated `inkhaven sources` command, for working with
reference managers like Zotero. It reads and writes the same Sources book, from
outside the Research screen:

```
inkhaven sources list
inkhaven sources export --format csl-json --out sources.json
inkhaven sources import zotero-export.bib
inkhaven sources check
```

`export` writes your citations out in one of two formats: *BibTeX*, as
`/bibliography` does, or *CSL-JSON* — the format Zotero, Mendeley, and the wider
citation ecosystem read and write. Because Inkhaven also *imports* BibTeX, the
round-trip is complete: a library curated in Zotero comes in, your research adds to
it, and it goes back out. And `sources check` validates the book — flagging a
missing key or a malformed entry, and exiting non-zero — so you can wire it into a
continuous-integration step and never ship a broken reference.

#term("CSL-JSON")[
  *CSL-JSON* is the JSON citation format of the Citation Style Language ecosystem —
  what Zotero and most modern reference managers speak natively. Where BibTeX is the
  LaTeX world's lingua franca, CSL-JSON is the interchange format between citation
  tools. `sources export --format csl-json` closes the round-trip with them.
]

#section("The loop closes")

Step back to the project arc you met in Chapter 2 — acquire, cross-check, maintain,
compose. You have now travelled the whole circle. You *acquired* facts from every
rung of the ladder; you *cross-checked* the load-bearing ones and audited the rest;
you *maintained* the corpus by upgrading guesses and flagging stale facts; and in
this part you *composed* it back out — a cited synthesis, a working outline, a list
of what's missing, and a bibliography that built itself. The knowledge base that
began as an empty tree has become something that writes back into your book.

Two things remain. Everything so far has been hands-on, one command at a time — but
some research is better done in bulk, headlessly, while you do something else; that
is the next part. And then a single worked example, start to finish, to see the
whole workflow move as one. The tools are all in your hands now; what's left is
learning to run them at scale and in concert.

#recap((
  [Your *Sources* book filled itself the whole time: every scholarly `/fact` and
   every `/import` filed a citation automatically.],
  [`/bibliography` collects those into *BibTeX* — the portable, resolvable citation
   format — ready to copy or typeset.],
  [`inkhaven research --bibliography --out file.bib` writes the same list from the
   command line, no interface needed.],
  [The free bibliography is the reward of *provenance* — the habit of every fact
   recording its origin, from Chapter 1, paying off.],
  [With this the project arc closes — acquire, cross-check, maintain, *compose* —
   and the corpus writes back into your book.],
))
