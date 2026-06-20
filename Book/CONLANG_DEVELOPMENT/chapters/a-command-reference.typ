#import "../design.typ": *

#appendix(letter: "A", title: "Command reference")

Every conlang command, grouped by what it does. All take the form `inkhaven
language <action> <language> [options]`. Add `--help` to any for full details.

#section("Setup")

/ `init <name>`: Create a language sub-book with its five chapters.
/ `list`: List every defined language with summary counts.

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

#section("Grammar")

/ `paradigm <lang> --root R --template T --gloss G`: Generate a word's inflected forms.
/ `agree <lang> --word W --pos P --features "number=pl,case=nom" [--gloss G]`: Inflect a dependent word to agree with its head's features.
/ `gloss <lang> --text "…"`: Interlinear (word-by-word) gloss of a sentence.
/ `derive <lang> --root R --gloss G --pos P [--yes]`: Coin derived words from a root.
/ `grammar <lang> [--set feature=value]`: View or set the typology questionnaire.
/ `sentence <lang> --subject W --verb W --object W [--*-adj W] [--*-number N]`: Assemble a clause — order, case, and agreement — with an interlinear gloss.
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

/ `link-place <place> <lang> [--secondary]`: Link a place to a language spoken there.
/ `link-character <char> <lang> <level>`: Record a character's fluency in a language.
/ `speakers <lang>`: List who speaks a language.
