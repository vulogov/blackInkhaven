// Inkhaven 1.2.4 — printable cheat sheet
// Compile: `typst compile Documentation/INKHAVEN_CHEAT_SHEET.typ`
// Or in the TUI: open this file via F3 then `Ctrl+B B` to build.

#set document(
  title: "Inkhaven 1.2.4 — Cheat Sheet",
  author: "Inkhaven",
)
#set page(
  paper: "a4",
  margin: (top: 1.2cm, bottom: 1.2cm, left: 1.2cm, right: 1.2cm),
  footer: context [
    #set text(7pt, fill: luma(120))
    Inkhaven 1.2.4 cheat sheet  ·  page #counter(page).display() / #counter(page).final().first()
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

= Inkhaven 1.2.4 — Cheat Sheet

A one-page reference. For workflows see
`Documentation/Tutorials/`. For the full chord list see
`Documentation/KEYBINDING.md`.

#columns(2, gutter: 0.6cm)[

== Global

#kv-table(
  row([Ctrl+S], [Save current paragraph]),
  row([Ctrl+/], [Focus search bar]),
  row([Ctrl+I], [Focus AI prompt]),
  row([Tab],    [Cycle Tree → Editor → AI]),
  row([Shift+Tab], [Cycle in reverse]),
  row([Ctrl+Q], [Hard quit]),
  row([Q],      [Quit (tree pane; autosaves)]),
)

== Meta — Ctrl+B prefix

#kv-table(
  row([B / C / S / P], [Add Book / Chapter / Subchapter / Paragraph]),
  row([D],   [Delete cursor node (confirm modal)]),
  row([R],   [Cycle workflow status]),
  row([N],   [Snapshot current buffer (= F5)]),
  row([F],   [Typst function picker]),
  row([T],   [Re-title from first sentence]),
  row([P],   [Places RAG / image picker]),
  row([C],   [Characters RAG]),
  row([G],   [Notes RAG]),
  row([Y],   [Artefacts RAG]),
  row([M],   [Cycle leaf type (¶ / json / .bund)]),
  row([H],   [Quick-reference overlay]),
  row([L],   [Live-switch LLM provider]),
  row([I],   [Book info]),
  row([V],   [Credits / version]),
  row([A / B / O], [Assemble / Build (typst) / Take (PDF→cwd)]),
  row([W / K], [Typewriter / AI full-screen]),
  row([E],   [Toggle typewriter SFX]),
  row([1..7],[Status-filter modal]),
  row([↑ / ↓], [Reorder cursor row]),
)

== View — Ctrl+V prefix (1.2.4)

#kv-table(
  row([1 / 2], [Md export: buffer / subchapter (save-as)]),
  row([1 (tree)], [Md export: subtree (save-as)]),
  row([S],   [Toggle similar-paragraph mode]),
  row([G],   [Writing-progress modal]),
  row([T],   [Per-paragraph word target]),
  row([A / I], [Add outgoing / incoming wiki-link]),
  row([L / K], [List outgoing / backlinks]),
  row([B / M], [Toggle bookmark / open picker]),
  row([P],   [Fuzzy paragraph picker]),
  row([Esc], [Cancel chord]),
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
  row([F4],  [Toggle split-edit]),
  row([F5],  [Save versioned snapshot]),
  row([F6],  [Snapshot picker — V diff, D del, Enter restore (1.2.4: pre-restore safety snapshot)]),
  row([F7],  [Grammar check]),
  row([F9],  [Cycle AI scope]),
  row([F10], [Toggle inference mode]),
)

== Tree pane

#kv-table(
  row([↑ ↓], [Move cursor]),
  row([→ / ←], [Expand / collapse (or up to parent)]),
  row([Enter], [Open paragraph]),
  row([Home / End], [Jump first / last]),
  row([Z / X], [Collapse subchapter / collapse all]),
  row([B C V A S +], [Add / insert variants]),
  row([D / -], [Delete branch / paragraph]),
  row([U / J], [Move up / down]),
  row([Space], [(1.2.4) Mark / unmark for multi-select]),
  row([T],     [(1.2.4) Cycle type (single / bulk)]),
  row([O],     [(1.2.4) Cycle status (single / bulk)]),
  row([Esc],   [Clear marks · focus search]),
)

== Editor pane

