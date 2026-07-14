#import "../design.typ": *

#chapter(number: 2, title: "Gathering the primary sources")

Philosophy and theology are conversations centuries deep, and a position that
recalls its sources from memory is weaker for it — and, with a large language
model in the loop, actively dangerous, because a model will confabulate a plausible
verse or a Kantian page number that says exactly what you wished it said. The
discipline of this chapter is the opposite: ingest the _real_ texts, keep each with
its provenance, and let every later judgement rest on a passage that is actually on
disk. Our three primary sources are Kant, the Bible, and the Book of Mormon. All
three are public-domain, and all three come in through one command each.

#section("Kant, from the public domain")

The Research Assistant (`inkhaven research`) is where gathering happens.
`/gutenberg` searches the keyless Project Gutenberg catalogue and ingests a
public-domain book's full text as a research source, chunked and embedded so the
relevant passages are retrievable later:

#transcript("/gutenberg Kant Critique of Pure Reason", [
  Ingested *The Critique of Pure Reason* (Immanuel Kant, tr. Meiklejohn) as a
  research source — 214 chunk(s). Cited as `@kant-cpr`. \
  _Other matches (re-run with an exact title):_ Kant, _Critique of Practical
  Reason_ · Kant, _Fundamental Principles of the Metaphysic of Morals_.
])

Run it a second time for the second _Critique_ — the _Critique of Practical
Reason_ is where Kant's immortality postulate actually lives, and our thesis
leans on it — and Inkhaven files a citation for each to the Sources book
automatically as it ingests. We now have Kant on disk, in a translation whose
public-domain status is not in doubt, retrievable by meaning rather than by our
memory of where a passage sat.

#term("Public-domain sourcing")[
  The rule, on this track, that every ingested source is a text free of copyright
  restriction — Gutenberg's catalogue, the Internet Archive's scans, and the
  public-domain scripture translations the adapters reach for. It is a discipline,
  not a limitation: it keeps the corpus legally clean, and it happens to align
  with the texts a serious study of a historical tradition wants anyway — the
  editions the tradition itself was formed on.
]

#section("The scripture adapters")

Scripture is not an ordinary source. A book of it is a structured thing —
chapter and verse, surah and ayah — and the unit a theologian cites is not the
work but the _passage_. Inkhaven has three adapters built for exactly this shape,
each keyless and each drawing only on public-domain translations:

#config("the three adapters", [
  `/bible <book> <chapter>` — a Bible passage, verse by verse \
  `/quran <surah>` — a surah of the Qur'an, ayah by ayah \
  `/bookofmormon <book> <chapter>` — a Book of Mormon passage, verse by verse
])

Our essay engages the Bible and the Book of Mormon, so we pull the passages the
thesis turns on. The Sermon on the Mount's command to perfection is the Biblical
hinge of "eternal progression"; Moroni's promise of being "perfected in Christ"
is its Book-of-Mormon counterpart:

#transcript("/bible Matthew 5", [
  Ingested *The Holy Bible · Matthew 5 (WEB)* — 48 verse(s), 4 chunk(s). \
  Cited as `@bible` — reference a locus with `@bible[Matthew 5:<verse>]`.
])

#transcript("/bookofmormon Moroni 10", [
  Ingested *The Book of Mormon · Moroni 10 (1830)* — 34 verse(s), 3 chunk(s). \
  Cited as `@book-of-mormon` — reference a locus with `@book-of-mormon[Moroni 10:<verse>]`.
])

Two details in those replies do quiet, important work. First, the text is
_verse-structured_: each chunk carries its verse references alongside the text,
so when a passage is retrieved later you see exactly which verses it covers — and
the reference is quotable straight into a citation. Second, each work is cited
under a _stable key_ — `bible`, `book-of-mormon`, `quran` — no matter which
chapter seeded it. Ingest Matthew today and John next week, and both file under
the one `@bible` key, so every passage you ever cite from scripture gathers under
its work in the Index Locorum of Chapter 6.

#insight[
  The stable cite key is what turns a pile of ingested chapters into a citable
  edition. Because every Bible passage shares the `bible` key, `@bible[John 3:16]`
  and `@bible[Matthew 5:48]` are two _loci_ of one source, not two unrelated
  citations — and the machinery that builds the index of cited passages can group
  them under "The Holy Bible" without your telling it they belong together. You
  gather chapters; you cite passages; the key holds the two together.
]

#section("Native tongue, native text")

The `language` line you set in Chapter 1 is not decoration here. The adapters
choose a public-domain translation _by the project's language_: English draws the
World English Bible and, for the Qur'an, the Sahih International rendering;
Russian draws the Synodal Bible and Kuliev's Qur'an; French, German, and Spanish
each draw a public-domain translation of their own. A Russian-language study of
the same question would run `/bible Иоанна 3` — the book named in Russian — and
receive the Synodal text, in Cyrillic, with no further configuration.

#note[
  You are never locked to the default. `research.scripture.bible_translation`
  forces a specific translation code regardless of the project language, and
  `research.scripture.quran_translation` does the same for the Qur'an — set it to
  `quran-uthmani` when you want the Arabic original alongside a translation. The
  Book of Mormon is English-only by necessity: its 1830 text is public-domain, but
  the modern translations are under copyright, and this track sources public-domain
  only.
]

#section("The same discipline, other doors")

Two more adapters round out the primary-source gathering when a tradition's texts
are not scripture in the narrow sense. `/archive` searches the Internet Archive's
public-domain scans — a way in to older translations, patristic editions, or a
philosopher not in Gutenberg — and `/wikisource` pulls a public-domain page from
Wikisource _in the book's language_, so a native author reaches native texts.
Both ingest exactly as the scripture adapters do — chunked, provenance-tagged,
auto-cited — so from the corpus's point of view a surah, a Gutenberg _Critique_,
and an Archive scan are the same kind of thing: a retrievable source with a name.

#pitfall[
  Do not paste a passage you "remember" into your notes and treat it as grounded.
  The entire point of ingesting the real text is that later stages retrieve from
  _it_ and not from anyone's recollection — yours or the model's. If a passage is
  not in the corpus, it cannot be retrieved, related, or confronted; it is just an
  assertion wearing a citation's clothes. Gather the source first; cite from what
  you gathered.
]

#recap((
  [*Gather the real texts, never memory:* `/gutenberg` ingests a public-domain book (Kant's two _Critiques_) and auto-files its citation; a model's recalled verse or page number is confabulation and must not be trusted.],
  [*The scripture adapters* — `/bible`, `/quran`, `/bookofmormon` — ingest passages verse-by-verse from public-domain translations, each keyless, each auto-cited under a *stable key* so every passage of a work gathers under one source.],
  [*Language drives translation:* the project's `language` selects the public-domain translation (English→WEB/Sahih, Russian→Synodal/Kuliev, …); override per work with `research.scripture.bible_translation` / `quran_translation`.],
  [*`/archive` and `/wikisource`* extend the same public-domain, provenance-tagged, auto-cited discipline to non-scriptural primary texts — a surah, a Gutenberg _Critique_, and an Archive scan are one kind of thing to the corpus.],
))
