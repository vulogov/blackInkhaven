# Scholarly Apparatus

*(the native tools for theology / philosophy / classics / law — LOCI 1.6.18+,
LEXICON 1.6.21+, ARG-1)*

A critical edition carries an apparatus at the back: an *index locorum* of every
passage it cites, an *index verborum* of its key terms, a working lexicon of those
terms' senses, and — behind it all — an argument that holds together. Inkhaven
builds each of these from the manuscript you already have. They read your prose and
your Sources / Glossary books; they **measure and report**, they never rewrite.

> **Four commands, one job: turn a scholarly manuscript into a scholarly book —
> the indices, the lexicon, and the argument audit a serious work of theology or
> philosophy is expected to stand behind.**

Three of the four are **deterministic and free** (`index-locorum`,
`index-verborum`, `lexicon`); only `argue` calls a model. All are advisory.

---

## `inkhaven index-locorum` — the Index Locorum

Every `@key[locus]` primary-source citation across the manuscript, grouped by
source and sorted by passage — the "index of places" a work of scripture,
classics, or law is judged by (`@bible[John 3:16]`, `@kant[A51/B75]`,
`@quran[2:255]`).

Only a citation that **carries a locus** contributes. A bare `@key` is an ordinary
citation and belongs in the bibliography, not here.

```
inkhaven index-locorum [--book-name <NAME>] [--format md|typst|json]
                       [-o|--out <FILE>] [--strict]
```

| Flag | Effect |
| ---- | ------ |
| `--book-name <NAME>` | Limit to one user book (default: every user book). |
| `--format <FMT>` | `md` (default), `typst`, or `json`. |
| `-o`, `--out <FILE>` | Write to a file instead of stdout (and print a source/locus count). |
| `--strict` | Exit non-zero if any locus is malformed — a CI gate. |

**What it does.** It reads the *raw* prose (so the `[…]` supplement survives),
harvests each `@key[locus]`, and resolves the key to its title from the Sources
book. Loci are grouped under their source, sources sorted by title, and loci sorted
*naturally* (numeric-aware, so `3:2` precedes `3:16`). Each locus lists the chapters
that cite it, deduped and in first-seen order.

