// Inkhaven 1.2.13 — printable cheat sheet
// Compile: `typst compile Documentation/INKHAVEN_CHEAT_SHEET.typ`
// Or in the TUI: open this file via F3 then `Ctrl+B B` to build.

#set document(
  title: "Inkhaven 1.2.13 — Cheat Sheet",
  author: "Inkhaven",
)
#set page(
  paper: "a4",
  margin: (top: 1.2cm, bottom: 1.2cm, left: 1.2cm, right: 1.2cm),
  footer: context [
    #set text(7pt, fill: luma(120))
    Inkhaven 1.2.13 cheat sheet  ·  page #counter(page).display() / #counter(page).final().first()
  ],
)
#set text(font: "New Computer Modern", size: 8pt)
#show heading.where(level: 1): it => [
  #set text(13pt, weight: "bold")
  #v(0.15em)
  #it.body
  #v(0.2em)
  #line(length: 100%, stroke: 0.5pt)
]
#show heading.where(level: 2): it => [
  #set text(9.5pt, weight: "bold")
  #v(0.2em)
  #it.body
]

#let chord(c) = box(
  inset: (x: 3pt, y: 1pt),
  outset: (y: 1pt),
  radius: 2pt,
  fill: luma(235),
  text(font: "DejaVu Sans Mono", size: 7.5pt, c),
)

#let row(c, a) = (chord(c), a)

#let kv-table(..rows) = table(
  columns: (auto, 1fr),
  stroke: none,
  inset: (x: 3pt, y: 1.5pt),
  align: (left, left),
  ..rows.pos().flatten(),
)

= Inkhaven 1.2.13 — Cheat Sheet

A two-page reference. For workflows see
`Documentation/Tutorials/` and the *Book of Inkhaven* at
`Book/`. For the full chord list see
`Documentation/KEYBINDING.md`.

#text(size: 7.5pt, style: "italic")[
  Layout: search input at the top, three panes in the middle
  (tree · editor · AI — temporary modal panes can replace
  one or all), AI prompt input at the bottom, status line
  beneath it.
]

#columns(2, gutter: 0.6cm)[

== Global

#kv-table(
  row([Ctrl+S], [Save current paragraph]),
  row([Ctrl+/], [Focus search bar (top)]),
  row([Ctrl+I], [Focus AI prompt (bottom)]),
  row([Tab],    [Cycle Tree → Editor → AI]),
  row([Shift+Tab], [Cycle in reverse]),
  row([Ctrl+1..5], [Focus Editor / Tree / AI / Search / AI prompt]),
  row([Ctrl+Q], [Quit (autosaves dirty paragraph)]),
)

== Meta — Ctrl+B prefix

#kv-table(
  row([B / C / S / P], [Add Book / Chapter / Subchapter / Paragraph]),
  row([D],   [Delete cursor node (confirm modal)]),
  row([R / Shift+R],   [Cycle workflow status fwd / back]),
  row([N],   [Snapshot current buffer (= F5)]),
  row([F],   [Typst function picker]),
  row([T],   [Re-title from first sentence]),
  row([P],   [Places RAG / image picker]),
  row([C],   [Characters RAG  (or: clear chat history)]),
  row([G],   [Notes RAG]),
  row([Y],   [Artefacts RAG]),
  row([M],   [Cycle leaf type · or inference-mode read]),
  row([H],   [Quick-reference overlay (auto-pulls all chord layers)]),
  row([L],   [Live-switch LLM provider]),
  row([I],   [Book info]),
  row([V],   [Credits / version (with embedded logo)]),
  row([A / B / O], [Assemble / Build (PDF) / Take (extra formats)]),
  row([Shift+B], [(1.2.6) Backup the project now (zip archive)]),
  row([W / K], [Typewriter mode / AI full-screen]),
  row([E],   [Toggle typewriter SFX]),
  row([1..7],[Status-filter modal]),
  row([↑ / ↓], [Reorder cursor row]),
  row([\]],   [(1.2.5) Tag picker for open ¶]),
  row([\}],   [(1.2.5) Search-by-tag picker]),
  row([U],   [(1.2.7) Undo last paragraph delete]),
  row([0],   [(1.2.8) Full-screen HJSON editor for inkhaven.hjson]),
  row([Shift+G], [(1.2.9) Writing-streak heatmap (last 91 days)]),
  row([Shift+F], [(1.2.9) Toggle style-warning overlays]),
  row([< / >], [(1.2.9) Prev / next scene-break line]),
  row([Shift+H], [(1.2.9) Sentence-rhythm gauge]),
  row([Shift+L], [(1.2.9) Concordance modal · Enter jumps to source (1.2.11)]),
  row([Shift+P], [(1.2.9) POV / character chip toggle]),
  row([Shift+T], [(1.2.9) AI show-don't-tell scan]),
  row([Shift+M], [(1.2.11) AI sentence-rhythm rewrite · auto-diff · snapshot on accept]),
  row([Shift+N], [(1.2.12) Toggle prompt-language mode (book ↔ paragraph-detected)]),
  row([Q],   [(1.2.13) Translate open ¶ INTO an invented language (picker on ambiguity)]),
  row([Shift+Q], [(1.2.13) Translate open ¶ FROM invented back to working language]),
)

Inside the tag picker (`Ctrl+B ]` / `Ctrl+B }`):
#kv-table(
  row([Space], [Toggle cursor tag]),
  row([A],     [Add new tag (one-line prompt)]),
  row([R],     [(1.2.6) Rename project-wide (merges if exists)]),
  row([D],     [Delete project-wide (confirm)]),
  row([T],     [Commit marked tags to target]),
  row([Enter (search)], [Open per-tag paragraph list]),
)

