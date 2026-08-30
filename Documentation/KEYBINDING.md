# Inkhaven keybinding reference

Every keystroke the TUI recognizes, organized by which pane or overlay has
focus. Keys flagged **configurable** are read from
`<project>/inkhaven.hjson` under the `keys` block; the values below are the
shipping defaults. Everything else is hard-coded and not user-overridable in
this release.

> **Printable companion**: [`INKHAVEN_CHEAT_SHEET.typ`](INKHAVEN_CHEAT_SHEET.typ)
> — a two-column A4 reference. Compile with `typst compile
> Documentation/INKHAVEN_CHEAT_SHEET.typ` or open the file in the TUI
> (F3 → pick → `Ctrl+B B` to build).

The TUI has five focus states (Tree, Editor, AI, Search bar, AI prompt) plus
three transient overlays (Search results, Prompt picker, and a modal stack of
Add prompt / Delete confirm). Overlays absorb keys; the underlying pane keeps
its visual focus state but does not see input until the overlay closes.

---

## Terminal compatibility (the enhanced keyboard protocol)

Some chords need the **kitty keyboard protocol** — a modern terminal extension
that lets an app receive keys the legacy TTY encoding can't represent. Inkhaven
requests it on startup and, when the terminal doesn't answer (crossterm's
`supports_keyboard_enhancement()` returns false), prints a one-time status-line
notice naming your terminal.

**Terminals that support it:** kitty, WezTerm, Ghostty, foot, and **iTerm2 ≥ 3.5**.
**Terminals that don't:** macOS **Terminal.app** (no support at all).

Without the protocol, these are affected:

- **Control-code aliases collide.** The classic TTY has one byte for each of
  these pairs, so the terminal literally cannot tell them apart:
  `Ctrl+I` ≡ `Tab`, `Ctrl+M` ≡ `Enter`, `Ctrl+[` ≡ `Esc`, `Ctrl+H` ≡ `Backspace`.
  So e.g. `Ctrl+I` (submit inference) fires the `Tab` action instead. This is a
  terminal limitation, not something inkhaven can work around on that terminal.
- **`Alt`/`Option`+arrow doesn't arrive.** Chords that put `Alt` on an arrow key
  — browser back/forward (`Alt+Left`/`Alt+Right`) and vertical **block selection**
  (`Alt+arrows`) — need the arrow to carry the `Alt` modifier. On macOS,
  Terminal.app re-encodes Option+arrow as word-navigation escape sequences, so
  inkhaven never sees the `Alt`; and kitty needs `macos_option_as_alt yes` in
  `kitty.conf` for Option to act as `Alt` at all.