**Reference schemes and canonicalization.** Scripture keys (`bible`,
`book-of-mormon`, `quran`) validate with **zero configuration** via built-in
schemes; any other source can declare a `scheme:` in its Sources entry, resolved
against `sources.ref_schemes`. For the built-in scripture schemes the locus is also
*canonicalized* — the chapter–verse separator is normalized to `:` and the book is
resolved to its canonical English name — so `Jn 3.16`, `иоанна 3:16`, and
`John 3:16` collapse into one entry (a two-character abbreviation like `Jn` that the
table can't resolve keeps its spelling but still gets the fixed separator). A source
with no scheme leaves its loci verbatim and always valid, so a tradition that
legitimately uses dots (`Sutra 1.24`) is never mangled.

**Malformed loci** — a reference that fails its source's scheme — are listed on
stderr with the expected format (`⚠ @bible[John 3] — expected {book} {ch}:{v}`).
Without `--strict` this is advisory; with `--strict` it fails the run.

**Output.** An `Index Locorum` chapter, heading localized (`Index Locorum` for
en/fr/es, `Указатель мест` for ru, `Stellenregister` for de). Typst is emitted as a
`#include`-able chapter; JSON carries `{index_locorum, sources, loci}`.

---

## `inkhaven index-verborum` — the Index Verborum

The term-level twin of the Index Locorum: every scholarly-lexicon term that
**actually appears** in the manuscript, with its original-language form, its
distinct senses, and the chapters that use each. The apparatus a critical edition
carries at the back — for theology, philosophy, and classics.

```
inkhaven index-verborum [--book-name <NAME>] [--format md|typst|json]
                        [-o|--out <FILE>]
```

| Flag | Effect |
| ---- | ------ |
| `--book-name <NAME>` | Limit to one user book (default: every user book). |
| `--format <FMT>` | `md` (default), `typst`, or `json`. |
| `-o`, `--out <FILE>` | Write to a file instead of stdout (and print a term count). |

**What it does.** It takes the *lexicon* terms from the Glossary book — an entry
counts as a lexicon term only if it carries an `original_forms` field or `senses`
(a plain consistency-glossary entry does not) — and looks for each in the prose. A
term is recorded for a chapter when it (or one of its `synonyms`, treated as
surface variants of the same lemma) appears there as a whole word. A term you
defined but never used is **dropped** — the index cannot flatter you.

**Sense-level indexing.** Where the author tags a use with the Typst superscript
convention `term#super[N]` (which renders as a scholarly sense number *and* is
harvestable), the N-th sense records that chapter — so the index can show which
sense of a polysemous term was used where.

**Output.** An `Index Verborum` chapter — each term with its original-language
form(s) in italics, its numbered senses (label — gloss, with per-sense chapters
where tagged), and a term-level "used in" chapter list. Heading localized
(`Index Verborum` for en/fr/es, `Указатель терминов` for ru, `Wortregister` for de).
JSON carries `{index_verborum, terms}`.

---

## `inkhaven lexicon` — the scholarly lexicon

The working sense-inventory behind the Index Verborum: the terms a scholarly work
tracks, each with its original-language form(s), its distinct senses, and whether
it is watched for **equivocation** (an argument sliding between a term's senses).
This is the inventory the reasoning-rigor reader consults when it polices a
paragraph for equivocation.

```
inkhaven lexicon list [--book <NAME>] [--watched] [--json]
```

`list` is the only subcommand.

| Flag | Effect |
| ---- | ------ |
| `--book <NAME>` | Scope to one book's Glossary (default: all). |
| `--watched` | Only the equivocation-watched, multi-sense terms. |
| `--json` | Machine-readable output. |

**What it does.** It reads valid Glossary entries, sorts them by term, and prints
each term with its original-language forms (`⟨…⟩`), its numbered senses
(`[1] label — gloss`), and a `⊬ watched` marker. A term is *equivocation-watched*
only when it both sets `watch_equivocation` **and** declares at least two senses —
declaring the senses is what lets the tool tell a legitimate polysemy from an
equivocation, so a one-sense term is never policed. `--watched` narrows the list to
exactly those terms.

Unlike `index-verborum`, `lexicon list` reports the **declared** inventory whether
or not each term is used in the prose — it is the source of truth you curate, not a
usage report.

---

## `inkhaven argue` — the argument outline (ARG-1)

Per chapter, the load-bearing claims the chapter argues for and the support it
gives each — an argument outline — flagging the two cheapest structural gaps: a
**central claim with no support** and an **orphan citation** (a source cited but
supporting no identified claim).

```
inkhaven argue [--book-name <NAME>] [--provider <NAME>] [--json]
```

| Flag | Effect |
| ---- | ------ |
| `--book-name <NAME>` | Limit to one user book (default: every user book). |
| `--provider <NAME>` | LLM provider override. |
| `--json` | Machine-readable report. |

**What it does.** For each chapter it hands the model the plain prose plus the list
of `@key` citations the chapter uses (labelled from the Sources book), and asks for
the central claims, each claim's support (a `@key`, a short reason, or `NONE`), and
any orphan citations. It is the one command here that calls a model — it needs a
configured LLM provider, and cost is previewed against the daily cap (cost informs,
it never blocks).

**Anti-hallucination guard.** Every returned claim must be quoted from the chapter,
and a claim whose text is not *actually present* in the prose is **dropped** — the
guard checks that at least ~60% of a claim's significant words (or, for a very short
claim, an exact substring) appear in the chapter. A paraphrase the model invented
does not survive; only an argument the author really made is reported.

**Output.** Per chapter, each claim with its support (`← @key`) or a
`⚠ unsupported` mark, and each orphan citation. Progress prints to stderr; the
report to stdout (text or, with `--json`, `{chapters, gaps}`). A *gap* is an
unsupported claim or an orphan citation; the command **exits non-zero when any gap
is found**, so it drops into a CI step.

---

## Multilingual

Each command keys off the project language and matches Unicode-aware, so Cyrillic,
Greek, and accented forms behave exactly as Latin ones do. The index headings are
localized (Latin term for en/fr/es; native equivalents for de/ru). `index-locorum`
canonicalizes Russian Synodal book names to their canonical form alongside the
English ones; `index-verborum` and `lexicon` match your own terms and synonyms in
the project's script; `argue`'s prompt and quote guard are language-agnostic
(the guard is a Unicode word overlap, not an English word list).

## What they are not

- Not generators — they index, list, and audit what you wrote; they never author
  prose or invent a citation, a term, or an argument.
- Not correctors — every finding is advisory. `index-locorum --strict` and a
  non-zero `argue` exit are gates you opt into, not edits.
- `index-verborum` reports **used** terms; `lexicon list` reports the **declared**
  inventory. They answer different questions and will differ whenever you define a
  term you have not yet used.
