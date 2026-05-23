# Screenshot catalog

Each entry here is one `#figure_slot(id: "...")` placeholder
in the typst book + its `![figure: id](...)` mirror in the
markdown book. Capture each as a PNG, save to
`Book/images/<id>.png`, and the next `typst compile` swaps
the placeholder for the real image.

## Conventions

- **Filename**: `<id>.png` (kebab-case, exactly as the id
  string in the `figure_slot`).
- **Resolution**: aim for terminal at 120-140 columns × 35-45
  rows. The book renders the images at 60-70mm height; an
  exact 16:9 isn't required but keep the aspect close.
- **Theme**: Catppuccin Mocha (the inkhaven default) unless
  otherwise noted.
- **Font**: a monospace font that renders box-drawing
  characters cleanly. JetBrains Mono / Fira Code / Iosevka
  all work.
- **Cropping**: leave one row of empty space around the
  relevant area. Don't crop tight; the book's grey
  placeholder background bleeds.
- **Naming when in doubt**: ASCII only, single hyphen
  between words, no underscores.

## Catalog

Each table column:

- **id** — the filename + the figure placeholder id.
- **chapter** — which chapter wants this screenshot.
- **state** — what the TUI / CLI should be showing when you
  capture.
- **suggested caption** — already in the book; included
  here for confirmation.

### Chapter 1 — What Inkhaven is

| id | state | caption |
|----|-------|---------|
| `tui-overview` | Default layout with a small book loaded, a paragraph open in the editor, AI pane showing one chat turn. The full TUI must be visible: search input at the top, three centre panes (Tree · Editor · AI), AI prompt input at the bottom, status line beneath. | The layout: search input along the top; three panes in the middle — Tree (left), Editor (centre), AI (right); AI prompt input along the bottom; status line beneath. Modal pickers temporarily replace one or more of the centre panes. |

### Chapter 2 — Installation and your first book

| id | state | caption |
|----|-------|---------|
| `init-output` | Terminal output of `inkhaven init /tmp/MyBook --force`. | Running `inkhaven init` shows the layout it just created — config path, books directory, metadata DB, vector store. |
| `first-book-tree` | TUI just after three `inkhaven add` commands: book + chapter + paragraph. Tree pane has focus. | Tree pane after the three add commands: book → chapter → paragraph. |
| `ctrl-b-b-splash` | Building modal mid-compile. Splash + progress spinner visible. | Building the book — splash + spinner while typst compiles. Cancellable with Esc. |

### Chapter 3 — The project tree

| id | state | caption |
|----|-------|---------|
| `tree-paragraph-row` | A close-up of a tree row that has: status letter, paragraph glyph, title, progress pip, two tag pips, +N indicator. | A paragraph row carries five things at a glance: indent (depth), kind glyph (¶), status letter (N/F/R/…), title (truncated), and tag pips (#draft, #weather). |

### Chapter 7 — Editor workflow

| id | state | caption |
|----|-------|---------|
| `split-edit` | F4 split-edit mode active. Left half shows a snapshot; right half shows a longer modified version with visible additions. | Split-edit (F4) — left half is the last snapshot, right half is the live buffer. Ctrl+H/J scroll the snapshot. |

### Chapter 8 — Saving and snapshots

| id | state | caption |
|----|-------|---------|
| `f5-annotation-prompt` | F5 annotation prompt over the editor; user typing "before the lighthouse rewrite". | F5 — annotation prompt. Type a note describing this version, Enter commits, Esc cancels. |
| `f6-picker` | F6 snapshot list with at least two annotated rows + two un-annotated rows. | F6 picker. Annotated snapshots show their note on a second line; un-annotated ones are single-line. |
| `snapshot-diff` | Side-by-side diff view (`V` inside F6) with visible +/- markers. | Snapshot diff (`V` in F6). Left = snapshot; right = current buffer. Coloured markers show insertions / deletions per line. |

### Chapter 9 — Status and writing goals

| id | state | caption |
|----|-------|---------|
| `status-filter` | Ctrl+B 4 modal listing every Status:Second paragraph in a project with ~3-5 chapters. | Ctrl+B 4 — every paragraph at Status:Second across the project. Enter opens; n/N walk in tree order. |
| `ctrl-v-g-progress` | Ctrl+V G progress modal. Today's words, streak, per-book deadline burn-down visible. | Ctrl+V G — progress modal. Today's words, current streak (with grace), per-book burn-down to deadline. |

### Chapter 10 — Search and discovery

| id | state | caption |
|----|-------|---------|
| `search-results` | Ctrl+/ results overlay with a 4-6 result list ranked by semantic + exact. | Search results overlay — semantic similarity scores on the left, exact-text match icon on the right. Arrows + Enter open. |

### Chapter 11 — Backups and recovery

| id | state | caption |
|----|-------|---------|
| `exit-backup-splash` | Auto-backup splash on Ctrl+Q with a progress bar mid-zip. | Ctrl+Q with stale backup — splash + progress bar while the zip is written. Esc cancels. |
| `doctor-output` | `inkhaven doctor` CLI output with green checks + at least one yellow warning. | `inkhaven doctor` — health report. Green check + actionable warnings (yellow) + errors (red). |

### Chapter 12 — Exporting your book

(No new figures; rely on Chapter 2 + 25 captures.)

### Chapter 13 — Places and characters

| id | state | caption |
|----|-------|---------|
| `lexicon-highlight` | Editor pane showing a paragraph with cyan + yellow + mauve highlights for Character / Place / Artefact matches. | Lexicon overlay — character names in cyan, place names in yellow, artefacts in mauve. Subtle but always-visible. |

### Chapter 14 — Tags

| id | state | caption |
|----|-------|---------|
| `tag-picker` | Ctrl+B ] picker over a paragraph with 3-5 tags + the ✓ marker. | Tag picker — checkmarked tags are on the open paragraph. Space toggles; T commits; A adds new; R renames; D deletes. |
| `tree-tag-pips` | A tree pane showing several paragraph rows with #tag pips and at least one row with `+N`. | Tree paragraph rows with tag pips. `+N` shows when more than two tags are present. |

