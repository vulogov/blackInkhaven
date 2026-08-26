# Bund word reference — every `ink.*` word

The complete list of inkhaven-specific Bund words. This is the exhaustive
reference; `BUND_TUTORIAL.md` is the teaching companion. Generated for 3.0.6 —
if a word is registered it should be here (the build's
`every_registered_word_is_classified` guard keeps the policy table honest, and
`ink.words` dumps the live list at runtime).

Every word also answers to its **short alias** — the same name without the
`ink.` prefix (`node.list` == `ink.node.list`). Canonical `ink.*` names are
listed here.

## How the category gates a word

Each word carries one **policy category**. Reads are allowed out of the box;
destructive categories are denied by default and opt in per project via
`inkhaven.hjson`:

```hjson
scripting: { enabled_categories: ["store_write", "fs_write", "ai_write"] }
```

| Category | Default | Meaning |
|----------|---------|---------|
| `store_read` | allowed | read the project store / derived caches |
| `fs_read` | allowed | read a file (path confined to the project unless `fs_unsandboxed`) |
| `editor_read` | allowed | read the live editor buffer / open UI panes |
| `ai_read` | allowed | read AI chat history |
| `audio` | allowed | TTS playback (also gated by `editor.tts.enabled`) |
| `pure` | always | in-memory value transform — touches no protected resource |
| `store_write` | **denied** | mutate the project store (nodes, tags, events, ledgers) |
| `fs_write` | **denied** | write a file / artefact |
| `editor_write` | **denied** | mutate the live editor buffer |
| `ai_write` | **denied** | call the LLM / mutate AI state (costs money) |
| `net` | **denied** | network access |
| `keymap` | **denied** | rebind runtime keys |
| `theme_write` | **denied** | recolour the interface |
| `shell` / `code_eval` | **denied** | (reserved) |

Filesystem paths passed to `fs_read`/`fs_write` words are confined to the
project root unless `scripting.fs_unsandboxed: true`.

---

## Core project & store

Read and mutate the project tree, tags, snapshots, and DB. Reads are
`store_read` (default-allowed); every mutator is `store_write` (default-denied).

**`ink.node.*` / `ink.paragraph.*` / `ink.path.*` / `ink.snapshot.*` / `ink.search.*`**

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.node.list` | store_read | `( -- list )` | every node as a hash (id, kind, title, slug, parent, order, status…) |
| `ink.node.get` | store_read | `( uuid -- hash \| NODATA )` | a single node's fields |
| `ink.node.children` | store_read | `( uuid_or_empty -- list )` | child nodes of uuid; empty = root (top-level books) |
| `ink.paragraph.text` | store_read | `( uuid -- string \| NODATA )` | the paragraph's body text |
| `ink.paragraph.target` | store_read | `( path -- int \| NODATA )` | the paragraph's word-count target |
| `ink.paragraph.save` | store_write | `( path body -- )` | overwrite the paragraph's content and sync |
| `ink.paragraph.set_status` | store_write | `( path status -- )` | set the workflow status |
| `ink.paragraph.set_target` | store_write | `( path target -- )` | set the word-count target |
| `ink.path.to_uuid` | store_read | `( slug_path -- uuid \| NODATA )` | resolve a slug path to its node UUID |
| `ink.snapshot.list` | store_read | `( paragraph_uuid -- list )` | history entries for a paragraph |
| `ink.search.text` | store_read | `( query limit -- list )` | full-text search: up to limit hits (id, title, score, document, kind) |
| `ink.search.load` | editor_read | `( query index -- bool )` | load the index-th search hit into the editor; push success |

**`ink.tree.*` — structural mutations (all `store_write`)**

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.tree.add` | store_write | `( parent_path kind title -- uuid )` | create a node of kind under parent; return new UUID |
| `ink.tree.delete` | store_write | `( path -- )` | delete the node at path |
| `ink.tree.rename` | store_write | `( path new_title -- )` | rename the node |
| `ink.tree.move_up` | store_write | `( path -- )` | move the node up among its siblings |
| `ink.tree.move_down` | store_write | `( path -- )` | move the node down among its siblings |
| `ink.tree.morph` | store_write | `( path -- )` | cycle a leaf's flavour: Paragraph(typst) → Paragraph(hjson) → Script(bund) → back |

**`ink.tag.*`**

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.tag.list` | store_read | `( -- list )` | de-duplicated sorted list of every tag in use |
| `ink.tag.list_for` | store_read | `( path -- list )` | the tag list of the node at path |
| `ink.tag.search` | store_read | `( tag -- list )` | slug-paths of paragraphs carrying tag (case-insensitive) |
| `ink.tag.add` | store_write | `( path tag -- )` | add tag to the node at path |
| `ink.tag.remove` | store_write | `( path tag -- )` | remove tag from the node at path |

**`ink.thread.*` / `ink.review.*` / `ink.outline.*` / `ink.db.*`**

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.thread.list` | store_read | `( -- list )` | every thread `{uuid, title, slug, status, weight}` |
| `ink.review.list` | store_read | `( -- list )` | review comments |
| `ink.review.add_comment` | store_write | `( para_uuid char_start char_end body -- comment_uuid )` | add a review comment on a char range |
| `ink.review.resolve` | store_write | `( comment_uuid -- bool )` | resolve (close) a comment by id |
| `ink.outline.print` | store_read | `( -- text )` | a text rendering of the project outline |
| `ink.outline.paragraph_copy` | store_write | `( src_path dest_path -- uuid )` | copy a paragraph across parents; return new UUID |
| `ink.outline.paragraph_move` | store_write | `( src_path dest_path -- )` | move a paragraph to a new parent |
| `ink.db.sync` | store_write | `( -- )` | flush/sync the project Store |
| `ink.db.checkpoint` | store_write | `( -- )` | checkpoint the Store to disk |
| `ink.db.reindex` | store_write | `( -- int )` | re-read Paragraph/Script nodes from disk, updating drift; push count |

