#import "../design.typ": *

#appendix(letter: "A", title: "The research workflow")

Every command used in this book, grouped by the stage of study it belongs to. Most
follow the shape `inkhaven language <action> <language> [options]`; add `--help` to any
command for its full signature. Throughout, `<lang>` is the language you are studying —
`Russian`, in this book.

#section("Modelling the language")

/ `init <name>`: Create the language and its (empty) chapters.
/ `stats <lang> [--json]`: A descriptive profile — inventory size, average word shape.
/ `add-word <lang> <word> --type POS --translation M`: Add a lexicon entry (or `--import file.csv`).
/ `query <lang> [--pos] [--text] [--json]`: Search the lexicon.

The phoneme inventory and morphology are declared as HJSON blocks in the language's
Phonology and Morphology chapters (Chapters 2 and 4); see the companion book's HJSON
reference for the full schema.

#section("The sound system")

/ `metrics <lang> [--json]`: Quantitative sound-system metrics — entropy, the Zipf fit, phonotactic saturation, mora weight.
/ `pairs <lang> [--limit N] [--json]`: Minimal pairs and the feature each turns on (feature naming needs an IPA-based model — Chapter 3).
/ `naturalness <lang> [--json]`: Score the inventory against cross-linguistic tendencies (IPA-based; see Chapter 3).
/ `distribution <lang> [--json]`: Where each segment appears (onset / nucleus / coda, word edges) and any restricted distributions.

#section("Words and sentences")

/ `paradigm <lang> --root R --template T --gloss G`: Generate a word's inflected forms from the declared endings.
/ `parse <lang> --word W [--json]`: Analyse a surface form into root + affixes (the inverse of `paradigm`).
/ `igt <lang> --text "…" [--save --name N] [--json]`: Interlinear glossed text — the segmented sentence, its gloss, a literal translation; `--save` keeps it in the `Texts` chapter.
/ `check-agreement <lang> --dependent D --form W --root R --head-features "…"`: Does a dependent (adjective, …) agree with its head's features?
/ `tree <lang> --verb V --args "subj,obj" [--word-order O] [--json]`: The X-bar phrase-structure tree of a clause.
/ `movement <lang> --verb V --args "…" --move ROLE`: Front a constituent, leaving a coindexed trace.
/ `check-clause <lang> --verb V --args "…" [--subject-features "…"]`: The clause Oracle — argument structure and subject–verb agreement.

#section("The corpus")

/ `frequency <lang> [--source texts|prose|all] [--lemma] [--top N] [--json]`: Word-frequency list and statistics (tokens, types, TTR, Zipf) over the corpus.
/ `concordance <lang> --word W [--source …] [--lemma] [--window N] [--json]`: A KWIC concordance — every occurrence of a word with its context.
/ `collocations <lang> --word W [--source …] [--lemma] [--window N] [--json]`: A word's collocates, ranked by co-occurrence and PMI.
/ `texts <lang> [--name N] [--set-translation "…"] [--format text|latex]`: List, print, curate or export the stored interlinear texts.

#section("Typology and history")

/ `grammar <lang> [--set feature=value]`: View or set the typological profile.
/ `universals <lang> [--json]`: Check the profile against head-directionality harmony and the Greenberg/Dryer implicational universals.
/ `hypothesize <lang> --kind sound-change|cognacy|borrowing --claim "…" [--evidence "…"] [--id N]`: Record a diachronic/comparative hypothesis.
/ `hypotheses <lang> [--status S] [--json]`: List the hypothesis register.
/ `hypothesis-check <lang> --id N`: Run the Consequence Tracer over a sound-change hypothesis's claim.
/ `hypothesis-status <lang> --id N --status S`: Move a hypothesis along as the evidence comes in.
/ `cognates <proto> --form W` · `reconstruct --forms "…"`: Trace a proto-form's reflexes, or reconstruct an ancestor from a cognate set.

#section("The companion")

/ `inkhaven linguistic [--language L]`: Open the full-screen Linguistic companion over your languages — a grounded chat plus the slash-commands `/parse`, `/tree`, `/clause`, `/igt`, `/frequency`, `/kwic`, `/coll`, `/hypotheses`, `/hcheck`, each running the tool above inline.