#kv-table(
  row([Ctrl+F / Ctrl+R], [Find / replace (regex)]),
  row([Ctrl+G],   [Repeat last find]),
  row([Ctrl+Z / Ctrl+Y], [Undo / redo]),
  row([Ctrl+A / Ctrl+E], [Line start / end]),
  row([Ctrl+K],   [Kill to end of line]),
  row([Ctrl+U],   [Kill to start of line]),
  row([Ctrl+W],   [Delete word backward]),
  row([Alt+B / Alt+F], [Word back / forward]),
  row([Shift+arrows], [Selection]),
  row([Ctrl+Space], [Vertical block selection mode]),
)

== Snapshot picker (F6)

#kv-table(
  row([↑ ↓], [Navigate]),
  row([Enter], [Restore (1.2.4: safety snapshot first)]),
  row([V],   [(1.2.4) Side-by-side diff vs current]),
  row([D / Del], [Delete snapshot]),
  row([Esc], [Cancel · diff returns to picker]),
)

== AI prompt (Ctrl+I)

#kv-table(
  row([Enter], [Send]),
  row([↑ / ↓], [(1.2.4) Prompt history (cap 500)]),
  row([/],     [Prompt-library picker (prefix-ranked)]),
  row([Tab],   [Commit picker selection]),
  row([Esc],   [Close]),
)

== AI full-screen (Ctrl+B K)

#kv-table(
  row([Ctrl+F], [Search chat history]),
  row([Ctrl+C], [Selection mode — copy / insert turns]),
  row([F9 / F10], [Scope / inference mode]),
  row([Esc],   [Leave full-screen]),
)

== CLI cheatsheet

#kv-table(
  row([init `<path>`], [Create project]),
  row([add ¶ "title" --parent path], [Add a node]),
  row([list],                        [Print hierarchy]),
  row([search "query"],              [Semantic search]),
  row([reindex --prune --adopt],     [Reconcile disk ↔ store]),
  row([export typst|pdf|markdown|tex|epub], [Export]),
  row([export --status=ready],       [(1.2.4) Status floor]),
  row([export --book-name "Tides"], [Scope to one book]),
  row([backup --out DIR],            [Backup zip]),
  row([restore ARCH --to DIR],       [Restore zip]),
  row([import-help --documents-directory DIR], [Wipe + import Help]),
  row([import-typst-help],           [Bundle Typst reference]),
  row([import-scrivener PATH.scriv], [(1.2.4) Scrivener importer]),
  row([stats --book-name "Tides"], [(1.2.4) Per-¶ stats table]),
  row([ai "prompt"],                 [One-shot inference]),
  row([bund "40 2 + ."],             [Bund REPL one-shot]),
)

== HJSON quick reference

```hjson
keys: {
  meta_prefix: "Ctrl+b"
  bund_prefix: "Ctrl+z"
  view_prefix: "Ctrl+v"   // 1.2.4
}
editor: {
  autosave_seconds: 5
  startup_splash:   true  // 1.2.4
}
goals: {
  daily_words:           1500
  active_minutes_daily:  60    // 1.2.4
  streak_grace_per_week: 1
  auto_promote_on_target: true // 1.2.4
  books: { tides: { target_words: 80000,
                    deadline: "2026-12-31" } }
}
output: { extra_formats: ["markdown", "epub"] }
scripting: {
  enabled_categories: ["keymap"]
  // 1.2.4 adds fs_read (default-allowed),
  //           fs_write (default-DENIED)
}
```

== Bund stdlib (1.2.4 additions)

#kv-table(
  row([ink.editor.replace_all], [`( old new -- count )`]),
  row([ink.search.load],        [`( query -- )` · top hit → editor]),
  row([ink.ai.send_blocking],   [`( prompt -- response )`]),
  row([ink.ai.poll],            [`( -- string )` · empty if none]),
  row([ink.fs.read],            [`( path -- string )` · allowed]),
  row([ink.fs.write],           [`( path content -- )` · DENIED]),
  row([ink.paragraph.target],   [`( path -- int | NODATA )`]),
  row([ink.paragraph.set_target], [`( path n -- )` · 0 clears]),
)

== Hook points (1.2.4)

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
)

]

#v(0.3em)
#line(length: 100%, stroke: 0.4pt)
#text(7pt, fill: luma(120))[
  Tutorials referenced above (`Documentation/Tutorials/`):
  01 getting-started · 02 organising · 03 editor · 04 search · 05 AI ·
  06 grammar · 07 places + characters · 08 importing · 09 export ·
  10 backup · 11 theming · 12 providers · 13 AI full-screen ·
  14 status · 15 multi-format export · 16 similar paragraphs ·
  17 writing goals · 18 Bund pane · *19 wiki-links · 20 snapshot diff ·
  21 navigation · 22 tree multi-select · 23 Scrivener import*.
]
