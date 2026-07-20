#import "../design.typ": *

#appendix(letter: "A", title: "Command reference")

Every conlang command, grouped by what it does. Most follow the shape
`inkhaven language <action> <language> [options]`.
A few order their arguments differently — `cross <from> <to>`,
`link-place <place> <lang>`, `reconstruct --forms` — and the project-wide
`list`, `family-tree`, and `ecology` take no language at all.
Add `--help` to any command for its full signature.

#section("Setup")

/ `init <name>`: Create a language sub-book with its five chapters.
/ `scaffold --from "<description>" [--out F]`: (AI) Propose a starter phoneme inventory from a short description, as pasteable Phonology-chapter HJSON (preview-only).
/ `list`: List every defined language with summary counts.
/ `doctor <lang> [--json]`: A health check — phonotactic and lexical problems plus how much of your manuscript's language-words the lexicon already covers.

#section("Phonology")

/ `generate-word <lang> --role root --count N`: Invent word-shapes that obey your templates and constraints.
/ `syllabify <lang> --word W`: Show how a word divides into syllables.
/ `ipa <lang> --word W`: Show a word's *surface* pronunciation after allophony.
/ `stress <lang> --word W`: Mark which syllable carries stress.
/ `romanize <lang> --text T [--reverse] [--scheme S]`: Convert between spelling and sounds.
/ `tone <lang> --tones "3 3 3"`: Apply tone-sandhi rules to a tone sequence.

#section("Lexicon")

/ `add-word <lang> <word> --type POS --translation M`: Add a dictionary entry (or `--import file.csv`).
/ `remove-word <lang> <word>`: Delete an entry.
/ `query <lang> [--pos] [--register] [--domain] [--era] [--text] [--json]`: Search the lexicon.
/ `generate-lexicon <lang> --topic T --count N [--semantic] [--yes]`: AI-generate themed words behind the dedup gate.
/ `audit <lang> [--json]`: Report phonotactic violations, homophones, duplicate meanings.
/ `scan-manuscript <lang> [--json]`: Find language-like words in your prose that are not yet defined.
/ `stats <lang> [--json]`: A descriptive profile of the language's sounds and words.
/ `gaps <lang> [--scope swadesh_100|file.hjson] [--json]`: Report which reference concepts the lexicon is still missing.
/ `compose <lang> --kind names|prose|poem|blessing|curse|incantation [--count N] [--meter 5,7,5] [--seed N] [--provider P]`: Generate creative text grounded in your language.
/ `import <lang> --file F --format toolbox|polyglot [--yes]`: Import a lexicon from another tool (previews without `--yes`).
/ `export <lang> --format json|csv|anki|xliff|linguex|ipa-chart|dictionary-twocol|grammar|phrasebook [--out F]`: Export the lexicon to a portable or typeset format.

#section("Grammar")

/ `paradigm <lang> --root R --template T --gloss G`: Generate a word's inflected forms.
/ `agree <lang> --word W --pos P --features "number=pl,case=nom" [--gloss G]`: Inflect a dependent word to agree with its head's features.
/ `gloss <lang> --text "…"`: Interlinear (word-by-word) gloss of a sentence.
/ `igt <lang> --text "…" [--save] [--name N] [--json]`: Interlinear glossed text — auto-gloss a sentence and lay it out as an aligned Leipzig block (the morpheme-segmented sentence, the gloss, a literal translation). `--save` stores it in the language's `Texts` chapter; `--json` emits the structured IGT.
/ `texts <lang> [--name N] [--set-translation "…"] [--format text|latex] [--json]`: The language's stored interlinear texts. With no `--name`, list them; with `--name`, print that one. `--set-translation` replaces the named text's free translation (curating the auto literal one; the gloss is untouched). `--format latex` emits a `linguex` LaTeX document (`\gll` / `\glt`) of the selected text or all — ready to paste into a grammar sketch or paper.
/ `frequency <lang> [--lemma] [--source texts|prose|all] [--top N] [--json]`: A word-frequency list and descriptive statistics (tokens, types, type–token ratio, the Zipf fit) over the corpus. `--lemma` counts a root's inflected forms together.
/ `concordance <lang> --word W [--lemma] [--source texts|prose|all] [--window N] [--json]`: A KWIC concordance — every occurrence of a word across the corpus, with its context. `--lemma` matches a root's inflected forms too.
/ `collocations <lang> --word W [--lemma] [--source texts|prose|all] [--window N] [--top N] [--json]`: The collocates of a word — the neighbours within its window, ranked by co-occurrence and scored by PMI (a distinctive collocate outranks a merely-frequent word).

