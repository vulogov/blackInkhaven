# LANG-2 — Sociolinguistics & Contact (implementation plan)

_Status: proposal. Authored during 1.3.21-dev (the ConLang-Suite Bund
integration). Implementation targets a future release line; this document is the
RFC, committed now so the design is reviewable before any code lands._

## 0. Framing — a language lives in a society, not a vacuum

RFC **LANG-1** built a constructed language as a *standalone artefact*: sounds →
words → grammar → history → script → books → a syntax engine → full scripting.
But a real language is never uniform and never alone. It **varies** — across
regions, classes, ages, and situations — and it **contacts** other languages,
trading words, structures, and whole feature-bundles. LANG-2 models both, and
ties them to the world the language is spoken in.

This is, like LANG-1, an **expansion of a shipped feature**, not a greenfield
build. It reuses the LANG-1 engines almost wholesale and stays inside the same
`Language` system book. The headline insight that makes it cheap:

> **A dialect is a sound-change chain applied _synchronously_ instead of
> _diachronically_, and a loanword is a phonotactic repair.** LANG-1 already
> ships both engines (`diachronic`/`phonology::rewrite` and the phonotactic
> validator). LANG-2 is mostly *new orchestration over existing engines*, plus a
> world-integration layer over the `conlang::links` sidecar that already maps
> Places and Characters to languages.

## 1. What LANG-1 already gives us (the substrate)

| Capability | Module | LANG-2 reuse |
| --- | --- | --- |
| Ordered SPE rewrites | `phonology::rewrite::apply_ordered` | **dialect sound changes** (synchronic deltas) |
| Sound-change chains | `diachronic::apply::derive_form` | dialect derivation; contact-induced change |
| Phonotactic validator + inventory | `phonology` | **loanword repair** (nativisation) |
| Lexicon + register tags | `lexicon`, `DictionaryEntry.registers` | register/dialect lexical overrides |
| Morphology + syntax engine | `morphology`, `syntax` | render a sentence *in a variety* |
| Places/Characters ↔ language links + proficiency | `conlang::links` (`conlang-links.json`) | **speech communities, idiolect, bilingualism** |
| SVG → PNG | `resvg`/`usvg` (in tree) | the **ecology / dialect atlas** map |
| Output renderers, Bund `ink.lang.*`, `Ctrl+B X` hub | `output`, `scripting::stdlib::lang`, `tui` | LANG-2 surfaces |

**No new dependencies are anticipated.**

## 2. Storage architecture (extend the book in place)

The `Language` sub-book stays the system of record. LANG-2 adds:

- a **`variety`** block (one or more) under the Grammar chapter — each declares a
  dialect / register / sociolect as a delta on the base language;
- a **`contact`** block — areal-feature bundles + declared loan-phonology;
- **world links** in the existing `.inkhaven/conlang-links.json` sidecar, extended
  with per-Place spoken varieties and per-Character commanded languages + native
  variety (advisory sidecar; the Places/Characters prose books are never touched,
  per `[[feedback-ai-advisory]]`).

Everything reconstructs into in-memory models exactly as LANG-1 does.

## 3. The two core models

```hjson
// A VARIETY — a delta on the base language.
variety: {
  id: "lowland"
  kind: "dialect"            // dialect | register | sociolect | idiolect
  axis: "region"            // region | class | age | situation | formality
  prestige: "low"
  sound_changes: [ { rule: "t > d / V _ V" }, { rule: "a: > o:" } ]  // SPE, reused engine
  lexicon: { "water": "móru", "to-eat": "nâ" }   // gloss → variety-specific form
  morphology: { plural: "-an" }                  // optional cell overrides
}

// A CONTACT relationship — what spreads across languages in touch.
contact: {
  region: "the Inner Sea"
  languages: [ "Eldar", "Sindar", "Khuz" ]
  areal_features: { word_order: "sov", alignment: "ergative_absolutive" }  // Sprachbund
  loan_phonology: {                            // how THIS language nativises borrowings
    repair: "epenthesis"                       // epenthesis | deletion | substitution
    epenthetic_vowel: "u"
    substitutions: { "θ": "t", "r": "l" }      // donor sound → nearest native
  }
}
```

## 4. Phases (each independently shippable)

**Status (1.3.22-dev):** P1–P6 **shipped**. The only deferred piece is
code-switching, **postponed to the translation RFC track** (it needs that work's
cross-language machinery); the per-character bilingualism data it will draw on
already ships in P4.

