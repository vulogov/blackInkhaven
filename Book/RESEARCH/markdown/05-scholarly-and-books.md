# 5 — The Literature and the Library

Some facts are not data points — they are **findings**. The result of a study, the argument of a paper, the passage in a book. For these the structured sources of the last chapter have nothing to hand you; you need the **literature** and the **library**. This chapter adds three sources: two scholarly indexes and a shelf of seventy-five thousand public-domain books.

## Scholarly work: `/openalex` and `/arxiv`

The scholarly record is where careful claims live, and Inkhaven reaches two large, free indexes of it.

**`/openalex`** searches OpenAlex — a comprehensive index of scholarly works across every field — and returns the top matching paper: its title, authors, year, venue, and abstract, identified by a DOI.

**`/arxiv`** does the same against arXiv, the preprint server for the sciences — useful when you want the newest work, before it has cleared peer review.

```
/openalex Roman aqueduct hydraulic capacity
```

**DOI** — A **DOI** (Digital Object Identifier) is a permanent code that points to a specific scholarly work — the scholarly equivalent of a Q-id. A claim cited by DOI can be resolved to the exact paper by any reader, forever.

**Preprint** — A **preprint** is a scholarly paper shared publicly **before** formal peer review. It is fast and current, but less settled than a published article — worth citing with that caveat in mind. arXiv is the largest preprint server.

A `/fact` taken from a scholarly result records `openalex` or `arxiv` provenance, with the paper's identifier — and it does one more thing for you automatically.

### The citation files itself

When you keep a fact grounded on a paper, the Assistant also writes a proper bibliography entry — author, title, year, DOI — into your **Sources** book, under a Research chapter, without you lifting a finger. You research; the bibliography assembles itself in the background. A much later chapter will show you turning that accumulated Sources book into a formatted reference list with a single command. The habit that makes it possible starts here: every scholarly fact you keep quietly deposits its citation.

> **Grounding on the paper, honestly:** A scholarly `/fact` grounds on the paper's **metadata and abstract** — enough to cite the claim and point a reader to the work. It is not a promise that you read the full text. For claims where that matters, the next section — ingesting a whole source — lets you draw facts from the actual pages.

## The library: `/gutenberg`

Project Gutenberg is a library of some seventy-five thousand **public-domain** books — out of copyright, free to read and reuse. Inkhaven can pull one straight into your corpus:

```
/gutenberg pride and prejudice
```

It searches the catalogue, fetches the book's text, strips the boilerplate, and **ingests** it — breaks it into passages and files them as research material, credited to the book. From then on, when you ask a question, the relevant passages of that book can be retrieved and quoted back to you, cited as `[source: <title>]`.

**Ingesting a source** — To **ingest** a source is to bring its full text into your corpus, split into searchable passages. Afterwards, the Research Assistant can **retrieve** the passages relevant to a question and ground an answer on them — so a fact can quote the actual text, not a summary of it. This is how a whole book becomes something you can search and cite.

Because Gutendex (the catalogue behind `/gutenberg`) searches by **title, author, and subject**, you find the book by its metadata; the passage-level searching happens afterwards, inside your own corpus. If the top hit is not the edition you want, the reply lists alternatives by their catalogue number, and you can ingest a specific one — or even a single chapter of a long book — by asking for it directly.

**For fiction —** Ingest a period text — a Victorian novel, a real memoir, a public-domain history — and let its **voice and texture** inform your prose. Ask "how did they describe a railway journey in 1870?" and get actual period passages to steep in, quoted and credited.

**For non-fiction —** Ingest a primary source — a public-domain treatise, a historical document — and quote from it **by page**, cited. A claim grounded on an ingested source points the reader at the exact text, not a paraphrase.

> **Public domain, used politely:** Gutenberg texts carry no copyright; ingesting them is unproblematic. The catalogue is a free service, and Inkhaven fetches one book per command with a polite cap on how much of a very long book it embeds — enough to be useful, bounded enough to be a good citizen. It all degrades cleanly when you are offline.

## Where each source sits

You now have the whole authoritative fan except the web (next chapter). It helps to hold them in one view — same gesture, different rung:

- **`/wikidata`, `/geonames`** — structured data, cited by id, gate-skipped (Chapter 4).
- **`/openalex`, `/arxiv`** — the scholarly record, cited by DOI, auto-filed to your bibliography.
- **`/gutenberg`** — full public-domain books, ingested so their passages are searchable and quotable.

Each one starts a fact higher than a guess and hands you a citation. What they share is authority you can point at. The web — vast, immediate, uneven — is the one source where that authority has to be **earned** at the gate. That is the last piece of the fan, and the next chapter.

**Recap**

- **`/openalex`** and **`/arxiv`** return scholarly papers cited by **DOI**; arXiv adds current **preprints**, with the peer-review caveat.
- A scholarly `/fact` **files its own citation** into the Sources book — the bibliography assembles itself as you research.
- **`/gutenberg`** **ingests** whole public-domain books; their relevant passages are then retrieved and quoted, cited by title.
- **Ingesting** means the full text becomes searchable in your corpus, so a fact can quote the actual pages rather than a summary.
- All the authoritative sources share one thing: they hand you a citation and start the fact high on the ladder.