The corpus `--source` selects what the three commands read: `texts` (the stored interlinears), `prose` (the conlang words used in your manuscript, detected as `scan-manuscript` does), or `all` (both, the default).
/ `parse <lang> --word W [--json]`: Analyse a surface word into root + affixes (the morphological parser — the inverse of paradigm generation).
/ `link <lang> --verb V --args "a,b,c" [--valence V] [--json]`: Work out a clause's thematic roles, RRG macroroles (actor / undergoer) and grammatical relations from the verb's valence.
/ `tree <lang> --verb V --args "subj,obj,iobj" [--word-order O] [--json]`: Build the X-bar phrase-structure tree (CP → TP → VP) of a clause, placing heads and complements by word order.
/ `movement <lang> --verb V --args "…" --move ROLE [--word-order O] [--json]`: Front a constituent (wh-movement / topicalisation) over the tree, leaving a coindexed trace.
/ `binding <lang> --verb V --args "…" [--antecedent R] [--anaphor R] [--type reflexive|pronoun|name] [--json]`: Decide whether one argument may refer to another, by c-command and the binding principles.
/ `derive <lang> --root R --gloss G --pos P [--yes]`: Coin derived words from a root.
/ `grammar <lang> [--set feature=value]`: View or set the typology questionnaire.
/ `define-rule <lang>`: Open your `$EDITOR` to hand-author a grammar or phonology rule — the direct authoring path when a rule is easier written than configured.
/ `sentence <lang> --subject W --verb W --object W [--*-adj W] [--*-number N] [--negate --negator W] [--question --q-particle W]`: Assemble a clause — order, case, agreement, optional negation / yes-no question — with an interlinear gloss.
/ `relative <lang> --head H --role subject|object --verb V [--with O] [--relativizer W]`: Build a noun phrase modified by a relative clause.
/ `coordinate <lang> [--np W …] [--clause "subj verb obj" …] --conjunction W`: Join nouns or clauses with a conjunction.
/ `complement <lang> --subject S --verb V [--complementizer W] --comp-subject S2 --comp-verb V2 [--comp-object O2]`: Make a clause the object of a matrix verb ("I know that …").
/ `idiom-add <lang> --form F --literal L --meaning M [--register R]`: Record an idiom.
/ `metaphor-add <lang> --source S --target T [--example E]`: Record a conceptual metaphor.
/ `idioms <lang>`: List recorded idioms and metaphors.

#section("Diachronics")

/ `sound-change <lang> --form W`: Evolve one proto-form through the language's sound-change chain.
/ `derive-lexicon <lang> [--yes]`: Evolve the proto's whole dictionary into this daughter.
/ `family-tree`: Draw the genealogical tree of all languages.
/ `cognates <proto> --form W`: Trace a proto-form's reflex in every daughter.
/ `reconstruct --forms "…" [--gloss G]`: (AI) Propose a proto-form from cognates.
/ `realism-check <lang>`: (AI) Assess whether the sound-change chain is plausible.
/ `trace <lang> --rule "s > ʃ / _ i" [--limit N] [--json]`: Preview a pending sound change across the lexicon — which words shift, which merge into new homophones — without committing it.

#section("Analysis")