### Chapter 15 — Wiki-links and backlinks

| id | state | caption |
|----|-------|---------|
| `link-pick-tree` | Tree pane in link-pick mode (after Ctrl+V A). Title bar shows the special "select paragraph to link" text. | Tree in link-pick mode (Ctrl+V A). Title bar shows the purpose; Enter confirms; Esc cancels. |
| `link-picker` | Ctrl+V L outgoing-links modal with 3-4 entries; arrow direction (→) per row. | Linked-paragraphs picker. Each row shows direction (→) and slug-path. D removes; Enter opens. |

### Chapter 16 — The story view

| id | state | caption |
|----|-------|---------|
| `story-view-book` | Ctrl+V Shift+W book view with a moderately-sized book (3-5 chapters, ~20 paragraphs, several wiki-links + a few lexicon mentions). | Ctrl+V Shift+W — book story view. Book at centre. Chapters, paragraphs, wiki-link dashed edges, lexicon mentions dotted edges. |
| `story-view-paragraph` | Ctrl+V w with an open paragraph that has 2-3 wiki-link neighbours + 1-2 lexicon mentions. | Ctrl+V w — paragraph mini view. Open paragraph at centre; hop-1 neighbours on inner ring; lexicon on outer. |

### Chapter 17 — Story timeline

| id | state | caption |
|----|-------|---------|
| `timeline-event-picker` | Ctrl+V e picker with at least 6-8 events across 2 tracks. | Ctrl+V e — chronological event picker. Track filter via `t`. Enter opens the event paragraph. |
| `timeline-swim-lanes` | Ctrl+V t swim-lane view with at least 2 tracks + the orphan row. Axis labels visible along the top. | Ctrl+V t — swim-lane view. Per-track rows. ● instant; ─ duration; ◌ orphan. Axis labels along the top. |
| `timeline-descent-picker` | Descent picker (`d` inside Ctrl+V t) with 3-4 child scopes + event counts. | Descent picker (`d`) — immediate child scopes with their event counts. Enter descends; Esc returns. |

### Chapter 18 — Configuring AI providers

| id | state | caption |
|----|-------|---------|
| `ctrl-b-l-llm-picker` | Ctrl+B L provider picker. Three configured providers visible; one marked current. | Ctrl+B L — provider picker. Current provider marked. Enter switches. |

### Chapter 19 — The AI pane

| id | state | caption |
|----|-------|---------|
| `ai-pane-fullscreen` | Ctrl+B K full-screen AI pane with a multi-turn conversation. | Full-screen AI pane (Ctrl+B K). Chat history fills the screen; prompt at the bottom. |

### Chapter 20 — Prompts and the F7 grammar check

| id | state | caption |
|----|-------|---------|
| `prompts-book-tree` | Tree pane scrolled to the Prompts book showing the five `.example` paragraphs. | Prompts book in the tree pane — five .example seeds. Rename (F2) drops the suffix to activate. |
| `grammar-apply-diff` | Editor pane after `g` was pressed in the AI pane. Corrected text in place with green additions visible. | After `g` — corrected paragraph in place, additions highlighted green. Survives saves; cleared by Ctrl+B C. |

### Chapter 21 — Critique, memory, and the diff modal

| id | state | caption |
|----|-------|---------|
| `per-paragraph-memory-flow` | A small composite — three AI pane snapshots for the same paragraph across separate sessions, showing the memory carrying forward. | Per-paragraph memory — three Paragraph-scope prompts on the same paragraph. The model sees turn 1 + turn 2 as prologue to turn 3. |
| `ai-diff-modal` | AI diff modal open. Left = current buffer, right = proposed replacement, with at least 2-3 removed lines (red `-`) and 2-3 added (green `+`). | AI diff modal — left is current buffer, right is the proposed replacement. Removed lines marked -, added lines marked +. |