## Timeline & events

**`ink.event.*` — story timeline (reads `store_read`, writers `store_write`)**

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.event.list` | store_read | `( -- list )` | all timeline events, sorted by start, as dicts |
| `ink.event.list_orphans` | store_read | `( -- list )` | events not linked to any paragraph |
| `ink.event.critique.run` | store_read | `( -- dict )` | run both checks; `{orphans, overlaps, total}` |
| `ink.event.critique.orphan_check` | store_read | `( -- list )` | orphan-event findings |
| `ink.event.critique.fuzzy_overlap_check` | store_read | `( -- list )` | fuzzy-precision overlap findings |
| `ink.event.critique.config` | store_read | `( -- dict )` | critique config (enabled flags + thresholds) |
| `ink.event.critique.custom` | store_read | `( -- list )` | reserved no-op placeholder (empty list) |
| `ink.event.add` | store_write | `( book-name title spec -- uuid )` | create a timeline event under a book |
| `ink.event.set_end` | store_write | `( uuid spec -- )` | set an event's end from a date spec |
| `ink.event.set_precision` | store_write | `( uuid precision -- )` | set an event's date precision |
| `ink.event.set_track` | store_write | `( uuid track -- )` | set an event's timeline track |
| `ink.event.link_paragraph` | store_write | `( uuid paragraph-path -- )` | link an event to a paragraph by path |

**`ink.world.fact_check.timeline.*` — world-time queries (all `store_read`)**

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.world.fact_check.timeline.effective_date` | store_read | `( paragraph-uuid -- date )` | paragraph's effective world-time in ticks (-1 if none) |
| `ink.world.fact_check.timeline.events_for_character` | store_read | `( character-uuid -- list )` | events involving the character `{id, title, start_ticks}` |
| `ink.world.fact_check.timeline.events_for_place` | store_read | `( place-uuid -- list )` | events at the place `{id, title, start_ticks}` |
| `ink.world.fact_check.timeline.events_near` | store_read | `( point window -- list )` | events whose start is within window ticks of point |
| `ink.world.fact_check.timeline.season_for` | store_read | `( point -- season )` | the calendar season covering a point ("" if none) |

**`ink.world.*` — WORLD-REPORT (the `inkhaven world` reads, all `store_read`)**

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.world.report` | store_read | `( -- dict )` | world state `{facts_total, characters, places, artefacts, continuity_attributes, entity_total, issue_count, undescribed, stale, summary}` |
| `ink.world.undescribed` | store_read | `( -- list )` | entities defined but never named in the prose `{name, kind}` |
| `ink.world.findings` | store_read | `( -- list )` | the discrete world conflicts + anachronisms `{kind, …}` (fact_conflict / drift / anachronism) |
| `ink.world.check` | store_read | `( -- dict )` | gate `{issues, undescribed, clean}` (clean = no world issue) |

## Editor, AI & UI

**`ink.editor.*` — the live editor buffer (reads `editor_read`, writers `editor_write`)**

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.editor.cursor` | editor_read | `( -- list[row col] \| NODATA )` | current cursor position |
| `ink.editor.text` | editor_read | `( -- string \| NODATA )` | full editor buffer text |
| `ink.editor.find` | editor_read | `( needle -- list[row col] \| NODATA )` | first match position of needle |
| `ink.editor.goto` | editor_write | `( row col -- )` | move the cursor/view to row,col |
| `ink.editor.set_cursor` | editor_write | `( row col -- )` | set the cursor to 1-based row,col |
| `ink.editor.insert` | editor_write | `( text -- )` | insert text at the cursor |
| `ink.editor.scroll` | editor_write | `( delta -- )` | scroll the view by delta lines |
| `ink.editor.replace` | editor_write | `( find replace -- bool )` | replace the first occurrence |
| `ink.editor.replace_all` | editor_write | `( find replace -- count )` | replace all occurrences; push count |
| `ink.editor.delete_line` | editor_write | `( -- )` | delete the current line |
| `ink.editor.delete_to_bol` | editor_write | `( -- )` | delete from cursor to beginning of line |
| `ink.editor.delete_to_eol` | editor_write | `( -- )` | delete from cursor to end of line |

