#import "../design.typ": *

#chapter(number: 6, title: "Composing with loci")

A study of scripture and philosophy does not cite _works_; it cites _passages_. "As
Kant argues" is worthless to a reader who wants to check you; "as Kant argues at
A51/B75" sends them to the exact place. The unit of citation on this track is the
locus — the verse, the surah-and-ayah, the Academy-edition page — and Inkhaven cites
loci natively, then gathers every one you use into an Index Locorum: the scholarly
apparatus that lists, per source, every passage the work engages.

#term("Locus")[
  A specific passage within a source — `John 3:16`, `A51/B75`, `Ak. 5:122`,
  `Moroni 10:32` — as opposed to the source as a whole. In theology, classics, and
  law the locus is the true unit of citation: the tradition is a conversation about
  passages, and a claim about a source is checkable only when it names the passage it
  rests on.
]

#section("Cite a locus")

You already have the cite keys — every ingested source was auto-filed under one, and
scripture under a stable key (`bible`, `book-of-mormon`). To cite a locus, write the
key and put the passage in brackets:

#config("in the manuscript", [```typst
As the second Critique argues @kant-cpr[Ak. 5:122], holiness is
an infinite task; the Sermon on the Mount sets it as a standard,
not a report @bible[Matthew 5:48]; yet Moroni promises a real
completion @book-of-mormon[Moroni 10:32].
```])

That bracketed form is not an Inkhaven invention — it is native Typst, where the
bracket after a citation is the _supplement_. Which means the on-disk prose needs no
special handling and no transformation: `@bible[Matthew 5:48]` is a real citation to
the `bible` source with the supplement "Matthew 5:48", and it renders and resolves
exactly as a page-numbered citation would. Inkhaven's only addition is to _harvest_
these loci for the index.

The cite picker helps you not to mistype a key: `Ctrl+V @` opens a fuzzy search over
the Sources book and drops the matching `@key` where the cursor sits. You add the
`[locus]` yourself — the picker gets the key right, and the passage is yours to name.

#note[
  Because a locus is a native Typst supplement, the key must resolve to a real
  bibliography entry — the same requirement any citation carries. `@kant-cpr[Ak.
  5:122]` compiles only if `kant-cpr` is in your Sources book; a locus on a key with
  no entry is a compile error, exactly as a citation to a missing reference would be.
  This is a feature, not a friction: it means the index of passages can never list a
  source you never actually filed.
]

#section("Build the Index Locorum")

Once the draft cites loci, `inkhaven index-locorum` walks the whole manuscript,
harvests every `@key[locus]`, resolves each key to its source title from the Sources
book, and renders the apparatus — grouped by source, the loci within each sorted
naturally, so `Matthew 5:2` precedes `Matthew 5:48`:

```
inkhaven index-locorum                    # Markdown to the terminal
inkhaven index-locorum --format typst     # a Typst chapter
inkhaven index-locorum --format json      # structured, for tooling
```

The Markdown it produces reads like the back of a scholarly monograph:

#config("index-locorum output", [```md
## The Book of Mormon (`@book-of-mormon`)
- Moroni 10:32 — On arrival

## The Critique of Practical Reason (`@kant-cpr`)
- Ak. 5:122 — On trajectory