- **Some function keys may not arrive.** F-key delivery also depends on macOS
  **System Settings → Keyboard → “Use F1, F2, etc. keys as standard function
  keys”** (otherwise they're brightness/volume keys, consumed before any app).

**Recommendation:** use a protocol-capable terminal (above) and enable standard
function keys in macOS Keyboard settings. In iTerm2, also set the Option keys to
“Esc+” (Profiles → Keys) so `Alt`+chords are delivered. Nothing at the *shell*
level can fix the control-code aliasing — it's resolved by the terminal emulator
before the shell sees anything.

---

## 0. Mouse

Inkhaven captures mouse input on startup. Left-click moves focus to the
clicked pane; in the Tree pane the click positions the row cursor, in the
Editor it positions the character cursor (clicks in the gutter are
ignored).

**Scroll wheel** (1.2.8+):

* **Tree pane** — moves the tree cursor up / down 3 rows per tick.
* **Editor pane** — scrolls the viewport up / down 3 lines per tick.
* **AI pane** — scrolls the chat history (older messages on wheel up).
* **OS Shell pane modal** — scrolls the turn buffer (older turns on wheel up).
* **HJSON editor modal** — moves the textarea cursor up / down 3 lines.
* **Kill-ring picker / Fuzzy paragraph picker** — moves the cursor through entries.
* Other modals ignore the wheel (silent no-op).

Floating overlays (search results, prompt picker) still swallow mouse
input so a stray click can't focus a hidden pane.

Terminal-side text selection still works while the alternate screen is
up — hold **Shift** (or Option, depending on the terminal) while
selecting to bypass mouse capture and copy text through the terminal's
own clipboard integration.

## 1. Global

These chords work from any focus except where noted. Chords marked
**configurable** can be remapped in HJSON.

| Chord                | Action                                                      | Configurable |
| -------------------- | ----------------------------------------------------------- | ------------ |
| `Tab`                | Cycle focus Tree → Editor → AI → Tree.                      | `next_pane`  |
| `Shift+Tab`          | Cycle in reverse.                                           | `prev_pane`  |
| `Ctrl+/`             | Focus the top Search bar.                                   | `search`     |
| `Ctrl+I`             | Focus the bottom AI prompt bar.                             | `ai_prompt`  |
| `Ctrl+S`             | Save current paragraph + re-embed (no-op if nothing open).  | `save`       |
| `Ctrl+Q`             | Hard quit. Auto-saves the open paragraph first if dirty; if the save fails, refuses to quit so the error stays visible. | no |
| `Ctrl+1`             | Focus the **Editor** pane.                                  | no           |
| `Ctrl+2` / `Ctrl+B Shift+O` | (1.4.13, OUTLINE-1) Open the full-screen **Outline** pane (see §9). `Ctrl+B Shift+O` is the reliable backup if your terminal re-encodes `Ctrl+2` as NUL / `Ctrl+@`. | `outline.open` |
| `Ctrl+T`             | Focus the side **Tree** pane.                               | no           |
| `Ctrl+3`             | Focus the **AI** pane.                                      | no           |
| `Ctrl+4`             | Focus the **Search** bar (top).                             | no           |
| `Ctrl+5`             | Focus the **AI prompt** bar (bottom).                       | no           |
| `Ctrl+B`             | Enter **meta mode**. The next keystroke is the action selector (see §1.1). | `meta_prefix` |
| `Ctrl+V Space`       | (1.3.33) Open the **command palette** — fuzzy-find any command by name / chord / description and run it. Type to filter, `↑↓` select, `Enter` runs, `Esc` closes. The fast way to reach any command without memorising its chord; rebindable in the `view` layer (see §1.2). | `global.command_palette` |
| `Ctrl+B H`           | Open the pane-aware **Quick reference** floating pane. Works from every pane (Tree / Editor / AI). Scroll with arrows / PgUp / PgDn; close with `Esc`. Routed through the meta prefix so it never collides with the editor's `Ctrl+H` split-scroll. | no |
| `Ctrl+B V`           | Open the **Version / author / credits** floating pane. Shows the running Inkhaven version, the author block, the repository / licence, and the curated list of direct dependencies with their licences. Scrollable; `Esc` closes. | no |
| `Ctrl+B i`           | **Book info** — open the current book's info panel: paths, stats, PDF status. | `global.open_book_info` |
| `Ctrl+B Q` (tree)    | (1.3.0) **Imposition preview** (*Q* for *quire*) — open the production-layout overlay for the selected book: the plan `inkhaven pdf impose --dry-run` prints (signatures / sheets / creep / first-sheet schematic) for the `imposition.default` profile, rendered over the book's already-built PDF (`<artefacts>/<slug>/<slug>.pdf` — build with `Ctrl+B B` first). `Enter` imposes to `…-imposed.pdf`; `Esc` cancels. **Tree-scoped on purpose**: in the editor `Ctrl+B Q` stays *translate-into-invented-language* (1.2.13), so the two never collide. See [Tutorial 65](Tutorials/65-hand-binding.md). | no |
| `Ctrl+B A`           | **Assemble** the current user book — write its Typst sources (root `.typ` + includes) to the artefacts dir. No `typst compile`. Autosaves the open paragraph(s) first. | `global.schedule_assemble` |
| `Ctrl+B B`           | **Build** the current user book — assemble, then `typst compile` to PDF. On a compile error the build path opens a fresh AI chat for analysis. Autosaves first. (This is what `Ctrl+B B` does in every pane, including the Tree — it does **not** add a book; see §2.2 for the bare Tree `B` = add-book.) | `global.schedule_build` |
| `Ctrl+B O`           | **Take** the current user book — build, then copy the finished PDF into the launch cwd with a timestamped filename. Autosaves first. | `global.schedule_take` |
| `Ctrl+B X`           | **ConLang hub** (LANG-1) — a read-only overview of the project's invented-language sub-books (Language system book): the declared languages with their dictionary / grammar / phonology counts. `↑↓` scroll, `Esc` closes. Errors with a hint when there's no Language book. | `view.open_conlang_hub` |
| `F1`                 | Open the **Help-manual** query pane (RAG over the Help book). Type a question, Enter to ask. The default LLM provider streams a grounded answer into the AI pane; the model is constrained to use only the Help excerpts (no external knowledge). Same flow as typing `Help! <question>` into the AI prompt bar. Help answers are **one-shot** and do not enter the AI chat history. | no |
| `F7`                 | **Grammar check** the currently-open paragraph. Resolver precedence for the prompt template: (1) a paragraph titled / slugged `grammar-check` in the **Prompts** system book, (2) an entry of the same name in `prompts.hjson`, (3) a built-in fallback that runs a grammar/punctuation review against the configured `language` (HJSON top-level) and explicitly preserves any Typst markup. Result streams into the AI pane and focus moves there so you can watch the review in real time. | no |
| `F9`                 | **Cycle the AI scope** through `None → Selection → Paragraph → Subchapter → Chapter → Book → Facts → Socrates → Editor → Graph → None`. The next prompt sent from the AI prompt bar prepends the matching context (selection text / open paragraph / enclosing branch contents) then **auto-resets to `None`** — except the **sticky** conversation scopes (`Facts`, `Socrates`, `Editor`, `Graph`), which persist across follow-ups until you cycle away. `Book` and `Graph` are retrieval-grounded (relevant passages are retrieved and cited rather than the whole book sent); `Graph` additionally folds in the knowledge-graph relations touching each passage (see `Documentation/GRAPH.md`). Mode is shown in the AI prompt title (`AI prompt · scope: Paragraph`) and the status bar. Works from every pane. | no |
| `F10`                | **Toggle inference mode** between `Local` and `Full`. `Local` instructs the model to use only the supplied context (and prior chat turns); `Full` lets it augment with general knowledge. Both modes are shown in the AI pane title (`AI — gemini · done · infer=Full · scope=Paragraph`). Help inference (F1 / `Help! …`) is **pinned to Local** regardless of this toggle so the help-manual answer never invents features. Works from every pane. | no |
| `Ctrl+B C` (AI pane) | Clear the AI chat history + currently displayed inference. (F9's old behaviour; F9 now drives the scope cycle.) **Pane-scoped**: this is the AI-pane meaning — the same `Ctrl+B C` is Character-lexicon lookup in the Editor and Add-chapter in the Tree (see §1.1). | no |
| `Ctrl+B ]`           | (1.2.5) **Tag the open paragraph** — open the floating tag picker scoped to the editor buffer. Inside the picker: `↑↓` select, `Space` multi-selects, `T` applies selected tags (or the cursor tag if none selected), `A` adds a new tag (prompt), `D` deletes a tag project-wide (y/n confirm), `Enter` applies, `Esc` closes. | no |
| `Ctrl+B }`           | (1.2.5) **Search by tag** — open the floating tag picker in read-only mode. `Enter` on a tag lists every paragraph that carries it with a typeable filter input; `Enter` on a paragraph row opens it in the editor. `A` / `D` still work (tag management is project-wide). | no |
| `Ctrl+B 0`           | (1.2.8) **Edit project HJSON** — open `<project>/inkhaven.hjson` in a full-screen modal editor with HJSON syntax highlighting. `Ctrl+S` saves; when the saved bytes differ from the loaded bytes, a *Restart required* overlay pops up (config applies on next launch). **`Ctrl+R` (1.2.12+) fires an LLM review of the current buffer** — same "reviewer LLM, not executor" pattern the prompts-editor TUI uses; response streams into `App.inference` and is visible in the AI pane after closing the modal. `Esc` closes (status-line warning fires if there are unsaved edits). The editor mirrors the main paragraph editor's chord set: arrows / Home / End / PgUp / PgDn / Shift+arrows for selection / Ctrl+Home,End top/bottom / Ctrl+Left,Right word jumps / Ctrl+Backspace delete-word / Ctrl+U undo / Ctrl+Y redo / Ctrl+K cut / Ctrl+C copy / Ctrl+P paste / Ctrl+A select-all / Ctrl+D delete-line / Ctrl+E delete-to-EOL / Ctrl+W delete-to-BOL. | `bund.edit_project_hjson` |
| `Ctrl+B Shift+0`     | (1.2.15) **Project doctor panel** — same scan + repair flow as `inkhaven doctor --scan` + `--autofix`, but as a TUI modal.  Synchronous project scan runs on open; modal table lists every finding with class + severity + path + one-line detail.  Inside: `↑↓` navigate, `r` repair the highlighted finding, `R` repair every finding, `Esc` closes.  Repairs are logged to `<project>/.inkhaven/doctor.log` for audit.  Paired mnemonically with `Ctrl+B 0` (HJSON config editor): the digit-0 row is the "system inspection" cluster. | `view.doctor_panel` |
| `Ctrl+B W`           | **World overview** (WORLD-4) — a read-only, scrollable summary of the project's world simulation: the `world.hjson` definition (name / seed / star / planet / moons), the compiled astronomy layer (year length, divergence flag, tilt, seasons, lunar synodic periods, dominant tide, calendar consistency), and whether it has been materialized into the World system book. In the overview: `↑↓` scroll, **`C`** compiles every layer + seeds the Place proposal queue, **`P`** opens the proposal queue (`↑↓` navigate, `Enter` accept → commits a Place, `r` reject, `Esc` back), `Esc` closes. Full operations also on the CLI (`inkhaven realworld …`). | `world.open_overview` |
| `Ctrl+B Shift+W`     | **Distraction-free / focus mode** — hides Tree, AI, Search, and AI-prompt panes; the editor occupies the full window. Forces focus to the editor on enter. Re-press to restore the four-pane layout. Mutually exclusive with `Ctrl+B K` AI-fullscreen. *(Moved from `Ctrl+B W` in WORLD-4 to free that prefix for the World chord family.)* Internally called "typewriter mode" in legacy strings + the HJSON serde key (`global.toggle_typewriter`) — the binding key stays for backward compatibility, but the user-facing name is now "focus mode". | `global.toggle_typewriter` |
| `Ctrl+B e`           | **Toggle typewriter sound** — flip the Enter / focus-out click SFX on or off. Choice is persisted to `inkhaven.hjson`. | `global.toggle_sound` |
| `Ctrl+B k`           | **Toggle AI-pane fullscreen** — the AI pane (chat history + AI prompt) fills the window, other panes hidden; re-press to restore. Mutually exclusive with the `Ctrl+B Shift+W` focus mode. | `global.toggle_ai_fullscreen` |
| `Ctrl+B S` (editor)  | (1.2.9) **Read aloud (TTS)** — speak the open paragraph. The backend is chosen by `tts.engine` ∈ `auto` / `piper` / `system`: `auto` (default) prefers the neural **Piper** backend when its binary + voice resolve, else falls back to the **system** backend, which shells out to the macOS `/usr/bin/say` binary (each call a fresh subprocess — the earlier in-process `tts-rs`/AVFoundation path was dropped after a per-process reuse bug). The system backend is **macOS-only**; other platforms report "unavailable" at play time. The system-backend default voice is `Milena` (`tts.voice`); Piper voices are staged with `Ctrl+B Shift+V`. Gated by `editor.tts.enabled = true` in HJSON — disabled by default. While playing, a `Read aloud` modal shows the elapsed time, the chosen voice, and the first 80 chars of the paragraph; any key (Esc / Space) stops playback. Modal auto-closes when the paragraph finishes. Tree-scope `Ctrl+B S` still adds a subchapter. | `editor.tts_read_paragraph` |
| `Ctrl+B Shift+F`     | (1.2.9) **Toggle style warnings** — flip the inline filter-word overlay on / off without leaving the editor. When on, the editor underlines intensifier crutches and hedges (`just`, `really`, `very`, `просто`, `очень`, …) in amber so the writer can question + rewrite. Built-in word lists ship for English, Russian, French, German, Spanish; the active list is keyed by the project's top-level `language` field. Extra words via `editor.style_warnings.filter_words.extra_words` in HJSON. Master switch is `editor.style_warnings.enabled`; this chord is a session-local override. | `editor.toggle_style_warnings` |
| `Ctrl+B Shift+K` (editor) | (1.2.20) **Toggle echo overlay** — the live, in-editor companion to the `echo-repetition` doctor scan. When on, every occurrence in the *open* paragraph of a word echoing across nearby paragraphs of the chapter is underlined in its own colour (`theme.style_warning_echo_fg`, default a muted purple distinct from the repeated-phrase magenta), so you catch a reused distinctive word as you write rather than at scan time. Cross-paragraph: it reads the chapter's paragraphs (the open one live, including unsaved edits) and uses the shared `editor.echo_window` / `echo_min_repeats` / `echo_max_global` tunables. Multilingual via the project's Snowball stemmer (ё-folded for Russian). Master switch is `editor.echo_overlay`; this chord is a session-local override. | `editor.toggle_echo_overlay` |
| `Ctrl+B Shift+R` (editor) | (1.2.9) **Save paragraph as audio** — write the open paragraph to an AIFF file via macOS `say -o <path>`. Opens a path picker pre-filled with `<project>/audio/<paragraph-slug>.aiff` — edit the path then Enter to write, Esc cancels. Uses the same voice + speed as `Ctrl+B S`. Output format follows the file extension (`.aiff` / `.wav` / `.m4a` all work on macOS 13+). macOS-only. | `editor.tts_save_as_audio` |
| `Ctrl+B Shift+V`     | (1.2.17) **Piper TTS voice picker** — browse the Hugging Face piper-voices catalog + every voice already downloaded into the project's `.inkhaven/voices/` directory.  Catalog entries sorted by language code asc → quality tier desc → key asc.  Type characters to filter (matches voice key, language code, or English language name; case-insensitive).  ↑↓ / PgUp / PgDn / Home / End navigate; Enter on an `⬇ available` row downloads the voice (blocking, ~5–30 s on a fast connection) + sets the runtime `tts.voice`; Enter on a `✓ downloaded` row just sets the runtime voice; `d` removes a downloaded voice + updates the LRU index (skipped while a filter is active so it doesn't collide with type-to-filter); Esc closes.  Mirrored by the headless `inkhaven tts voice list / download / remove` CLI.  Picker is engine-agnostic — works even when the active backend is macOS `say`, so users can stage Piper voices before flipping `tts.engine`. | `tts.voice_picker` |
| `Ctrl+B Shift+E` (editor) | (1.2.18) **Reader-pace preview** — a teleprompter that advances a highlight word-by-word through the open paragraph at `editor.reading_wpm` (default 200), so you experience your prose at a reader's speed instead of editing-glance speed — surfaces pacing problems (a run-on that drags, a too-abrupt beat) invisible when you skim.  Already-read words dim, current word reverse-highlighted, upcoming words normal; footer shows position + time-left.  `Space` pause/resume (elapsed time carries across cycles), `←` / `→` step the highlight back/forward one word, `r` restart from the top, `Esc` close.  Reads clean prose (the same markup-stripping pass the audiobook export uses).  E for Experience. | `view.reader_pace` |
| `Ctrl+B Shift+G`     | (1.2.9) **Writing-streak heatmap** — GitHub-style 13×7 grid of the last 91 days of project-wide word deltas. Each cell colored by daily word count bucket (0 → dim, 1-249 → faint green, 250-499 → medium, 500-999 → bright, 1000+ → max). Footer shows current streak + longest streak in window + 91-day total + active-day average. Today's cell highlighted with a dark background. Any key closes. | `view.open_writing_streak_heatmap` |
| `Ctrl+B <` (editor)  | (1.2.9) **Previous scene break** — jump cursor to the previous scene-break line in the open paragraph. Scene breaks: `* * *`, `***`, `---`, `___`, `###`, `~~~`, or a lone `§`. Originally requested as `Ctrl+B Shift+{`; reassigned to `<` (vim-style) because `Shift+}` collides with the 1.2.5 `Ctrl+B }` = TagSearch chord. | `editor.scene_break_prev` |
| `Ctrl+B >` (editor)  | (1.2.9) **Next scene break** — same detector as `Ctrl+B <`, jumps forward. | `editor.scene_break_next` |
| `Ctrl+B Shift+L`     | (1.2.9) **Concordance view** — project-wide list of every distinct lexical stem with total count + up to 3 KWIC samples. Walks every paragraph under regular books (system books — Prompts / Characters / Places / Lore / Help / Notes / Artefacts — are excluded from the corpus since 1.2.11 as metadata, not prose), tokenises with UAX-#29, drops stop-words / single-char tokens / pure digits, and groups by Snowball stem so `walk`/`walked`/`walking` collapse to one row. Type to filter (substring match against headword + variants); `Ctrl+S` toggles sort (count ↔ alphabetical); ↑↓ / PgUp/PgDn / Home / End navigate; **Enter (1.2.11+) closes the modal and jumps to the first sample's source paragraph at the matching line — the heading offset is computed on the live editor body so the cursor lands on the right textarea row even though the index was built over heading-stripped bodies**; Esc closes. Multilingual via the project's `language` field (English / Russian / French / German / Spanish stop-word lists). | `view.open_concordance` |
| `Ctrl+B Shift+P`     | (1.2.9) **Toggle POV chip** — flip the status-bar POV / character chip on or off (session-local override on top of `editor.pov_chip_enabled` in HJSON). When on, the status bar shows the most-mentioned character in the open paragraph as the heuristic POV character, plus up to three additional named characters present. Driven by the project's existing `characters` lexicon — no separate tagging needed. Ties broken by first-mention order. | `view.toggle_pov_chip` |
| `Ctrl+B Shift+N`     | (1.2.12) **Toggle prompt-language mode** — cycle the prompt-language resolution mode through `None` (defer to `editor.prompt_language_mode` in HJSON) → `book_defined` → `paragraph_detected` → `None`. Session-local; does NOT rewrite HJSON. `book_defined` uses the top-level `language` field for every AI prompt resolution; `paragraph_detected` runs whatlang on the live paragraph (falls back to book language for paragraphs shorter than `editor.prompt_language_detection_min_chars`). The AI pane title bar's `lang=` chip reflects the active mode (`ru (book)` vs `ru (paragraph)`); the status bar echoes the new mode on toggle. Mnemonic: N for Natural language / laNguage picker. | `view.toggle_prompt_language_mode` |
| `Ctrl+B Shift+H` (editor) | (1.2.9) **Sentence-rhythm gauge** — open a modal that splits the open paragraph into sentences (hand-rolled walker with abbreviation suppression: Mr., Mrs., Dr., e.g., i.e., Ph.D., …), computes word-count mean / stdev / coefficient of variation (CV), and maps CV to a verdict: Monotone (`CV < 0.25` — drones), Steady (`0.25-0.45`), Varied (`0.45-0.80` — strong prose rhythm), Choppy (`≥ 0.80`). Shows a per-sentence bar list + the three shortest + three longest outliers. ↑↓ / PgUp/PgDn / Home/End scroll; any other key closes. Mnemonic: H for heartbeat. | `view.open_sentence_rhythm` |
| `Ctrl+B Shift+T` (editor) | (1.2.9) **AI show-don't-tell scan** — send the open paragraph to the configured LLM with a system prompt asking for telling passages plus suggested rewrites. The response streams into the AI pane. Complements the always-on regex overlay (`editor.style_warnings.show_dont_tell`) — the regex catches obvious 2-grams (`was angry`, `realised`); the AI scan catches subtler instances and proposes alternatives. Mnemonic: T for tell. | `ai.analyse_show_dont_tell` |
| `Ctrl+B Shift+X` (editor) | (1.2.21) **AI fact-check** — fact-check the open paragraph against the project's **Facts** book. Locks the AI scope to the local paragraph and grounds the check against every established world fact (climate, geography, seasons, distances, chronology), so the model flags any claim that contradicts the world — snow in a tropical region, a three-day ride that should be three weeks. With an empty Facts book it degrades to a generic local fact-check. Multilingual fact-analysis system prompt (en/ru/de/fr/es); prompt resolution: Prompts-book `fact-check` paragraph → `prompts.hjson` → embedded default. Streams the verdict into the AI pane. Pairs with the F9 **Facts** scope, which loads the same facts as an interactive chat session. Mnemonic: X for fact e**X**amination. | `ai.fact_check` |
| `Ctrl+B Shift+S` (editor) | (1.2.21) **Facts search** — open the Facts semantic-search modal. Two-phase: type a query (multi-word OK) and **Enter** runs a semantic search over the Facts book (the same vector index as `Ctrl+V S`, post-filtered to the Facts subtree); then **↑↓** navigate the ranked matches, **Space** marks several (multi-select), **Enter** sends the marked facts — or the cursor's row — to a **targeted Facts chat** grounded in just those facts (reuses the F9 Facts-scope seed + fact-analysis system prompt). Any printable / Backspace in the results drops back to refine the query; **Esc** closes. The scalable path for a large Facts book — ground in the relevant handful instead of loading the whole book. Mnemonic: S for **S**earch facts. | `ai.search_facts` |
| `Ctrl+B Shift+J` (editor) | (1.2.21) **Next fact finding** — after a fact-check (`Ctrl+B Shift+X`) flags contradictions, cycle through them: each press moves the editor cursor to the next flagged claim in the open paragraph (best-effort — the model's quoted phrase may be paraphrased) and shows the violated fact + explanation on the status bar. Wraps around. Findings are tied to the checked paragraph; switching paragraphs prompts a re-run. Mnemonic: J for **J**ump. | `ai.next_fact_finding` |
| `Ctrl+B Shift+D` (editor) | (TDOC-1) **Verify the open code listing** — run the `verify`-marked fenced code block in the open `para:code` paragraph through its configured language runner and report pass / fail / skip to the Output pane on that paragraph; refreshes the tree badge. Gated by `docs.verify.enabled` (off by default). Headless equivalent: `inkhaven docs verify`. See [`TDOC.md`](TDOC.md). | `docs.verify_paragraph` |
| `Ctrl+B Shift+M` (editor) | (1.2.11) **AI sentence-rhythm rewrite** — send the open paragraph to the configured LLM with a prompt asking it to break monotonous rhythm by mixing short and long sentences while preserving voice + meaning. Prompt resolution: Prompts book (slug or title `sentence-rhythm-rewrite`) → `prompts.hjson` → embedded multilingual fallback that respects the project's `language` setting. On stream completion, an AI diff modal pops automatically; `a` accept creates a snapshot annotated `Sentence rhythm rewrite` first then replaces the buffer; `r` reject leaves it untouched. Pairs with the `Ctrl+B Shift+H` rhythm gauge — and the chord also fires from **inside** that gauge modal, so the natural diagnose-then-rewrite path needs no extra keystrokes: open the gauge, see MONOTONE, press `Ctrl+B Shift+M`; the gauge dismisses as the rewrite spawns. Mnemonic: M for Modulate / Mix it up. | `ai.rewrite_sentence_rhythm` |
| `Ctrl+B Shift+Y` | (1.8.18, POEM-TUI PO-P13) **Next stanza** — create the next stanza: a sibling verse paragraph of the same `para:verse-*` type immediately after the current one, opened for editing, so a poem is built stanza by stanza without leaving the writing flow. Works on the open buffer or the tree cursor; no-ops with a hint on a non-verse paragraph. Writes structure only — it never generates a line of verse. *(Freed by relocating the WordNet thesaurus to its documented `Ctrl+V Shift+Y` home; the thesaurus had been mis-placed on the meta layer.)* While a verse paragraph is open, the status bar shows a live readout — the current line's syllable count and its position in the stanza (`♩ 8 syl · l2/4`). Declare a form first with `Ctrl+B J` → `P` → `D`. | `meta.create_verse_sibling` |
| `Ctrl+V Shift+Y` (editor) | (1.8.5) **WordNet thesaurus** — look up the word under the cursor in the sense-based WordNet index for the paragraph's language and replace it with a chosen synonym / antonym / hypernym / hyponym. For a non-English language the relations expand through the interlingual index. Needs the language's index installed (`inkhaven wordnet fetch` / `import`); nothing is sent to any server. *(Relocated here from the meta layer in PO-P13, where it had shadowed the documented chord.)* | `view.open_thesaurus` |
| `Ctrl+B z` | (2.0/2.x, SEMNET + GRAPHMIND) **Graph hub** — a small menu onto the knowledge graph: **`n`** opens a read-only tree of the open paragraph's one-hop edges (what it links to, contradicts, is sourced from, cites, the senses it mentions), grouped by kind with direction arrows (`→` out, `←` in, `⇄` symmetric), `↑↓` scroll; **`i`** opens the **edge inbox** — the advisory `judged` stance edges awaiting triage (from confront, `graph link`, deep research), where **`P`** promotes the selected edge (kept across `graph rebuild`) and **`d`** rejects it; **`w`** (GM-P8) **walks the graph** to answer the question typed in the AI prompt — the AI pane streams the exploration (search → neighbours → contradictions → paths) then the grounded prose answer, `Esc` stops the walk, status shows `turn k/N`; **`c`** (3.2, ENSEMBLE) opens the **Dramatis Personae** — the book's cast joined with their BONDS relationships and their CHAR-1 arc state (arc shape · latest state · ✦changes · bonds), `↑↓` scroll, **Enter** jumps to a character's bible node (the `inkhaven cast` view). `Esc` closes. Populate the graph with `inkhaven graph rebuild` / `graph lexical` / `graph link`. On a **confront finding** in the Output pane, **`P`**/**`d`** also promote/reject its edge. See `Documentation/GRAPH.md`. The graph's meta hub, alongside `Ctrl+B W`/`J`/`X`. | `graph.open_hub` |
| `Shift+F4` (editor) | (1.2.12) **Toggle fullscreen split-view** — left pane is the primary buffer; right pane is the `secondary` slot. Tab swaps focus. Tree + AI response panes hidden; AI prompt input stays at the bottom so `Ctrl+I` works from either pane. The right pane is populated via `Shift+Enter` in any paragraph picker — tree pane Enter, fuzzy paragraph picker (`Ctrl+V P`), bookmark picker (`Ctrl+V M`), recent-paragraph picker (`Ctrl+V Shift+P`). The existing `F4` (same-paragraph snapshot split) and `Ctrl+F4` (accept snapshot) are untouched. Both editors are full-featured with independent dirty tracking + idle autosave. | `editor.toggle_split_view` |
| `Shift+Enter` (tree, paragraph pickers) | (1.2.12) **Pin to split-view secondary pane** — works on tree-pane Enter, fuzzy paragraph picker (`Ctrl+V P`), bookmark picker (`Ctrl+V M`), recent-paragraph picker (`Ctrl+V Shift+P`). Loads the focused paragraph into the `secondary` slot instead of replacing the primary. Combined with `Shift+F4`, this is the universal "open this beside that" gesture. Errors surface on the status bar when the chosen paragraph is already primary (showing the same paragraph in both panes is rejected). | — (modifier on Enter) |
| `Ctrl+V Shift+B` | (1.2.12) **Sibling-book lookup for split-view** — given the open paragraph's slug, walks the project hierarchy for paragraphs with the same slug under a *different* top-level book. Zero matches → status message names the slug. Single match → auto-pin to the `secondary` slot. Two-or-more matches → open a fuzzy picker pre-scoped to those entries. Primary translation-workflow chord: from `manuscript-en/03-rain`, finds `manuscript-ru/03-rain` and pins it for side-by-side review via `Shift+F4`. | `view.sibling_book_lookup` |
| `Ctrl+V u` | (1.3.1) **Submission tracker** — open the `.inkhaven/submissions.json` log (where the manuscript went, when, what came back). Records show colour-coded status (drafting / sent / rejected / offer / withdrawn) + yellow next-action dates; the selected record expands to show its timestamped note trail. `↑↓` move, `Space`/`s` cycle the selected record's status (stamps a response date for rejected / offer), `d` removes — both persist. Add records + notes with `inkhaven submissions add` / `add-note`. Mnemonic: **U** for s**U**bmissions (`s` is taken); distinct from `Ctrl+V Shift+U` (kill-ring). | `view.open_submissions_tracker` |
| `Ctrl+V q` | (1.3.1) **Submission-package generator** — pick a piece (query letter / synopsis / comps / logline) and stream it into the AI pane from the current book's **cached** digest (build it first with `inkhaven submission digest`). The system prompt resolves through the three tiers (Prompts-book paragraph `submission-*` → `prompts.hjson` → built-in, language-aware), so you can tune the house voice. `↑↓` pick, `Enter` → AI pane, `Esc` cancels. Once the draft finishes, press **`L`** in the AI pane to file it as a paragraph in the **Submissions** book (overwrites a same-titled draft) — the same place `inkhaven submission …` saves to. Mnemonic: **Q** for **Q**uery. | `view.open_submission_generator` |
| `Ctrl+V Shift+K` | (1.3.2) **Structure outline** — the `inkhaven plan check` report for the current book as a position bar per beat: `|` is the framework's target position, `●` where the beat actually lands (`#` when they coincide), colour-coded on-target / drift / gap, with act word-share pacing, an `↪N` thread chip, and (1.3.4) a **tension overlay** — expected vs actual (vs 1.3.5 AI-rated `ai`) intensity sparklines beneath the bars, flagging beats that land flat. Scaffold a framework first with `inkhaven plan init`. The view is interactive: `↑↓` navigate, **`m`** maps the selected beat to a chapter from a picker (write-back, no HJSON editing), **`t`** links threads (Space toggles), **`s`** cycles its status, **`Enter`** opens the beat's mapped chapter in the editor, **`v`** flips to the scene / sequel board (cards grouped by chapter; in the board, **`g`** regenerates the selected card from its chapter prose — streams into the AI pane, `L` files it back), **`a`** streams the AI structure analysis into the AI pane (over the book's cached digest) — press **`L`** there to file the analysis as a *Structural Analysis* paragraph in the **Planning** book — `Esc` closes. Mnemonic: **K** for s**K**eleton. | `view.open_plan_outline` |
| `Ctrl+V Shift+R` | (1.3.6; **2.4 REDLINE**) **The Editorial Pass — the revision partner.** One ranked worklist unifying **every** reader — the editorial `doctor` classes, `plan check` structure, the Facts-scan sidecar, the prose-style detectors (show-don't-tell / filter-word / anachronism), **SENTINEL** continuity, the **LECTOR** read-through, the **Inner Editor**'s craft notes, and **CHORUS** voice — errors first; the same as `inkhaven edit`, deterministic (reads computed findings, no live AI to open it). Each row shows a **response glyph** for how it can be acted on: **`✎`** a diff-reviewed rewrite, **`⇄`** a guided decision, **`✉`** a revision brief (`→` = jump-only). `↑↓` navigate (message + hint expand below), `[`/`]` cycle the category filter and **`r`** the response-kind filter (✎ rewrite / ⇄ decision / ✉ brief — the two AND together, so you can narrow to just the decisions you owe or just the batchable rewrites), **`Enter`** jumps to the paragraph, **`f` acts on the selection**: `✎` streams an AI rewrite → diff-review (`a` accept snapshot-first, `r` reject) — echo/pacing/editor-note rewrite the whole paragraph, show-don't-tell/filter/anachronism just the flagged phrase (span-scoped); `⇄` asks *what's true / how to resolve*, then reconciles the paragraph to your decision as a confirmed rewrite; `✉` writes a developmental **brief** (advice, never a rewrite) to the Thoughts pane. **`F`** batch fix-all walks every `✎` Rewrite through that same review (`Esc` stops it; Decisions, Briefs, and finding-aware editor notes are handled one at a time, never batched). **`s`** skips for the session (the skip persists across reopening the pass, until you restart), **`d`** defers (persisted — hidden until the prose changes), **`D`** clears all deferrals + session skips, `Esc` closes. **The pass remembers your place:** after a jump / act / batch it reopens on the same finding (or the next, if you just fixed it) with your filters intact, and the `F` batch auto-reopens the pass when it finishes; a deliberate `Esc` forgets the position. Every prose change is snapshotted first (F6-restorable) — there is no unconfirmed write path. `inkhaven revise` synthesises the same worklist into one editorial letter. `edit` is *manuscript readiness*; `doctor` (`Ctrl+B Shift+0`) stays *project integrity*. Mnemonic: **R** for **R**evision. | `view.open_editorial_pass` |
| `Ctrl+V @` (editor) | (1.4.5) **Cite picker** — fuzzy-find a citation defined in the **Sources** book and insert its `@key` at the editor cursor. Type to filter by key / author / title, `↑↓` select, **Enter** inserts `@key`, `Esc` closes. Each row shows `@key   year · author — title`. Empty list → add entries to the Sources book first (add a paragraph under Sources — its title becomes the key — or `inkhaven sources import <file.bib>`). At Book assembly the cited keys resolve against the auto-generated `sources.bib`. Mnemonic: `@` is the Typst citation sigil. | `view.cite_picker` |
| `Ctrl+V #` | (1.6.15) **Typst Universe import picker** — fuzzy-find a Typst Universe package by name / description and insert a `#import "@preview/<name>:<version>": *` line at the currently-open paragraph. The catalog is fetched once from `typst_universe.url` and cached under `.inkhaven/` (TTL `typst_universe.ttl_hours`, default 24h), sorted by GitHub stars; `Ctrl+R` in the modal forces a refresh. Handy for pulling in `cetz`, `fletcher`, journal templates, etc. Works from any pane (`Any` scope). Mnemonic: `#` mirrors Typst's import sigil. | `view.universe_picker` |
| `Ctrl+V &` | (1.6.15) **Cross-reference picker** — fuzzy-find a label defined anywhere in the manuscript (`<fig:flux>`, `<eq:energy>`, `<sec:intro>`, …) and insert an `@label` reference at the currently-open paragraph, so a cross-reference always points at a label that exists. The mirror of the XREF Output finding (which catches a reference whose label is missing). Empty list → define a label first by placing `<name>` after a figure, heading, or equation. Works from any pane (`Any` scope). Mnemonic: `&` reads as "reference to". | `view.xref_picker` |
| `Ctrl+V j` (editor) | (1.5.9) **Reflow paragraph** — re-wrap the paragraph at the cursor to the editor's current text width. Takes the blank-line-delimited block of prose around the cursor — whether it arrived as a **single long line** (pasted from another source) or **hard-wrapped at some other width** — collapses its line breaks and re-wraps the words greedily to the pane width (minus the line-number gutter). Applied as **one editor edit**, so `Ctrl+Z` undoes it. Prose paragraphs only (skips read-only Help and hjson / bund / markdown buffers). Mnemonic: **j** for **j**ustify. | `editor.reflow_paragraph` |
| `Ctrl+V z` | (1.4.8) **Terminology overlay toggle** — flip the banned-synonym overlay, which red-underlines synonyms of canonical terms defined in the **Glossary** system book (so "auth token" is flagged while the canonical "access token" is clean). Default on (within the master style toggle, `Ctrl+B Shift+F`); self-gating — an empty Glossary flags nothing. The footer shows `terms: "auth token" → use "access token"` when the cursor sits on a hit. | `view.toggle_terms_overlay` |
| `Ctrl+V Shift+Z` (editor) | (1.4.8) **Declare term deliberate** — with the cursor on a red-underlined banned synonym, record its canonical term as a **deliberate variant** in the intent ledger, so the overlay and `inkhaven terms check` stop flagging it. The "I meant to write it this way" escape hatch. | `view.declare_term_intent` |
| `Ctrl+V x` (editor) | (1.4.9) **Snippet `#include`** — fuzzy-pick a reusable snippet from the **Snippets** system book and insert a Typst `#include` for it at the cursor; the depth-relative `../…/snippets/<slug>.typ` path is computed automatically from the paragraph's place in the tree. With the cursor **inside** an existing `#include "…/snippets/…"` path, it **replaces** that path in place (pre-selecting the current snippet). Define snippets as paragraphs under the Snippets book; assembly copies them to a `snippets/` sidecar so the include resolves at `typst compile`. A save-time validator flags any `#include` whose snippet slug isn't defined (status / F8 / `Ctrl+V N`). | `view.insert_snippet_include` |
| `Ctrl+V Shift+X` | (1.4.9) **Snippets overview** — every snippet defined in the Snippets book with how many times it's referenced (`#include`) across the project (`(N ref)` / `(unused)`). `↑↓` navigate, **`Enter`** jumps to the snippet's source paragraph, `Esc` closes. The terminal equivalent is `inkhaven snippets list`; `inkhaven snippets check` validates every reference (exit 1 on a missing snippet). | `view.snippets_overview` |
| `Ctrl+V V` | (1.4.12) **Prose voice check** (NARR-1) — run the narrative-voice profiler in the **background** (deterministic, zero-AI, zero-cost; content-hash lazy, so only edited chapters recompute). Any chapter metric (rhythm CV/burstiness/MATTR, modal density, interiority, …) that drifted past its threshold vs the baseline chapter is emitted to the **Output pane** as an informational `prose` finding that navigates to the chapter. Mnemonic: **V** for Voice. Terminal equivalent `inkhaven prose profile` / `prose drift`. | `view.prose_voice_engage` |
| `Ctrl+V Shift+V` | (1.4.12) **Toggle ambient prose check** (NARR-1) — when on, the prose check re-runs after an editing pause, gated by a cooldown floor (`prose.ambient_cooldown_secs`, default 90s). Off by default. | `view.prose_toggle_ambient` |
| `Ctrl+V Shift+Q` | (1.4.14) **Dialogue fingerprint** (DIALOG-1) — open the per-character dialogue voice signature for the nearest character (one named in the open paragraph, else the most-speaking): utterance count, average length, vocabulary diversity (MATTR), question / exclamation ratios, hedge density, as ASCII bars, with a compare line for the next two speakers. `↑↓` scroll, `Esc` closes. Built from confidently-attributed dialogue — run the `Ctrl+B Shift+C` review pass (or `inkhaven dialogue scan`) to populate it. Mnemonic: **Q** for Quote (`Ctrl+V D` was taken). The dialogue *findings* (zero-attribution / said-bookism density / talking-head sequences) ride the `Ctrl+B Shift+C` review pass into the Output pane. | `dialogue.open_view` |
| `Ctrl+Z p` | (1.4.17) **Haiku** (HAIKU-1) — emit a hand-curated haiku to the Output pane on demand, in the book's language (EN/RU/DE/FR/ES, falling back to English). The same pool greets you at startup and when you create a new manuscript paragraph; a process-global rotation counter advances on every trigger, so you rarely see the same poem twice in a session. Zero-AI, baked into the binary (present even airgapped). Toggle the automatic moments with `editor.startup_haiku` (default `true`); this chord works regardless. Mnemonic: **P** for Poem (`p` is free in the `Ctrl+Z` Bund sub-chord table). | `haiku.show` |
| `Ctrl+Z c` (editor) | (1.2.14) **Add inline comment** — anchor an editorial comment to the current selection's character range (or the word at the cursor); pops a multi-line body input; writes a sidecar `<paragraph>.comments.json` beside the `.typ` (travels with the prose in git, diffs cleanly). Commented spans render underlined + italic; the editor footer surfaces `comment by <author> · <age>`. Char-offset anchors for UTF-8 safety. **Moved here from `Ctrl+V c`** (which the 1.6.19+ LOCI citation check now owns and used to shadow this). Mnemonic: **c** for Comment (`c` / `Shift+C` are free under `Ctrl+Z`). | `view.add_comment` |
| `Ctrl+Z Shift+C` | (1.2.14) **Comments panel** — project-wide panel over every `.comments.json` sidecar (breadcrumb / author / age / snippet / `(N/M in ¶)`). Panel chords: ↑↓ navigate, Enter jump to the comment span, `r` resolve, `R` toggle resolved-filter, `d` delete, `/` filter, `a` AI digest, `Esc` close. **Moved here from `Ctrl+V Shift+C`** (now the sourcing check). | `view.comments_panel` |
| `Ctrl+V Shift+N` | (1.4.16) **Character arc** (CHAR-1) — open the tracked arc for the nearest character (one named in the open paragraph, else the first tracked one): the author-declared arc (start / midpoint / end), the chapter-by-chapter observable state chain (✦ marks a change) with each chapter's deterministic agency score, the arc-completeness checks, and any Planning-Board coverage gaps. Read-only over the cached `char.duckdb`; `↑↓` scroll, `Esc` closes. Populate it with the `Ctrl+B Shift+C` review pass (agency + stalls + planning, zero-AI) or `inkhaven character refresh` / `check` (the LLM passes). Mnemonic: **N** for the arc (the bend); plain `n` is next-diagnostic. The arc *findings* ride the `Ctrl+B Shift+C` review pass into the Output `character` category. | `character.open_arc` |
| `Ctrl+V Shift+O` | (3.1, BONDS-1) **Relationship dashboard** — the relationship-continuity findings: are the bonds you declare between characters earned on the page? Grouped by kind — `unwritten_bond` (a declared pair barely shares a scene), `unearned_shift` (a bond changes state — ally → enemy — with no shared scene to turn it: the relationship plot-hole), `dropped_bond` (an established bond goes dormant then resurfaces). `↑↓` scroll, **Enter** jumps to the offending paragraph, `Esc` closes. Declare bonds with `rel:<kind>:<A>:<B>` tags; co-presence is derived free (scene POV + prose mentions + timeline event participants). Deterministic + ≈$0; the CLI equivalent is `inkhaven bonds` (`--json` / `--deep` for the opt-in `implied_cooling` LLM pass). KEN's sibling (KEN is `Ctrl+B Shift+Z`). Mnemonic: **O** for b**O**nds, beside the Inner Editor's `o`. See [`BONDS.md`](BONDS.md). | `global.open_bonds` |
| `Ctrl+V Shift+M` | (1.4.19) **Mythology heatmap** (MYTH-1) — refresh the declared symbol / motif / archetype inventory from the Mythology system book, recompute the per-chapter symbol-density scan, explicit motif occurrences, and the deterministic findings (archetype vacant / absent, motif absent from the final act), render the heatmap (symbol-density bars, motif-presence dots, archetype-presence bars across chapter buckets) into the **Thoughts** pane, and jump to the nearest declared symbol (one named in the open paragraph, else the first). Zero-AI; the LLM consistency / completeness / role checks stay explicit on `inkhaven myth check`. Reads declarations only — never interprets, never edits prose. Mnemonic: **M** for Myth (plain `m` is list-bookmarks). The deterministic myth *findings* also ride the `Ctrl+B Shift+C` review pass into the Output `myth` category. | `myth.open_heatmap` |
| `Ctrl+V Shift+L` | (1.3.8) **Story bible** — a consolidated, navigable view of the world: every Character with the attributes `inkhaven continuity extract` has tracked across chapters beneath it (`eye_color: brown (ch.3)`), plus the Places, Artefacts, and Facts books, **and (1.4.8) the Glossary's canonical terms**. `↑↓` navigate, **`Enter`** jumps to an entry's source paragraph (a `→` marks the jumpable rows), `Esc` closes. Mnemonic: **L** for **L**ore. | `view.open_story_bible` |
| `Ctrl+V Shift+F` | (1.3.12) **Deep AI world refresh (background)** — runs the whole world-layer batch (facts check + facts scan + semantic drift + continuity extract, the `inkhaven world --deep` set, in the manuscript's language) on a **background thread**, so the editor stays fully responsive. A `⟳ deep refresh` status chip tracks progress; when it finishes, an open story bible / Editorial Pass rebuilds itself from the fresh findings and the status shows the new `inkhaven world` summary. Runs in-process on a clone of the editor's DuckDB connection pool (no second open; tune with `embeddings.pool_size`). **Advisory** — reads your manuscript and writes findings to `.inkhaven/*.json` sidecars only; it never edits your prose (rewrites stay manual via the Editorial Pass `f`/`F`). Needs an LLM provider; one job at a time. Mnemonic: **F** for re**F**resh. | `view.run_deep_refresh` |
| `Ctrl+B Q` (editor) | (1.2.13) **Translate paragraph INTO invented language** — composes a prompt envelope from the target language sub-book's `Meta/overview`, `Grammar` (all rules), `Phonology` (all rules), `Dictionary` (RAG-filtered to entries whose translation appears in the source), and `Sample texts` (up to 3 as register anchors); streams the response into the AI pane. With zero Language sub-books the chord errors; with exactly one it translates directly; with two-or-more it pops a picker (↑↓ + Enter, or type the first letter of the language name to jump-and-commit). The translation block is wrapped between `<<<TRANSLATION>>>` / `<<<END>>>` markers; the AI pane's `I` apply chord lifts only that block. Pane title shows italic `translate[on]` chip while extraction is armed. See Tutorial 49. | `ai.translate_to_invented` |
| `Ctrl+B Shift+Q` (editor) | (1.2.13) **Translate paragraph FROM invented language** — reverse direction. Same envelope shape, flipped from/to direction labels. Picker title shows `Translate FROM`. Roundtrip-test workflow: `Ctrl+B Q` an English paragraph → copy translation into the next paragraph → `Ctrl+B Shift+Q` → back-translation in AI pane. When the back-translation drifts beyond register the grammar rules or dictionary entries have an inconsistency the manuscript will eventually trip over. Closes the proposal's §13 roundtrip-test concept for interactive use. | `ai.translate_from_invented` |
| `Ctrl+B D` (editor) | (1.3.25) **Translate paragraph INTO invented language — deterministic** — rule-based morphology + syntax with a translation-memory retrieval fallback (LANG-3), **no model call**, fully reproducible. Unlike `Ctrl+B Q` (AI prose → AI pane), the result, its per-word trace, the confidence score, and any uncovered-word report land in the **Output pane** (`o` expands the trace, `r` remembers the pair into translation memory, `a` asks the AI about it). Same language resolution as `Ctrl+B Q`. Editor-scoped, so it's disjoint from the Tree's `Ctrl+B D` = DeleteNode. Mnemonic: **D** for Deterministic. | `lang.translate_to_output` |
| Tree pane `b` on `Language` system book | (1.2.13) **Scaffold a language sub-book** — when the tree cursor is on (or inside) the `Language` system book and the user presses `b` (Add Book), the new Book is parented under Language directly (bypassing the normal "Books are top-level" rule via a `validate_placement` carve-out) and the commit handler runs `scaffold_language_chapters` to populate the 5 standard chapters (`Meta`, `Dictionary`, `Grammar`, `Phonology`, `Sample texts`) + seeds `Meta/overview` with the full commented HJSON template. Status: `added language `<name>` — 5 chapters scaffolded`. Pressing `b` from anywhere else still slots a top-level user book above the system block (unchanged). Matches `inkhaven language init <name>`. See Tutorial 49. | `tree.add_book` (context-sensitive routing) |
| Tree pane `+` under Language `<lang>/Dictionary` | (1.2.13) **Add dictionary entry** — when the tree cursor sits anywhere under `Language/<lang>/Dictionary` and the user presses `+` (Add Paragraph), the commit handler intercepts: walks the parent chain to identify the per-language Book, derives the alphabet bucket from the typed word's first character (consulting `Meta/overview.alphabet` first, first-char uppercase fallback), auto-creates the bucket subchapter if missing, creates the entry under it, and seeds the body with the full commented HJSON dictionary-entry template + `word` pre-filled. Paragraph's `content_type` is set to `"hjson"`. Matches `inkhaven language add-word`. See Tutorial 49. | `tree.add_paragraph` (context-sensitive routing) |
| Tree pane `+` under Language `<lang>/Grammar` or `/Phonology` | (1.2.13) **Add rule paragraph** — under Grammar / Phonology chapters of a Language sub-book the commit handler seeds the body with the schema-aware HJSON rule template (proposal §4 for Grammar, §5 for Phonology). Paragraph `content_type` = `"hjson"`. See Tutorial 49. | `tree.add_paragraph` (context-sensitive routing) |

### 1.1 Meta mode (Ctrl+B prefix)

The meta prefix is a single `Ctrl+B`; the second key selects the action.
**The action table is pane-specific** — `Ctrl+B` then `S` means different
things depending on whether the Tree, Editor, or AI pane has focus. The
status bar shows a yellow **META** chip and a prompt listing the actions
for the current pane while it's pending.

`Esc` cancels meta mode without running anything. Any unrecognized key
cancels with a status hint telling you which pane's table it consulted.

**Tree pane (and Search bar focus)** — hierarchy management:

| Second key | Action                                              |
| ---------- | --------------------------------------------------- |
| `B` / `b`  | **Build** the current book (Any-scope `global.schedule_build`; see §1). There is **no** Tree-scope `Ctrl+B B` — adding a book is the bare Tree key `B` (§2.2). |
| `C` / `c`  | Open Add modal — new **chapter**.                   |
| `S` / `s`  | Open Add modal — new **subchapter**.                |
| `P` / `p`  | Open Add modal — new **paragraph**.                 |
| `D` / `d`  | Open Delete confirm modal for the cursor's node.    |
| `↑`        | Swap the cursor's node with its previous sibling.   |
| `↓`        | Swap the cursor's node with its next sibling.       |
| `H` / `h`  | Open the pane-aware **Quick reference** overlay.    |

**Editor pane** — paragraph operations:

| Second key | Action                                                          |
| ---------- | --------------------------------------------------------------- |
| `S` / `s`  | **Read aloud (TTS)** — speak the open paragraph (see the `Ctrl+B S` global row for engine details). *Not* Save — plain `Ctrl+S` saves. |
| `N` / `n`  | **New snapshot** of the current buffer (== F5).                 |
| `R` / `r`  | **Cycle status** — advance the open paragraph one rung up the status ladder (`napkin → first → … → ready → napkin`). *Not* the snapshot history picker — that is F6. |
| `F` / `f`  | **Typst function picker** — fuzzy-find a Typst function and insert it at the cursor. Needs an open paragraph. |
| `T` / `t`  | **Retitle paragraph** — re-derive the displayed title from the buffer's first sentence (same logic that fires on save for placeholder titles, but runnable on demand). |
| `M` / `m`  | **Cycle node type** — morph the open paragraph's leaf type (`paragraph(typst) → paragraph(hjson) → paragraph(jinja) → script`). |
| `P` / `p`  | **Place lookup** — context-sensitive: opens the image picker when the cursor sits inside `#image(...)`, otherwise treats the selection (or word under cursor) as a place name, sweeps matching paragraphs in the **Places** system book, and prepends them as RAG context to the next AI prompt. If the AI prompt is non-empty, the inference fires immediately and focus jumps to the AI pane; if empty, the context is stashed and focus jumps to the **AI prompt**. |
| `C` / `c`  | **Character lookup** — same lexicon-inference flow as `P` but against the **Characters** system book. |
| `G` / `g`  | **Notes lookup** — same lexicon-inference flow against the **Notes** system book. |
| `Y` / `y`  | **Artefacts lookup** — same lexicon-inference flow against the **Artefacts** system book. |
| `L` / `l`  | **LLM / model picker** — swap the active LLM (Any-scope `global.open_llm_picker`; works from every pane). *Not* the load-file dialog — that is F3. |
| `H` / `h`  | Open the pane-aware **Quick reference** overlay.                |

**AI pane (and AI prompt focus)** — inference management:

| Second key | Action                                              |
| ---------- | --------------------------------------------------- |
| `C` / `c`  | **Clear** the current inference (cancel streaming or discard a finished result). |
| `H` / `h`  | Open the pane-aware **Quick reference** overlay.    |

**Any pane** — these meta chords work from every pane (`Scope::Any`):

| Second key | Action                                                                 |
| ---------- | ---------------------------------------------------------------------- |
| `Shift+C`  | (1.3.34) **Unified review pass** — run every fast, deterministic checker at once: the world fact-checker + Inner Socrates over the **open paragraph**, plus the timeline critique over the **project**. Findings emit to the **Output pane** (filter them with `f`/`S`/`t`); the tree shows a per-node report-card badge (`⊗`/`⚠`/`●` + count). Instant and LLM-free. The CLI equivalent is `inkhaven check` (`--paragraph` / `--book-name` / `--no-fact` / `--no-socrates` / `--no-timeline`). |
| `Shift+I`  | (2.2, SENTINEL-1) **Continuity ledger** — a scrollable dashboard of the unified deterministic continuity findings (co-location · timeline · numeric · character-fact drift · the **referenced-before-introduced** invariant), ranked (`⊗`/`⚠`/`●`) and grouped by kind. `↑↓` scroll, **Enter** jumps to a finding's paragraph, **`k`** runs the LLM coherence pass over the open book (cross-paragraph contradictions; explicit + cost-capped → Output), `Esc` closes. Zero-AI at the core. The continuity findings also ride the `Ctrl+B Shift+C` review pass (Output `continuity` category), and with `continuity.ambient` on, every save re-checks what the edit touched. CLI equivalent: `inkhaven continuity check` (`--only`/`--skip`/`--json`/`--coherence`). See [`CONTINUITY.md`](CONTINUITY.md). |
| `Shift+A`  | (2.3, LECTOR-1) **Read-through** — the book read forward, once, as a first reader: a scrollable dashboard of the prose-measured intensity curve vs the framework's intended curve, the per-chapter scene/sequel beat (`▶`/`◉`/`·`), and the ranked reader findings (confusion · info-dump · attention-dip · put-down risk · unpaid setup · scene/sequel arrhythmia · shape sag). `↑↓` scroll, **Enter** jumps to the chapter, **`k`** runs the cost-capped LLM synthetic first-read (→ Output `readthrough` category), `Esc` closes. Deterministic + free at the core; also rides the `Ctrl+B Shift+C` review pass. CLI equivalent: `inkhaven readthrough` (`--deep`/`--json`). Mnemonic: **A** for Audience. See [`LECTOR.md`](LECTOR.md). |
| `Shift+U`  | (2.5, CHRONICLE-1) **Draft chronicle** — the draft-history dashboard: the trend since your last milestone (findings/errors/warnings/infos + the categories that changed, every count fewer-is-better, ▼/▲) and the **cleared vs introduced** split — which findings your revision resolved and which new ones it created. `↑↓` scroll, **Enter** jumps to an introduced finding's paragraph, **`m`** marks the current draft (labelled by today's date; rename via the CLI), `Esc` closes. Pure measurement — it never edits the manuscript. CLI equivalent: `inkhaven chronicle` (`mark` / `list` / `diff` / `--json`). See [`CHRONICLE.md`](CHRONICLE.md). |
| `Shift+Z`  | (2.6, KEN-1) **Knowledge dashboard** — epistemic continuity (who knows what, when): the deterministic findings grouped by kind — `premature_knowledge` (a character references a topic before their earliest grant), `leaked_secret` (a `secret:` used by someone never told it), `dropped_reveal` (a declared reveal that never surfaces). `↑↓` scroll, **Enter** jumps to the offending paragraph, **`l`** (3.5) toggles to the **ledger** — the grants model (who could know what, when) grouped by character, where Enter jumps to the paragraph the character learns the topic in — `Esc` closes. Grants come from timeline event-presence + `secret:` / `know:` / `reveals:` tags. Deterministic + free; the CLI equivalent is `inkhaven knowledge` (`--json` / `--deep` for the opt-in implied_irony LLM pass / `--ledger` for the grants model). See [`KNOWLEDGE.md`](KNOWLEDGE.md). |
| `$`        | (1.3.34) **AI cost dashboard** — a scrollable panel of today's LLM call tallies for each capped subsystem (world slow track, Inner Socrates slow track + any analytical-thread sub-budgets) against their daily caps. Read-only. CLI equivalent: `inkhaven cost`. |
| `*`        | (3.5, H-1) **Reader hub** — one door to the whole reader family: a menu of every reader dashboard (Knowledge, Bonds, Continuity, Read-through, Cast, Character arc, Myth, Chronicle, Story bible), each with its live finding-count (for the deterministic readers; own-store readers show `·`). `↑↓` scroll, **Enter** opens the selected reader's dashboard, `Esc` closes. Solves the scattered-chords problem. Mnemonic: `*` = all readers. CLI equivalent: `inkhaven read` (the *state of the manuscript* overview). |
| `J`        | (INNER_SOCRATES-1) **Inner Socrates overview** — active persona, recent questions, the intent ledger. Sub-keys: `F` fast-check ¶ · `E` engage (slow/LLM) · **`T` Inner Theologian** (1.4.18; INNER-THEOLOGIAN-1) · **`P` Inner Poet** (POEM-3) · **`Y` Inner Stylist** (2.1; CHORUS-1) · `S` persona · `L` ledger · `A` ambient · `C` conversation. **`J→Y`** opens the **Inner Stylist** overview — the voice-at-scale coach: **`F`** synthesises the CHORUS pillars (character distinctiveness / drift, POV/head-hop, tense, register) into Praise / Note / Concern observations in the Output `stylist` category, **`E`** engages the AI coach into the Thoughts pane (grounded coaching, never a rewrite), **`R`** opens the voice-report dashboard (scrollable). It also rides the `Ctrl+B Shift+C` review pass. Suppressions persist in `<project>/inner_stylist.db`; the CLI surface is `inkhaven chorus voices / scan / report / stylist` (`--coach` for grounded LLM coaching). See [`CHORUS.md`](CHORUS.md). **`J→P`** opens the **Inner Poet** for the open verse paragraph (`para:verse-*`), with its own sub-keys: `F` fast-scan metre + rhyme against the declared form → Output (Praise / Note / Concern) · `E` engage the LLM slow track (an observation, never a rewrite) · **`D` declare a form** (a picker that writes a language-localised `poem:` sidecar) · **`T` two-column translation view** (source ∥ translation + the Form/Sound trilemma) · **`A` ambient** (auto fast-scan each verse paragraph as you open it; free, no cost cap). Findings + suppressions persist in `<project>/inner_poet.db`; while a verse paragraph is open the status bar shows a live syllable count (`♩ 8 syl · l2/4`), and the Outline shows completion chips (`8/14`, `14/14 ✓`). The CLI surface is `inkhaven poetry forms / syllabify / metre / rhyme / scan / status / trilemma`. **`J→T`** runs a slow-track theological session over the open paragraph: a tradition-neutral reader poses moral/theological questions through eleven tradition lenses (Catholic / Protestant / Orthodox / Gnostic / LDS / Islam / Judaism / Hinduism / Buddhism / Confucianism / secular), in the book's language, naming which lens raises which question — **it asks, never judges**. Questions land in the Output `theologian` category (`⚖`). The deterministic fast-track signals (moral invisibility / consequence gap / sacred levity) ride the `Ctrl+B Shift+C` review pass; the CLI surface is `inkhaven theologian scan / session / suppress`. |

The Tree pane's plain-letter shortcuts (`B`, `C`, `V`, `A`, `S`, `+`, `P`,
`D`, `-`) still work directly without the meta prefix when Tree has focus —
see §2.2. To run a tree action from the Editor, switch focus first
(`Ctrl+2` or `Tab`) and then use either the plain letter or meta.

`Tab` / `Shift+Tab` do **not** cycle focus when the editor pane has an open
paragraph — they cycle anyway in our implementation because we intercept them
before tui-textarea sees them, so they never insert a literal tab.

### 1.2 View mode (Ctrl+V prefix)

The third meta prefix. Routes to in-process exporters, pickers,
the writing-progress modal, paragraph links, and bookmarks. In
1.2.4+ all view-sub chords are rebindable through HJSON
`keys.bindings.view_sub` and `ink.key.bind_view_sub`; the
prefix itself is rebindable via `keys.view_prefix` (default
`"Ctrl+V"`). See tutorials 15 / 16 / 17 / 19 / 21 for the
full workflows.

| Chord (after `Ctrl+V`) | Pane (focus filter)      | Action                                                                 |
| ---------------------- | ------------------------ | ---------------------------------------------------------------------- |
| `Space`                | any                      | (1.3.33) **Command palette** — fuzzy-find any command by name / chord / description and run it. Type to filter, `↑↓` select, `Enter` runs, `Esc` closes. The canonical palette chord (a two-key chord with no `Ctrl+Shift+<letter>` terminal ambiguity); also self-lists in the Quick reference. |
| `1`                    | Editor / AI-prompt       | Write the **open paragraph's buffer** as markdown via the save-as picker (1.2.4 — default path pre-filled; bare Enter writes there). |
| `2`                    | Editor / AI-prompt       | Write the **containing subchapter's subtree** as markdown via the save-as picker. |
| `1`                    | Tree                     | Write the tree-cursor's **node and all descendants** as markdown via the save-as picker. |
| `S` / `s`              | any                      | Toggle **similar-paragraph mode** — saves the buffer, picks via vector search, opens a second editor side-by-side. Re-press to save both and exit. Both editors autosave on idle (1.2.4). |
| `G` / `g`              | any                      | Open the **writing-progress modal** (today / streak — incl. lifetime best, 1.3.35 / per-book pace / 30-day sparkline / status-ladder counts / per-book bar chart). Press `e` inside it to open the **goals editor** (1.3.35): edit `goals.daily_words` / `active_minutes_daily` / `streak_grace_per_week`; Enter writes the changed keys back to `inkhaven.hjson` via the comment-preserving in-place patch (versioned backup in `.config-backups/`). Also available headless as `inkhaven goals`. |
| `T` / `t`              | any (needs open paragraph) | Set / clear the **per-paragraph word-count target** (1.2.4+). Empty / `0` clears. Saves that cross the target auto-promote status one ladder step when `goals.auto_promote_on_target` is true. |
| `A` / `a`              | any (needs open paragraph) | **Add outgoing paragraph link** (1.2.4) — tree pane enters select-paragraph-to-link mode; Enter confirms. Self-link / duplicate / cycle are rejected with a status-bar message. |
| `I` / `i`              | any (needs open paragraph) | **Add incoming paragraph link** (1.2.4) — tree pane enters select-paragraph-that-will-link-to-current mode. Reverse of `A`. |
| `L` / `l`              | any (needs open paragraph) | **List outgoing links** (1.2.4) — floating picker. Enter opens (autosaves prev); D removes the link. |
| `K` / `k`              | any (needs open paragraph) | **List backlinks** (1.2.4) — paragraphs that link TO the open one. D removes the source's outgoing link. |
| `B` / `b`              | any (needs open paragraph) | Toggle **bookmark** on the open paragraph (1.2.4). |
| `M` / `m`              | any                      | Open the **bookmark picker** (1.2.4). Enter opens; D removes the bookmark. |
| `P` / `p`              | any                      | **Fuzzy paragraph picker** (1.2.4) — type-to-filter modal over every user-book paragraph. Three-tier ranking (title-starts > title-contains > slug-contains). |
| `R` / `r`              | any (needs open paragraph) | (1.2.5) **Render paragraph** — save the buffer, compile it in-process via `typst-render`, float a PNG preview on top of the editor. Inside the preview: `←` / `→` navigate pages (multi-page documents), `Home` / `End` jump to first / last; `Esc` closes; `S` opens a save-as picker for the **current page** at full DPI (288 dpi); `A` opens the picker for **all pages** at full DPI (writes `<base>-page-NNN.png` per page). Cancelling the save picker restores the preview with navigation state intact. |
| `N` / `n`              | any (needs open paragraph) | (1.2.5) **Next typst diagnostic** — move the editor cursor to the next parse or semantic diagnostic in the buffer. Wraps at the end. Refreshes the diagnostic cache up-front so navigation reflects the current buffer state, not the last save. Status bar reports `diag N/M  line L:C  — <message>`. |
| `W` / `w`              | any (needs current user book) | (1.2.5) **Story view** — build a twopi-style radial graph of the current book (book at centre, each depth on a concentric ring) with the hierarchy (chapters / subchapters / paragraphs / scripts / images / json) plus paragraph links (dashed purple) and Characters / Places / Artefacts mentions on an outer ring (dashed green). Rasterised via `resvg` and floated on top of the editor. Inside the modal: `Esc` closes, `S` opens a save-as picker (default `<book-slug>-story-YYYYDDMM-HHMM.png`). |
| `Shift+H`              | any                      | (1.2.14) **Threads picker** — fuzzy picker over every thread in the project's `Threads` system book. Enter opens a swim-lane weave view of the selected thread plus its 4 neighbours (by waypoint overlap), with manuscript paragraphs as columns and threads as horizontal lanes; waypoints as `●`, gaps as `·`. Inside the weave: ←/→ scroll the manuscript axis, `Esc` returns to the picker. Mnemonic: H for tHread (lowercase `h` is hidden-character report). |
| `Shift+A`              | any                      | (1.2.14) **AI thread audit** — sends every thread's beats + waypoints to the configured LLM with a prompt asking for blind spots (zero links, payoff unfired, dormant arcs). The response streams into the AI pane. Pairs with `Shift+D` (thread doctor modal) which runs the same checks deterministically without the LLM. |
| `Shift+D`              | any                      | (1.2.14) **Thread doctor modal** — deterministic blind-spot report. Walks every thread, flags `ZERO LINKS` (no waypoints linked to any paragraph), `PAYOFF UNFIRED` (climax beat exists but no waypoint after it), `DORMANT` (last waypoint > 30 days old). The TUI equivalent of `inkhaven thread doctor`. Inside the modal: ↑↓ scroll, `Esc` closes. |
| `c`                    | any (needs open paragraph) | (1.6.19, LOCI) **Lint the loci** — check the open paragraph's `@key[locus]` citations against their sources' declared reference schemes (deterministic; headless via `inkhaven index-locorum`). *The 1.2.14 add-comment feature moved to `Ctrl+Z c` — this LOCI check took over `Ctrl+V c`.* |
| `Shift+C`              | any                      | (NF-CITE) **Sourcing pass** — flag sentences in the open paragraph that make a checkable factual claim but carry no `@key` citation (cite / claim coverage). *The 1.2.14 comments panel moved to `Ctrl+Z Shift+C`.* |
| `d`                    | Editor (needs open paragraph) | (1.2.14) **AI continuation drafting** — asks the configured LLM to continue the open paragraph in the author's voice. Envelope: previous N paragraphs as voice anchors (`editor.continuation_anchor_count`, default 3) + the open paragraph with `[[CURSOR_HERE]]` marker. Response wrapped in `<<<DRAFT>>> / <<<END>>>` markers; the AI pane's `I` apply lifts only the draft block and inserts it at the cursor. Pairs with snippet expansion (e.g. `\tdo` + `Ctrl+V d` for AI-generated TODOs). |
| `f`                    | Editor (needs open paragraph) | (1.2.14) **Insert inline footnote** — pops a multi-line text input for the footnote body; on commit inserts `#footnote[<body>]` at the cursor (Typst, the default) or `[^id]` plus a trailing `[^id]: <body>` line (markdown, when `editor.footnote_style = "markdown"`). For academic / reference writing; the Typst markup is honoured by the assembled-book renderer. |
| `Shift+G`              | any                      | (1.2.14) **Project word-count goal modal** — reads `project.word_count_goal` + `project.target_date` from HJSON, projects the finish date from the last-30-day word delta (`progress_cache.sparkline`), renders a progress bar + verdict (`Ahead` ✓ / `On track` · / `Behind` ✗ / `Complete`). When unconfigured, the modal explains how to add the keys. Pairs with the writing-progress modal (`Ctrl+V g` — current pace) but goal-oriented rather than retrospective. |
| `Shift+J`              | any                      | (1.2.16) **Manuscript intelligence dashboard** — synthesis pane that unifies every metric inkhaven collects since 1.2.5: word count (today / total / streak / active minutes), structure (books / chapters / paragraphs + chapter-word-count CV + pacing verdict), threads (active / dormant), comments (open / resolved this week / lifetime).  `↑↓` scrolls; `e` exports the snapshot to `<project>/journal-<UTC>.md`; `Esc` closes.  J for Journal.  Cycle headline. |
| `y`                    | Editor (needs open paragraph) | (1.2.14) **Style-transfer rewrite** — picker over previously-edited paragraphs (sorted by recency); selection becomes the style reference. Envelope asks the LLM to rewrite the open paragraph in the reference's voice (match sentence length / register / narrative distance / rhythm; preserve meaning + entities + facts). Response wrapped in `<<<REWRITE>>> / <<<END>>>` markers; the AI pane's `I` apply lifts only the rewrite block and replaces the paragraph body. |
| `O` / `o`              | any                      | (INNER_EDITOR-1) **Inner Editor overview** — the craft-reader family's home (O = Observe; `Ctrl+B E` was taken by ToggleSound). Inside: `E` engage the open paragraph, `C` converse, `A` toggle ambient auto-engage, `F` filter the Output pane to inner-editor findings, `Esc` closes. |
| `Esc`                  | any                      | Cancel the chord without acting.                                       |

While in similar-paragraph mode, `Tab` inside the editor toggles
keyboard focus between the left and right editor panes (instead
of cycling to the missing AI pane).

---

## 2. Tree pane

Focused on launch. Shows the project hierarchy with depth indentation, kind
glyphs (`📖` book, `▸` chapter, `▹` subchapter, `¶` paragraph), and a dim
`Nw` word-count suffix for paragraphs.

### 2.1 Navigation

| Key                  | Action                                                      |
| -------------------- | ----------------------------------------------------------- |
| `↑` / `↓`            | Move cursor one row up/down (within scroll).                |
| `→`                  | **Expand** the cursor's branch (book/chapter/subchapter), revealing its children. No-op on a paragraph or an already-expanded branch. |
| `←`                  | **Collapse** the cursor's expanded branch. If already collapsed (or on a paragraph), moves the cursor to the parent node. Same semantics as the F3 file picker. |
| `Home`               | Jump to first row.                                          |
| `End`                | Jump to last row.                                           |
| `PageUp`             | Move cursor 10 rows up (configurable: `page_up`).           |
| `PageDown`           | Move cursor 10 rows down (configurable: `page_down`).       |
| `Enter`              | Open the cursor's node. Paragraphs load into the editor and shift focus there; if a different paragraph was open with unsaved edits, it's autosaved first. Branches print a status hint and stay in Tree. |
| `F2`                 | Open the **Rename** modal pre-filled with the current node's title. Slug + filesystem entry stay; only the displayed title changes (re-embeds for search). |
| `F3`                 | Open the **file picker** dialog. Enter on a file creates a new paragraph (inserted after the current cursor) with that file's content. Enter on a directory **recursively imports** the tree — subdirectories become branches one level deeper (Book→Chapter→Subchapter), files become paragraphs. If the directory tree exceeds the hierarchy depth, the deeper files are flattened into the deepest legal branch (with `unbounded_subchapters: false`). See §12. |
| `q` or `Q`           | Quit (autosaves the open paragraph first if dirty).         |
| `Esc`                | Cycle focus to the **Search bar** (second leg of the Editor → Tree → Search → Editor rotation). |

**Open-paragraph indicator** — the row of the paragraph currently loaded in
the Editor is rendered with a **green bold "►"** marker (instead of the
usual `¶` glyph) regardless of focus. The marker stays visible whether the
Editor or Tree pane has focus, so you can always see which paragraph is
loaded. If your tree cursor happens to land on the open paragraph, the
REVERSED cursor highlight wins visually but the green color underneath
still marks the row.

### 2.2 Tree-pane shortcuts (modifier-free)

These plain-key shortcuts work only when the Tree pane has focus. They exist
alongside the global meta-prefix chords (§1.1) because terminals and
multiplexers commonly intercept those (see §13 for details). All four open
the same modals as their global equivalents — no destructive action without
confirmation.

**Append at end** — `B`, `C`, `A`, `+` open the Add modal and place the new node at the end of its parent's children. The parent is chosen by walking up from the tree cursor to the nearest node that can host the requested kind.

**Insert after current** — `V`, `S`, `P` open the same Add modal but place the new node immediately after the cursor's same-kind ancestor. All subsequent siblings get their `order` bumped by `+1` and their filesystem entries renamed. If no same-kind ancestor exists (e.g. pressing `P` on a book with no paragraphs), falls back to append-at-end so the action still does something.

| Key       | Action                                                                                  |
| --------- | --------------------------------------------------------------------------------------- |
| `B` / `b` | Add a new **book** at the root. User books are inserted **above** the system block (Notes, Research, Prompts, Places, Characters, Help) by shifting it down; the new book takes Notes' old order. (There is **no** meta equivalent — `Ctrl+B B` schedules a build, §1; add-book is only this bare Tree key.) |
| `C` / `c` | **Append** a chapter at the end of the book's children. Equivalent to `Ctrl+B` then `C`. |
| `V` / `v` | **Insert** a chapter immediately after the cursor's enclosing chapter.                  |
| `A` / `a` | **Append** a subchapter at the end of the chapter's children. Equivalent to `Ctrl+B` then `S`. |
| `S` / `s` | **Insert** a subchapter immediately after the cursor's enclosing subchapter.            |
| `+`       | **Append** a paragraph at the end of the parent's children. Equivalent to `Ctrl+B` then `P`. |
| `P` / `p` | **Insert** a paragraph immediately after the cursor's enclosing paragraph.              |
| `D` / `d` | Delete the cursor's node — only if it's a **branch** (book/chapter/subchapter). On a paragraph, shows a hint to press `-` instead. |
| `-`       | Delete the cursor's node — only if it's a **paragraph**. On a branch, shows a hint to press `D` instead. |
| `U` / `u` | **Move up** — swap the cursor's node with its previous sibling. Plain-letter form of `Ctrl+B ↑`. |
| `J` / `j` | **Move down** — swap the cursor's node with its next sibling. Plain-letter form of `Ctrl+B ↓`. |
| `Z` / `z` | **Collapse subchapter** — folds the cursor's enclosing Subchapter (or the cursor's node itself if it IS a Subchapter). Lands the tree cursor on the folded row. |
| `X` / `x` | **Collapse all** — folds every expanded branch in the tree. Empty branches and paragraphs are untouched. |
| `Space`   | (1.2.4) **Mark / unmark** the cursor row for multi-select. Status bar shows `marked N`. `Esc` clears all marks. |
| `T` / `t` | (1.2.4) **Cycle node type** (`paragraph(typst) → paragraph(hjson) → paragraph(jinja) → script`; STRUCT-1 added the **jinja** rung). No marks: cursor row only (folders skipped). With marks: every marked leaf. |
| `E` / `e` | (1.4.10) **New Jinja template** — open the Add modal for a `.jinja` paragraph (rendered to Typst at assembly). Seeds a manuscript template under a user book, or a reusable `{% include %}` fragment under the **Snippets** book; rejected elsewhere. (`t`/`T` *converts* an existing paragraph; `e` *creates* a fresh seeded one.) See [JINJA_TEMPLATES.md](JINJA_TEMPLATES.md). |
| `I` / `i` | (1.4.11) **New structural paragraph** — open a picker of structural subtypes (code listing · admonition note/warning/tip/caution · math · procedure · table) and create a `.typ` paragraph tagged `para:*` with the matching Typst boilerplate seeded. Structural paragraphs get a type-specific tree glyph (`⌨ ⚠ ∫ ≡ ⊞`), are skipped by the prose companions, and are excluded from prose word counts (except procedure). Add/remove the tag later via `Ctrl+B ]`. See [STRUCTURAL_PARAGRAPHS.md](STRUCTURAL_PARAGRAPHS.md). |
| `O` / `o` | (1.2.4) **Cycle status** one rung up the ladder (`napkin → first → … → ready → napkin`). No marks: cursor row. With marks: every marked paragraph. |
| `G` / `g` | (1.2.5) **Tag the marked set** — open the floating tag picker scoped to every marked paragraph (or just the cursor row when no marks). Same modal as `Ctrl+B ]`; T applies the selected tag set across every target at once. |
| `y`       | (1.4.13, OUTLINE-1) **Copy** the cursor paragraph onto the cross-pane clipboard (shared with the Outline pane). |
| `m`       | (1.4.13, OUTLINE-1) **Move** (cut) the cursor paragraph onto the clipboard. |
| `f`       | (1.4.13, OUTLINE-1) **Affix** the clipboard paragraph as the last child of the cursor's effective parent (INTO it when the cursor is a branch, alongside it when a paragraph). Copy duplicates (fresh uuid, keeps the clipboard); move relocates and clears it. |
| `?`       | (1.3.33) **Quick reference** — open the pane-aware Quick reference overlay. Tree-pane only (the editor / AI / search panes keep `?` as a typed character); `Ctrl+B H` opens it from anywhere. |

Empty paragraph titles are allowed for `+` and `P` — the first sentence of the body becomes the title on next save.

**Multi-select interaction** (1.2.4): mark rows with `Space`. With at
least one row marked, the bulk operations walk the mark set instead of
the cursor row — `Ctrl+B R` cycles the *status* of every marked
paragraph, and `Ctrl+B M` (or the tree `t` / `T` morph) cycles each
leaf's *content type*. `Esc` clears the marks.

Why kind-specific delete? Safety. `-` won't nuke an entire chapter if your
cursor accidentally landed on it, and `D` won't kill a paragraph you meant
to keep. If you want delete that doesn't care about kind, use the global
`Ctrl+B` then `D`.

Shortcuts ignore the `Shift` modifier (uppercase implies Shift on most
layouts) but reject `Ctrl` / `Alt` / `Super` — so `Ctrl+A` will *not* fire
Add-subchapter.

All global chords also fire from the Tree pane.

---

## 3. Editor pane

Focused automatically when a paragraph is opened. Backing widget is
`tui-textarea` driven by `input_without_shortcuts`, so emacs-style defaults
(Ctrl+A → start of line, Ctrl+P → previous line, etc.) are **off**. We
intercept the modern conventional shortcuts ourselves; everything else falls
through to tui-textarea's typing / cursor handling.

**Border color** carries the dirty state at a glance — but only while the
pane has focus:

- **Green (bold)** — focused, in sync with disk + bdslib (saved).
- **Yellow (bold)** — focused, with unsaved edits.
- **White** — pane is *unfocused*. Dirty signaling moves to the title's
  `[modified]` suffix and the red `●` chip in the status bar (both
  always-on indicators).

**Focus-loss autosave**: whenever focus moves away from the Editor pane —
via `Tab`, `Ctrl+1..5`, `Ctrl+T`, `Ctrl+/`, `Ctrl+I`, `Esc` from another
input, etc. — the open paragraph is automatically saved if dirty. So you
can shift focus mid-edit without worrying about losing work; the next save
trigger (idle/quit/switch) won't catch the same change twice.

### 3.1 Cursor movement

| Key                  | Action                                                      |
| -------------------- | ----------------------------------------------------------- |
| `←` / `→`            | One character left / right.                                 |
| `↑` / `↓`            | One line up / down.                                         |
| `Home`               | Start of current line.                                      |
| `End`                | End of current line.                                        |
| `PageUp` / `PageDown`| One viewport up / down (tui-textarea internal).             |
| `Ctrl+←`             | Previous word boundary.                                     |
| `Ctrl+→`             | Next word boundary.                                         |
| `Ctrl+Home`          | Top of document.                                            |
| `Ctrl+End`           | Bottom of document.                                         |

### 3.2 Editing

| Key                  | Action                                                      |
| -------------------- | ----------------------------------------------------------- |
| any character        | Insert at cursor. Replaces selection if one exists.         |
| `Enter`              | Insert newline.                                             |
| `Backspace`          | Delete character before cursor (or whole selection).        |
| `Delete`             | Delete character at cursor.                                 |
| `Ctrl+Backspace`     | Delete previous word.                                       |
| `Ctrl+S`             | Save current paragraph to disk and re-embed in bdslib. Triggers a tree reload so word counts refresh. |

### 3.3 Selection, clipboard, undo

`tui-textarea` maintains a single linear selection range. Shift+arrows extend
it. **Note:** the editor uses non-standard keys for cut and paste because the
conventional bindings now do other things (`Ctrl+X` is "repeat" for search,
`Ctrl+Z` is delete-to-end-of-line). The mapping below has been chosen so
each operation lives on a distinct key with no overlap.

| Key                  | Action                                                      |
| -------------------- | ----------------------------------------------------------- |
| `Shift+←` / `Shift+→`| Extend selection left / right one character.                |
| `Shift+↑` / `Shift+↓`| Extend selection up / down one line.                        |
| `Ctrl+A`             | Select entire document.                                     |
| `Ctrl+C`             | **Copy** selection to system clipboard (falls back to internal yank if `arboard` failed to init). |
| `Ctrl+K`             | **Cut** selection to clipboard. Marks doc dirty.            |
| `Ctrl+P`             | **Paste** from clipboard at cursor (or replace selection). Marks dirty. |
| `Ctrl+U`             | **Undo.**                                                   |
| `Ctrl+Y`             | **Redo.**                                                   |

The line-targeted delete shortcuts (§3.11) all preserve the yank buffer so
they don't clobber clipboard state.

If `arboard::Clipboard::new()` fails at startup (typical on headless or some
Wayland setups), copy/cut/paste silently fall back to tui-textarea's
internal yank buffer — the chords still work within the editor session, but
don't cross process boundaries.

### 3.4 Vertical block selection (rectangular)

A second, separate selection model independent of tui-textarea's native
range. Always rectangular: anchor + current cursor define inclusive
`(row_min..row_max, col_min..col_max)`. Drawn with REVERSED style on top of
the syntax highlighting.

| Key                          | Action                                                  |
| ---------------------------- | ------------------------------------------------------- |
| `Alt+↑` / `↓` / `←` / `→`    | Enter block-select mode (if not already), then move cursor by one cell without changing tui-textarea's linear selection. Rectangle redraws each frame. |
| `Alt+C`                      | Copy the rectangle to system clipboard as a multi-line string (each row a line). Clears the anchor. |
| `Esc`                        | Cancel block-select; keep the doc open.                 |
| any non-Alt key              | Cancels block-select implicitly (falls through to normal editor handling). |

**Deferred in this release**: rectangular cut and rectangular paste require
bulk character-deletion across multiple lines, which tui-textarea doesn't
expose cleanly. Copy-only covers the common cases (extracting a column of
leading numbers, a list of names, a verse stanza).

### 3.11 Line-targeted delete shortcuts

Four chords that delete chunks of the current line without touching the
clipboard. Each saves and restores the yank buffer around the operation, so
`Ctrl+P` paste still produces the last copy.

| Key       | Action                                                                 |
| --------- | ---------------------------------------------------------------------- |
| `Ctrl+D`  | **Delete current line** — removes the entire line + its trailing newline; cursor lands on the line that takes its place. On the very last line, the content is cleared and an empty line remains (no newline to delete). |
| `Ctrl+E`  | **Delete to end of line** — removes from the cursor to the line end.   |
| `Ctrl+W`  | **Delete to start of line** — removes from the cursor back to column 0. |

*(`Ctrl+Z` is the **Bund-meta prefix** in 1.2+ — runs scripting actions
like `Ctrl+Z R` run-buffer, `Ctrl+Z N` new-script, `Ctrl+Z E` eval,
`Ctrl+Z ?` script-picker. See [`KEYS_REASSIGNMENT.md`](KEYS_REASSIGNMENT.md).
Undo is `Ctrl+U`, delete-to-EOL is `Ctrl+E`.)*

*(`Ctrl+V` is the **view-meta prefix** in 1.2.3+ — markdown export,
similar-paragraph mode, and progress tracking. See section 1.2 below
and tutorials 15 / 16 / 17.)*

**Note on `Ctrl+W`**: bash, tmux, and some terminals interpret `Ctrl+W` as
"delete previous word" before forwarding the keystroke. If your shell layer
eats `Ctrl+W`, use the meta prefix path (`Ctrl+B`, then a future-defined
alias) or rebind the chord in `inkhaven.hjson` once configurable bindings
for it are added.

### 3.9 Split-edit mode

A two-pane "edit with lookback" view. Toggle with `F4`. While split is
active the editor area is divided 50/50 horizontally: the **upper pane** is
your normal read-write editor and the **lower pane** is a read-only
snapshot of the buffer captured at the moment you pressed F4. The lower
pane scrolls independently so you can keep an earlier passage visible
while you rewrite it above.

| Key       | Action                                                                  |
| --------- | ----------------------------------------------------------------------- |
| `F4`      | Toggle split. Capture the buffer on enter; drop the snapshot on exit.   |
| `Ctrl+F4` | **Accept** the snapshot — replace the live buffer with the captured copy, exit split, mark dirty (bold marks the diff; Ctrl+S commits the rollback). |
| `Ctrl+H`  | Scroll the lower (snapshot) pane up by one line. Only active in split.  |
| `Ctrl+J`  | Scroll the lower pane down by one line. Only active in split.           |

The upper pane behaves exactly like the full editor — same shortcuts, same
syntax highlighting, same selection / clipboard / undo, same idle autosave,
same diff bolding. The lower pane is fully passive: no cursor, no
highlighting, dim grey text. Its header shows the current visible line and
the snapshot's total line count, plus a reminder of the available keys.

`Ctrl+H` and `Ctrl+J` are routed to the split pane **only while split is
active**. When split is off they fall through to normal editor handling
(tui-textarea's defaults), so they don't shadow anything in regular use.
The Quick-reference overlay is opened via `Ctrl+B` `H` (meta prefix)
precisely so it never contends with the split-scroll chord.

### 3.10 Find and replace (regex)

In-buffer regex search with optional replacement. Matches are highlighted
in **red** on top of the syntax coloring; the cursor's current match gets a
brighter **LightRed + bold** style so it stands out among siblings.

| Key                | Action                                                                |
| ------------------ | --------------------------------------------------------------------- |
| `Ctrl+F`           | Open the **Find** modal (magenta-bordered). Type a regex, Enter to run. Cursor jumps to the first match; all matches stay highlighted. Status bar reports `match 1 / N`. |
| `Ctrl+X`           | **"Repeat"** (multifunction). In search mode: jump to the next match (wraps). In replace mode: replace the current match and advance to the next. Only active while a search is in progress; otherwise the keystroke falls through. |
| `Ctrl+R`           | **First press**: open the **Find & Replace** modal (search + replace fields, `Tab` switches between them). Enter applies the **first** replacement automatically and stays in replace mode. **Second press while in replace mode**: replace every remaining match and exit replace mode. |
| `Ctrl+B` (in the modal) | (1.2.22) **Toggle scope** between *this paragraph* and *the whole book* (replace mode only; a chip shows which). In book scope, Enter scans every user-book paragraph and opens the **review modal**: matches shown in context, `↑↓` move, `Space` skip the one under the cursor, `a` keep all / `n` skip none, `Enter` apply, `Esc` cancel. Starts in whole-word literal mode (the safe default); **(1.2.23)** `w` / `i` / `x` toggle whole-word / ignore-case / regex in place (each re-runs the scan), and the header shows the active mode. Each changed paragraph is snapshotted (annotated `replace: X → Y`) before the write, so `F6` is the undo. Also available as `inkhaven replace <pat> <repl> [--regex] [--substring] [--ignore-case] [--book] [--include-system] [--dry-run] [--yes]`. See [Tutorial 64](Tutorials/64-project-find-and-replace.md). |
| `Esc` (in editor)  | Clear the active search (drops the highlights, exits replace mode).   |

**Regex flavor:** full Rust [`regex`](https://docs.rs/regex) syntax. Use
flags via `(?i)` (case-insensitive), `(?s)` (dot matches newlines), etc.

**Per-line matching:** v1 searches line-by-line so cross-line patterns
won't match. Most literary search/replace tasks (word substitution, name
changes) are within-line anyway.

**Layer order in the renderer:** syntax color → `[modified]` bold → match
red bg → current-line highlight → selection REVERSED. Selection wins
visually when a char is both selected and matched; matches win over the
subtle current-line highlight.

**Pre-fill:** opening `Ctrl+F` or `Ctrl+R` again after an active search
pre-populates the modal inputs with the previous pattern (and replacement).
Edit them and Enter to re-run.

### 3.5 Snapshots and file loading

| Key  | Action                                                              |
| ---- | ------------------------------------------------------------------- |
| `F3` | Open the **file picker** dialog. Pick a file with Enter to replace the open paragraph's editor buffer (bold marks the change vs the saved version). Directories are rejected in this context. See §12 for navigation. |
| `F4` | Toggle **split-edit** mode — see §3.9. |
| `F5` | Save a versioned **snapshot** of the open paragraph's current body (stored as a bdslib document with `kind:"snapshot"` and a `parent_id` back-reference; doesn't appear in vector search). |
| `F6` | Open the **snapshot picker** overlay listing every snapshot for the open paragraph, newest first. `↑↓` navigates, `Enter` loads the selected snapshot (1.2.4: takes a **pre-restore safety snapshot** of the live buffer first), `V` opens a **side-by-side diff** of the snapshot vs current (1.2.4 — Esc returns to picker), `D` / `Del` removes the snapshot, `Esc` cancels. |
| `Ctrl+F6` | (1.3.36) Open the **project-wide snapshot browser** — every snapshot across *all* paragraphs, newest first, one row each (timestamp · words · paragraph · annotation). `↑↓` navigates, `/` filters by paragraph title or annotation (Enter/Esc leaves the filter; a second Esc closes), `V` diffs the selection against its paragraph's current text (reuses the same diff modal — Esc returns to the browser), `Enter` opens that paragraph and drops into its `F6` picker, `Esc` closes. Available from any pane. |

Snapshots are independent documents — they survive paragraph saves and aren't
deleted when their parent is deleted, so they can act as a recovery hatch.

**Pre-restore safety net (1.2.4)**: Enter in the snapshot picker first creates
a snapshot of the live buffer, then replaces. If creating the safety snapshot
fails, the load aborts entirely — the buffer stays untouched. To undo an
unwanted restore: F6 again, the safety snapshot is at the top, Enter.
Currently they're not surfaced from the CLI; that's an easy follow-up if you
need scripted access.

### 3.6 Autosave and background sync

Three save triggers, plus manual `Ctrl+S`:

- **Idle**: when the editor has unsaved edits and the user hasn't pressed a
  key for `editor.autosave_seconds` (default 5; set to 0 to disable).
- **Paragraph switch**: opening another paragraph from the Tree pane
  autosaves the current one first.
- **Quit**: `Ctrl+Q` and the `q` quit chords autosave before exiting.

In addition, a background task calls `Store::sync()` every
`sync_interval_seconds` (default 60). This flushes the HNSW vector index +
DuckDB checkpoint without blocking the UI. Set to 0 to disable.

Every save also resets the bold "added since last save" overlay (§3.7).

### 3.7 Visual change tracking

Characters added to the editor since the last save (Ctrl+S, autosave, or
load) are rendered **bold** on top of the syntax highlighting. The marker
goes away the moment you save. Implemented with a per-line longest-common-
prefix/suffix diff — fast at literary scale, accurate for the common case
of typing within or appending to a line. Cross-line inserts may
misattribute briefly until the next save resets the snapshot.

### 3.8 Pane management while focused

| Key                  | Action                                                      |
| -------------------- | ----------------------------------------------------------- |
| `Esc`                | Defocus to Tree without closing the document. If a block selection is active, Esc clears it first. |
| `Tab` / `Shift+Tab`  | Cycle focus (intercepted globally so they don't insert tab).|

### 3.6 When no paragraph is open

If the editor pane is focused but `opened` is `None`, only one key matters:

| Key       | Action |
| --------- | ------ |
| `q` / `Q` | Quit.  |

Plus all global chords.

---

## 4. AI pane

Focus lands here automatically only via the AI prompt's `Esc` bounce — when
you submit a query the prompt bar **keeps** focus so follow-ups are one
keystroke away. Pane title shows provider, streaming status, and a
`· N turn(s)` chip whenever the chat history has accumulated content.

| Key       | Condition                       | Action                                              |
| --------- | ------------------------------- | --------------------------------------------------- |
| `Esc`     | always                          | Bounce focus back to the **AI prompt** bar (mirror of the AI-prompt → AI Esc). |
| `r` / `R` | inference done, doc open        | Replace editor selection (or entire doc if no selection) with the AI text. Marks dirty, refocuses Editor. |
| `i` / `I` | inference done, doc open        | Insert AI text at cursor. Marks dirty.              |
| `t` / `T` | inference done, doc open        | Prepend AI text to top of paragraph (with blank line separator). |
| `g` / `G` | inference done, doc open        | **Grammar-check apply**: lifts only the corrected paragraph from the response (between `<<<CORRECTED>>>` / `<<<END>>>` markers, or last fenced code, or after a "Corrected …" heading) and overwrites the editor buffer wholesale (constructs a fresh `TextArea` so nothing of the old buffer survives). Skips the markdown→Typst conversion because the grammar prompt preserves Typst markup verbatim. Changed characters render in `theme.grammar_change_fg` (default red) and the highlight **survives saves** — dismiss it explicitly with `Ctrl+B` then `C`, or by switching paragraphs. Refuses with a status message if no extraction pattern matches. |
| `b` / `B` | inference done, doc open        | Append AI text to bottom of paragraph.              |
| `c` / `C` | inference done                  | Copy AI text to system clipboard only (no editor change). |
| `q` / `Q` | always                          | Quit.                                               |

Action keys fire only when `inference.status == Done` and the response is
non-empty. While streaming or on error, single-character keys do nothing
(except `q` to quit and `Esc` to bounce).

**Chat history.** Each non-Help inference appends a `(User, Assistant)` pair
to the in-memory chat history; the next prompt replays the whole history to
the model so the conversation is continuous. The title's `· N turn(s)` chip
shows the current depth. Press `F9` (or `Ctrl+B` then `C`) at any time to
clear both the history and the currently displayed inference.

Help (`F1` / `Help! …`) inferences are deliberately **one-shot** — they use
a strict RAG system prompt and are not added to the chat history, so a
prior set of chat turns won't dilute their grounding.

### 4.1 Output pane

`Ctrl+B Tab` / `Ctrl+B Shift+Tab` cycle the right region between the **Output**
pane (structured one-way notifications from every subsystem — fact-check,
Socratic, timeline critique, translation, lexicon, Bund, …), the **AI** pane,
and (1.4.18; THOUGHTS-1) the **Thoughts** pane — a read-only, scrollable home for
long reflective output, e.g. the Inner Theologian slow track (`Ctrl+B J→T`). In
the Thoughts pane: `↑↓`/`j`/`k` scroll, `PageUp`/`PageDown`, `g`/`G` top/bottom,
`c` clear, `Esc` to the editor. Plain **`Tab`** now cycles Tree → Editor → *the
currently-shown right pane* (no longer forced to AI). The active right pane is
remembered across restarts. **`Ctrl+Z f`** fullscreens the current right pane
(Output / Thoughts; the AI pane uses `Ctrl+B K`). When content arrives for a
pane it auto-surfaces — unless you're actively working in a right pane, so it
never steals focus mid-read.

| Key       | Action                                                                          |
| --------- | ------------------------------------------------------------------------------- |
| `↑` / `↓` (or `k` / `j`) | Select previous / next message.                                  |
| `g` / `G` | First / last message.                                                           |
| `o` / `Space` | Expand / collapse the selected message's structured detail.                 |
| `a`       | Ask the AI about the selected message (carries its full detail by reference).   |
| `d`       | Dismiss the selected message.                                                   |
| `p`       | Pin / unpin (pinned messages sort to the top).                                  |
| `Enter`   | Primary action — kind-specific (open target, accept proposal, jump to event, …).|
| `r` / `e` | (translations) remember / edit+remember.                                        |
| `i` / `m` / `x` | (Socratic) record-as-intent / make-note / mark-addressed.                 |

**Filtering (1.3.33+)** — narrow a busy Output pane; the title shows
`shown/total · <filter>` and the filter persists in `.session.json`:

| Key | Action                                                                              |
| --- | ----------------------------------------------------------------------------------- |
| `f` | Cycle the **source** filter: off → fact-check → socrates → timeline-critique → world → translation → lexicon → variety → ai → bund → other → off. |
| `S` | Cycle the **minimum severity**: off → Info → Warning → Contradiction → off (hides lower-severity *and* transient progress ticks). |
| `t` | Toggle **this paragraph only** — show just the messages tied to the open paragraph. |
| `c` | Clear all filters.                                                                  |

---

## 5. Search bar (top input)

Activated by `Ctrl+/` from any non-modal focus. Cursor appears as a `│`
character at the buffer's character position.

| Key                  | Behavior                                                    |
| -------------------- | ----------------------------------------------------------- |
| any printable char (no Ctrl/Alt) | Insert at cursor. Closes the results overlay if it was open (query has changed). |
| `Backspace`          | Delete char before cursor; closes results overlay.          |
| `Delete`             | Delete char at cursor; closes results overlay.              |
| `←` / `→`            | Move cursor one char left / right.                          |
| `Home`               | Cursor to start.                                            |
| `End`                | Cursor to end.                                              |
| `↑`                  | (overlay open) Move result cursor up.                       |
| `↓`                  | (overlay open) Move result cursor down.                     |
| `Enter`              | If results overlay is open: open the highlighted result. Otherwise: run `Store::search_text(query, 10)` and show results. |
| `Esc`                | If results overlay is open, close it (one press); else cycle focus to the **Editor** pane (third leg of the Editor → Tree → Search → Editor rotation). |

Opening a result from this overlay positions the tree cursor on the target
node. Paragraphs additionally load into the editor (focus moves to Editor).

---

## 6. AI prompt bar (bottom input)

Activated by `Ctrl+I`. Behaves like the Search bar with a different submit
action and the `/`-triggered Prompt picker overlay.

| Key                  | Behavior                                                    |
| -------------------- | ----------------------------------------------------------- |
| any printable char (no Ctrl/Alt) | Insert at cursor. If the buffer starts with `/`, opens the Prompt picker; otherwise closes it. |
| `Backspace`          | Delete char before cursor. Refreshes the picker if visible. |
| `Delete`             | Delete char at cursor. Refreshes the picker.                |
| `←` / `→`            | Move cursor one char left / right.                          |
| `Home`               | Cursor to start.                                            |
| `End`                | Cursor to end.                                              |
| `↑`                  | (picker open) Move selection up.                            |
| `↓`                  | (picker open) Move selection down.                          |
| `Tab`                | (picker open) Expand selected prompt template into the buffer with `{{selection}}` / `{{context}}` substituted. |
| `Enter`              | If picker open: same as Tab — expand selected template. Otherwise: spawn a streaming inference. Focus **stays** on the AI prompt bar (it does not jump to the AI pane). The buffer is sent verbatim, except: a leading `/name` is resolved against the prompt library, and a leading `Help!` (case-sensitive) routes the rest of the line through the F1 Help-RAG flow. |
| `Esc`                | If picker open, close it; else bounce focus to the **AI pane** so you can read or scroll the answer. Pressing `Esc` again from the AI pane brings you straight back here. |
| `Ctrl+1`             | Focus the **Editor** pane (global shortcut, works from this input too). |
| `Ctrl+T`             | Focus the **Tree** pane (global shortcut, works from this input too). |

Submitting a query when no API key is set in the environment surfaces a
status-line error like `GEMINI_API_KEY not set in environment — `export
GEMINI_API_KEY=...`` and does not spawn a request. **Local providers** like
Ollama omit `api_key_env` in their `llm.providers` block entirely; the
check is skipped and genai routes to `http://localhost:11434/` from the
model name. Provider, model, and API key env var are all driven by the
`llm` block in `inkhaven.hjson`.

**Continuous chat.** Each submitted query plus its assistant response is
appended to the chat history and replayed back on the next prompt. The AI
pane title shows the current `· N turn(s)`. Press `F9` (or `Ctrl+B` then
`C`) to clear it.

---

## 7. Search results overlay

Floating yellow panel rendered over the body when a search has run. Top line
shows `Results for `<query>` (N)`; each result occupies three rows
(score+kind+path, title, snippet).

Keys are routed to this overlay implicitly while it is open and the Search
bar is focused (see §5). The pane's own keys are:

| Key                  | Action                                                      |
| -------------------- | ----------------------------------------------------------- |
| `↑` / `↓`            | Move result cursor.                                         |
| `Enter`              | Open the highlighted result.                                |
| `Esc`                | Close the overlay (Search bar stays focused).               |
| Typing               | Closes the overlay and continues editing the query.         |

---

## 8. Prompt picker overlay

Floating magenta panel anchored just above the AI prompt bar. Two sources
are merged, in this order:

1. **System prompts** from `prompts.hjson` — well-known templates that ship
   with the project. Shown with a cyan `[ system ]` chip.
2. **Book prompts** — every paragraph nested under the **Prompts** system
   book. The paragraph's slug supplies the `/name` identifier and the
   title supplies the description. Body is the template. Shown with a
   green `[ book ]` chip.

A name or description that contains the text after `/` in the bar (case-
insensitive) is included. Filter updates live as you type.

Routed to the AI prompt bar (§6) — the picker has no separate focus.

| Key                  | Action                                                      |
| -------------------- | ----------------------------------------------------------- |
| `↑` / `↓`            | Move selection.                                             |
| `Enter` or `Tab`     | Expand the selected prompt's template into the buffer.      |
| `Esc`                | Close the picker without expanding.                         |
| Backspace / Delete   | Modify the filter; picker re-filters live.                  |

The leading `= Title` Typst heading is stripped from book-prompt bodies on
expansion so it doesn't end up in the LLM prompt — the heading is editor
chrome, not prose. `{{selection}}` / `{{context}}` substitutions in the
expanded body still fire for both sources.

A direct `/name` typed into the AI prompt bar and submitted with `Enter`
(no picker open) is also resolved against both sources, system-first.

---

## 9. Add modal

Triggered by `Ctrl+B` followed by `B`/`C`/`S`/`P` (or by the Tree pane's plain-letter shortcuts, §2.2). Green-bordered floating box.

```
┌── Add chapter ──────────────────────────────────┐
│  Parent: midnight-library                       │
│  Title : My chapter title│                      │
│                                                 │
│  Enter to confirm · Esc to cancel               │
└─────────────────────────────────────────────────┘
```

| Key                          | Action                                              |
| ---------------------------- | --------------------------------------------------- |
| any printable char (no Ctrl/Alt) | Insert into title buffer.                       |
| `Backspace`                  | Delete previous char.                               |
| `Delete`                     | Delete char at cursor.                              |
| `←` / `→` / `Home` / `End`   | Cursor navigation in the title buffer.              |
| `Enter`                      | Commit: derives slug, creates filesystem entry, inserts bdslib record, reloads tree, moves tree cursor to the new node. |
| `Esc`                        | Cancel without creating anything.                   |
| `Ctrl+Q`                     | Hard quit (modal does not absorb this).             |

Empty title shows a status hint and keeps the modal open. Validation errors
(e.g. trying to add a subchapter under a paragraph) close the modal and
display the error in the status line.

---

## 9. Outline pane (1.4.13, OUTLINE-1)

A full-screen, foldable view of the whole manuscript over the live hierarchy
— the structural counterpart to the side Tree pane. Open it with `Ctrl+2`
(or `Ctrl+B Shift+O` if your terminal eats `Ctrl+2`). The view state (expand
flags, cursor, scroll, filter) persists per project to
`.inkhaven/outline-state.json`, so it reopens where you left it. On first
open Books and Chapters are expanded and everything deeper is collapsed — a
structural overview you drill into.

When the pane is ≥ 80 columns wide, a right-hand **detail panel** shows the
cursor node's title, kind, ancestor breadcrumb, status, word count vs target
(or child count for branches), tags, and last-modified date.

| Key                | Action                                                                                 |
| ------------------ | -------------------------------------------------------------------------------------- |
| `j` / `↓`, `k` / `↑` | Move the cursor down / up.                                                            |
| `g` / `G`          | Jump to the first / last visible row.                                                   |
| `Enter` / `l` / `→` | Expand a collapsed branch; on an already-open branch, step in to the first child.      |
| `h` / `←`          | Collapse an open branch; otherwise step out to the parent.                              |
| `Space`            | Toggle the fold on a branch (no cursor move).                                            |
| `Shift+K` / `Shift+J` | **Reorder** — swap the cursor node with its previous / next sibling (filesystem-aware). |
| `<` / `>`          | **Promote / demote** one nesting level (childless nodes only): promote appends under the grandparent, demote nests into the preceding sibling. Placement-rule violations leave the manuscript untouched. |
| `y` / `m`          | **Copy / move** the cursor paragraph onto the cross-pane clipboard (shared with the Tree pane). |
| `f`                | **Affix** the clipboard paragraph as the last child of the cursor's effective parent (INTO a branch, alongside a paragraph). Copy duplicates (fresh uuid, keeps the clipboard); move relocates and clears it. |
| `/`                | **Filter** — type to narrow to the path-to-match tree (every node whose title or slug matches, plus its ancestors; case-insensitive, Unicode-aware). `Enter` applies. |
| `Esc`              | Staged: exit filter editing → clear an active filter → save the view state and close. |

CLI parity: `inkhaven outline [--filter <s>]` prints the same tree as text;
`inkhaven paragraph copy|move <src> <dest>` does the cross-parent relocation
by slug path. Bund: `ink.outline.print`, `ink.outline.paragraph_copy`,
`ink.outline.paragraph_move` (the mutators need the `store_write` category).

Reorder, promote/demote, and copy/move all reuse the same filesystem-aware
store primitives as the Tree pane, so the two panes stay in lock-step.

---

## 10. Delete confirm modal

Triggered by `Ctrl+B` then `D` (or the Tree pane's `D`/`-` shortcuts). Red-bordered floating box. Shows the kind,
title, and descendant count.

```
┌── Confirm delete ───────────────────────────────┐
│  Delete chapter `Storm` and 4 descendants?      │
│                                                 │
│  Removes files from disk AND records from bdslib│
│  y / Enter to confirm · n / Esc to cancel       │
└─────────────────────────────────────────────────┘
```

| Key                  | Action                                                      |
| -------------------- | ----------------------------------------------------------- |
| `y` / `Y` / `Enter`  | Confirm. fs subtree removed, bdslib records deleted, tree reloads, cursor lands on the deleted node's parent (or stays put if parent vanished too — i.e. you deleted a book). |
| `n` / `N` / `Esc`    | Cancel.                                                     |
| `Ctrl+Q`             | Hard quit.                                                  |

If the open paragraph is inside the deleted subtree, the editor closes too.

---

## 11. Configurable bindings (HJSON)

The `keys` block in `inkhaven.hjson` accepts the chord strings below. Parser
recognizes:

- **modifiers**: `Ctrl` (or `Control`), `Shift`, `Alt` (or `Meta` / `Option`), `Super` (or `Cmd` / `Command`)
- **named keys**: `Tab`, `Enter` / `Return`, `Esc` / `Escape`, `Space`, `Backspace`, `Delete` (or `Del`), `Insert` (or `Ins`), `Home`, `End`, `PageUp` (or `PgUp`), `PageDown` (or `PgDown` / `PgDn`), `Up`, `Down`, `Left`, `Right`, `F1` through `F24`
- **single characters**: any printable ASCII character

Modifiers are case-insensitive; named keys are case-insensitive; single-letter
chars are normalized (Ctrl+s, Ctrl+S, and Ctrl+Shift+S all parse and match
the same way — useful because terminals vary in how they report case with
modifiers).

Defaults shipped in `assets/default_project.hjson`:

```hjson
keys: {
  save:             Ctrl+s
  search:           Ctrl+/
  ai_prompt:        Ctrl+i
  next_pane:        Tab
  prev_pane:        Shift+Tab
  page_up:          PageUp
  page_down:        PageDown
  meta_prefix:      Ctrl+b           // chord prefix for tree / editor / AI actions
  bund_prefix:      Ctrl+z           // chord prefix for Bund scripting (1.2+)
  view_prefix:      Ctrl+v           // chord prefix for view sub-chords (1.2.4+)
  bindings:         []               // user overlay; see KEYS_REASSIGNMENT.md
}

editor: {
  // ...
  autosave_seconds: 5      // idle-trigger save in editor; 0 disables
}

// Background flush interval. 0 disables.
sync_interval_seconds: 600
```

**Rebinding sub-chords** (the letters under `Ctrl+B …`, `Ctrl+Z …`,
and `Ctrl+V …`) went data-driven in 1.2 — list overrides in
`keys.bindings` or, at runtime, via the `ink.key.*` Bund stdlib.
The full action table and both rebinding channels are documented
in [`KEYS_REASSIGNMENT.md`](KEYS_REASSIGNMENT.md).

**F-keys in the binding table (1.2.4)** — F1 through F10 and the
Shift-F variants migrated from hardcoded matches into
`Layer::TopLevel`. HJSON overlays accept single-token chords:

```hjson
keys: {
  bindings: [
    { layer: "view_sub",  key: "P",  action: "view.fuzzy_paragraph_picker" }
    { layer: "top_level", key: "F7", action: "view.add_link" }  // rebind F7
  ]
}
```

Non-configurable bindings (the editor's modern shortcut overrides, the
AI-action `r/i/t/b/c` keys, the modal `y/n` confirmations, and
`Ctrl+Q` hard-quit) remain hard-coded.

---

## 12. File picker dialog (F3)

Tree-style filesystem browser overlay, rooted at the shell's current working
directory. Same navigation in both contexts (Editor F3 and Tree F3); only
the Enter action differs.

```
┌── Pick file — /Users/you/some/dir ────────────────────────────────────────┐
│  ▸ 📁 books                                                               │
│  ▾ 📁 imports                                                             │
│      ▸ 📁 chapter-one                                                     │
│      ▸ 📁 chapter-two                                                     │
│        📄 preface.md                                                      │
│    📄 README.md                                                           │
│    📄 todo.txt                                                            │
│                                                                           │
│ ↑↓ navigate · → expand · ← collapse/parent · Enter pick · Esc cancel      │
└───────────────────────────────────────────────────────────────────────────┘
```

| Key                  | Action                                                      |
| -------------------- | ----------------------------------------------------------- |
| `↑` / `↓`            | Move cursor one entry up / down.                            |
| `PageUp` / `PageDown`| Jump by 10.                                                 |
| `Home` / `End`       | First / last entry.                                         |
| `→`                  | If cursor is on a directory: expand it (children inline immediately below). No-op for files or already-expanded directories. |
| `←`                  | If cursor is on an *expanded* directory: collapse it. Otherwise: move cursor to the parent entry. |
| `Enter`              | Commit (see action table below).                            |
| `Esc`                | Cancel; modal closes, nothing happens.                      |

**Sort order within each level**: directories first, then files, each
alphabetical. Hidden entries (names starting with `.`) are skipped.

**Action on Enter:**

| Context (F3 fired in) | Picked entry | What happens |
| --------------------- | ------------ | ------------ |
| Editor pane           | file         | Replaces the open paragraph's buffer with the file content. Marks the document dirty so the next save commits the change (a save will also re-create the snapshot baseline). |
| Editor pane           | directory    | Rejected — status hint says to pick a file. |
| Tree pane             | file         | Creates a new paragraph inserted **after** the cursor's same-kind ancestor (same as the `P` shortcut), titled from the filename, body = the file's bytes. |
| Tree pane             | directory    | **Recursive import**: the directory itself becomes a subchapter under the cursor's nearest valid host, every subdirectory becomes a nested subchapter, every file becomes a paragraph inside its containing subchapter. Sorted alphabetically with dirs-first. Requires `hierarchy.unbounded_subchapters: true` if the dir tree is deeper than two levels under a chapter. |

## 13. When chords don't reach Inkhaven

Some of the configured chords — especially `Ctrl+S`, `Ctrl+Q`, and the
`Ctrl+B` meta prefix — can be eaten by your terminal emulator, your shell,
or a terminal multiplexer (tmux / screen) before they reach Inkhaven. This
is not a bug in Inkhaven; it's a layer above us deciding the chord means
something else.

Common interceptors:

| Chord                  | Often intercepted by                                                |
| ---------------------- | ------------------------------------------------------------------- |
| `Ctrl+S`               | Terminal flow control (XOFF / freeze output). Run `stty -ixon` in your shell to disable. |
| `Ctrl+Q`               | Terminal flow control (XON). Same `stty -ixon` fix.                |
| `Ctrl+B`               | **tmux default prefix.** If you run inkhaven inside tmux, either rebind tmux's prefix (`set -g prefix C-a`) or remap inkhaven's `meta_prefix` in `inkhaven.hjson` to something tmux doesn't eat (e.g. `Ctrl+g`). |
| `Ctrl+Shift+Up/Down`   | Some terminals don't transmit the Ctrl modifier with arrow keys. Use the plain-letter shortcuts (`B`, `C`, `A`, `+`, `D`, `-`) in the Tree pane instead. |

**Workarounds Inkhaven provides:**

- The Tree pane has modifier-free `A` / `+` / `D` / `-` shortcuts (§2.2) for
  the most common add/delete operations.
- For reorder, both `Ctrl+B ↑/↓` (TUI) and `inkhaven mv ... up`
  /`down` (CLI) exist; use the CLI in a second pane if the TUI chord is
  blocked.
- Save is also reachable via the CLI: open the `.typ` in an external
  editor, save there, then `inkhaven reindex` from a shell.

**If your terminal swallows Ctrl+S**, the simplest fix is to add this to
your shell rc:

```bash
stty -ixon
```

Then `Ctrl+S` reaches applications normally.

## 14. Quick cheat sheet

For when you just want the high-level map:

```
GLOBAL          Ctrl+Q       quit (autosaves if dirty)
                Ctrl+1..5    focus Editor / Tree / AI / Search / AI prompt
                Tab/S-Tab    cycle Tree / Editor / AI panes
                Ctrl+/       focus search
                Ctrl+I       focus AI prompt
                Ctrl+S       save current paragraph
                Ctrl+B       meta prefix (table depends on focused pane):
                  Tree:       B/C/S/P add · D delete · ↑/↓ reorder
                  Editor:     S save · N snapshot · H history · L load · F split
                  AI:         C clear inference
                  Esc         cancel meta

TREE            ↑↓ Home End  navigate
                ←/→          collapse/expand branch (← steps to parent if not expanded)
                PgUp PgDn    by 10
                Enter        open paragraph (autosaves the previous one)
                F2           rename current node
                F3           file picker → insert file or import dir
                B            add book at root
                C            append chapter         (V = insert after current)
                A            append subchapter      (S = insert after current)
                +            append paragraph       (P = insert after current)
                D            delete branch          (or Ctrl+B then D)
                -            delete paragraph       (or Ctrl+B then D)
                Ctrl+B ↑/↓   reorder within siblings
                q            quit (autosaves if dirty)

EDITOR          arrows       move cursor
                Ctrl+arrows  word / top / bottom
                Shift+arrows extend linear selection
                Ctrl+U/Y     undo / redo
                Ctrl+K/C/P   cut / copy / paste (system clipboard)
                Ctrl+A       select all
                Ctrl+D       delete current line
                Ctrl+E       delete cursor → end of line
                Ctrl+W       delete cursor → start of line
                Alt+arrows   extend rectangular block selection
                Alt+C        copy rectangular block
                Ctrl+S       save + re-embed
                Ctrl+F       open find (regex)
                Ctrl+X       "repeat" (next match / replace+next, search active only)
                Ctrl+R       open find&replace · replace all (in replace mode)
                F3           load file → replaces buffer
                F4 / Ctrl+F4 toggle split / accept snapshot
                Ctrl+H/J     (split only) scroll lower pane up/down
                Ctrl+B H     open Quick reference overlay (global)
                F5           create snapshot
                F6           open snapshot picker
                Esc          clear search (if active) · else cycle to Tree
                (idle autosave fires after editor.autosave_seconds)
                (new text since last save is rendered bold)

AI              r            replace selection / doc
                i            insert at cursor
                t            prepend to top
                b            append to bottom
                c            copy to clipboard only
                g            grammar-apply (lift only the corrected text)
                L            file into system book (only when the response
                             came from a generator: submission draft →
                             Submissions, structure analysis → Planning)

SEARCH BAR      Enter        run search (or open highlighted result)
                ↑↓           navigate results overlay
                Esc          close overlay → defocus

AI PROMPT       /            open prompt picker
                ↑↓           navigate picker
                Tab/Enter    expand template (in picker)
                Enter        send to LLM (outside picker)
                Esc          close picker → defocus

MODALS          Enter        confirm
                Esc          cancel
                y/n          (delete only)
```


---

## 1.2.5 + 1.2.6 — chord additions

Every chord introduced between the original document and the
1.2.6 release, organised by feature. This section is
maintained in delta-style so the canonical chord-by-pane
tables above stay readable; the entries below are the
new ground.

### Tag workflows (1.2.5+)

```
Ctrl+B ]   open the tag picker on the open paragraph
Ctrl+B }   open the project-wide tag-search picker
g (tree)   open the tag picker for the tree-cursor's paragraph
           (or every marked paragraph at once)
```

Inside the tag picker (`Ctrl+B ]` / `Ctrl+B }` / `g`):

```
Space      toggle the cursor tag
A          add a new tag (one-line prompt)
R          rename project-wide (1.2.6+; merges if name exists)
D          delete project-wide (confirm)
T          commit marked tags onto the target
Enter      Search mode: open the per-tag paragraph list
↑↓ Home/End navigate
```

### Story view (1.2.5–1.2.6)

```
Ctrl+V Shift+W   book story view (1.2.5+)
Ctrl+V w         paragraph mini story view (1.2.6+)
```

Inside either view:

```
S       save the rendered PNG to cwd
Esc     close
```

### Diagnostics (1.2.5–1.2.6)

```
F8                 (1.2.6+) typst diagnostics list modal
Ctrl+V N           next diagnostic in the open buffer
Ctrl+V Shift+N     previous
Ctrl+F12           (1.2.6+) AI explain the diagnostic at cursor
                   (was F11 pre-1.2.6 — macOS grabs F11)
```

Inside the F8 modal:

```
↑↓ Home/End    navigate
Enter          jump editor cursor to the diagnostic, close modal
Esc            close
```

### AI critique + diff modal (1.2.6+)

```
F12       AI critique (mode-aware: critique-edit / critique-changes)
```

Inside the AI diff-review modal (`r` / `g` in the AI pane
when `ai.diff_review_on_apply: true`):

```
a / A / Enter   accept — apply and refocus editor
r / R           reject — buffer unchanged
e / E           alias for `a`
↑ ↓ PgUp PgDn   scroll the diff
Home / End      jump top / bottom
Esc             same as reject
```

### Snapshot annotation prompt (1.2.6+)

```
F5    open the annotation prompt over the editor
```

Inside the prompt:

```
Type a line   build up the annotation
Enter         commit (empty = un-annotated)
Esc           cancel — no snapshot
```

### Render-preview zoom (1.2.6+)

Inside `Ctrl+V R`:

```
+ / =     zoom in  (multiply ticks/cell by 0.66)
- / _     zoom out (multiply by 1.5)
0         reset to 1.00×
```

### Story timeline (1.2.6 — opt-in)

```
Ctrl+V e         chronological event picker
Ctrl+V Shift+T   swim-lane timeline view
                 (lowercase Ctrl+V t stays bound to the
                  per-paragraph word-count target modal)
Ctrl+V Shift+E   add a timeline event (opens the swim-lane
                 view + the new-event prompt)
Ctrl+V Shift+I   edit the open event ¶'s start / end / track
```

Inside Ctrl+V Shift+T:

```
← / →             scroll by ~10 cells
PgUp / PgDn       page by ~60 cells
+ / =             zoom in   (0.66× ticks/cell)
- / _             zoom out  (1.5×)
0                 reset zoom to 1.00×
Home / End        jump to first / last event in the visible set

u / U             up-scope    (subchapter → chapter → book)
d / D             open the inline descent picker
b / B             jump to book scope
p / P             toggle project overlay

Tab               cycle highlighted track
Enter             open the event closest to cursor
n / N             new event at cursor tick (annotation prompt)

y                 AI critique — current scope + current track
Y                 AI critique — current scope + all tracks
Ctrl+Y            AI critique — book scope (widens regardless)
Esc               close
```

Inside the descent picker (`d` from the swim-lane view):

```
↑ ↓ Home/End   navigate
Enter          descend into the selected scope
Esc            return to the same scope
```

Inside Ctrl+V e:

```
↑ ↓ Home/End   navigate
t / T          cycle the track filter (None → t0 → … → None)
Enter          open the event paragraph
Esc            close
```

## 1.2.7 — chord additions

### Paragraph undelete (1.2.7+)

```
Ctrl+B U       restore the most recently deleted paragraph
               (single-slot kill-ring; new uuid; paragraph links to
                old id stay broken).  Cleared by any branch
                delete or another single-¶ delete (the new one
                takes the slot).
```

See [`Tutorials/32-paragraph-undelete.md`](Tutorials/32-paragraph-undelete.md).

### Navigation history (1.2.7+)

```
Alt+←          step backward through visited paragraphs
Alt+→          step forward (after stepping back)
Ctrl+V Shift+P recent-paragraph picker (most-recent-first list,
               up to 32 entries, deduped against the previous)
```

The ring is in-memory only — restart clears it.  Opening a
new paragraph (via Enter / picker / paragraph link / undelete /
similar / timeline-Enter) clears the forward stack.

See [`Tutorials/33-navigation-history.md`](Tutorials/33-navigation-history.md).

### Mouse + external-change behaviour (1.2.7+)

```
Ctrl+Shift+M   toggle mouse capture on / off
               OFF lets the terminal handle drag-select +
                   native clipboard (Cmd/Ctrl+Shift+C).
               ON  restores click-to-focus + wheel-scroll
                   for the active pane.  Session-only;
                   defaults to ON.
```

External-change auto-reload has no chord — it runs passively
on every autosave tick.  Status bar reads:

```
↻ reloaded `<title>` — file changed on disk   (clean buffer)
⚠ `<title>` changed on disk while you have unsaved edits —
  Ctrl+S to overwrite the external change      (dirty buffer)
```

See [`Tutorials/34-mouse-and-external-changes.md`](Tutorials/34-mouse-and-external-changes.md).

### Timeline polish (1.2.7+)

Inside `Ctrl+V Shift+T` (swim-lane view) the navigation model
gained a second focus level mirroring the tree pane:

```
Focus = Track                         (default on open)
  Tab / Shift+Tab    cycle highlighted track
  Space              collapse / expand the focused track
  Enter              expand + drop focus into Event mode

Focus = Event
  Tab / Shift+Tab    cycle events of the expanded track in time
  Enter              open the linked-paragraphs picker for the
                     focused event
  Esc / Backspace    pop back to Track focus

Anywhere in swim lanes:
  ↑ / ↓              select previous / next event by start tick;
                     viewport auto-pans to show whole span
  F12                full-book AI health critique (same payload
                     as Ctrl+Y; alternative chord)
```

Session-restored state (per book, in `.session.json`):
collapsed tracks, expanded track, track highlight, zoom
(`ticks_per_cell`), scroll tick, cursor tick.

See [`Tutorials/31-story-timeline.md`](Tutorials/31-story-timeline.md) "1.2.7 polish".

### F8 from any pane (1.2.7+)

`F8` (typst-diagnostics list modal) now works from any pane,
not just the editor.  Opens against the most-recently-active
paragraph's cached diagnostics.

## 1.2.8 — chord additions

### Kill-ring picker (1.2.8+)

```
Ctrl+V Shift+U   open the kill-ring picker — list of the most
                 recent (up to 10) deleted paragraphs.  Enter
                 restores the cursor selection at its original
                 position.  Esc cancels.

Ctrl+B U         (existing) restores the front of the ring
                 without opening the picker.  Branch deletes
                 (chapter/book) no longer clear the ring —
                 older single-¶ entries remain valid recoveries.
```

### Hidden-character report (1.2.8+)

```
Ctrl+V h         one-shot scan of the open paragraph; status
                 bar reads e.g. "hidden chars: 3 tab(s), 5
                 line(s) with trailing whitespace, 0 CR(s)".
                 Clean buffers report "no tabs, trailing
                 whitespace, or CRs".  No buffer rewrite —
                 visual editor overlay is 1.2.9 work.
```

### Breadcrumb status-line chord (1.2.8+)

```
Ctrl+V Shift+S   print the cursor's hierarchy path on the
                 status bar (`Book ▸ Chapter ▸ Subchapter
                 ▸ Paragraph`).  Pane-aware: tree pane walks
                 from the tree cursor, editor pane walks from
                 the open paragraph.
```

### F1 query history (1.2.8+)

```
Inside the F1 Help-query input:
  Up             previous query (newest first); shell-style.
  Down           next; past the newest entry clears the input.
  Enter          submit; pushes the query onto the ring
                 (dedup against the immediate predecessor).
```

Session-only; F1 history is intentionally not persisted.

### Tag autocomplete (1.2.8+)

Inside the `A` (add-new-tag) prompt opened from `Ctrl+B ]`:

```
Tab              completes to the first existing project
                 tag whose name starts with the typed prefix
                 (case-insensitive).  No-op when no match.
```

### F6 annotation filter (1.2.8+)

Inside the F6 snapshot picker:

```
/                enter filter-focus mode — typed characters
                 narrow the visible list to snapshots whose
                 annotation contains the substring (case-
                 insensitive).
Esc (in filter)  exit filter focus (keeps the query).  Picker
                 returns to chord mode — Up/Down/Enter/D/V
                 again.
Backspace        edit the filter (in focus mode only).
Enter (in filter) commit filter (exits focus) — second Enter
                 loads the snapshot.
```

Filter resets each `F6` open — previous session's filter
doesn't haunt the next picker.

### Active-LLM chip in AI pane (1.2.8+)

The AI pane title always shows `· llm=<provider>` (the bound
`llm.default` from HJSON) so `Ctrl+B L` swap effect is visible
without opening `Ctrl+B I`.  In-flight provider fragment is
suppressed when it matches the bound default; surfaces only
when they diverge (user swapped default mid-stream).

### Shift+letter chord fix (1.2.8+)

Pre-existing bug — `Ctrl+V Shift+P` (recent-¶ picker) collapsed
onto `Ctrl+V p` (fuzzy picker) on terminals without the kitty
disambiguation protocol because the chord matcher required the
SHIFT modifier flag.  Now uppercase letters arriving without
SHIFT are treated as implicit-Shift — `Ctrl+V Shift+P`,
`Ctrl+V Shift+U`, `Ctrl+V Shift+S` all route to their distinct
actions.

### Mouse-capture default knob (1.2.8+)

```hjson
editor: {
  mouse_captured: true    // 1.2.8+ default
}
```

Setting `false` releases mouse capture at startup so the
terminal's native drag-select + system-clipboard copy work
without pressing `Ctrl+Shift+M` first.  The runtime
`Ctrl+Shift+M` toggle still flips state regardless.

### Embedded nushell pane (1.2.8+)

```
Ctrl+Z o         open / close the floating shell pane.
                 Engine state (env vars, defs) + turn
                 buffer + on-disk history all preserved
                 across close+reopen.
Ctrl+Z O         (Shift) drop the cached engine + in-
                 memory turn buffer and open fresh.  Does
                 NOT wipe `.inkhaven/shell_history.db`.
Ctrl+Z h         (inside the pane) toggle history-
                 selection mode.

Inside the pane (normal mode):
  Enter          run the line through the embedded
                 nu_engine; output + stderr land as a new
                 turn in the buffer.  Scroll is reset so
                 the new output is auto-visible.  Typing
                 `exit` (or `quit`) closes the pane
                 instead of forwarding to nu (whose
                 built-in `exit` would kill inkhaven
                 itself).
  Tab            autocomplete the token under the cursor.
                 In command position (start of line or
                 after `|` / `;`) matches against nu's
                 declared command set + executables on
                 $PATH; otherwise filesystem entries
                 under `$env.PWD`.  Single match →
                 splice + trailing space; multiple →
                 splice the longest common prefix and
                 surface the candidates on the status
                 line.

Line editing (readline-style):
  Ctrl+A / Ctrl+E    move cursor to start / end of line
  Ctrl+U             kill from cursor to start
  Ctrl+K             kill from cursor to end
  Ctrl+W             kill the word before the cursor
  Ctrl+Left/Right    move cursor by word
  Alt+B / Alt+F      move cursor by word (readline alias)
  Alt+Backspace      kill word backward
  Ctrl+L             clear scrollback (engine + history kept)
  Ctrl+D             clear input; if input is empty, close pane

Pane help:
  Ctrl+B H           open the OS Shell help overlay.
                     Any key dismisses it; pane state
                     (input, scroll, history) is preserved
                     unchanged underneath.
  ↑ / ↓          walk the per-project command history
                 ring (shell-style; Down past newest
                 clears the input).
  PgUp / PgDn    scroll the turn buffer up / down by 10
                 logical lines.  Title bar shows
                 `↑ scrolled` while above the newest turn.
  Shift+Home     jump to the top of the buffer.
  Shift+End      jump back to the newest output.
  Esc            close the pane (state preserved).

Inside selection mode:
  ↑ / ↓               walk the turn cursor.
  Home / End          jump to first / last turn AND scroll
                      the buffer to match.
  PgUp / PgDn         scroll independently of the cursor.
  c                   copy the highlighted turn's output
                      (stderr appended when failed).
  i                   insert the output into the editor
                      at cursor, wrapped in
                      `shell.insert_template`.  Pane
                      closes + editor refocuses.
  Esc                 exit selection (keep pane open).
  Ctrl+Z h            same — toggle back.
```

Pane gated on `shell.enabled = true` in HJSON (default
true).  See [`Tutorials/35-embedded-shell.md`](Tutorials/35-embedded-shell.md).

## `inkhaven research` (1.5.0+, RESRCH-1)

Standalone TUI for AI-assisted research that transfers verified findings into the
**Facts** / **Notes** corpus. Launched outside the main editor:
`inkhaven research [--thread <name>]` (also `--list-threads`, `--export-thread`).
Three panes — Facts tree (left 40%), streaming RAG chat (right 60%), a two-line
query prompt — needing ≥ 80 columns.

| Key | Action |
|:---|:---|
| `Tab` / `Shift+Tab` | Cycle focus: Facts tree → query prompt → chat |
| `F10` | Cycle RAG mode (Facts+Full → Facts only → Full only) |
| `Ctrl+B h` | Full quick reference of every chord + `/command` (any key closes) |
| `?` | Toggle the keybind hints bar |
| `q` / `Ctrl+C` / `Ctrl+Q` | Quit (restores the terminal; Ctrl+Q works in text fields too) |
| **Facts tree** | `j`/`k` nav, `Enter`/`l`/`h` expand/collapse, `g`/`G` top/bottom |
| `Ctrl+P` (tree) | Pin / unpin the cursor node for RAG context (max 3, `⬡`) |
| `n` (tree) | Manually add a fact (title → `Ctrl+S` body) |
| **Query prompt** | `Enter` send, `↑`/`↓` recall history, `Esc` clear-then-defocus |
| **Chat** | `j`/`k`/`g`/`G` scroll, `Ctrl+F` search (`n`/`N` between matches) |
| **Confirm overlay** | `Tab` switch title/body, `Ctrl+S` / `Ctrl+Enter` insert, `Esc` discard |

Commands (type in the prompt): `/fact "..." [→ path]`, `/note "..." [→ path]`,
`/promote [notes/path] [→ facts/path]`, `/sources`, `/import [path]`,
`/forget <name>`, `/web [--ingest|--chat] <query>`, `/goto <slug-path>`,
`/diff`, `/verify`, `/factcheck`, `/chain a → b → c`, `/rag [mode]`, `/clear`,
`/save [name]`. `inkhaven research --import <path>` imports non-interactively. See
[Tutorial 103](Tutorials/103-research-assistant.md).

## `inkhaven worldbuilder` (1.9.0+, WBLD-1)

Standalone full-screen TUI companion to the `realworld` pipeline: a **Facts** tree
over a **World** tree (left), a cycling right pane (`Chat | Research | Map | Ledger`),
a full-width query prompt, and a status bar with the live plausibility score. Every
change lands in `world.hjson`; it never generates prose. Launched outside the editor:
`inkhaven worldbuilder [--session <name>] [--interview] [--from-map]`. Needs ≥ 40×10.

| Key | Action |
|:---|:---|
| `Tab` / `Shift+Tab` | Cycle panes: Facts → World → Query → Right |
| `Ctrl+R` | Cycle the right pane (Chat / Research / Map / Ledger) |
| `?` | Toggle the keybind hints bar |
| `Ctrl+Q` / `Ctrl+C` | Quit (restores the terminal) |
| **Left trees** | `j`/`k` nav, `Enter`/`l`/`h` expand/collapse, `g`/`G` top/bottom |
| `Ctrl+P` (tree) | Pin / unpin the cursor node for AI context (`⬡`) |
| `Ctrl+T` (Facts) | Toggle the `fact:world` tag on the selected paragraph (`◎`) |
| `Shift+F` (Facts) | Filter the Facts tree to `fact:world` only |
| `z` (tree) | Zoom the focused left tree to fill the column |
| `{` / `}` · `[` / `]` | Resize the left split · the left/right column ratio |
| **Query prompt** | `Enter` send/answer, `Esc` clear-then-defocus (or leave interview) |
| **Chat** | `j`/`k`/`g`/`G` scroll |
| **Confirm overlay** | `y` accept the delta into pending, `n`/`Esc` discard |
| **Map editor** (1.10.0+) | `e` edit; `hjkl`/arrows move (`Shift` fine); left-click positions the cursor |
| `t` · `n` · `g` (Map edit) | place a town · landmark · region at the cursor |
| `r` · `o` (Map edit) | draw a river (source→mouth) · a road (landmark→landmark) |
| `+` `-` · `,` `.` (Map edit) | raise / lower terrain · brush size (`/terrain` writes the DEM) |
| `d` · `f` (Map edit) | delete the feature under the cursor · jump to the next `/mapcheck` issue |

Commands (type in the prompt): `/interview`, `/set <path> <value>`, `/star`, `/tilt`,
`/moon`, `/nation`, `/magic on|off`, `/rule <kind> <cat,cat> [desc]`, `/wfact <text>`,
`/research <query>`, `/compile`, `/validate`, `/roll [n]`, `/adopt <seed>`, `/map`,
`/mapcheck`, `/terrain`, `/diff`, `/write`, `/undo`, `/reset`, `/journey`, `/sessions`,
`/switch <name>`, `/export [--pdf]`. See
[Worldbuilding reference](WORLDBUILDING.md#the-interactive-worldbuilder-inkhaven-worldbuilder)
and *Building the World with Inkhaven*, chs. 20–21.

## `inkhaven prompts-editor` (1.2.10+)

Standalone four-pane TUI for editing
`<project>/prompts.hjson` — the prompt library
the main TUI's F7 / F12 / `Ctrl+B C / P / Y / G`
flows read from.  Launched outside the main TUI:

```
inkhaven prompts-editor -p <project-dir>
```

Layout: prompts list (left) · prompt editor
(centre, full tui-textarea chord set) · AI
response (right, display-only) · AI prompt input
(3-row bottom strip).

### Global chords

```
Ctrl+S              save library (atomic + .prompts-backups/ snapshot)
Ctrl+R              rollback picker (list, preview, restore, delete)
Ctrl+H / ?          focus-aware help pane
Tab / Shift+Tab     cycle pane focus (3 stops: list → editor → ai prompt → list)
Esc / Ctrl+Q        quit (confirm if unsaved)
```

### Prompts list pane

```
↑↓ / PgUp / PgDn / Home / End   navigate (cursor auto-loads into editor)
Enter                            load focused prompt + jump focus to editor
a                                add new prompt (name prompt → empty body)
d                                delete focused prompt (confirm modal)
                                   second `d` on a staged-deleted entry revokes
```

### Editor pane

Full tui-textarea defaults: arrows, Home/End,
PgUp/PgDn, Shift+arrows selection, Ctrl+A/E
start/end-of-line, Ctrl+B/F cursor left/right,
Ctrl+N/P up/down, Ctrl+K kill-to-end, Ctrl+W
delete-previous-word, Ctrl+U/Y undo/redo.

Plus one prompts-editor-only chord (meta-prefix
because terminals eat plain Ctrl+G as ASCII BEL):

```
Ctrl+B G            "Get" — insert latest AI pane response at the editor
                    cursor and jump focus to the editor.  Works from any
                    pane.  No-op (with status) when the response is
                    missing or still streaming.
```

### AI prompt input pane

```
type / Backspace / Delete         buffer edit
Left / Right / Home / End         cursor movement
Ctrl+A / Ctrl+E                   start / end of line (readline-style)
Up / Down                         in-session history walk (deduped)
Enter                             send for analysis
Ctrl+L                            clear input
Ctrl+K                            clear input + clear history
```

### Send semantics

The LLM acts as a prompt-engineering **reviewer**
— it does NOT execute the template.  Placeholders
like `{{selection}}` are NOT substituted; the
reviewer sees them as literal text and comments
on their use.  Pressing Enter sends:

  * `system` — fixed framing that explains the
    reviewer role + the placeholder conventions.
  * `user` — fenced template body verbatim +
    "Analysis request:" + your typed instruction
    (or an embedded default critique if empty).

Single-shot per send; multi-turn isn't planned
for this surface.

### Save chips

Top bar shows a red `N unsaved` chip when any
prompt is staged for change.  List rows carry
per-prompt markers:

  * `✱` unsaved edit (red bold)
  * `✚` newly-added (green bold)
  * `✗` staged for deletion (red strike-through)

### Rollback

`Ctrl+R` lists every
`.prompts-backups/prompts_YYYYMMDD_HHMMSS.hjson`
newest-first.  Inside the picker:

```
↑↓ / PgUp / PgDn / Home / End   navigate
Enter                            stage the backup as the working library
                                   (Ctrl+S commits)
v                                preview the file's contents
d                                delete with confirm
Esc                              back to the main view
```

The first Ctrl+S after a rollback writes a fresh
backup of the pre-rollback state, so the safety
chain stays intact.

See [`Tutorials/44-prompts-editor.md`](Tutorials/44-prompts-editor.md)
for the full workflow walkthrough.


