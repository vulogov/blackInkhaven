# B — The Trust Ladder and Provenance

Every fact you keep records a **provenance** — where it came from — and that origin places it on the trust ladder. This appendix lists the origins from firmest to softest, with the glyph each wears in the Facts tree and whether it crosses the fact-check gate.

## The rungs, top to bottom

- ≡ **computed** — A fact you derived by calculation (/calc). The firmest rung — anyone can re-run it. *Gate-skipped.*
- ≡ **simulation** — A deterministic fact from the World book's simulation. As firm as computed, for the same reason. *Gate-skipped.*
- ◆ **wikidata** — A structured datum, cited by Q-id. *Gate-skipped.*
- ⊕ **geonames** — A real place from the gazetteer, cited by id. *Gate-skipped.*
- § **openalex** — A scholarly work, cited by DOI; auto-filed to Sources. *Gate-skipped.*
- § **arxiv** — A preprint, cited by id; auto-filed to Sources. *Gate-skipped.*
- ▪ **document** — Drawn from a source you imported into your corpus. *Fact-checked at the gate.*
- ↑ **promoted** — A Note you promoted into the Facts book. *As its underlying source.*
- ◇ **web** — Grounded on a cited web page. *Fact-checked at the gate.*
- · **model** — The model's unaided answer — an educated guess. *Fact-checked (and refuted, if enabled).*

> **Two glyphs outside the ladder:** Two marks are not rungs at all. The verdict glyphs from an audit — **✓** passed, **?** dubious, **✗** failed (`/factcheck`) — sit **on top of** a fact's tier glyph to report its last check. And **※** marks an **undisputed** (authorial) fact, which sits outside the ladder entirely: it is exempt from `/factcheck` and checked only for internal coherence by `/undisputed`. Its ※ takes on the coherence verdict's colour — plausible, odd, or incoherent.

## Reading a fact's provenance

The tier glyph is the at-a-glance version. The full record — the specific source, the query that produced it, any check verdict folded in — travels with the fact and answers the one question this whole book is built around: **how do you know?**

The point of the ladder is not to forbid the low rungs. A novelist grounding the feel of a place on a web page has done legitimate work; the `◇` simply keeps that fact honest about where it stands. What the ladder insists on is only this — that a fact never pretend to be firmer than its origin, and that **you always know which rung you are standing on.**
