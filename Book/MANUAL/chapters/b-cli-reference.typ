#import "../design.typ": *

#appendix(letter: "B", title: "The Command Line")

Everything Inkhaven does in the terminal is one binary: `inkhaven`. Run it
bare and it opens the full-screen editor on the project in the current
directory. Give it a subcommand and it runs *headless* — it does one job,
prints to standard output, exits with a status code, and never draws a
screen. That headless surface is what this appendix catalogues: every
subcommand, grouped by the work it does, with its important flags and a
one-line statement of purpose.

The headless commands exist so that everything the editor can do, a script,
a `Makefile`, or a continuous-integration gate can do too. Most of the
reading intelligences — continuity, dialogue, sources, terms, knowledge —
exit non-zero when they find a problem, which is exactly what a
pre-submission check wants.

#section("Invocation and global options")

The shape is always the same: the program name, optional global flags, a
subcommand, and that subcommand's own arguments.

#screen(caption: "The general form")[```
  inkhaven [--project <dir>] <subcommand> [args…]
  inkhaven                       # no subcommand → open the TUI
  inkhaven --version             # print the version and exit
  inkhaven <subcommand> --help   # the subcommand's own help
```]

Only one option is global — it applies to every subcommand:

#chord_table((
  ("--project <dir>", [The project root to act on. Aliases: `-p` and the long
    `--project-directory`. For `init` this is the directory to create; for
    every other subcommand it defaults to the current working directory.]),
  ("--help / -h", [Print help. Given after a subcommand, prints that
    subcommand's arguments and flags — the authoritative, always-current
    reference for a single verb.]),
  ("--version", [Print the binary version (the same string `doctor` reports)
    and exit.]),
))

#callout(label: "Convention in this appendix")[
  Flags shown in `<angle brackets>` take a value; flags shown bare are
  switches. A command that "exits non-zero on findings" is meant for a CI
  gate — pair it with `--json` and `jq` to fail a build. Because `-p` is
  reserved for `--project`, no subcommand offers `-p` for anything else;
  provider overrides are always the long `--provider`.
]

#section("Project and setup")

The lifecycle of a project: create it, grow its tree, keep it healthy, and
recover it when something goes wrong.

#screen(caption: "Creating and scaffolding")[```
inkhaven init <path> [--template <t>] [--force]
    Create a new project. Templates: empty (default), novel,
    nonfiction, rpg-sourcebook, technical, nanowrimo.
inkhaven template list
    List every project template with a one-line description.
inkhaven add <kind> <title> [--parent <p>] [--slug <s>]
                            [--after <sibling>]
    Add a node: book | chapter | subchapter | paragraph |
    script. --after inserts beside an existing sibling.
inkhaven list
    Print the whole hierarchy as an indented tree.
