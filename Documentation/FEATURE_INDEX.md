# Inkhaven — Feature Index

The canonical map of Inkhaven's features as of the **3.0.0** freeze, maintained through **3.0.6**: what each one is, how you reach it (CLI verb and/or editor chord), its authoritative doc, and its scripting surface. Use this as the maintenance reference — every user-facing feature should appear here with at least a doc.

Legend: **CLI** = `inkhaven <verb>` · **Chord** = in-editor keystroke · **Doc** = the topic guide · **Bund** = the `ink.*` scripting namespace (read-only unless noted). The exhaustive per-word list is [Bund/WORD_REFERENCE.md](Bund/WORD_REFERENCE.md).

## Manuscript & editor

| Feature | CLI | Chord | Doc | Bund |
|---|---|---|---|---|
| The editor & tree | — | (pane keys) | [Tutorials 01–03](Tutorials/README.md) | `ink.tree` `ink.node` `ink.paragraph` |
| Search (semantic + full-text) | `search` | `Ctrl+V S` | [Tutorial 04](Tutorials/04-search-and-discovery.md) | `ink.search` |
| AI writing assistant | — | `F9` scopes | [Tutorial 05](Tutorials/05-ai-writing-assistant.md) | `ink.ai` |
| Chat with your book (RAG) | `book-rag` | Book scope | [Tutorial 87](Tutorials/README.md) | `ink.book_rag` |
| Snapshots & split-edit | — | `F6` / `Ctrl+F6` | [Tutorial 03](Tutorials/03-the-editor.md) | `ink.snapshot` |
| Outline pane | `outline` | `Ctrl+2` / `Ctrl+B Shift+O` | [Tutorial 96](Tutorials/96-manuscript-outline.md) | `ink.outline` |
| Story structure (planning board) | `plan` | `Ctrl+B Shift+K` | [Tutorial](Tutorials/README.md) | `ink.planning` |
| Structural paragraphs (`para:*`) | — | `i` (Tree) | [STRUCTURAL_PARAGRAPHS.md](STRUCTURAL_PARAGRAPHS.md) | — |
| Jinja template paragraphs | — | `e` (Tree) | [JINJA_TEMPLATES.md](JINJA_TEMPLATES.md) | — |
| Reusable snippets | `snippets` | `Ctrl+V x` | [Tutorial 92](Tutorials/README.md) | `ink.snippets` |
| Output pane | `output` | `Ctrl+B Tab` | [OUTPUT_PANE.md](OUTPUT_PANE.md) | `ink.io` `ink.pane` |
| Writing goals & streaks | `goals` | `Ctrl+V g` / `Ctrl+B Shift+G` | [Tutorial](Tutorials/README.md) | `ink.goals` |

## The world & knowledge layer

| Feature | CLI | Chord | Doc | Bund |
|---|---|---|---|---|
| World simulation | `realworld` | `Ctrl+B W` | [WORLDBUILDING.md](WORLDBUILDING.md) | `ink.world` |
| Interactive worldbuilder | `worldbuilder` | (own TUI) | [BUILDING_THE_WORLD book] | `ink.world` |
| Utopia/dystopia coherence | `world utopia-check` | (review pass) | [WORLDBUILDING.md](WORLDBUILDING.md) | `ink.utopia` |
| Facts & fact-check | `fact-check` | `Ctrl+B Shift+X` | [Tutorial](Tutorials/README.md) | `ink.research` |
| Semantic net (graph) | `graph` | `Ctrl+B z` | [GRAPH.md](GRAPH.md) | `ink.graph` |
| Timeline & events | `event` | — | [Tutorials 31, 80](Tutorials/README.md) | `ink.event` |
| Places / Characters | — | `Ctrl+B P` / `Ctrl+B C` | [LOCATIONS.md](LOCATIONS.md) · [CHARACTERS.md](CHARACTERS.md) | `ink.char` |
| Character arcs | `character` | `Ctrl+V Shift+N` | [CHARACTERS.md](CHARACTERS.md) | `ink.char` |
| Plot threads | `thread` | `Ctrl+V Shift+D` (doctor) | [THREADS.md](THREADS.md) | `ink.thread` |
| Mythological patterns | `myth` | `Ctrl+V Shift+M` | [Tutorial 102](Tutorials/README.md) | `ink.myth` |

## The book watches / reads itself (the intelligences)

| Feature | CLI | Chord | Doc | Bund |
|---|---|---|---|---|
| **SENTINEL** — continuity | `continuity check` | `Ctrl+B Shift+I` | [CONTINUITY.md](CONTINUITY.md) | `ink.continuity` |
| **KEN** — who knows what | `knowledge` | `Ctrl+B Shift+Z` | [KNOWLEDGE.md](KNOWLEDGE.md) | `ink.knowledge` |
| **BONDS** — earned relationships | `bonds` | `Ctrl+V Shift+O` | [BONDS.md](BONDS.md) | `ink.bonds` |
| **LECTOR** — read-through | `readthrough` | `Ctrl+B Shift+A` | [LECTOR.md](LECTOR.md) | `ink.readthrough` |
| **CHORUS** — voice at book scale | `chorus` | `Ctrl+B J → Y` | [CHORUS.md](CHORUS.md) | `ink.chorus` `ink.stylist` |
| **NARR-1** — narrator voice | `prose` | `Ctrl+V V` | [PROSE_VOICE.md](PROSE_VOICE.md) | `ink.prose` |
| Dialogue quality & attribution | `dialogue` | `Ctrl+V Shift+Q` | [Tutorial 97](Tutorials/97-dialogue-quality.md) | `ink.dialogue` |
| **REDLINE** — revision partner | `revise` | `Ctrl+V Shift+R` | [REDLINE.md](REDLINE.md) | `ink.revise` `ink.review` |
| **CHRONICLE** — draft history | `chronicle` | `Ctrl+B Shift+U` | [CHRONICLE.md](CHRONICLE.md) | `ink.chronicle` |
| Unified review pass | `check` | `Ctrl+B Shift+C` | [MAINTENANCE.md](MAINTENANCE.md) | — |