**`ink.ai.*` — the AI pane (reads `ai_read`, writers `ai_write`)**

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.ai.history` | ai_read | `( -- list )` | chat history as role/content hashes |
| `ink.ai.poll` | ai_read | `( -- hash )` | poll pending async AI response state |
| `ink.ai.send` | ai_write | `( prompt -- )` | send a prompt asynchronously (non-blocking) |
| `ink.ai.send_blocking` | ai_write | `( prompt timeout_ms -- response \| NODATA )` | send and block up to timeout_ms |
| `ink.ai.clear_history` | ai_write | `( -- )` | clear the AI chat history |
| `ink.ai.set_system_prompt` | ai_write | `( text -- )` | set the AI system prompt |

**`ink.input` / `ink.pane.*` / `ink.key.*` / `ink.theme.*`**

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.input` | editor_read | `( prompt hook -- )` | open an input modal; run `hook` on the entered text |
| `ink.pane.show` | editor_read | `( title -- )` | open/show the Bund output pane |
| `ink.pane.line` | editor_read | `( text -- bool )` | append a line (auto-opens); push whether routed |
| `ink.pane.clear` | editor_read | `( -- bool )` | clear the pane |
| `ink.pane.close` | editor_read | `( -- )` | close the pane |
| `ink.key.bind` | keymap | `( chord action -- )` | bind a chord to a named built-in action ("none" disables) |
| `ink.key.bind_lambda` | keymap | `( chord lambda -- )` | bind a chord to an inline lambda |
| `ink.key.unbind` | keymap | `( chord -- )` | drop every binding for the chord |
| `ink.key.list` | keymap | `( -- list )` | active bindings `{layer, chord, action, scope}` |
| `ink.theme.set` | theme_write | `( field hex -- )` | set a theme colour field to a hex value |

## Output, messaging & files

**`ink.io.*` — the Output pane**

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.io.print` | store_read | `( text -- )` | emit a `bund_print` message to the Output pane |
| `ink.io.log` | store_read | `( text level -- )` | emit a `bund_log` message (severity from level) |
| `ink.io.message.list` | store_read | `( kind -- list )` | active messages of kind ("" = all) `{id, kind, severity, text}` |
| `ink.io.message.count` | store_read | `( kind -- n )` | count active messages of kind |
| `ink.io.notify` | fs_write | `( kind metadata -- id )` | emit a structured message of arbitrary kind |
| `ink.io.message.dismiss` | fs_write | `( id -- )` | dismiss a message by UUID |
| `ink.io.message.pin` | fs_write | `( id -- )` | pin a message |
| `ink.io.message.unpin` | fs_write | `( id -- )` | unpin a message |

**`ink.typst.*` / `ink.story.*` / `ink.export.*` — assembly & export**

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.typst.assemble` | store_write | `( -- )` | assemble the Book's typst source into the artefacts dir |
| `ink.typst.build` | store_write | `( -- )` | compile the assembled Book via typst |
| `ink.typst.take` | store_write | `( -- )` | take/capture the built artefact |
| `ink.story.render` | fs_write | `( book-name path -- )` | render the twopi story-view PNG to path |
| `ink.export.docx` | fs_write | `( book path -- )` | export the book as a `.docx` |
| `ink.export.epub` | fs_write | `( book path -- )` | export the book as an EPUB |
| `ink.export.manuscript` | fs_write | `( book path -- )` | export as Shunn-format manuscript typst |
| `ink.export.markdown` | fs_write | `( book path -- )` | export as Markdown |
| `ink.export.tex` | fs_write | `( book path -- )` | export as LaTeX |

**`ink.fs.*` / `ink.tts.*`**

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.fs.read` | fs_read | `( path -- string )` | read a sandboxed file as UTF-8 (rejects symlink escapes) |
| `ink.fs.write` | fs_write | `( path content -- )` | atomically write content to a sandboxed path |
| `ink.tts.speak` | audio | `( text -- )` | read text aloud (honours `editor.tts.enabled`) |

**`ink.pdf.*` — PDF pipeline.** In-memory `handle`s flow on the stack; `load`
(`fs_read`) and `save` (`fs_write`) are the only disk gates. Every other op is
`pure` (transforms a handle, persists nothing until `save`).

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.pdf.load` | fs_read | `( path -- handle )` | load a sandboxed PDF into a new handle |
| `ink.pdf.save` | fs_write | `( handle path -- )` | write the handle's doc to a sandboxed path |
| `ink.pdf.pages` | pure | `( handle -- n )` | push the doc's page count |
| `ink.pdf.title` | pure | `( handle -- title )` | read the Title metadata |
| `ink.pdf.extract` | pure | `( handle spec -- handle' )` | keep only the pages in spec (new handle) |
| `ink.pdf.delete` | pure | `( handle spec -- handle )` | delete the pages in spec in place |
| `ink.pdf.rotate` | pure | `( handle spec degrees -- handle )` | rotate the pages in spec |
| `ink.pdf.reorder` | pure | `( handle mapping -- handle )` | reorder pages per a 1-based permutation |
| `ink.pdf.merge` | pure | `( handle1 handle2 -- handle' )` | concatenate two docs (new handle) |
| `ink.pdf.sample` | pure | `( handle n -- handle' )` | quick-proof subset of n pages |
| `ink.pdf.impose` | pure | `( handle profile -- handle' )` | impose pages into signatures per profile |
| `ink.pdf.cover` | pure | `( handle isbn -- handle' )` | build a cover PDF (page count/title/author + `cover:` config) |
| `ink.pdf.barcode` | pure | `( isbn -- handle )` | build a standalone EAN-13 barcode PDF |
| `ink.pdf.preflight` | pure | `( handle profile -- warning_count )` | preflight against a profile (0 = print-ready) |
| `ink.pdf.grayscale` | pure | `( handle -- handle )` | convert to grayscale in place |
| `ink.pdf.optimize` | pure | `( handle -- handle )` | prune + compress in place |
| `ink.pdf.watermark` | pure | `( handle text -- handle )` | stamp a centred 45° watermark on every page |
| `ink.pdf.set_title` | pure | `( handle title -- handle )` | set the Title metadata |
| `ink.pdf.set_author` | pure | `( handle author -- handle )` | set the Author metadata |
| `ink.pdf.strip_metadata` | pure | `( handle -- handle )` | strip all document metadata |

