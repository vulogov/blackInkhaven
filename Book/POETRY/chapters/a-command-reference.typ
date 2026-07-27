#import "../design.typ": *

#appendix(letter: "A", title: "The poetry workflow")

Every command, key, and script-word used in this book, in one place. The commands share
the shape `inkhaven poetry <action> [options]`; add `--help` to any of them for the full
signature. Throughout, `--language` (or `-l`) takes one of `en`, `ru`, `fr`, `de`, `es`
and defaults to `en`.

#section("Forms — declaring intent")

/ `poetry forms`: List the eighteen built-in forms with one-line descriptions.
/ `poetry forms --form <name> [--language L]`: Print a form's `poem:` block, localised to the language (Russian adds `allow_pyrrhic` / `require_final_stress`, and so on).
/ `poetry forms --new [--name M]`: Scaffold a `form: custom` block to edit and paste onto a poem or into `.inkhaven/custom-forms.hjson`.

#section("Measuring — sound, metre, rhyme")

/ `poetry syllabify <word> [--language L]`: Break a word into syllables (cuts from #emph[hypher], count from vowel nuclei), marking the stressed syllable with `ˈ`.
/ `poetry syllabify --line "…" [--language L]`: The same, word by word, over a whole line.
/ `poetry metre --line "…" [--language L]`: Scan a line into `/` (stressed) `×` (unstressed) `·` (flexible) marks, name the detected metre, and report a conformance.
/ `poetry metre --line "…" --form <name> [--language L]`: Also check the line against the form's *declared* metre — syllable count, fit, and any feminine-ending or catalectic tag.
/ `poetry rhyme <w1> <w2> [--language L]`: Classify a rhyme — quality (perfect / near / eye / none), type (masculine / feminine / dactylic), and the shared tail; normalised by the language's own rules (German devoicing, French mute-e, Russian akanye).

#section("English pronouncing dictionary — exact scansion")

/ `poetry phonemes import <cmudict-file>`: Install a CMUdict-format dictionary so English syllables, stress, and rhyme are read from a word's actual phonemes (e.g. `love`/`move` → *eye* rhyme, not a false perfect). English-only; other languages are near-phonemic already. Out-of-dictionary words fall back to the spelling heuristic.
/ `poetry phonemes lookup <word>`: Show a word's syllables, stress, and rhyme tail.
/ `poetry phonemes status`: Whether a dictionary is installed and how many words it holds.

#section("The Inner Poet — reading a stanza")

/ `poetry scan --text "…" --form <name> [--language L]`: The fast track — findings over every line, each with a severity (Praise / Note / Concern), kind (Metre / Rhyme), and message.
/ `poetry scan … --json`: The same findings as a structure, with a top-level `concerns` count.
/ `poetry scan … --fail-on-concern`: Exit non-zero if any Concern is present — a CI gate. Opt-in; stops only for Concerns.
/ `poetry status --text "…" --form <name> [--language L] [--json]`: Completion — the `written / expected` line count, the `drafting` / `complete` state, and any structural issues (a villanelle's refrains, a sonnet's volta).

#section("Translation")

/ `poetry trilemma --source "…" --translation "…" [--form <name>] [--language L] [--to-language L2]`: Score a verse translation on *form* (metre + rhyme fidelity) and *sound* (alliteration/repetition texture); the *meaning* axis is left blank — engage the Inner Poet's slow track for it.

#section("In the editor")

/ `Ctrl+B J`: Open the Inner-reader overview.
/ `P` (in the overview): Open the Inner Poet.
/ `F` (Inner Poet): Fast check the poem paragraph under the cursor — findings to the Output pane (offline, instant).
/ `E` (Inner Poet): Engage the slow track — a bounded LLM reading, returned as a thought. Advisory only; never edits the poem.

Poems live as paragraphs of the `para:verse-*` family — `verse-line` (‖), `verse-stanza`
(♩), `verse-couplet` (‗), `verse-tercet` (⁚), `verse-quatrain` (⁛), `verse-translation`
(⇄) — whose glyphs show a poem's shape in the Tree and Outline panes.

#section("In Bund")

All read-only (`store_read`); each takes its arguments off the stack and pushes a value or
dictionary. Every `ink.poem.X` also answers to the shorter `poem.X`.

/ `<word> <lang> ink.poem.syllable_count`: → the syllable count (an integer).
/ `<line> <lang> ink.poem.scan_line`: → `{ pattern, syllables, metre }`.
/ `<w1> <w2> <lang> ink.poem.rhyme`: → `{ quality, type, shared }`.
/ `<text> <form> <lang> ink.poem.status`: → `{ complete, lines, expected, issues }`.

Run a one-off with `inkhaven bund '<code>'`; add `--help` to any command in this book for
its full option list.