== View — Ctrl+V prefix

#kv-table(
  row([1 / 2], [Md export: buffer / subchapter]),
  row([1 (tree)], [Md export: subtree]),
  row([S],   [Toggle similar-paragraph mode]),
  row([G],   [Writing-progress modal]),
  row([t],   [Per-¶ word-count target modal]),
  row([Shift+T], [(1.2.6) Timeline swim-lane view]),
  row([A / I], [Add outgoing / incoming wiki-link]),
  row([L / K], [List outgoing / backlinks]),
  row([B / M], [Toggle bookmark / open picker]),
  row([P],   [Fuzzy paragraph picker]),
  row([R],   [Render paragraph → floating PNG (1.2.5)]),
  row([N / Shift+N], [Next / previous typst diagnostic]),
  row([Shift+W], [Story view — book graph (1.2.5)]),
  row([w],   [(1.2.6) Paragraph mini story view]),
  row([e],   [(1.2.6) Event picker — chronological list]),
  row([Shift+E], [(1.2.6) New event (opens timeline + prompts for title)]),
  row([Shift+I], [(1.2.6) Edit open event's start | end | track]),
  row([Shift+P], [(1.2.7) Recent paragraphs picker (mod-time desc)]),
  row([Shift+U], [(1.2.8) Kill-ring picker (deleted-paragraph history)]),
  row([Shift+B], [(1.2.12) Sibling-book lookup — pin same-slug ¶ from other book]),
  row([Esc], [Cancel chord]),
)

== Render preview (Ctrl+V R)

#kv-table(
  row([← / →], [Previous / next page]),
  row([Home / End], [First / last page]),
  row("+ / =",  [(1.2.6) Zoom in (0.66× ticks/cell)]),
  row("- / _",  [(1.2.6) Zoom out (1.5×)]),
  row([0],      [(1.2.6) Reset zoom to 1.00×]),
  row([S],     [Save *current* page · full-DPI PNG]),
  row([A],     [Save *all* pages]),
  row([Esc],   [Close back to editor]),
)

== Timeline view (Ctrl+V Shift+T) — 1.2.6

#kv-table(
  row([← / →], [Scroll ~10 cells]),
  row([PgUp / PgDn], [Scroll ~60 cells]),
  row("+ / -", [Zoom in / out (anchored to cursor)]),
  row([0],     [Reset zoom]),
  row([Home / End], [Jump first / last event]),
  row([Tab],   [Cycle highlighted track]),
  row([Enter], [Open event closest to cursor]),
  row([n / N], [New event at cursor tick]),
  row([u / U], [Up-scope (subch → ch → book)]),
  row([d / D], [Descent picker]),
  row([b / B], [Book scope]),
  row([p / P], [Project overlay (cross-book)]),
  row([y],     [AI critique · scope + current track]),
  row([Y],     [AI critique · scope + all tracks]),
  row([Ctrl+Y], [AI critique · book-wide]),
  row([Esc],   [Close]),
)

== Event picker (Ctrl+V e) — 1.2.6

#kv-table(
  row([↑ / ↓ / Home / End], [Navigate]),
  row([t / T], [Cycle track filter]),
  row([Enter], [Open event paragraph]),
  row([Esc],   [Close]),
)

== Bund — Ctrl+Z prefix