/ `metrics <lang> [--json]`: Quantitative sound-system metrics — phoneme entropy, the Zipf fit, phonotactic saturation, mora weight.
/ `naturalness <lang> [--json]`: Judge the phoneme inventory against cross-linguistic tendencies (voicing symmetry, place coverage, near-universals, size) into a 0–1 score.
/ `suggest-phonemes <lang> [--json]`: Recommend phonemes that would round out the inventory (voiced counterparts, missing near-universals). Advisory.
/ `pairs <lang> [--limit N] [--json]`: Find minimal pairs and the distinctive feature each turns on — the functional load of your contrasts.
/ `harmony <lang> [--json]`: Detect vowel harmony (backness, rounding) by how consistently a word's vowels agree.
/ `distribution <lang> [--json]`: Where each phoneme appears (onset / nucleus / coda, word edges) and any restricted distributions.
/ `universals <lang> [--json]`: Check the grammar's head-directionality harmony and the classic implicational universals (Greenberg/Dryer).
/ `grammar-check <lang> [--json]`: Validate the typed grammar blocks (`ug_parameters`, `verb_classes`) and their consistency with the WALS feature answers.
/ `check <lang> --word W [--json]`: The Oracle — judge a candidate word for well-formedness by level (phonotactics, morphology).
/ `sketch <lang> [--out F]`: A one-page prose overview of the language, assembling all of the above.

#section("Morphology and syntax")

/ `parse <lang> --word W [--json]`: The morphological parser — analyse a surface word into root + affixes by stripping known affixes (concatenative, full and partial reduplication, and ablaut) until what remains is a dictionary root. The inverse of paradigm generation.
/ `link <lang> --verb V --args "A,B,C" [--valence …] [--json]`: Argument linking — a clause's thematic roles, RRG macroroles (actor / undergoer) and grammatical relations from the verb's valence.
/ `tree <lang> --verb V --args "subj,obj,iobj" [--word-order O] [--json]`: Build the X-bar phrase-structure tree (CP → TP → VP), placing heads and complements by the language's word order.
/ `movement <lang> --verb V --args "…" --move ROLE [--word-order O] [--json]`: Syntactic movement — front a constituent (wh-movement / topicalisation) over the tree, leaving a coindexed trace.
/ `binding <lang> --verb V --args "…" [--antecedent R] [--anaphor R] [--type reflexive|pronoun|name] [--json]`: Binding theory — decide whether one argument may refer to another, by c-command and Principles A / B / C.
/ `check-clause <lang> --verb V --args "…" [--verb-root R] [--subject-features "number=pl,…"] [--valence …] [--json]`: The Oracle over a clause (levels 3–4) — subject–verb agreement (does the verb inflect for the subject's features?) and argument structure (does the argument count match the verb's valence?).
/ `check-agreement <lang> --dependent D --form W --root R --head-features "number=pl,gender=fem" [--json]`: The Oracle's agreement check over any head–dependent pair — does a dependent word (adjective, determiner, verb) correctly inflect for its head's features, under the declared agreement rule?
/ `trace <lang> --rule "X > Y / A _ B" [--limit N] [--json]`: The Consequence Tracer — preview a sound change across the lexicon (which words shift, which distinctions merge, which new homophones appear) without committing it.

#section("The Linguistic companion")

/ `inkhaven linguistic [--language L] [--session S]`: Open the full-screen Linguistic companion over the `Language` book — the tree, a grounded chat with the Inner Linguist, and the phonology / universals / minimal-pair views. Not a `language` subcommand; run it directly.
/ Chat slash-commands: `/trace <rule>` previews a sound change, `/parse <word>` analyses a surface form, `/check <word>` runs the word Oracle, `/tree <verb> <subject> [object] [indirect]` builds the X-bar tree, `/clause <verb> <subject> …` runs the clause Oracle's argument-structure check, `/igt <sentence>` glosses a sentence as interlinear text, `/texts` lists the stored texts, `/settrans <name> = <translation>` curates a stored text's free translation, `/frequency` reports corpus statistics, `/kwic <word>` is a concordance over the stored texts, and `/coll <word>` lists a word's collocates. Most run locally over the current language book and print inline; only `/settrans` writes (the text's translation).

