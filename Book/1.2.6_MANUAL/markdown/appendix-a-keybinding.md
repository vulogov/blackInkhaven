# Appendix A — Keybinding reference

Every chord that ships with inkhaven, by layer. The canonical machine-readable list lives at `Documentation/KEYBINDING.md`; this appendix is the printed companion.

## Global (anywhere)

| Chord | What it does |
|-------|--------------|
| Ctrl+Q | Quit (autosaves dirty paragraph). |
| Tab / Shift+Tab | Cycle Tree → Editor → AI. |
| Ctrl+1/2/3/4/5 | Focus Editor / Tree / AI / Search / AI prompt. |
| Ctrl+T | Focus Tree. |
| Ctrl+S | Save current paragraph. |
| Ctrl+/ | Focus search bar. |
| Ctrl+I | Focus AI prompt. |
| Ctrl+B | Meta prefix (next key = action). |
| Ctrl+Z | Bund prefix. |
| Ctrl+V | View prefix. |
| F1 | Help RAG query. |
| F7 | Grammar check (editor scope). |
| F8 | Typst diagnostics list (1.2.6+). |
| F9 / Shift+F9 | Cycle AI scope forward / backward. |
| F10 | Toggle inference mode (Local ↔ Full). |
| F12 | AI critique — mode-aware (1.2.6+). |
| Ctrl+F12 | AI explain diagnostic (1.2.6+ — was F11). |
| Esc | Close overlay / cancel. |

## Tree pane

| Chord | What it does |
|-------|--------------|
| ↑ / ↓ / Home / End | Navigate. |
| PgUp / PgDn | Jump by 10. |
| ← / → | Collapse / expand branch. |
| Enter | Open paragraph. |
| F2 | Rename current node. |
| F3 | File picker — load file / import tree. |
| B / C / A / + | Add book / chapter / subchapter / paragraph. |
| V / S / P | Insert chapter / subchapter / paragraph after current. |
| D | Delete branch (confirm). |
| - | Delete paragraph at cursor. |
| U / J | Reorder up / down among siblings. |
| Z / X | Collapse subchapter / collapse every branch. |
| Space | Multi-select toggle. |
| T | Cycle type (works on marks). |
| O | Cycle status (works on marks). |
| g | Tag picker (works on marks). |

## Editor pane

| Chord | What it does |
|-------|--------------|
| Ctrl+S | Save. |
| F5 | Snapshot with annotation prompt (1.2.6+). |
| F6 | Snapshot picker. |
| F4 | Toggle split-edit. |
| Ctrl+F4 | Accept split-edit snapshot as the new baseline. |
| F7 / F12 / Ctrl+F12 | Grammar / critique / explain (see Global). |
| F8 | Diagnostics list (1.2.6+). |
| Ctrl+F | Find in buffer. |
| Ctrl+H | Find and replace in buffer. |

## AI pane

| Chord | What it does |
|-------|--------------|
| r / R | Replace buffer (routes through diff modal). |
| g / G | Replace with grammar-corrected text only. |
| i / I | Insert at cursor. |
| t / T | Prepend (top). |
| b / B | Append (bottom). |
| c / C | Copy to clipboard. |
| Ctrl+F | Search chat history. |
| Ctrl+C | Enter selection mode. |

## Meta prefix (Ctrl+B)

| Chord | What it does |
|-------|--------------|
| Ctrl+B A | Schedule assemble. |
| Ctrl+B B | Schedule build (PDF). |
| Ctrl+B O | Schedule extra-format builds. |
| Ctrl+B C | Clear chat history. |
| Ctrl+B G | Notes RAG. |
| Ctrl+B H | Cheat-sheet overlay. |
| Ctrl+B I | Book info modal. |
| Ctrl+B K | Toggle AI full-screen. |
| Ctrl+B L | LLM provider picker. |
| Ctrl+B M | Show inference mode. |
| Ctrl+B N / U / P / C / A | Open Notes / Research / Places / Characters / Artefacts listing or RAG. |
| Ctrl+B R / Shift+R | Cycle status forward / backward. |
| Ctrl+B 1..7 | Status filter modal. |
| Ctrl+B V | Credits. |
| Ctrl+B ] / } | Tag picker / tag search (1.2.5+). |

## Bund prefix (Ctrl+Z)

| Chord | What it does |
|-------|--------------|
| Ctrl+Z R | Run current buffer as Bund. |
| Ctrl+Z N | New Script node. |
| Ctrl+Z E | Open eval modal. |
| Ctrl+Z ? | Script picker. |

## View prefix (Ctrl+V)

| Chord | What it does |
|-------|--------------|
| Ctrl+V 1 / 2 | Markdown extract: paragraph / subchapter. |
| Ctrl+V S | Toggle similar-paragraph mode. |
| Ctrl+V G | Open progress modal. |
| Ctrl+V t / Shift+T | Per-¶ word-count target / open timeline view (1.2.6+). |
| Ctrl+V A / I | Add outgoing / incoming link. |
| Ctrl+V L / K | List outgoing / incoming links. |
| Ctrl+V B / M | Toggle / list bookmarks. |
| Ctrl+V P | Fuzzy paragraph picker. |
| Ctrl+V R | Render paragraph preview. |
| Ctrl+V N / Shift+N | Next / previous diagnostic. |
| Ctrl+V w / Shift+W | Paragraph mini story view / book story view (1.2.6+). |
| Ctrl+V e | Event picker (1.2.6+). |
| Ctrl+V Shift+E | New event from any pane (1.2.6+). |
| Ctrl+V Shift+I | Edit open event's start \| end \| track (1.2.6+). |

## Inside the timeline view (Ctrl+V Shift+T)

| Chord | What it does |
|-------|--------------|
| ← / → / PgUp / PgDn | Scroll. |
| + / = / - / _ | Zoom in / out. |
| 0 | Reset zoom. |
| Home / End | Jump to first / last event. |
| u / U / d / D / b / B / p / P | Up / down / book / project scope. |
| Tab | Cycle highlighted track. |
| Enter | Open the event closest to cursor. |
| n / N | New event at cursor tick. |
| y / Y / Ctrl+Y | AI critique — track / scope / book-wide. |

## Inside Ctrl+V R (render preview)

| Chord | What it does |
|-------|--------------|
| ← / → / Home / End | Page navigation. |
| + / = / - / _ | Zoom (1.2.6+). |
| 0 | Reset zoom. |
| S | Save current page. |
| A | Save every page. |