#kv-table(
  row([R],   [Eval open `.bund` buffer]),
  row([N],   [New Script under Scripts book]),
  row([E],   [One-shot Bund eval modal]),
  row([?],   [Script picker (branch-scoped)]),
)

== Function keys

#kv-table(
  row([F1],  [RAG over Help book]),
  row([F2],  [Rename modal]),
  row([F3],  [File picker (import / load)]),
  row([F4],  [Toggle split-edit (same-paragraph snapshot)]),
  row([Ctrl+F4], [Accept snapshot into live buffer]),
  row([Shift+F4], [(1.2.12) Toggle fullscreen split-view (two paragraphs)]),
  row([F5],  [Snapshot (1.2.6: with annotation prompt)]),
  row([F6],  [Snapshot picker]),
  row([F7],  [Grammar check]),
  row([F8],  [(1.2.6) Diagnostics list modal]),
  row([F9 / Shift+F9],  [Cycle AI scope fwd / back]),
  row([F10], [Toggle inference mode (Local ↔ Full)]),
  row([F12], [(1.2.6) AI critique (mode-aware)]),
  row([Ctrl+F12], [(1.2.6) AI explain diagnostic at cursor — was F11; macOS grabs F11]),
)

== Tree pane

#kv-table(
  row([↑ ↓], [Move cursor]),
  row([→ / ←], [Expand / collapse]),
  row([Enter], [Open paragraph]),
  row([Home / End], [Jump first / last]),
  row([Z / X], [Collapse subchapter / collapse all]),
  row([B C V A S P +], [Add / insert variants]),
  row([D / -], [Delete branch / paragraph]),
  row([U / J], [Move up / down]),
  row([Space], [Mark for multi-select]),
  row([T],     [Cycle type (single / bulk)]),
  row([O],     [Cycle status (single / bulk)]),
  row([g],     [(1.2.5) Tag selection]),
  row([F2],    [Rename]),
  row([F3],    [File picker]),
  row([Esc],   [Clear marks · focus search]),
)

== Editor pane

#kv-table(
  row([Ctrl+F / Ctrl+R], [Find / replace (regex)]),
  row([Ctrl+G],   [Repeat last find]),
  row([Ctrl+A / Ctrl+E], [Line start / end]),
  row([Ctrl+K],   [Kill to end of line]),
  row([Ctrl+U],   [Kill to start of line]),
  row([Ctrl+W],   [Delete word backward]),
  row([Alt+B / Alt+F], [Word back / forward]),
  row([Shift+arrows], [Selection]),
  row([Ctrl+Space], [Vertical block selection]),
)

Editor gutter (1.2.6): red `●` marks lines that carry a typst
diagnostic. Both parse + semantic. Marker keeps colour on the
current-line highlight.

== Snapshot picker (F6)

#kv-table(
  row([↑ ↓], [Navigate]),
  row([Enter], [Restore (safety snapshot fires first)]),
  row([V],   [Side-by-side diff vs current]),
  row([D / Del], [Delete snapshot]),
  row([Home / End], [Newest / oldest]),
  row([Esc], [Cancel]),
)

(1.2.6) Annotated snapshots show an italic-cyan `✎`-prefixed
second line beneath each row.

== Split view (Shift+F4) — 1.2.12

Three-column layout: tree pane · primary editor ·
secondary editor.  AI prompt input still spans the
bottom; AI response pane is hidden.

#kv-table(
  row([Shift+F4], [Toggle fullscreen split-view]),
  row([Shift+Enter], [Pin focused paragraph to secondary pane (universal modifier on tree-Enter, Ctrl+V P / M / Shift+P, Ctrl+V Shift+B)]),
  row([Ctrl+V Shift+B], [Auto-pin same-slug ¶ from another book (single match) or open picker (multi)]),
  row([Tab],   [Swap focus between primary and secondary editors]),
  row([F12],   [Critique-compare prompt fires when both panes hold distinct paragraphs]),
)

Exiting Shift+F4 clears the secondary slot so the
standard layout's AI pane reappears.  Re-pin via any
of the above on next Shift+F4.

== Language book — 1.2.13

Invented-language workbench (Documentation/Tutorials/49,
50). Five chapters per sub-book: Meta · Dictionary ·
Grammar · Phonology · Sample texts.