## The inner family (readers who question)

| Feature | CLI | Chord | Doc | Bund |
|---|---|---|---|---|
| Inner Socrates (Dialectician) | `inner-socrates` | `Ctrl+B J` | [INNER_SOCRATES.md](INNER_SOCRATES.md) | `ink.inner_socrates` |
| Inner Editor | `inner-editor` | `Ctrl+V O` | [INNER_SOCRATES.md](INNER_SOCRATES.md) | `ink.inner_editor` |
| Inner Theologian | `theologian` | `Ctrl+B J → T` | [Tutorial 101](Tutorials/README.md) | `ink.theologian` `ink.inner_theologian` |
| Inner Poet | `poetry` | `Ctrl+B J → P` | [Tutorial 110](Tutorials/README.md) · [POETRY book] | `ink.poem` |
| Reasoning rigor | `rigor` | — | [RIGOR.md](RIGOR.md) | `ink.rigor` |
| Companions cockpit | `companions` | — | [MAINTENANCE.md](MAINTENANCE.md) | `ink.companions` |

## Language & poetry

| Feature | CLI | Chord | Doc | Bund |
|---|---|---|---|---|
| ConLang suite | `language` | `Ctrl+B X` | [CONLANG.md](CONLANG.md) | `ink.lang` |
| Linguistic analysis | `language scan` | — | [LINGUISTIC book] | `ink.lang` |
| WordNet thesaurus | `wordnet` | `Ctrl+V Shift+Y` | [Tutorial 109](Tutorials/README.md) | `ink.wordnet` |
| Poetry (measure verse) | `poetry` | `Ctrl+B J → P` | [POETRY book] | `ink.poem` |

## Research & scholarship

| Feature | CLI | Chord | Doc | Bund |
|---|---|---|---|---|
| Research assistant | `research` | (own TUI) | [RESEARCH book](../Book/RESEARCH) | `ink.research` |
| Sources / bibliography | `sources` | `Ctrl+V @` | [Tutorial 89](Tutorials/README.md) | `ink.sources` |
| Terminology / glossary | `terms` | `Ctrl+V z` | [Tutorial 91](Tutorials/README.md) | `ink.terms` |
| Index Locorum / Verborum | `index-locorum` · `index-verborum` | — | [SCHOLARLY_APPARATUS.md](SCHOLARLY_APPARATUS.md) | `ink.locorum` `ink.verborum` |
| Scholarly lexicon | `lexicon` | — | [SCHOLARLY_APPARATUS.md](SCHOLARLY_APPARATUS.md) | — |
| Argument outline | `argue` | — | [SCHOLARLY_APPARATUS.md](SCHOLARLY_APPARATUS.md) | — |
| Back-of-book index | `index` | — | [BACK_OF_BOOK_INDEX.md](BACK_OF_BOOK_INDEX.md) | — |

## Export & production

| Feature | CLI | Chord | Doc | Bund |
|---|---|---|---|---|
| Typst / PDF assembly | `export` / `build` | `Ctrl+B A` / `Ctrl+B B` | [Tutorial 09](Tutorials/README.md) | `ink.export` `ink.typst` |
| EPUB (import + export) | `import-epub` / `export` | — | [Tutorial](Tutorials/README.md) | `ink.export` `ink.import` |
| HTML website | `export html` | — | [WEBSITE book] | — |
| Technical-doc checks (TDOC) | `docs verify` / `links` / `review` | `Ctrl+B Shift+D` | [TDOC.md](TDOC.md) | — |
| arXiv / scientific bundle | `export --bundle` | — | — | — |
| Audiobook / TTS | `tts` | `Ctrl+B S` / `Ctrl+B Shift+V` | [Tutorials 56, 57](Tutorials/README.md) | `ink.tts` |
| Scrivener import | `import-scrivener` | `F3` | [Tutorial 08](Tutorials/README.md) | `ink.import` |

## System & scripting

| Feature | CLI | Chord | Doc | Bund |
|---|---|---|---|---|
| Bund scripting | `bund` | (Script nodes) | [Bund/](Bund/README.md) | the whole `ink.*` surface |
| AI cost dashboard | `cost` | `Ctrl+B $` | [MAINTENANCE.md](MAINTENANCE.md) | `ink.cost` |
| Project doctor | `doctor` | `Ctrl+B Shift+0` | [MAINTENANCE.md](MAINTENANCE.md) | `ink.doctor` |
| Config editor | — | `Ctrl+B 0` | [CONFIGURATION.md](CONFIGURATION.md) | `ink.db` |
| Keybindings | — | — | [KEYBINDING.md](KEYBINDING.md) · [KEYS_REASSIGNMENT.md](KEYS_REASSIGNMENT.md) | `ink.key` |
| Backup / restore / reindex | `backup` · `reindex` | — | [MAINTENANCE.md](MAINTENANCE.md) | `ink.backup` `ink.db` |
| Prompt templates | `prompts` | — | [PROMPTS.md](PROMPTS.md) | — |

---

Every configuration block for these features is documented field-by-field in [CONFIGURATION.md](CONFIGURATION.md). Chords are authoritative in [KEYBINDING.md](KEYBINDING.md); where a chord cell is blank the feature is CLI-only. The companion books (in `../Book/`) give the long-form, worked-example treatment of the worldbuilding, conlang, poetry, research, theology, and web-publishing tracks.