inkhaven outline [--filter <text>]
    Print the manuscript outline (the Ctrl+2 pane's twin).
```]

#screen(caption: "Reshaping the tree")[```
inkhaven delete <slug-path> [--yes]
    Delete a node and its descendants. Dry-runs without --yes.
inkhaven mv <slug-path> <up|down>
    Reorder a node among its siblings.
inkhaven paragraph copy <src> <dest>
    Duplicate a paragraph under another parent (fresh uuid).
inkhaven paragraph move <src> <dest>
    Relocate a paragraph under another parent.
inkhaven reindex [--prune] [--adopt]
    Re-index every .typ file from disk into the store.
    --prune drops rows whose file vanished; --adopt registers
    orphan files under the matching hierarchy branch.
```]

#screen(caption: "Health, backup, recovery")[```
inkhaven doctor [--scan] [--json] [--class <c>]
                [--autofix [--yes]] [--voices]
                [--tts-test <text>] [--filter-words-snippet]
    Health report: binary, project, actionable notes. --scan
    walks for zero-byte files, orphan rows, missing files,
    corrupt comment sidecars; --autofix repairs them (logged).
inkhaven stats [--book-name <b>]
    Per-paragraph table: status, word count, target %, mtime.
inkhaven backup [--out <dir>]
    Zip the project into a dated archive.
inkhaven restore <archive.zip> --to <fresh-dir>
    Restore a backup into a new directory.
inkhaven recover <crash-report.hjson> [--yes] [--keep]
    Walk a crash report's rescued buffers and apply them back
    to disk (a .pre-recover backup is written first).
```]

The two standalone TUI editors for a project's own settings live here as
well — they open their own screens rather than running headless:

#chord_table((
  ("config", [Open the schema-aware TUI editor for `inkhaven.hjson` (the
    `Ctrl+B 0` raw editor stays the power-user fallback).]),
  ("prompts-editor", [Open the four-pane TUI workbench for
    `prompts.hjson` — the project's prompt overrides.]),
  ("comments", [Headless view of the per-paragraph `.comments.json`
    sidecars: `list`, `resolve <id>`, `reopen <id>`, `delete <id>`,
    `export`. Mirrors the `Ctrl+V Shift+C` panel.]),
))

#section("Writing and search")

The everyday text operations — find things, change things, watch the prose,
track the daily count.

#screen(caption: "Finding and changing prose")[```
inkhaven search <query> [--limit <n>]
    Semantic search across the whole project (default 10 hits).
inkhaven book-rag retrieve <query> [--book-name <b>]
                  [--top-k <n>] [--context]
    Show the passages Book-scope chat would ground on — the
    exact retrieval, with no LLM call. --context prints the
    composed block the model would receive.
inkhaven replace <pattern> <replacement>
                 [--regex] [--substring] [--ignore-case]
                 [--book <b>] [--include-system]
                 [--dry-run | --yes]
    Project-wide find & replace. Literal whole-word by default;
    snapshots each touched paragraph. --dry-run previews; --yes
    applies. System books excluded unless --include-system.
```]

#screen(caption: "Watching the prose")[```
inkhaven style [--book-name <b>] [--language <l>] [--json]
    Run the deterministic style detectors (filter-words,
    repeated phrase, show-don't-tell, anachronism). CI/CLI
    parity for the Ctrl+V w overlay.
inkhaven goals
    Writing-goals report: word totals, today vs the daily goal,
    the streak, per-book pace and deadline. Read-only.
inkhaven snippets list [--json]
inkhaven snippets check [--book <b>] [--json]
    List reusable snippets + reference counts, or validate
    every #include against the Snippets book (exit 1 on a miss).
inkhaven wordnet fetch <lang…> | import <lang> <file>
              | lookup <word> [--lang <l>] | list
    The multilingual thesaurus: fetch/import WordNet data, then
    look a word up for senses, synonyms, antonyms, hypernyms.
```]

#section("The intelligences")

These are the readers that watch a whole manuscript for what a long book
gets wrong. Each has an in-editor chord; each also runs headless here. The
deterministic ones are free and exit non-zero on a finding; the ones that
call a model are gated behind an explicit `--deep`/`--slow`/`--coherence`
flag with an informative token cap.

#screen(caption: "The unified passes")[```
inkhaven check [--paragraph <id>] [--book-name <b>]
               [--no-fact] [--no-socrates] [--no-timeline]
               [--no-continuity] [--no-lector]
    The unified review pass: every fast deterministic checker
    over a scope, one consolidated summary. (Ctrl+B Shift+C.)
inkhaven edit [--json] [--only <cats>] [--book-name <b>]
              [--show-deferred] [--deep [--provider <p>]]
    The Editorial Pass: one ranked revision worklist unifying
    every detector. --deep refreshes the AI sidecars first.
inkhaven revise [--book-name <b>] [--json]
    REDLINE — the editorial letter: every reader's findings
    synthesised into one prioritized developmental letter.
    Advises; never rewrites. (Ctrl+V Shift+R.)
```]

#screen(caption: "Continuity and knowledge")[```
inkhaven continuity check [--only <d>] [--skip <d>] [--json]
                          [--coherence [--max-cost <n>]
                                       [--force]]
    SENTINEL — every continuity detector (co-location, timeline,
    numeric, char-facts, referenced-before-introduced), deduped
    and ranked. Exit non-zero on a Contradiction. (Ctrl+B Shift+I.)
inkhaven continuity extract [--provider <p>]
inkhaven continuity list
    Build / dump the AI continuity bible.
inkhaven knowledge [--book-name <b>] [--json]
                   [--deep [--max-cost <n>]]
    KEN — who knows what, when: premature_knowledge,
    leaked_secret, dropped_reveal. Deterministic; --deep adds
    the LLM implied-irony pass. (Ctrl+B Shift+Z.)
```]

#screen(caption: "Reading, voice, and dialogue")[```
inkhaven readthrough [--deep [--max-cost <n>] [--force]]
                     [--json]
    LECTOR — the book read forward once as a first reader: the
    intensity curve, scene/sequel beats, ranked findings
    (confusion, info-dump, dip, put-down). (Ctrl+B Shift+A.)
inkhaven chorus voices  [--book <b>] [--character <c>] [--json]
inkhaven chorus scan    [--book <b>] [--json]
inkhaven chorus report  [--book <b>] [--json]
inkhaven chorus stylist [--book <b>] [--coach]
                        [--suppress <k>] [--unsuppress <k>]
    CHORUS — voice at book scale: character fingerprints, POV /
    head-hop discipline, the book-scale report, and the Inner
    Stylist's synthesis. (Ctrl+B J → Y.)
inkhaven dialogue scan    [--book <b>] [--findings <f>] [--json]
inkhaven dialogue profile [--book <b>] [--character <c>] [--json]
inkhaven dialogue refresh [--book <b>] [--chapter <n>]
inkhaven dialogue suggest [--book <b>] [--chapter <n>]
    Dialogue quality: zero-attribution / said-bookism /
    talking-head findings + per-character fingerprints. scan
    exits non-zero on a bare span. (Ctrl+V Shift+Q.)
```]

#screen(caption: "Prose, drift, terms, chronicle, character")[```
inkhaven prose profile [--book <b>] [--deep] [--json]
                       [--language <l>]
inkhaven prose refresh | drift | suggest …
    NARR-1 — deterministic narrative-voice metrics per chapter
    (rhythm, diversity, hedging, interiority). (Ctrl+V V.)
inkhaven drift list [--entity <e>] [--json]
inkhaven drift scan [--provider <p>] [--json]
    WORLD-2 — descriptions of one entity that diverge across
    the book (list = deterministic; scan = the AI judgment).
inkhaven chronicle mark <label> [--ref <r>] [--book-name <b>]
inkhaven chronicle list | trend | diff <from> <to>  [--json]
    CHRONICLE — draft-history: stamp the readers' metrics as a
    milestone, then trend/diff them. (Ctrl+B Shift+U.)
inkhaven character arc <name> | check | refresh | plan
    CHAR-1 — the arc report, completeness checks, agency
    re-scoring, and Planning-Board coverage gaps.
```]

#section("The inner family")

The Inner readers question rather than correct: they ask, they never edit
your prose. Each keeps its findings and an *intent ledger* — the deliberate
choices it should stop flagging.

#screen(caption: "Socrates and the Editor")[```
inkhaven inner-socrates check [--text <t> | --paragraph <id>
                              | --path <p>] [--slow
                              [--max-cost <n>] [--force]]
    The Dialectician's fast track over prose (--slow adds the
    LLM Socratic pass). (Ctrl+B J.)
inkhaven inner-socrates timeline | findings | ledger
                        | persona … | suggestions … | bundle …
    The timeline pass, persisted findings, the intent ledger,
    reader personas, promotion candidates, and .isl bundles.
inkhaven inner-editor engage [--text <t> | --paragraph <id>]
                             [--force]
    One Inner Editor pass over a paragraph. (Ctrl+V O.)
inkhaven inner-editor findings | intent <cat> | suggestions …
                     | config show | usage
    Findings, declaring a category deliberate, and cost.
```]

#screen(caption: "Theologian, Rigor, Poet, cockpit")[```
inkhaven theologian scan    [--book <b>] [--signal <s>] [--json]
inkhaven theologian session [--book <b>] [--chapter <n>]
                            [--category <1-6>] [--lens <code>]
inkhaven theologian suppress --para <p> --reason <r>
    The moral/theological reader: the fast ethical-signal
    detector + the slow LLM session. It asks, never judges.
inkhaven rigor scan [--book <b>] [--signal <s>] [--strict] [--json]
    The reasoning-rigor reader: false dichotomy, question-
    begging, straw man, overgeneralization, non-sequitur. Zero-AI.
inkhaven poetry forms | syllabify | metre | rhyme | scan
             | status | trilemma | phonemes …
    The Inner Poet: measure verse (metre, rhyme, syllables,
    forms) — never generate it. (Ctrl+B J → P.)
inkhaven companions
    The examined-authorship cockpit: findings across the whole
    inner family, the intent ledger, today's cost per companion.
```]

#section("The world and the graph")

The layer beneath the prose: the simulated world, the knowledge graph, the
timeline, and the fact-checker that holds the prose to all three.

#screen(caption: "The world simulation")[```
inkhaven realworld new <name> [--force]
inkhaven realworld validate | show [--json]
inkhaven realworld compile [--layer <l>] [--json] [--materialize]
    Scaffold / validate / compile world.hjson. Layers: all,
    astronomy, geology, climate, hydrology, demographics.
inkhaven realworld propose | propose-myth | propose-rulers
                  | propose-language | proposals <list|accept|…>
    The world proposes Places, myths, rulers, languages; you
    work the queue — nothing commits until you accept.
inkhaven realworld travel --from <a> --to <b> --days <d>
                          [--mode foot|horse|cart|ship]
inkhaven realworld scene | weather | gazetteer | history
                  | calendar | trade | name | culture | ecology
    Derived world reference: is a journey plausible, the local
    season, a scene brief, the gazetteer, the chronology.
inkhaven realworld critique [--write-notes] [--lints-only]
inkhaven realworld map [--spec-only] [--no-ingest]
    The AI world critique and the plakat map render.
```]

The `worldbuilder` command opens the interactive TUI front-end to that
pipeline; `world` is its read-only snapshot:

#screen(caption: "Worldbuilder, snapshot, graph, timeline")[```
inkhaven worldbuilder [--session <s>] [--interview] [--from-map]
    The interactive worldbuilder TUI — a front-end to realworld.
inkhaven world [--json] [--deep [--provider <p>]] [--entity <e>]
    A world-consistency snapshot: facts, contradictions, drift,
    continuity coverage, anachronisms. Sub-checks: utopia-check,
    utopia-model, utopia-suppress, utopia-refresh (coherence).
inkhaven graph stats | rebuild | neighbors <n> | paths <a> <b>
             | contradicting <n> | loci <n> | pending
             | promote <e> | dismiss <e> | link <n> | ask <q>
             | lexical
    SEMNET — the typed-edge knowledge graph: inspect it, walk
    it, triage advisory edges, or ask it a question. (Ctrl+B z.)
inkhaven event add <title> --start <t> [--end <t>] [--track <k>]
inkhaven event list | show <path> | critique [--track <k>]
inkhaven event link-character <path> <name>
inkhaven event link-place <path> <name>
    Story-timeline events (needs timeline.enabled). link-character
    / link-place attach an explicit participant (feeds KEN presence).
inkhaven fact-check [--text <t> | --paragraph <id>] [--slow]
                    [--timeline-aware auto|on|off] [--timeline-only]
    Check prose against the simulated world; respects the
    magic: ledger. (Ctrl+B Shift+X.)
```]

The AI world scans that populate the `edit`/`world` sidecars also live as
their own verbs: `facts scan|check|extract|import|init|list`,
`tension scan|list`, and `drift scan|list` (above).

#section("Language")

The constructed-language suite is Inkhaven's single largest command family —
over a hundred verbs under `language`, all headless twins of the interactive
`linguistic` companion. The full treatment is in the LINGUISTIC companion
book; the groups below are the map.

#screen(caption: "The linguistic companion and its verbs")[```
inkhaven linguistic [--language <name>] [--session <s>]
    Launch the interactive linguistics TUI (dev/verify/
    analyze/research over the Language system book). Ctrl+B X.

inkhaven language <verb> …   — the headless twin, by group:
  Build     init, add-word, remove-word, define-rule,
            generate-word, generate-lexicon, scaffold
  Phonology syllabify, ipa, stress, tone, harmony, romanize,
            transliterate, distribution, suggest-phonemes
  Grammar   parse, tree, gloss, igt, paradigm, sentence,
            agree, relative, coordinate, complement, movement
  Analyse   audit, stats, metrics, universals, naturalness,
            frequency, concordance, collocations, texts
  Translate translate, reverse, cross, corpus, eval, remember
  Diachrony reconstruct, cognates, sound-change, family-tree,
            derive-lexicon, borrow, areal, propose-loans
  Socio     lect, dialects, varieties, speakers, idiolect,
            ecology, propose-dialect
  Produce   export, dictionary, grammar-book, tutorial, igt
  Script    font-build, font-compose, glyph-draft, glyph-lint
  Link      link-place, link-character, scan-manuscript, query
  Check     doctor, check, check-clause, check-agreement,
            grammar-check, realism-check, areal-check
```]

The multilingual-coverage helpers for the *manuscript's* natural language
(not a conlang) are separate:

#chord_table((
  ("lang status", [The coverage matrix — stemming, detector word-lists,
    prompts, embeddings — for the project (or `--language`) language.]),
  ("lang bootstrap <lang>", [Generate the full detector vocabulary for any
    language via one LLM pass; `--yes` patches `inkhaven.hjson` in place.]),
  ("prompts bootstrap <lang>", [Generate per-language variants of the seven
    embedded prompts; `--update` merges them into `prompts.hjson`.]),
  ("show-dont-tell bootstrap <lang>", [Generate the four show-don't-tell word
    lists for a language; `--update` merges them in place.]),
))

#section("Research and scholarship")

The Research Assistant is its own TUI (`research` with no research flag), but
almost all of its acquisition and analysis runs headless through flags on the
same command — one non-interactive step per flag.

#screen(caption: "The Research Assistant, headless")[```
inkhaven research [--thread <name>]
    Open the Research Assistant TUI (its own screen).
inkhaven research --import <path> | --sync <folder>
    Ingest a document (md/txt/pdf/.bib) or register a folder
    for re-import-on-change.
inkhaven research --agentic <topic> [--out <f>]
    Autonomous deep research: decompose, gather, emit Facts
    (each with model provenance) for review.
inkhaven research --batch <file> [--auto-confirm]
                  [--confidence <0..1>] [--out <f>]
    Research a question list headlessly into a report.
inkhaven research --gutenberg <q> | --archive <q>
                  | --wikisource <q> | --bible <ref>
                  | --quran <s> | --bookofmormon <ref>
    Ingest a public-domain source by search or reference.
inkhaven research --snowball <seed> | --contradict | --converge
                  | --socrates <topic> | --report | --bibliography
    Cross-check the corpus: citation neighbourhoods, source
    contradictions, triangulated convergence, the Dialectician's
    questions, the clustered report, a BibTeX export.
```]

#screen(caption: "Sources, terms, the scholarly apparatus")[```
inkhaven sources check    [--book-name <b>] [--json]
inkhaven sources coverage [--book-name <b>] [--ai] [--json]
inkhaven sources list | import <file.bib> | export [--format <f>]
    Validate every @key against the Sources book; flag uncited
    factual claims (coverage); import/export BibTeX / CSL-JSON.
inkhaven terms check   [--book <b>] [--json]
inkhaven terms suggest [--book <b>] [--auto-create]
    Flag banned synonyms of canonical Glossary terms; cluster
    drifting terminology into proposed entries.
inkhaven index-locorum  [--book-name <b>] [--format <f>] [--strict]
    Index Locorum — every @key[locus] cited, grouped by source.
inkhaven index-verborum [--book-name <b>] [--format <f>]
    Index Verborum — every scholarly-lexicon term, by sense.
inkhaven lexicon list [--book <b>] [--watched] [--json]
    The scholarly lexicon: original-language forms + senses.
inkhaven argue [--book-name <b>] [--provider <p>] [--json]
    Extract each chapter's central claims and their support;
    flag unsupported claims and orphan citations (exit non-zero).
inkhaven index [--book-name <b>] [--format md|typst|json] [--out <f>]
    Generate a back-of-book index from the Glossary terms.
```]

#section("Producing the book")

The finishing line: turn the tree into a file someone can read — a PDF, an
EPUB, a Word manuscript, a web page, an audiobook — or operate on the PDF
itself.

#screen(caption: "Assembly and export")[```
inkhaven build [--book-name <b>] [--compile]
    Assemble a book into the artefacts tree; --compile runs
    typst on the root .typ. Headless twin of Ctrl+B B.
inkhaven export <format> [--output <o>] [--book-name <b>]
                [--status <floor>] [--tag <t>] [--profile <d=v>]
                [--blind] [--bundle <path>]
    Export to typst | pdf | markdown | tex | epub | html.
    --status/--tag filter paragraphs; --blind strips identity;
    --bundle writes a self-contained arXiv LaTeX submission.
inkhaven epub [--book-name <b>] [--output <o>] [--title <t>]
              [--author <a>]
    Export a book to EPUB 3.
inkhaven import-epub <file.epub> [--book-name <b>] [--dry-run]
    Import an EPUB as a user book (the inverse of epub).
inkhaven manuscript [--book-name <b>] [--output <o>] …
inkhaven docx [--book-name <b>] [--font times|courier] …
    Shunn standard manuscript format: typst or Word (.docx).
```]

#screen(caption: "Timeline, concordance, audio, docs")[```
inkhaven export-timeline [--book-name <b>] <typst|svg|png>
                         --output <o> [--track <k>]
    Render a book's timeline as a listing or a swim-lane.
inkhaven export-concordance <csv|json> --output <o>
                            [--min-count <n>]
    Export the project-wide concordance (stems + KWIC).
inkhaven audiobook [--book-name <b>] [--output <o>] …
    Synthesise a book to a chaptered .m4b (needs ffmpeg + TTS).
inkhaven tts engine | test <phrase> [--voice <v>]
          | binary <status|download>
          | voice <list|download <n>|remove <n>>
          | catalog refresh
    Headless management of the Piper TTS stack (Ctrl+B Shift+V).
inkhaven docs verify [--yes | --dry-run] | links [--external]
          | review [--floor <f>] [--since <ref>]
    TDOC — verify marked code blocks, check links, review
    currency. (Ctrl+B Shift+D.)
```]

Most of the `pdf` family operates on an existing PDF — typically Inkhaven's own
`build` output. Those mutating ops write a `<stem>-<op>.pdf` sibling unless
`--out` is given, and writes are atomic. The exceptions are `cover`, `barcode`,
and `merge`: they take no single input PDF to derive a stem from, so `--out` is
required.

#screen(caption: "PDF page and print operations")[```
inkhaven pdf info <in>
    Page count, size, metadata, outline size.
inkhaven pdf extract | delete --pages <2-4,7> [--out <o>]
inkhaven pdf rotate --pages <p> --degrees <90|180|270>
inkhaven pdf reorder --mapping <3,1,2>
inkhaven pdf split --every <n> | --at <p,p>
inkhaven pdf merge <in…> --out <o>
    Page-level surgery: keep, drop, rotate, reorder, split, join.
inkhaven pdf metadata <in> [--title …] [--strip]
inkhaven pdf outline <in> [--set <toc>]
    Read / set document metadata and bookmarks.
inkhaven pdf impose | booklet — print-ready signatures.
inkhaven pdf cover --pages <n> --out <o> [--isbn <i>][--image <p>]
inkhaven pdf barcode <isbn> --out <o>
    A full cover-and-spine PDF, or a standalone EAN-13 barcode.
inkhaven pdf preflight | grayscale | optimize | watermark | sample
    Print-readiness check, colour, size, stamping, proofs.
```]

The submission apparatus tracks where a finished manuscript went and drafts
its package:

#chord_table((
  ("submissions add / list / status / add-note / remove", [The submission
    tracker — record a market, a status, a response, a note.]),
  ("submission digest / query / synopsis / comps / logline", [Build the
    book digest, then draft the query letter, synopsis, comp titles, and
    logline from it (AI).]),
))

#section("Scripting")

One command reaches the embedded Bund language directly:

#chord_table((
  ("bund <code>", [Evaluate a Bund expression against the Adam VM and print
    the top of the workbench — e.g. `inkhaven bund "40 2 + ."`. A phase-0
    smoke command that does not open the project store; use it to verify the
    scripting layer works and to experiment with syntax. The full `ink.*`
    surface is documented in the Bund guide.]),
  ("output show / emit / dismiss / clear", [The CLI surface over the Output
    message channel (the pane itself is a TUI feature) — useful when
    scripting, or when ssh'd into a project with no TUI.]),
))

#section("Planning and threads")

Two families sit between structure and prose — the Planning Board and the
plot-thread ledger:

#screen(caption: "The Planning Board and plot threads")[```
inkhaven plan init [--framework <f>]
    Scaffold a framework's beats: three_act (default),
    save_the_cat, story_circle, hero_journey, seven_point.
inkhaven plan check [--book-name <b>] [--drift <pct>] [--json]
inkhaven plan analyze [--book-name <b>] [--provider <p>]
    Diagnose beat coverage / pacing (deterministic), then map
    beats to chapters and name structural problems (AI).
inkhaven plan map <beat> <chapter> | unmap <beat>
inkhaven plan scaffold [--premise <p>] [--chapters]
inkhaven plan scene … | sequel … | tension rate
    Map beats, expand a premise, manage scene / sequel cards,
    and rate chapter intensity.
inkhaven thread add <name> [--status <s>] [--weight <w>]
inkhaven thread list [--status <s>] [--weight <w>]
inkhaven thread doctor [--json] | export [--format <f>] [--out <o>]
    Plot-thread paragraphs under the Threads book, with a
    health report. (Ctrl+V Shift+D.)
inkhaven myth scan | check | profile | refresh | suppress
    Mythological & symbolic pattern library. (Ctrl+V Shift+M.)
```]

#section("Miscellaneous and imports")

The remaining verbs: a one-shot model call, the Help-book importers, the
Scrivener bridge, and the cost dashboard.

#screen(caption: "Odds and ends")[```
inkhaven ai <prompt> [--provider <p>]
    One-shot AI inference from the command line.
inkhaven cost
    Today's LLM call tallies per capped subsystem vs their caps.
inkhaven import-scrivener <path.scriv> [--draft-as-book <t>]
                          [--skip-research] [--dry-run]
    Import a Scrivener project (RTF → Typst, single-binary).
inkhaven import-help --documents-directory <dir>
    Import a directory tree into the Help system book.
inkhaven import-typst-help
    Refresh the bundled Typst reference in the Help book (F1).
```]

#callout(label: "Hidden commands")[
  A handful of verbs are hidden from `--help` because they exist only for the
  benchmark and endurance harnesses — `gen-fixture`, `_bench-load`,
  `_bench-render`, `_bench-embed`, `_bench-graph`, `_bench-report`, and
  `_soak`. They build synthetic projects and time the store, render, embed,
  and graph paths. An end-user project never needs them; they are listed here
  only so an unexpected entry in a process list is not a mystery.
]

#callout(label: "The single source of truth")[
  This appendix is a curated map, and Inkhaven's command surface grows with
  every release. The authoritative, always-current reference for any single
  verb is the binary itself: `inkhaven <subcommand> --help` prints that
  subcommand's exact arguments, flags, defaults, and value sets as the
  installed version defines them. When this page and `--help` disagree,
  `--help` is right.
]