#kv-table(
  row([Tree b on `Language`], [Scaffold a new language sub-book (5 chapters + seeded Meta/overview)]),
  row([Tree + under Dictionary], [Add dictionary entry — bucket auto-derived, HJSON template seeded]),
  row([Tree + under Grammar / Phonology], [Add rule paragraph — schema-aware HJSON seeded]),
  row([Ctrl+B Q], [Translate ¶ INTO invented language · picker on ambiguity · first-letter jump-and-commit]),
  row([Ctrl+B Shift+Q], [Translate ¶ FROM invented · roundtrip-test workflow]),
  row([AI pane I], [Inserts only the `<<<TRANSLATION>>>` block (chip `translate[on]` shows when extraction is armed)]),
)

CLI:
#kv-table(
  row([`language init <name>`], [Scaffold]),
  row([`language add-word <lang> <w> --type <pos> --translation <t>`], [Single add]),
  row([`language add-word <lang> --import <csv>` `[--new] [--force]`], [Bulk CSV import · `--new` wipes first · `--force` skips alphabet/phonology validation]),
  row([`language remove-word <lang> <w>`], [Delete entry]),
  row([`language list`], [Summary table]),
  row([`language doctor <lang> [--json]`], [Health report — coverage, missing paradigms, manuscript gap]),
  row([`language export <lang> --format <json|anki|dictionary-twocol>`], [Export]),
)

== AI prompt (Ctrl+I)

#kv-table(
  row([Enter], [Send]),
  row([↑ / ↓], [Prompt history (cap 500)]),
  row([/],     [Prompt-library picker]),
  row([Tab],   [Commit picker selection]),
  row([Esc],   [Close]),
)

== AI pane apply chords

#kv-table(
  row([r / R], [Replace buffer (1.2.6: → diff modal)]),
  row([g / G], [Grammar-replace (extracts corrected block)]),
  row([i / I], [Insert at cursor]),
  row([t / T], [Prepend (top)]),
  row([b / B], [Append (bottom)]),
  row([c / C], [Copy to clipboard]),
)

Inside the AI diff modal (1.2.6, when `ai.diff_review_on_apply`):
#kv-table(
  row([a / A / Enter], [Accept — apply + refocus editor]),
  row([r / R], [Reject — buffer unchanged]),
  row([e / E], [Alias for accept]),
  row([↑ ↓ PgUp PgDn], [Scroll the diff]),
  row([Esc],   [Same as reject]),
)

== AI full-screen (Ctrl+B K)

#kv-table(
  row([Ctrl+F], [Search chat history]),
  row([Ctrl+C], [Selection mode — copy / insert turns]),
  row([F9 / F10], [Scope / inference mode]),
  row([Esc],   [Leave full-screen]),
)

== Privacy posture

#text(size: 7.5pt)[
  Inkhaven does *not* provide inherent privacy when external
  LLM providers are used (Gemini, Claude, OpenAI, DeepSeek,
  Grok). Every prompt + RAG-attached paragraph travels to
  the provider's servers per their terms. For increased
  privacy use a *local Ollama* installation — set
  `llm.default_provider: "ollama"` — and inkhaven's RAG,
  embedding, and search stay fully on-device.
]

== CLI cheatsheet

#kv-table(
  row([init `<path>`], [Create project]),
  row([add ¶ "title" --parent path], [Add a node]),
  row([list],                        [Print hierarchy]),
  row([search "query"],              [Semantic search]),
  row([reindex --prune --adopt],     [Reconcile disk ↔ store]),
  row([export typst|pdf|markdown|tex|epub], [Export]),
  row([export --status=ready],       [Status floor]),
  row([export --tag draft],          [(1.2.6) Tag filter]),
  row([export --book-name "Tides"], [Scope to one book]),
  row([backup --out DIR],            [Backup zip]),
  row([restore ARCH --to DIR],       [Restore zip]),
  row([import-help --documents-directory DIR], [Wipe + import Help]),
  row([import-typst-help],           [Bundle Typst reference]),
  row([import-scrivener PATH.scriv], [Scrivener importer (keywords → tags 1.2.6)]),
  row([stats --book-name "Tides"], [Per-¶ stats table]),
  row([doctor],                      [Health report]),
  row([event add "Storm" --start "1A.2.3"], [(1.2.6) Add event]),
  row([event list --track main],     [(1.2.6) Chronological event list]),
  row([event show <path>],           [(1.2.6) Show event details]),
  row([ai "prompt"],                 [One-shot inference]),
  row([bund "40 2 + ."],             [Bund REPL one-shot]),
)

== HJSON quick reference