## Bibliography, glossary & reuse

**`ink.sources.*` — bibliography**

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.sources.list` | store_read | `( -- list )` | every entry as a summary `{key, type, author, title, year, chapter}` |
| `ink.sources.get` | store_read | `( key -- dict \| NODATA )` | the full entry (case-sensitive key) |
| `ink.sources.bibtex` | store_read | `( -- string )` | compiled BibTeX for all defined entries |
| `ink.sources.check` | store_read | `( -- list )` | undefined `@key` citations in prose `{key, book, paragraph}` |

**`ink.terms.*` — glossary**

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.terms.list` | store_read | `( -- list )` | every entry `{term, definition, synonyms, scope, note}` |
| `ink.terms.get` | store_read | `( term -- dict \| NODATA )` | the entry for a canonical term |
| `ink.terms.check` | store_read | `( book_slug -- list )` | banned-synonym findings `{path, line, synonym, canonical}` |
| `ink.terms.declare_intent` | store_write | `( canonical scope -- )` | declare a canonical term a deliberate variant |

**`ink.snippets.*` — reuse**

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.snippets.list` | store_read | `( -- list )` | every snippet `{slug, title}` |
| `ink.snippets.get` | store_read | `( slug -- dict \| NODATA )` | the snippet `{slug, title, body}` |
| `ink.snippets.check` | store_read | `( -- list )` | missing `#include` references `{slug, path, line}` |

## ConLang suite — `ink.lang.*`