#section("Sociolinguistics and contact")

/ `varieties <lang>`: List the language's dialects, registers, and sociolects with their deltas.
/ `lect <lang> <variety> --word W | --text "…"`: Render a form or text in a chosen variety.
/ `dialects <lang> [--count N]`: Print the dialectology comparison table (a `*` marks a word override).
/ `borrow <recipient> --form W --from L [--gloss G] [--yes]`: Adapt a donor word to the recipient's sounds (add it with `--yes`).
/ `areal [lang]`: Show one language's convergence overlay, or (no language) the whole-region Sprachbund view.
/ `propose-dialect <lang> --describe "…" [--id N] [--provider P] [--yes]`: (AI) Suggest a coherent dialect; rules are validated and previewed.
/ `propose-loans <recipient> --from L --topic T --count N [--provider P]`: (AI) Propose borrowings, each nativised by the adapter.
/ `areal-check <lang>`: (AI) Judge whether a declared Sprachbund is typologically plausible.
/ `ecology [--svg F]`: Report who speaks what variety where (or write a node-link atlas).
/ `idiolect <character> --word W | --text "…"`: Render a form or text in a character's native variety.

#section("Translation")

A rule-based translation engine that runs on the phonology, lexicon, and grammar
you built — with a growing memory of the sentences you approve.

/ `translate <lang> --text "…"`: Translate English into your language, using its lexicon and grammar.
/ `reverse <lang> --text "…"`: Translate a sentence in your language back into English.
/ `cross <from> <to> --text "…"`: Translate between two of your own languages.
/ `remember <lang> --source "…" --target "…"`: Add an approved sentence pair to the translation memory.
/ `memory <lang> [--json]`: Show the translation memory — the approved pairs the engine reuses.
/ `corpus <lang> [--json]`: Show the parallel corpus gathered from your Sample-texts chapter.
/ `eval <lang> [--json]`: Score the engine against the corpus — how much it gets right, and where it fails.
/ `export-translation <lang> --out F`: Write an `.itm` translation-memory pack for backup or interchange.

#section("Writing systems")

/ `glyph-lint --svg F`: Check whether an SVG is suitable as a font glyph.
/ `glyph-draft <lang> --describe "…" --phoneme P [--codepoint C] [--out F] [--yes]`: (AI) Draft a glyph from a description.
/ `font-import-glyph <lang> --svg F --phoneme P [--codepoint C] [--name N]`: Bind a glyph into the font.
/ `font-config <lang> [--json]`: List the font's glyph bindings and artwork status.
/ `font-build --language <lang> --format ufo|ttf|both [--out F] [--upm N]`: Compile the font.
/ `font-templates <lang>`: List spatial templates for composed blocks.
/ `font-compose <lang> --template T --name N --slot SLOT=GLYPH … [--yes]`: Bake a composed block glyph.
/ `spatial-typst <lang> --template T --name N --slot SLOT=GLYPH …`: Emit a layout-time Typst quadrat.
/ `transliterate <lang> --text "…" [--json]`: Type romanized text into the script's codepoints.

#section("Output")

/ `dictionary <lang> --format md|typ [--out F] [--font Fam]`: Render the dictionary.
/ `grammar-book <lang> --format md|typ [--out F] [--font Fam] [--study --provider P]`: Render the reference grammar (with optional AI study guide).
/ `tutorial <lang> --format md|typ [--out F] [--font Fam] [--provider P]`: (AI) Render a learner's textbook.

#section("Worldbuilding links")

/ `link-place <place> <lang> [--secondary] [--variety V]`: Link a place to a language (and the variety) spoken there.
/ `link-character <char> <lang> <level> [--native-variety V]`: Record a character's fluency, and their native variety.
/ `speakers <lang>`: List who speaks a language.