### Chapter 22 — AI for diagnostics and the timeline

| id | state | caption |
|----|-------|---------|
| `ctrl-f12-explain` | AI pane just after Ctrl+F12 was pressed. The streaming response is visible explaining a diagnostic; the editor's red ● is visible on the relevant line at the left. | Ctrl+F12 — diagnostic message + ±5 lines of context sent to AI, which explains the cause + suggests a fix. |

### Chapter 23 — Typst inside Inkhaven

| id | state | caption |
|----|-------|---------|
| `doctor-typst-section` | `inkhaven doctor` output scrolled to the typst section. Engine choice, fonts list, package cache visible. | `inkhaven doctor` — typst section. Engine, fonts (system + bundled), package cache, and any warnings. |

### Chapter 24 — Diagnostics and render preview

| id | state | caption |
|----|-------|---------|
| `gutter-diagnostic` | Editor pane close-up showing a paragraph with a red ● on at least one line. The diagnostic line should reference an undefined typst function. | Editor gutter — line 3 has a red ● because it references an undefined function. Marker stays visible while you fix. |
| `f8-list` | F8 diagnostics list modal with 3-5 diagnostics visible. | F8 — diagnostics list. Each row shows line:col + message. Enter jumps cursor; Esc closes. |
| `ctrl-v-r-modal` | Ctrl+V R render preview modal with a rendered paragraph PNG visible. Footer shows zoom + page state. | Ctrl+V R — rendered PNG of the open paragraph. ← / → navigate pages, + / - zoom, S saves current page. |

### Chapter 25 — Multi-format export

| id | state | caption |
|----|-------|---------|
| `ctrl-b-o-extra-formats` | Ctrl+B O splash mid-walk through multiple formats. Spinner + format-name text visible. | Ctrl+B O — splash showing each format being built, one at a time. Esc cancels (already-built formats survive). |

### Chapter 27 — Theming and the cheat sheet

| id | state | caption |
|----|-------|---------|
| `startup-pulse-splash` | Startup pulse splash visible on first launch. Words today, streak, active time, status counts. | Startup pulse splash — today's words, current streak, active time, by-status counts. Auto-closes after 7s or any key. |
| `ctrl-b-h-cheat` | Ctrl+B H cheat-sheet overlay on top of a populated TUI. | Ctrl+B H — pane-aware quick reference. Scoped to current focus + the layer-aware chord tables. |

### (Cover + author photo + back-cover figures)

| id | state | caption |
|----|-------|---------|
| `author-portrait` | Portrait of Vladimir Ulogov for the "About the author" page — provided. Close-crop headshot. The chapter wraps its opening text alongside the portrait in a 56mm-wide column. | Vladimir Ulogov. |
| `book-cover-art` | Cover image of the book — provided. Generated from `Book/images/book-cover-art.typ` (`typst compile --format png --ppi 300 Book/images/book-cover-art.typ Book/images/book-cover-art.png`). 800×1200pt typst source rasterised to ~1.0 MB PNG at 300 ppi. Edit the .typ source to retune; rebuild to refresh. | The Book of Inkhaven — typeset cover. Warm cream paper, burnt-sienna ink. A stylized tree of words with paragraph-pilcrow leaves rooted at the editor cursor glyph; a quill in the lower-right margin; an ink drop in the lower-left. Author + version at the foot. |

## Workflow tips

1. **Capture in batches.** Open the project once; navigate to each chord one after another. The reset between captures is fast.

2. **Sample project.** Build a small reference project for screenshots — book "Aerin Saga", 3 chapters, ~20 paragraphs, a handful of tags + wiki-links + events. Carry it forward; consistent screenshots = professional book.

3. **Tools.** macOS: `Cmd+Shift+4` then drag to select. Linux: `gnome-screenshot --area` or `spectacle`. Windows: Snipping Tool. Aim for PNG + no compression.

4. **Verify the placeholder swap.** After dropping a PNG into `Book/images/`, recompile the book. The placeholder rectangle should be replaced. If not, check the filename — must match the `id` exactly.

5. **Optional: build the swap as part of CI.** A small shell loop:

   ```
   for f in Book/images/*.png; do
     id="$(basename "$f" .png)"
     echo "  $id  $(file "$f" | cut -d, -f2-3)"
   done
   ```

   Cross-reference against the catalog here.

## Status of this catalog

| Chapter range | Figures |
|---------------|---------|
| 0–9 | 8 |
| 10–19 | 11 |
| 20–29 | 8 |
| Appendices | 0 |
| **Total** | **~27** |

If you find a `#figure_slot` in the typst sources that isn't catalogued here, add it — keeping the catalog and the placeholders in lockstep is what makes the "drop a PNG to fill" workflow safe.