Observe and analyse an existing constructed language; it never generates prose
except where AI-backed (`ai_write`). Reads are `store_read`; lexicon/book
mutations are `store_write`; file/font output is `fs_write`.

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.lang.list` | store_read | `( -- list )` | all language book names |
| `ink.lang.stats` | store_read | `( lang -- dict )` | phonological/lexical profile statistics |
| `ink.lang.audit` | store_read | `( lang -- dict )` | lexicon health audit (homophones, gaps, anomalies) |
| `ink.lang.query` | store_read | `( lang text -- list )` | filter dictionary entries by headword/gloss |
| `ink.lang.gaps` | store_read | `( lang scope -- dict )` | lexical gaps vs a scope (`swadesh_100` or file) |
| `ink.lang.generate_word` | store_read | `( lang role seed -- word )` | one phonotactically-valid word for a template role |
| `ink.lang.names` | store_read | `( lang count seed -- list )` | deterministic proper names |
| `ink.lang.ipa` | store_read | `( lang word -- ipa )` | surface IPA (allophony applied) |
| `ink.lang.syllabify` | store_read | `( lang word -- list )` | split a word into syllables |
| `ink.lang.stress` | store_read | `( lang word -- string )` | mark primary stress |
| `ink.lang.tone` | store_read | `( lang tones -- result )` | apply tone sandhi to a tone sequence |
| `ink.lang.transliterate` | store_read | `( lang text -- script )` | transliterate into the conscript |
| `ink.lang.gloss` | store_read | `( lang text -- string )` | interlinear-gloss a conlang text |
| `ink.lang.derive` | store_read | `( lang root gloss pos -- list )` | derive words from a root (observe only) |
| `ink.lang.paradigm` | store_read | `( lang root template gloss -- list )` | generate an inflectional paradigm |
| `ink.lang.agree` | store_read | `( lang word pos features -- dict )` | inflect for agreement features |
| `ink.lang.sentence` | store_read | `( lang subject verb object -- dict )` | assemble a clause (surface/gloss/literal) |
| `ink.lang.relative` | store_read | `( lang head role verb with relativizer -- dict )` | build a relative-clause noun phrase |
| `ink.lang.complement` | store_read | `( lang subj verb comp comp-subj comp-verb comp-obj -- dict )` | assemble a complement clause |
| `ink.lang.coordinate` | store_read | `( lang clause-list conjunction -- dict )` | coordinate clauses with a conjunction |
| `ink.lang.translate` | store_read | `( lang text -- dict )` | English → conlang (RBMT + memory): surface/gloss/confidence/alts |
| `ink.lang.reverse` | store_read | `( lang surface -- dict )` | conlang → English (RBMT) |
| `ink.lang.cross` | store_read | `( from to surface -- dict )` | cross-translate conlang `from` to `to` via English |
| `ink.lang.eval` | store_read | `( lang -- dict )` | translation quality (coverage + round-trip similarity) |
| `ink.lang.corpus` | store_read | `( lang -- dict )` | synthetic corpus over the bundled English pool |
| `ink.lang.memory` | store_read | `( lang -- list )` | the translation memory's confirmed pairs |
| `ink.lang.prose` | store_read | `( lang count seed -- list )` | deterministic sample sentences |
| `ink.lang.poem` | store_read | `( lang meter seed -- list )` | deterministic poem lines to a syllable meter |
| `ink.lang.sound_change` | store_read | `( lang form -- form' )` | evolve a proto-form through the daughter's rule chain |
| `ink.lang.cognates` | store_read | `( proto form -- list )` | each daughter language's reflex of a proto-form |
| `ink.lang.family_tree` | store_read | `( -- string )` | render the language family tree |
| `ink.lang.borrow` | store_read | `( lang donor-form -- dict )` | nativise a loanword into the recipient's phonology |
| `ink.lang.varieties` | store_read | `( lang -- list )` | varieties/lects with axes, prestige, change counts |
| `ink.lang.lect` | store_read | `( lang variety word -- rendered )` | render a base form in a named variety |
| `ink.lang.idiolect` | store_read | `( character word -- rendered )` | render a form in a character's idiolect |
| `ink.lang.areal` | store_read | `( lang -- dict )` | areal-convergence overlay (region, contact langs, features) |
| `ink.lang.ecology` | store_read | `( -- dict )` | speech-community ecology (places/characters + their languages) |
| `ink.lang.init` | store_write | `( name -- )` | create a new language book scaffold |
| `ink.lang.define` | store_write | `( lang chapter block -- )` | write an HJSON block as a paragraph under a book chapter |
| `ink.lang.add_word` | store_write | `( lang word pos translation -- )` | add a headword to the dictionary |
| `ink.lang.remove_word` | store_write | `( lang word -- )` | remove a headword |
| `ink.lang.derive_add` | store_write | `( lang root gloss pos -- count )` | derive words from a root AND commit them |
| `ink.lang.grammar_set` | store_write | `( lang feature value -- )` | set a typology/grammar feature |
| `ink.lang.idiom_add` | store_write | `( lang form literal meaning -- )` | add an idiom |
| `ink.lang.metaphor_add` | store_write | `( lang source target -- )` | add a conceptual metaphor |
| `ink.lang.remember` | store_write | `( lang english conlang -- )` | remember a confirmed translation pair |
| `ink.lang.compose` | ai_write | `( lang kind provider -- text )` | AI-composed themed text (blessing/curse/…); advisory |
| `ink.lang.generate_lexicon` | ai_write | `( lang topic count provider -- list )` | AI-assigned meanings on generated forms; advisory |
| `ink.lang.realism_check` | ai_write | `( lang provider -- text )` | AI plausibility assessment of the sound-change chain |
| `ink.lang.reconstruct` | ai_write | `( forms gloss provider -- text )` | AI-reconstruct a proto-form from cognates |
| `ink.lang.dictionary` | fs_write | `( lang format out font -- path )` | render the lexicon to a dictionary file |
| `ink.lang.grammar_book` | fs_write | `( lang format out font -- path )` | render a full grammar book |
| `ink.lang.export` | fs_write | `( lang out-path -- path )` | export the translation system as a `.itm` bundle |
| `ink.lang.font_build` | fs_write | `( lang format out -- stem )` | compile the writing system's glyphs into a font |
| `ink.lang.glyph_lint` | fs_read | `( svg-path -- dict )` | lint an SVG for font suitability |
| `ink.lang.glyph_draft` | fs_write | `( lang describe phoneme out provider -- path )` | AI-draft a glyph SVG (also needs ai_write) |
| `ink.lang.dict` | pure | `( list -- dict )` | build a dict Value from a flat `[k v …]` list |

## Intelligence readers

Advisory readers over the manuscript. Almost all are `store_read` that recompute
derived caches (never the manuscript); a few `suppress`/`promote`/`dismiss` are
`store_write`; the LLM engagements are `ai_write`.

**`ink.graph.*` — SEMNET / GRAPHMIND, the knowledge graph**

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.graph.stats` | store_read | `( -- dict )` | node/edge counts + per-kind breakdown |
| `ink.graph.neighbors` | store_read | `( node -- list )` | one-hop edges (kind, dir, other, reason) |
| `ink.graph.contradicting` | store_read | `( node -- list )` | stance-clash edges touching a node |
| `ink.graph.loci` | store_read | `( node -- list )` | cited primary-source loci `{key, locus}` |
| `ink.graph.paths` | store_read | `( from to -- list \| nil )` | bounded citation/link path between two nodes |
| `ink.graph.pending` | store_read | `( -- list )` | the judged-edge inbox `{id, kind, src, dst, reason}` |
| `ink.graph.rebuild` | store_write | `( -- dict )` | re-derive structural edges `{cleared, added}` |
| `ink.graph.promote` | store_write | `( edge -- bool )` | promote a judged edge |
| `ink.graph.dismiss` | store_write | `( edge -- )` | delete a stance edge |

**`ink.chorus.*` — CHORUS, voice & style at book scale**

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.chorus.voices` | store_read | `( -- list )` | per-character voice fingerprints |
| `ink.chorus.distinct` | store_read | `( -- dict )` | voice distinctiveness matrix (indistinguishable/closest pairs) |
| `ink.chorus.drift` | store_read | `( -- list )` | per-character voice drift (metric deltas crossing thresholds) |
| `ink.chorus.headhops` | store_read | `( -- list )` | POV / head-hop findings per scene |
| `ink.chorus.tense` | store_read | `( -- dict )` | tense summary + per-scene slips (or why unsupported) |
| `ink.chorus.register` | store_read | `( -- dict )` | per-chapter register + register drifts |

**`ink.continuity.*` — SENTINEL, deterministic continuity ledger**

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.continuity.findings` | store_read | `( -- list )` | ranked continuity findings `{kind, severity, chapter, source, message, entities}` |
| `ink.continuity.check` | store_read | `( -- dict )` | summary `{total, contradictions, warnings, info, by_kind}` |

**`ink.readthrough.*` — LECTOR, forward read-through**

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.readthrough.report` | store_read | `( -- list )` | ranked reader findings `{kind, severity, chapter, source, message, entities}` |
| `ink.readthrough.curve` | store_read | `( -- list )` | per-chapter shape `{chapter, title, position, measured, expected, kind}` |
| `ink.readthrough.check` | store_read | `( -- dict )` | summary `{chapters, findings, concerns, notices, info, by_kind}` |

**`ink.revise.*` — REDLINE, unified revision worklist**

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.revise.findings` | store_read | `( -- list )` | worklist `{category, severity, response, location, message, source}` |
| `ink.revise.check` | store_read | `( -- dict )` | gate `{findings, high, med, low, clean, by_response, by_category}` |

**`ink.chronicle.*` — CHRONICLE, draft-history intelligence**

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.chronicle.marks` | store_read | `( -- list )` | milestones `{label, ts, book, findings, errors, warnings, infos}` |
| `ink.chronicle.trend` | store_read | `( -- dict )` | live-vs-latest trend (recaptures current state) |
| `ink.chronicle.check` | store_read | `( -- dict )` | gate `{baseline, cleared, introduced, introduced_errors, clean}` |

**`ink.knowledge.*` — KEN, epistemic continuity**

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.knowledge.grants` | store_read | `( -- list )` | who-could-know-what ledger `{character, topic, chapter, source}` |
| `ink.knowledge.findings` | store_read | `( -- list )` | epistemic breaks `{kind, severity, chapter, character, topic, message}` |
| `ink.knowledge.check` | store_read | `( -- dict )` | gate `{premature, leaked, dropped, clean}` |

**`ink.bonds.*` — BONDS, relationship continuity**

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.bonds.ties` | store_read | `( -- list )` | declared bond ledger `{a, b, kind, chapter}` |
| `ink.bonds.findings` | store_read | `( -- list )` | relationship breaks `{kind, severity, chapter, a, b, message}` |
| `ink.bonds.check` | store_read | `( -- dict )` | gate `{unwritten, unearned, dropped, clean}` |

**`ink.prose.*` — NARR, narrative-voice profiling** · **`ink.dialogue.*` — DIALOG**

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.prose.profile` | store_read | `( -- list )` | stored per-scope voice profiles |
| `ink.prose.drift` | store_read | `( -- list )` | per-chapter metric deltas vs the baseline chapter |
| `ink.prose.violations` | store_read | `( -- list )` | threshold crossings `{chapter, metric, baseline, value, delta}` |
| `ink.prose.refresh` | store_read | `( -- count )` | recompute the voice-profile cache (not the manuscript) |
| `ink.dialogue.stats` | store_read | `( -- list )` | per-chapter dialogue stats |
| `ink.dialogue.fingerprint` | store_read | `( -- list )` | per-character dialogue fingerprints |
| `ink.dialogue.spans` | store_read | `( -- list )` | every detected span (chapter, para, form, speech, attribution, tag) |
| `ink.dialogue.violations` | store_read | `( -- list )` | chapters with a zero-attribution or talking-head violation |
| `ink.dialogue.refresh` | store_read | `( -- count )` | recompute the dialogue cache; return #findings |

**`ink.stylist.*` — Inner Stylist** · **`ink.myth.*` — MYTH** · **`ink.char.*` — character arcs** · **`ink.utopia.*` — WORLD-6 coherence**

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.stylist.findings` | store_read | `( -- list )` | synthesised Praise/Note/Concern findings minus suppressions |
| `ink.stylist.suppressions` | store_read | `( -- list )` | the currently silenced finding keys |
| `ink.stylist.suppress` | store_write | `( key -- )` | silence a finding by key |
| `ink.stylist.unsuppress` | store_write | `( key -- )` | restore a suppressed finding |
| `ink.myth.symbols` | store_read | `( -- list )` | declared symbols `{para_id, vocabulary, meaning, valence, traditions}` |
| `ink.myth.motifs` | store_read | `( -- list )` | declared motifs `{para_id, name, description, valence}` |
| `ink.myth.archetypes` | store_read | `( -- list )` | declared archetypes `{para_id, role, character, function}` |
| `ink.myth.density` | store_read | `( -- list )` | per-symbol per-chapter occurrence counts |
| `ink.myth.findings` | store_read | `( -- list )` | deterministic findings `{id, type, description, evidence, entry_para_id}` |
| `ink.myth.suppress` | store_write | `( finding -- bool )` | suppress a finding by id |
| `ink.char.arc` | store_read | `( -- list )` | per-character arc summary `{character, arc_type, chapters, changes, mean_agency}` |
| `ink.char.stalls` | store_read | `( -- list )` | stall findings `{character, chapter, description}` |
| `ink.char.checks` | store_read | `( -- list )` | arc-completeness checks `{character, check, verdict, problem, chapter, description}` |
| `ink.char.plan` | store_read | `( -- list )` | Planning-Board coverage gaps `{character, type, description}` |
| `ink.char.refresh` | store_read | `( -- count )` | recompute agency + planning layers |
| `ink.utopia.model` | store_read | `( -- list )` | extracted claims `{group, type, text, source_para_id}` |
| `ink.utopia.findings` | store_read | `( -- list )` | unsuppressed findings `{type, domain, group, description, chapter, para_id}` |
| `ink.utopia.violations` | store_read | `( -- list )` | chapter ords having an entailment violation |
| `ink.utopia.suppress` | store_write | `( finding reason -- )` | suppress a finding by id with a reason |

**`ink.theologian.*` / `ink.inner_theologian.*` — Inner Theologian** (aliases of each other)

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.theologian.signals` | store_read | `( -- list )` | fast-track signals `{signal_type, chapter_ord, para_id, description, suppressed}` |
| `ink.inner_theologian.signals` | store_read | `( -- list )` | same as `ink.theologian.signals` |
| `ink.theologian.suppress` | store_write | `( para -- count )` | suppress a paragraph's signals |
| `ink.inner_theologian.suppress` | store_write | `( para -- count )` | same as `ink.theologian.suppress` |

**`ink.poem.*` — inner Poet / poetry engines** (observe/measure, never generate prose)

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.poem.syllable_count` | store_read | `( word lang -- n )` | syllable count of a word |
| `ink.poem.scan_line` | store_read | `( line lang -- dict )` | scan a line's metre |
| `ink.poem.rhyme` | store_read | `( word1 word2 lang -- dict )` | rhyme relation between two words |
| `ink.poem.status` | store_read | `( text form lang -- dict )` | evaluate text against a poetic form |
| `ink.poem.findings` | store_read | `( paragraph_id -- list )` | inner-Poet findings for a paragraph |
| `ink.poem.suppress` | store_write | `( paragraph_id finding_key -- )` | suppress a poem finding |

## Companion inspectors

**`ink.inner_socrates.*` — Inner Socrates**

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.inner_socrates.check.fast` | store_read | `( text -- list )` | run the Fast track on text; push the questions |
| `ink.inner_socrates.findings.list` | store_read | `( -- list )` | persisted findings `{category, severity, question}` |
| `ink.inner_socrates.ledger.list` | store_read | `( -- list )` | intent-ledger entries `{id, kind, description}` |
| `ink.inner_socrates.persona.active` | store_read | `( -- id )` | the active persona's id |
| `ink.inner_socrates.personas.list` | store_read | `( -- list )` | the available persona ids |
| `ink.inner_socrates.usage.today` | store_read | `( -- n )` | slow-track LLM calls made today |

**`ink.inner_editor.*` — Inner Editor**

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.inner_editor.findings.list` | store_read | `( -- list )` | persisted findings `{id, paragraph_id, severity, category, observation}` |
| `ink.inner_editor.usage.today` | store_read | `( -- n )` | today's engagement LLM calls |
| `ink.inner_editor.config` | store_read | `( -- dict )` | the active Inner Editor config |
| `ink.inner_editor.categories` | store_read | `( -- list )` | the eight category ids |
| `ink.inner_editor.suggestions` | store_read | `( -- list )` | promotion candidates `{category, chapter, count}` |
| `ink.inner_editor.system_prompt` | store_read | `( lang -- text )` | the localized Editor system prompt |
| `ink.inner_editor.intent.declare` | store_write | `( category chapter -- )` | declare a category deliberate (empty chapter = project-wide) |
| `ink.inner_editor.engage` | ai_write | `( paragraph_id -- list )` | run one engagement (LLM); push its findings |

**`ink.book_rag.*` — Chat with Your Book** (all `store_read`, local retrieval)

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.book_rag.retrieve` | store_read | `( anchor query -- passages )` | retrieved passages `{id, breadcrumb, body, score, is_hit}` |
| `ink.book_rag.context` | store_read | `( anchor query -- text )` | the composed grounding block |
| `ink.book_rag.scope` | store_read | `( anchor -- ids )` | node ids in the retrieval pool |
| `ink.book_rag.config` | store_read | `( -- dict )` | the active book_rag config |
| `ink.book_rag.system_prompt` | store_read | `( lang -- text )` | the localized grounding prompt (EN/RU/ES/FR/DE) |
| `ink.book_rag.estimate_tokens` | store_read | `( text -- n )` | rough token estimate (≈ chars/4) |
| `ink.book_rag.cited_ids` | store_read | `( passages -- tokens )` | each passage's breadcrumb — the valid-citation set |
| `ink.book_rag.validate_citations` | store_read | `( response tokens -- text )` | flag any `[chapter/scene]` citation not in tokens |

## Feature-coverage words (3.0.5)

Read/observe wrappers over existing features; writes are default-denied.

**`ink.rigor.*` — reasoning-rigor reader (deterministic, zero-AI)**

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.rigor.scan` | store_read | `( -- list )` | argument-rigor findings across user books `{signal, label, chapter, para_id, description}` |
| `ink.rigor.check` | store_read | `( -- dict )` | summary `{findings, clean, by_signal}` |
| `ink.rigor.paragraph` | store_read | `( text -- list )` | scan one passed-in paragraph `{signal, label, description}` |

**`ink.planning.*` — story-structure planner**

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.planning.frameworks` | store_read | `( -- list )` | every framework `{slug, label}` |
| `ink.planning.beats` | store_read | `( framework -- list )` | a framework's canonical beats `{name, act, target_position, expected_tension}` |
| `ink.planning.check` | store_read | `( -- dict )` | structural report for the first user book `{book, beats, gaps, acts, warnings, scenes, clean, tension}` |
| `ink.planning.gaps` | store_read | `( -- list )` | the unmapped-beat names |

**`ink.cost.*` — AI cost ledger** · **`ink.goals.*` — writing goals** · **`ink.wordnet.*` — thesaurus**

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.cost.usage` | store_read | `( -- list )` | today's ledger `{category, calls}` |
| `ink.cost.caps` | store_read | `( -- dict )` | configured caps `{world, inner_socrates, inner_editor, retention_days}` |
| `ink.cost.today` | store_read | `( -- dict )` | today's spend `{day, total_calls, entries}` |
| `ink.goals.streak` | store_read | `( -- dict )` | writing streak `{days, best, grace_used, grace_per_week}` |
| `ink.goals.snapshot` | store_read | `( -- dict )` | full pacing dashboard `{project, books, status, streak, sparkline, …}` |
| `ink.wordnet.list` | store_read | `( -- list )` | sources `{lang, name, installed}` |
| `ink.wordnet.lookup` | store_read | `( word lang -- dict )` | senses `{word, senses:[{pos, definition, synonyms, antonyms, hypernyms, hyponyms}]}` |
| `ink.wordnet.suggest` | store_read | `( word lang -- list )` | replacement pick-list `{kind, word}` |

**`ink.companions.*` — examined-authorship cockpit** · **`ink.research.*` — evidence base**

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.companions.findings` | store_read | `( -- dict )` | open findings `{socrates:[…], editor:[…]}` |
| `ink.companions.promotions` | store_read | `( -- dict )` | promotion candidates `{socrates:[…], editor:[…]}` |
| `ink.companions.world` | store_read | `( -- dict )` | World story-bible health |
| `ink.companions.summary` | store_read | `( -- dict )` | the whole cockpit in one call |
| `ink.research.facts` | store_read | `( -- list )` | disputed Facts `{id, location, text}` |
| `ink.research.undisputed` | store_read | `( -- list )` | the `fact:undisputed` authorial facts |
| `ink.research.provenance` | store_read | `( node-id -- dict \| NODATA )` | where a fact came from `{origin, detail, query, thread, created_at, summary}` |
| `ink.research.sources` | store_read | `( query k -- list )` | source chunks near query `{name, body}` (k clamped ≤ 200) |
| `ink.research.report` | store_read | `( -- string )` | the persisted SCHOLAR report |

**`ink.locorum.*` / `ink.verborum.*` — scholarly indexes**

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.locorum.build` | store_read | `( -- list )` | Index Locorum `{key, title, loci:[{locus, chapters, valid}]}` |
| `ink.locorum.malformed` | store_read | `( -- list )` | loci failing their scheme `{key, title, locus, expected}` |
| `ink.locorum.render` | store_read | `( fmt -- string )` | compiled index (`md`\|`typst`\|`json`) |
| `ink.verborum.build` | store_read | `( -- list )` | Index Verborum `{term, original_forms, senses, chapters}` |
| `ink.verborum.render` | store_read | `( fmt -- string )` | compiled index (`md`\|`typst`\|`json`) |

**`ink.doctor.*` — project health** · **`ink.backup.*` — backups** · **introspection**

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.doctor.integrity` | store_read | `( -- dict )` | DuckDB integrity `{meta, blobs, ok}` |
| `ink.doctor.vectors` | store_read | `( -- dict )` | vector-index parity `{status, rows, vectors, detail}` |
| `ink.doctor.scan` | store_read | `( -- list )` | full project scan `{class, severity, path, detail}` |
| `ink.doctor.autofix` | store_write | `( -- dict )` | scan + apply auto-fixable repairs `{applied, failed, results}` |
| `ink.backup.last` | store_read | `( -- dict \| NODATA )` | last-backup timestamp `{last_at}` |
| `ink.backup.list` | fs_read | `( -- list )` | backup zips, newest first `{name, bytes, modified}` |
| `ink.backup.make` | fs_write | `( -- dict )` | create a backup zip now `{archive, kept}` |
| `ink.words` | pure | `( prefix -- list )` | registered `ink.*` words `{word, category}` starting with `prefix` ("" = all) |

**`ink.import.*` — importers (writes).** Enable with `enabled_categories:
["store_write"]`; an out-of-project bundle also needs `fs_unsandboxed`.

| Word | Category | Stack | Description |
|------|----------|-------|-------------|
| `ink.import.scrivener` | store_write | `( path -- dict )` | import a `.scriv` `{books_created, chapters_created, subchapters_created, paragraphs_created, paragraphs_skipped, errors}` |
| `ink.import.scrivener_preview` | fs_read | `( path -- dict )` | dry-run a `.scriv` import (same shape; writes nothing) |
| `ink.import.epub` | store_write | `( path -- dict )` | import an `.epub` `{book_title, author, chapters_created, paragraphs_created, images_imported, images_extracted, errors}` |
