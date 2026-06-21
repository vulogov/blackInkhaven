#import "../design.typ": *

#appendix(letter: "C", title: "Bund API reference")

Every conlang word reachable from *Bund* (Chapter 26), grouped by what it does.
Each is listed by its short `lang.` name; the full form `ink.lang.<name>` works
identically. The notation in parentheses is the *stack effect* — the inputs taken
from the stack, then `--`, then the outputs left behind:

```bund
"Eldar" "tap" lang.ipa        // ( lang word -- surface ) → "tav"
```

Where an output is written `{ … }` it is a Bund dictionary whose listed fields a
script can read directly; a *list* is a sequence you can loop over. A trailing
`provider` argument on the AI words names a non-default model (an empty string
uses the configured default).

#section("The safety gate")

Words carry a *category* that decides whether a script may run them. Inspectors
(`store_read`) are always allowed. The rest are off until you opt in, naming the
categories a script may use in `inkhaven.hjson`:

```hjson
scripting: { enabled_categories: ["store_write", "ai_write", "fs_write"] }
```

The categories are `store_read` (read the project — default on), `store_write`
(change the project), `ai_write` (call a language model), and `fs_read` /
`fs_write` (read or write files, always inside the project sandbox).

#section("Inspectors — sounds and words")

These read the language and return a value; all are `store_read` (always allowed).

/ `lang.list ( -- names )`: Every defined language in the project.
/ `lang.generate_word ( lang role seed -- word )`: Invent a word-shape obeying the templates (a `seed` makes it repeatable).
/ `lang.syllabify ( lang word -- list )`: Split a word into its syllables.
/ `lang.ipa ( lang word -- surface )`: A word's surface pronunciation after allophony.
/ `lang.stress ( lang word -- marked )`: Mark which syllable carries stress.
/ `lang.tone ( lang tones -- result )`: Apply tone sandhi to a tone sequence (`"3 3 3"` → `"2 2 3"`).
/ `lang.transliterate ( lang text -- script )`: Convert romanized text into the script's codepoints.
/ `lang.gloss ( lang text -- gloss )`: Interlinear (word-by-word) gloss of a sentence.
/ `lang.query ( lang text -- entries )`: Search the lexicon; `text` filters by gloss or headword.

#section("Inspectors — grammar and sentences")

/ `lang.paradigm ( lang root template gloss -- rows )`: All inflected forms of a word.
/ `lang.derive ( lang root gloss pos -- forms )`: Productive derivations of a word (does not commit them).
/ `lang.agree ( lang word pos features -- form )`: Inflect a word to agree with given features (`"number=pl,case=dat"`).
/ `lang.sentence ( lang subject verb object -- {surface,gloss,literal} )`: Assemble a subject–verb–object clause.
/ `lang.relative ( lang head role verb with relativizer -- dict )`: Build a relative-clause construction.
/ `lang.complement ( lang subject verb complementizer comp-subject comp-verb comp-object -- dict )`: Build a complement-clause sentence.
/ `lang.coordinate ( lang clause-list conjunction -- dict )`: Join clauses (each `"subj verb [obj]"`) with a conjunction.

#section("Inspectors — analysis and history")

/ `lang.stats ( lang -- profile )`: A descriptive profile of the language's sounds and words.
/ `lang.audit ( lang -- report )`: Phonotactic violations, homophones, and duplicate meanings.
/ `lang.gaps ( lang scope -- report )`: Reference concepts the lexicon still lacks (`scope` = `"swadesh_100"` or a file path).
/ `lang.sound_change ( lang form -- evolved )`: Evolve a proto-form through the daughter's sound-change chain.
/ `lang.cognates ( proto form -- reflexes )`: A proto-form's reflex in every daughter language.
/ `lang.family_tree ( -- tree )`: The genealogical tree of all languages.

#section("Inspectors — creative text")

/ `lang.names ( lang count seed -- list )`: Generate names grounded in the language.
/ `lang.prose ( lang count seed -- sentences )`: Generate deterministic prose clauses.
/ `lang.poem ( lang meter seed -- lines )`: Generate verse to a meter (`"5,7,5"`).

#section("Inspectors — sociolinguistics")

/ `lang.varieties ( lang -- list )`: The dialects and registers, each with its delta sizes.
/ `lang.lect ( lang variety word -- rendered )`: Render a form in a chosen variety.
/ `lang.idiolect ( character word -- rendered )`: Render a form in a character's native variety.
/ `lang.borrow ( recipient donor-form -- {donor,adapted,steps} )`: Nativise a loanword (advisory; commit with `add_word`).
/ `lang.areal ( lang -- {region,with,convergence} )`: A language's areal-convergence overlay.
/ `lang.ecology ( -- {places,characters} )`: The whole speech-community picture.

#section("Data constructor")

Uncategorised (always allowed) — it builds a value, touching nothing.

/ `lang.dict ( [k v k v …] -- dict )`: Turn a flat list into a dictionary. Also answers to `word`, `rule`, `phoneme`, and `block`, so a script reads like the artefact it builds.

#section("Mutators")

These change the project; all are `store_write` (enable `"store_write"`).

/ `lang.init ( name -- )`: Create a language and its five chapters.
/ `lang.define ( lang chapter block -- )`: Write an HJSON `block` as a paragraph under a chapter (`Phonology`, `Grammar`, `Sample texts`, `Meta`).
/ `lang.add_word ( lang word pos translation -- )`: Add a dictionary entry.
/ `lang.remove_word ( lang word -- )`: Delete a dictionary entry.
/ `lang.derive_add ( lang root gloss pos -- count )`: Derive *and* commit forms to the lexicon; leaves the number added.
/ `lang.grammar_set ( lang feature value -- )`: Set one typology answer.
/ `lang.idiom_add ( lang form literal meaning -- )`: Add an idiom.
/ `lang.metaphor_add ( lang source target -- )`: Add a conceptual metaphor.

#section("AI-backed words")

These call a language model; all are `ai_write` (enable `"ai_write"`). They are
*advisory* — they return data and never write the book, so a script commits what
it likes (typically through `add_word`). The trailing `provider` is empty for the
default.

/ `lang.compose ( lang kind provider -- text )`: Themed text — `kind` is `blessing`, `curse`, or `incantation` — constrained to the lexicon.
/ `lang.reconstruct ( forms gloss provider -- text )`: Propose a proto-form from cognate forms.
/ `lang.realism_check ( lang provider -- text )`: Assess whether the sound-change chain is plausible.
/ `lang.generate_lexicon ( lang topic count provider -- words )`: Themed words — forms from the deterministic generator, meanings from the AI, behind the dedup gate. Returns survivor `{ word, gloss, pos }` dicts.

#section("File output")

These read or write files (always inside the project sandbox).

/ `lang.glyph_lint ( svg-path -- report )`: Check whether an SVG is usable as a glyph. (`fs_read`)
/ `lang.dictionary ( lang format out font -- out-path )`: Render the dictionary (`format` = `md` or `typ`; empty `font` uses the configured one) to a file. (`fs_write`)
/ `lang.grammar_book ( lang format out font -- out-path )`: Render the reference grammar to a file. (`fs_write`)
/ `lang.font_build ( lang format out -- out-stem )`: Compile the font (`format` = `ufo`, `ttf`, or `both`). (`fs_write`)
/ `lang.glyph_draft ( lang describe phoneme out provider -- out-path )`: AI-draft a glyph SVG, preflight it, and write it (advisory). (`fs_write`, calls the AI)