## The Holy Bible (`@bible`)
- Matthew 5:48 — On trajectory
```])

Each locus carries the chapter it was cited in, so the index is not only a list of
passages but a small map of where in your argument each one does its work. The stable
scripture keys from Chapter 2 pay off here: every Bible passage you ever cited, from
whichever ingested chapter, gathers under the one "The Holy Bible" heading.

#section("Validate the loci, and let them canonicalize")

A citation to a passage is only as good as the passage being real. `@bible[John
3:sixteen]` compiles without complaint — Typst has no idea "sixteen" is not a verse —
and quietly ships a reference no reader can follow. So a locus is checked against a
_reference scheme_: a grammar for what a well-formed reference to that source looks
like. The three scripture keys carry built-in schemes (`{book} {ch}:{v}` for the
Bible, `{surah}:{ayah}` for the Qur'an), so their loci validate with nothing to
configure; for anything else — Kant's A/B pagination, a Stephanus number — you
declare the scheme once:

#config("inkhaven.hjson", [```hjson
sources: {
  ref_schemes: {
    kant-ab: { pattern: "^A\\d+(/B\\d+)?$", format: "A{n}/B{n}" }
  }
}
```])

and name it on the source (a `scheme: kant-ab` line in its Sources entry). A locus
that does not match its scheme is flagged — never silently dropped. Catch them from
the shell, where `--strict` turns a malformed locus into a failed exit fit for a
continuous-integration step:

#transcript("inkhaven index-locorum --strict", [
  ⚑ `@bible[John 3:sixteen]` — expected {book} {ch}:{v} (The Holy Bible) \
  ⚑ `@bible[Romans 8]` — expected {book} {ch}:{v} (The Holy Bible) \
  index-locorum: 2 malformed locus(es) → _exit 1_
])

or catch them as you write: `Ctrl+V c` lints the open paragraph's loci against their
schemes and drops a ⚑ warning into the Output pane for each malformed one, anchored
to the paragraph — the deterministic, editor-side twin of the check above, beside the
`Ctrl+V Shift+C` sourcing pass.

#insight[
  The scheme does a second, quieter service: it _canonicalizes_. Write `@bible[Jn
  3.16]` today and `@bible[John 3:16]` tomorrow — or, in a Russian project,
  `@bible[Иоанна 3:16]` — and the index resolves all three to one entry, "John 3:16",
  rather than three near-duplicates scattered under the same source. A reference
  scheme is not only a gate that rejects the malformed; it is the rule that makes the
  well-formed _consistent_, so the apparatus reads as one hand wrote it even when your
  citations, across months of drafting, did not.
]

#section("Fold it into the finished book")

You do not have to run the command by hand. Turn one switch on, and the Index
Locorum is assembled into the book automatically, after the bibliography:

#config("inkhaven.hjson", [```hjson
sources: {
  index_locorum: true
}
```])

With that set, `inkhaven build` emits an `index_locorum.typ` alongside the
`sources.bib` and includes it in the assembled root, right after the reference list —
so the finished PDF carries both the bibliography (the works) and the Index Locorum
(the passages), each generated from what the manuscript actually cites. It is off by
default, because it is a specialised apparatus most books do not want; on this track
it is the natural close of the citation story.

#insight[
  A bibliography answers "which works did this engage?"; an Index Locorum answers
  "which _passages_?" — and for a study whose entire argument turns on what specific
  verses and pages say, the second is the more honest record. It is also
  self-auditing: because it is generated from the loci actually in the prose, it
  cannot flatter you with a passage you meant to engage but did not. The index is the
  argument's contact sheet — every place it touched a source, laid out where a critic
  can check each one.
]

#recap((
  [Cite a *passage*, not a work: `@key[locus]` (`@kant-cpr[Ak. 5:122]`, `@bible[Matthew 5:48]`) is native Typst — the bracket is the supplement — so the prose needs no special handling and the locus renders as a real citation.],
  [The cite picker (`Ctrl+V @`) gets the *key* right from the Sources book; you name the *locus*. A locus on a key with no entry is a compile error — so the index can never list a source you never filed.],
  [`inkhaven index-locorum` harvests every `@key[locus]`, resolves titles, and renders the apparatus (Markdown / Typst / JSON) — grouped by source, loci sorted naturally, each tagged with the chapter it was cited in.],
  [*Validate* loci against a *reference scheme* — built-in for the scripture keys, `sources.ref_schemes` for the rest: `--strict` fails a build on a malformed reference, and `Ctrl+V c` lints the open paragraph as you write. The scheme also *canonicalizes*, so `Jn 3.16`, `John 3:16`, and `Иоанна 3:16` collapse to one entry.],
  [Set `sources.index_locorum: true` and `inkhaven build` folds the Index Locorum into the finished book after the bibliography — the works *and* the passages, both generated from what the manuscript actually cites.],
))