| Phase | Scope | New deps | Status |
| --- | --- | --- | --- |
| **P1** | **Variation core** — variety model; render a word/sentence *in a variety* (sound changes via the rewrite engine + lexical/morph overrides); `variety` / `lect` CLI; dialect comparison | none | ✅ shipped |
| **P2** | **Borrowing** — deterministic loanword-adaptation engine (perceive → repair to recipient phonotactics); calques (structural borrow); `borrow` / `calque` | none | ✅ shipped |
| **P3** | **Areal & contact-induced change** — `contact` bundles; apply convergence (advisory typology overlay); contact as a diachronic event-type that composes with LANG-1 inheritance; `areal` | none | ✅ shipped |
| **P4** | **Speech communities & ecology** — extend the links sidecar (Places speak varieties, Characters command languages + a native variety); `ecology` graph (text + an SVG **dialect/contact atlas** via resvg); per-Character **idiolect** | none | ✅ shipped (code-switching → translation RFC) |
| **P5** | **Output & surfaces** — variation + contact sections in the grammar book; dialect-comparison tables; the full `ink.lang.*` Bund surface | none | ✅ shipped |
| **P6** | **AI advisory** — AI-proposed *plausible* dialects (a coherent sound-change set + lexical swaps), realistic loanword candidates, an areal-plausibility check. Advisory, `--yes`-gated, deterministic forms where possible | none | ✅ shipped (code-switched dialogue → translation RFC) |

## 5. Pillar I — Variation (P1)

A language is a *base* plus a set of **varieties**, each a delta. Rendering a form
*in* a variety is mechanical and deterministic:

1. take the base surface form (the LANG-1 phonology/morphology output);
2. apply the variety's `sound_changes` with `phonology::rewrite::apply_ordered`
   — the **same engine** diachronics uses, just applied to a living variety;
3. swap any `lexicon` override for the requested concept;
4. apply any `morphology` cell overrides through `paradigm::generate`.

Registers are already half-built (lexicon `registers` tags); P1 promotes register
to a first-class variety axis so you can *generate and render in a register*.

- **`inkhaven language lect <lang> <variety> --word W`** / **`--sentence "…"`** —
  render in a dialect/register, with a base↔variety diff.
- **`inkhaven language varieties <lang>`** — list the declared varieties + their
  axes and a one-line characterisation.
- **Dialect comparison** — a table of a Swadesh-ish set across all varieties (the
  classic dialectology display), reusing the LANG-1 `gaps` Swadesh data.

#term: *lect, isogloss, prestige, koine, register, sociolect, idiolect, dialect
continuum.* (Each lands in the developer's-guide glossary.)

## 6. Pillar II — Contact (P2–P3)

### Borrowing (P2)
**Loanword adaptation is phonotactic repair.** Given a donor form and the
recipient's inventory + `loan_phonology`:

1. perceive the donor string against the recipient's phonemes (nearest-match
   substitution per `substitutions`, falling back to the closest by feature);
2. validate against the recipient's phonotactics (the LANG-1 validator);
3. repair each violation by the declared strategy — *epenthesis* (insert the
   `epenthetic_vowel` to break an illegal cluster, Japanese *sutoraiku*),
   *deletion*, or *substitution*;
4. optionally re-derive through the recipient's morphology so the loan inflects
   natively.

- **`inkhaven language borrow <recipient> --from <donor> --form W [--gloss G] [--yes]`**
  — adapt and (advisory) add the loanword to the recipient's lexicon, recording
  the donor in the etymology (so LANG-1 cognate/etymology stay coherent).
- **`calque`** — borrow the *structure* (a compound or metaphor) rather than the
  form, reusing the LANG-1 derivation + metaphor blocks.

### Areal & contact-induced change (P3)
A **`contact`** bundle names a set of languages in a region and the features that
have converged. Applying it is an advisory typology overlay — it shows what each
member would look like under Sprachbund pressure (e.g. all shift to SOV), without
silently rewriting their grammar blocks. Contact also composes with LANG-1
diachronics: a daughter can gain a feature by *contact*, not only inheritance, so
the family tree gains horizontal edges alongside the vertical ones.

## 7. Pillar III — Speech communities & ecology (P4)

Sociolinguistics is inherently *who speaks what, where*. LANG-2 leans on the
`conlang::links` sidecar that already records Place↔language and
Character↔proficiency, and extends it:

- a **Place** declares the varieties spoken there (a region → `lowland` dialect);
- a **Character** commands one or more languages with a proficiency *and a native
  variety* (so dialogue can be rendered in that character's idiolect);
- **`inkhaven language ecology`** prints the **language ecology**: which
  languages/varieties live where, contact edges, prestige — as text and as an
  **SVG atlas** (resvg, already in tree), the dialect-map analogue of the LANG-1
  family tree.
- **Code-switching** — generating an utterance that mixes two languages a
  bilingual character commands, marking the switch points. **Postponed to the
  translation RFC track** (it needs the cross-language machinery that work
  introduces); the per-character bilingualism data that feeds it ships here in P4.

This is the pillar that makes LANG-2 *worldbuilding*, not just linguistics: a
character from the lowlands speaks the low-prestige dialect, borrows words from
the trade language, and code-switches when addressing the court.

## 8. Output, surfaces, Bund, TUI (P5)

- **Grammar book** gains a *Variation* section (the varieties, their isoglosses, a
  dialect-comparison table) and a *Contact* section (loans, areal features).
- **Sociolinguistic profile** (`stats`-analogue): variety count, lexical-distance
  between dialects, loanword share, the prestige ladder.
- **Bund** — the whole pillar is scriptable on the LANG-1 pattern:
  `ink.lang.lect`, `ink.lang.borrow`, `ink.lang.areal`, `ink.lang.ecology`,
  `ink.lang.code_switch`, plus `variety`/`contact` define blocks. Read words
  `store_read`; mutators `store_write`; loanword/dialect AI proposals `ai_write`.
- **`Ctrl+B X` hub** — the read-only ConLang overview gains a variety/ecology pane.

## 9. AI advisory layer (P6)

Thin AI layers in the LANG-1 mould — forms stay deterministic, the AI proposes
*choices* and *prose*, nothing auto-commits ([[feedback-ai-advisory]]):

- **Propose a dialect** — the model suggests a *coherent* set of sound changes +
  lexical swaps for a named social/regional flavour; the deterministic engine
  then *applies* them, so the dialect is always phonologically legal.
- **Realistic loanwords** — propose which concepts a language in this contact
  situation would actually borrow (trade, religion, technology), then the
  deterministic adapter nativises them.
- **Areal-plausibility check** — assess whether a declared Sprachbund is typologically
  believable (the contact analogue of LANG-1 `realism-check`).
- **Code-switched dialogue** — author a short bilingual exchange constrained to
  the two lexicons.

All multilingual ([[feedback-multilingual]]): proposals and glosses key off the
project working language.

## 10. Non-goals / deferred

- Real-world language data or a built-in loanword corpus (everything is the
  author's own languages).
- Full acoustic/perceptual modelling of borrowing (we repair on the phoneme
  inventory, not on formants).
- Automatic *discovery* of contact from a corpus (the author declares contact;
  LANG-2 applies it). Corpus-driven inference is a possible LANG-3.
- **Translation** is explicitly out of scope — it has its own RFC track. LANG-2
  produces forms *within / across* the author's own languages (dialect rendering,
  loanword adaptation, code-switching); it never translates working-language prose.
  Any later in-editor prose rendering composes with the translation track but does
  not depend on it.

## 11. Dependencies, principles, test posture

- **Zero new dependencies** anticipated — LANG-2 is orchestration over the
  LANG-1 engines + `resvg` (in tree).
- **Deterministic core, advisory AI** — every variety/loanword/areal operation is
  a pure, unit-tested function; AI only proposes. ([[feedback-ai-advisory]])
- **Book is system-of-record; extend in place** — `variety`/`contact` blocks +
  the existing links sidecar; no new system books.
- **Multilingual + stability bar** — per `[[feedback-multilingual]]` and
  `[[feedback-stability-standards]]` (no panics, atomic writes, poison recovery).
- **Test posture** — each phase ships with deterministic unit tests (dialect
  derivation, loanword repair across strategies, areal overlay, ecology graph) and
  one live AI validation per AI increment, mirroring LANG-1.

---

**Recommended first cut:** P1 (variation core) — it is self-contained, reuses the
rewrite engine directly, and immediately gives authors dialects + registers, the
most-requested sociolinguistic feature. P2 (borrowing) is the natural second,
being equally deterministic and self-contained.
