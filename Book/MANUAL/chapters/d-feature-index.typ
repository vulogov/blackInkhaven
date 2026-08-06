#import "../design.typ": *

#appendix(letter: "D", title: "The Feature Index")

This is the single map of the whole product: every feature Inkhaven ships, wired to the four ways you reach it. Read a row left-to-right — the feature name, then its terminal verb (`inkhaven <verb>`), its in-editor chord, the *Manual chapter* that teaches it, and the `ink.*` Bund namespace that scripts it. Use it two ways: forward, when you know a feature and want its command or chord; and backward, when you meet a `Ctrl+…` chord or an `ink.*` symbol and want to know what it belongs to. It mirrors `Documentation/FEATURE_INDEX.md` and adds the chapter column so the map points back into this book. Chords are authoritative in Appendix A; verbs in Appendix B; every config block in Appendix C.

#callout(label: "How to read a cell")[
Each right-hand cell reads `CLI` · `chord` · *ch N* · `ink.*`, in that order, and drops whatever does not apply. A missing verb means the feature is chord-only; a missing chord means it is terminal-only; *ch —* means no single chapter owns it (reach it by CLI or its topic guide); a missing namespace means it is not scriptable from Bund. Where two chapters share a feature both are named.
]

#section("Manuscript & editor")

#chord_table((
  ("The editor & tree", [chord `(pane keys)` · *ch 4, 5* · `ink.tree` `ink.node` `ink.paragraph`]),
  ("Search (semantic + full-text)", [CLI `search` · chord `Ctrl+V S` · *ch 7* · `ink.search`]),
  ("AI writing assistant", [chord `F9` (scope cycle) · *ch 9* · `ink.ai`]),
  ("Chat with your book (RAG)", [CLI `book-rag` · chord `F9` → Book · *ch 10* · `ink.book_rag`]),
  ("Snapshots & split-edit", [chord `F6` / `Ctrl+F6` · *ch 6* · `ink.snapshot`]),
  ("Outline pane", [CLI `outline` · chord `Ctrl+2` / `Ctrl+B Shift+O` · *ch 5* · `ink.outline`]),
  ("Structural paragraphs (`para:*`)", [chord `i` (Tree) · *ch 5*]),
  ("Jinja template paragraphs", [chord `e` (Tree) · *ch 5*]),
  ("Reusable snippets", [CLI `snippets` · chord `Ctrl+V x` · *ch 5* · `ink.snippets`]),
  ("Style & filter-word overlays", [chord `Ctrl+B Shift+F` · *ch 8*]),
  ("Output pane", [CLI `output` · chord `Ctrl+B Tab` · *ch 3* · `ink.io` `ink.pane`]),
  ("Writing goals & streaks", [CLI `goals` · chord `Ctrl+V g` / `Ctrl+B Shift+G` · *ch —*]),
))

#section("The world & knowledge layer")

#chord_table((
  ("World simulation", [CLI `realworld` · chord `Ctrl+B W` · *ch 14* · `ink.world`]),
  ("Interactive worldbuilder", [CLI `worldbuilder` (own TUI) · *ch 14* · `ink.world`]),
  ("Utopia / dystopia coherence", [CLI `utopia` · *ch 14* · `ink.utopia`]),
  ("Facts & fact-check", [CLI `factcheck` · chord `Ctrl+B Shift+X` · *ch 14*]),
  ("Semantic net (graph)", [CLI `graph` · chord `Ctrl+B z` · *ch 15* · `ink.graph`]),
  ("Timeline & events", [CLI `timeline` · *ch 16* · `ink.event`]),
  ("Places / Characters", [chord `Ctrl+B P` / `Ctrl+B C` · *ch 13* · `ink.char`]),
  ("Character arcs", [CLI `character` · chord `Ctrl+V Shift+N` · *ch 13* · `ink.char`]),
  ("Plot threads", [CLI `thread` · chord `Ctrl+V Shift+D` (doctor) · *ch —* · `ink.thread`]),
  ("Mythological patterns", [CLI `myth` · chord `Ctrl+V Shift+M` · *ch —* · `ink.myth`]),
))

#section("The intelligences — the book watches & reads itself")