```hjson
keys: {
  meta_prefix: "Ctrl+b"
  bund_prefix: "Ctrl+z"
  view_prefix: "Ctrl+v"
}
editor: {
  autosave_seconds: 5
  startup_splash:   true
}
goals: {
  daily_words:           1500
  active_minutes_daily:  60
  streak_grace_per_week: 1
  auto_promote_on_target: true
  books: { tides: { target_words: 80000,
                    deadline: "2026-12-31" } }
}
output: { extra_formats: ["markdown", "epub"] }
scripting: {
  enabled_categories: ["keymap"]
  // store_write opens ink.tag.add / ink.event.add etc.
  // fs_write opens ink.story.render, ink.fs.write.
}
typst_compile: {
  engine:               "external"  // | "inprocess"
  diagnostics:          true        // typst-syntax
  semantic_diagnostics: false       // (1.2.5+) full compile
  bundle_fonts:         true        // CM + Linux Libertine
  use_system_fonts:     true
  packages_enabled:     true        // @preview/<pkg>
  wait_for_key_after_compile: true  // (1.2.6) hold splash
}

// (1.2.6) AI behaviour
ai: {
  per_paragraph_memory:           false  // opt in: chat
                                         // continuity per ¶
  per_paragraph_memory_max_turns: 10
  diff_review_on_apply:           true   // r/g via modal
  reseed_prompt_examples:         true
}

// (1.2.6) Story timeline — opt in
timeline: {
  enabled:        false               // ← flip to true
  default_track:  "main"
  calendar: { preset: "gregorian" }   // | "sols" | "custom"
  display: {
    show_orphans:       true
    swim_lane_max_rows: 12
    default_zoom:       1.0
  }
}
```

== Bund stdlib (1.2.6 additions)

#kv-table(
  row([ink.tag.list],           [`( -- list )`]),
  row([ink.tag.list_for],       [`( path -- list | NODATA )`]),
  row([ink.tag.search],         [`( tag -- list )`]),
  row([ink.tag.add],            [`( path tag -- )`  · store_write]),
  row([ink.tag.remove],         [`( path tag -- )`  · store_write]),
  row([ink.event.list],         [`( -- list )`]),
  row([ink.event.list_orphans], [`( -- list )`]),
  row([ink.event.add],          [`( book title spec -- uuid )` · store_write]),
  row([ink.event.set_end],      [`( uuid spec -- )` · store_write]),
  row([ink.event.set_precision], [`( uuid prec -- )` · store_write]),
  row([ink.event.set_track],    [`( uuid track -- )` · store_write]),
  row([ink.event.link_paragraph], [`( uuid path -- )` · store_write]),
  row([ink.story.render],       [`( book path -- )` · fs_write]),
  row([ink.editor.set_cursor],  [`( row col -- )` · 1-based]),
)

== Hook points (1.2.4–1.2.6)

#kv-table(
  row([hook.on_save],      [`( uuid -- )`]),
  row([hook.on_rename],    [`( uuid title -- )`]),
  row([hook.on_snapshot],  [`( parent snap -- )`]),
  row([hook.on_delete],    [`( uuid -- )`]),
  row([hook.on_status_promoted], [`( uuid from to -- )`]),
  row([hook.on_goal_hit],  [`( today goal -- )`]),
  row([hook.on_streak_break], [`( prev_days -- )`]),
  row([hook.on_assemble],  [`( uuid slug root files -- )`]),
  row([hook.on_take],      [`( uuid slug pdf -- )`]),
  row([hook.on_diagnostic], [`( uuid count first-message -- )` · 1.2.6]),
  row([hook.on_event_added], [`( uuid -- )` · 1.2.6]),
  row([hook.on_event_orphaned], [`( uuid -- )` · 1.2.6]),
)

]

#v(0.3em)
#line(length: 100%, stroke: 0.4pt)
#text(7pt, fill: luma(120))[
  Tutorials referenced (`Documentation/Tutorials/`):
  01 getting-started · 02 organising · 03 editor · 04 search · 05 AI ·
  06 grammar · 07 places + characters · 08 importing · 09 export ·
  10 backup · 11 theming · 12 providers · 13 AI full-screen ·
  14 status · 15 multi-format export · 16 similar paragraphs ·
  17 writing goals · 18 Bund pane · 19 wiki-links · 20 snapshot diff ·
  21 navigation · 22 tree multi-select · 23 Scrivener import ·
  24 typst-in-process · *25 tag-workflows · 26 story-view ·
  27 diagnostics · 28 ai-critique-and-memory · 29 snapshot-annotations ·
  30 render-preview · 31 story-timeline*. The full author's guide
  lives at `Book/BOOK_OF_INKHAVEN.typ` (compile → PDF) with a
  markdown mirror at `Book/markdown/`.
]