#chord_table((
  ("SENTINEL — continuity", [CLI `continuity check` · chord `Ctrl+B Shift+I` · *ch 17* · `ink.continuity`]),
  ("KEN — who knows what, when", [CLI `knowledge` · chord `Ctrl+B Shift+Z` · *ch 17* · `ink.knowledge`]),
  ("LECTOR — read-through", [CLI `readthrough` · chord `Ctrl+B Shift+A` · *ch 18* · `ink.readthrough`]),
  ("CHORUS — voice at book scale", [CLI `chorus` · chord `Ctrl+B J` → `Y` · *ch 18* · `ink.chorus` `ink.stylist`]),
  ("NARR — narrator voice", [CLI `prose` · chord `Ctrl+V V` · *ch 18* · `ink.prose`]),
  ("Dialogue quality & attribution", [CLI `dialogue` · chord `Ctrl+V Shift+Q` · *ch 18* · `ink.dialogue`]),
  ("REDLINE — revision partner", [CLI `revise` · chord `Ctrl+V Shift+R` · *ch 19* · `ink.revise` `ink.review`]),
  ("CHRONICLE — draft history", [CLI `chronicle` · chord `Ctrl+B Shift+U` · *ch 19* · `ink.chronicle`]),
  ("Unified review pass", [CLI `check` · chord `Ctrl+B Shift+C` · *ch 19*]),
))

#section("The inner family — readers who question")

#chord_table((
  ("Inner Socrates (Dialectician)", [CLI `inner-socrates` · chord `Ctrl+B J` · *ch 20* · `ink.inner_socrates`]),
  ("Inner Editor", [CLI `inner-editor` · chord `Ctrl+V O` · *ch 20* · `ink.inner_editor`]),
  ("Inner Theologian", [CLI `theologian` · chord `Ctrl+B J` → `T` · *ch 20* · `ink.theologian` `ink.inner_theologian`]),
  ("Inner Poet", [CLI `poetry` · chord `Ctrl+B J` → `P` · *ch 20* · `ink.poem`]),
  ("Reasoning rigor", [CLI `rigor` · *ch 20*]),
  ("Companions cockpit", [CLI `companions` · *ch 20*]),
))

#section("Language & poetry")

#chord_table((
  ("ConLang suite", [CLI `language` · chord `Ctrl+B X` · *ch 21* · `ink.lang`]),
  ("Linguistic analysis", [CLI `language scan` · *ch 21* · `ink.lang`]),
  ("WordNet thesaurus", [CLI `wordnet` · chord `Ctrl+V Shift+Y` · *ch 21*]),
  ("Poetry (measure verse)", [CLI `poetry` · chord `Ctrl+B J` → `P` · *ch 22* · `ink.poem`]),
))

#section("Research & scholarship")

#chord_table((
  ("Research assistant", [CLI `research` (own TUI) · *ch 23*]),
  ("Sources / bibliography", [CLI `sources` · chord `Ctrl+V @` · *ch 23* · `ink.sources`]),
  ("Terminology / glossary", [CLI `terms` · chord `Ctrl+V z` · *ch 23* · `ink.terms`]),
  ("Index Locorum / Verborum", [CLI `index-locorum` · `index-verborum` · *ch 23*]),
  ("Scholarly lexicon", [CLI `lexicon` · *ch 23*]),
  ("Argument outline", [CLI `argue` · *ch 23*]),
  ("Back-of-book index", [CLI `index` · *ch 23*]),
))

#section("Export & production")

#chord_table((
  ("Typst / PDF assembly", [CLI `export` / `assemble` · chord `Ctrl+V R` · *ch 24* · `ink.export` `ink.typst`]),
  ("EPUB (import + export)", [CLI `import-epub` / `export` · *ch 25* · `ink.export`]),
  ("HTML website", [CLI `export html` · *ch 25*]),
  ("Technical-doc checks (TDOC)", [CLI `docs verify` / `links` / `review` · chord `Ctrl+B Shift+D` · *ch 26*]),
  ("arXiv / scientific bundle", [CLI `export bundle` · *ch 25*]),
  ("Audiobook / TTS", [CLI `tts` · chord `Ctrl+B S` / `Ctrl+B Shift+V` · *ch 25* · `ink.tts`]),
  ("Scrivener import", [CLI `import-scrivener` · chord `F3` · *ch 25*]),
))

#section("System & scripting")

#chord_table((
  ("Bund scripting", [CLI `bund` · Script nodes · *ch 27* · the whole `ink.*` surface]),
  ("AI cost dashboard", [CLI `cost` · chord `Ctrl+B $` · *ch 12*]),
  ("Project doctor", [CLI `doctor` · chord `Ctrl+B Shift+0` · *ch 28*]),
  ("Config editor", [chord `Ctrl+B 0` · *ch 29* · `ink.db`]),
  ("Keybindings", [*ch 29* (+ Appendix A) · `ink.key`]),
  ("Backup / restore / reindex", [CLI `reindex` · *ch 28*]),
  ("Prompt templates", [CLI `prompts` · *ch 11*]),
))

#callout(label: "Keeping it current")[
When a feature is added, it earns a row here, in `Documentation/FEATURE_INDEX.md`, and in Appendices A–C — a feature with no doc and no chapter is not finished. If a chord in this map disagrees with your terminal, Appendix A and `Documentation/KEYBINDING.md` are the arbiters; a cell left blank there is deliberate, not an omission.
]
