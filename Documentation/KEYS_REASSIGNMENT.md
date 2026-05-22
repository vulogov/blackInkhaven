# Reassigning chord keys

Inkhaven's chord layout (`Ctrl+B C` adds a chapter, `Ctrl+B M`
cycles a node's type, …) is data-driven. You can rewrite any
sub-chord under the meta- or bund-prefix through two channels:

1. **HJSON** — `inkhaven.hjson` → `keys.bindings`. Static; applied
   at startup; survives restarts. The recommended path.
2. **Bund** — `ink.key.*` stdlib words. Dynamic; runs from a Script
   node or the bootstrap; sandboxed (default-denied). Useful for
   tools that conditionally remap based on project state.

Both channels mutate the same binding table the TUI dispatcher
reads on every key press, so a chord you bind from Bund is live
the next time you press it.

## What's rebindable, and what isn't

| Slot | How to change |
|------|---------------|
| Top-level chords (`Ctrl+S` save, `Ctrl+/` search, `Ctrl+I` ai_prompt, `Tab` next-pane, `Shift+Tab` prev-pane, `PgUp`/`PgDn`, `Ctrl+B` meta-prefix, `Ctrl+Z` bund-prefix) | `inkhaven.hjson` → individual `keys.*` fields |
| **Meta** sub-chords (everything after `Ctrl+B`) | `keys.bindings` in HJSON OR `ink.key.bind` |
| **Bund** sub-chords (everything after `Ctrl+Z`) | same |
| Hard-quit `Ctrl+Q` | **not rebindable** — intercepted before the chord dispatcher |
| F-keys (F1/F3/F5/F6/F7/F9/F10) | **not yet** — currently hardcoded; data-driven migration is a planned follow-up |

## Channel 1 — HJSON `keys.bindings`

```hjson
keys: {
  meta_prefix: "Ctrl+b"
  bund_prefix: "Ctrl+z"
  view_prefix: "Ctrl+v"     // 1.2.4+

  bindings: [
    // Re-letter: Ctrl+B Y also cycles type
    { chord: "Ctrl+b y", action: "tree.morph_type" }

    // Disable: Ctrl+B W no longer toggles typewriter mode
    { chord: "Ctrl+b w", action: "none" }

    // Pane-scoped: Ctrl+B X is "save" but ONLY in the editor pane
    { chord: "Ctrl+b x", action: "editor.save", scope: "editor" }

    // Add a bund sub-chord
    { chord: "Ctrl+z m", action: "bund.run_buffer" }

    // 1.2.4+: view sub-chord (Ctrl+V prefix)
    { chord: "Ctrl+v X", action: "view.fuzzy_paragraph_picker" }

    // 1.2.4+: rebind a top-level F-key
    { chord: "F7", action: "view.add_link" }
  ]
}
```

Each entry is `{ chord, action, scope? }`:

- **`chord`** — `"<prefix> <suffix>"` for meta / bund / view
  sub-chords (two whitespace-separated tokens; the prefix must
  match `meta_prefix`, `bund_prefix`, or `view_prefix`). For
  top-level keys (F-keys, 1.2.4+) use a single token: `"F7"`.
- **`action`** — dotted name from the [action table](#action-table).
  Use `"none"` to disable a default chord.
- **`scope`** — `"any"` (default) / `"editor"` / `"tree"` / `"ai"`.
  Narrow scopes beat broad ones when both match the focus.

Resolution order when multiple entries match a chord: most-recently-
added wins. Overlay entries are prepended to the table, so they
beat the built-in defaults; later overlay entries beat earlier
ones.

## Channel 2 — Bund `ink.key.*`

```bund
// Bind a chord to a built-in action by name
"Ctrl+b y" "tree.morph_type" ink.key.bind

// Bind a chord to an inline lambda — gets a synthetic name and
// runs via the same hooks::fire pathway as save hooks
"Ctrl+b j" { "jot!" println } ink.key.bind_lambda

// Drop a chord
"Ctrl+b y" ink.key.unbind

// JSON dump of every active binding
ink.key.list
```

All four `ink.key.*` words live in the `keymap` policy category,
which is in the default-deny set. Opt in:

```hjson
scripting: {
  enabled_categories: ["keymap"]
}
```

Then put your `ink.key.*` calls in `scripting.bootstrap` or a
`.bund` Script node — both run at project open after the policy
has been applied.

## Action table

Every reachable handler appears here exactly once. The "Default
chord" column shows the out-of-the-box meta-sub or bund-sub
chord that fires the action; the chord after `Ctrl+B` / `Ctrl+Z`
is what you type, not what the table key is.

### Tree pane

| Action | Default chord | Effect |
|--------|---------------|--------|
| `tree.add_book` | `Ctrl+B B` *(plain B in the tree pane, outside meta — listed here for completeness)* | Open Add modal for a new Book |
| `tree.add_chapter` | `Ctrl+B C` | Open Add modal for a new Chapter |
| `tree.add_subchapter` | `Ctrl+B S` | Open Add modal for a new Subchapter |
| `tree.add_paragraph` | `Ctrl+B P` | Open Add modal for a new Paragraph |
| `tree.delete_node` | `Ctrl+B D` | Open Delete confirmation modal for the cursor row |
| `tree.morph_type` | `Ctrl+B M` | Cycle the cursor leaf's type: Paragraph(typst) → Paragraph(hjson) → Script(bund) |
| `tree.reorder_up` | `Ctrl+B U` *or* `Ctrl+B ↑` | Move cursor row up among its siblings |
| `tree.reorder_down` | `Ctrl+B J` *or* `Ctrl+B ↓` | Move cursor row down among its siblings |

### Editor pane

| Action | Default chord | Effect |
|--------|---------------|--------|
| `editor.save` | `Ctrl+B S` *(or `Ctrl+S`)* | Save current buffer |
| `editor.create_snapshot` | `Ctrl+B N` *(or `F5`)* | Snapshot the current buffer's body |
| `editor.cycle_status` | `Ctrl+B R` | Cycle workflow status: None → Napkin → First → … → Ready → None |
| `editor.open_function_picker` | `Ctrl+B F` | Open the Typst-function autocomplete picker |
| `editor.rename_to_first_sentence` | `Ctrl+B T` | Re-derive the paragraph's title from its first sentence |
| `editor.lookup_places_or_image` | `Ctrl+B P` | If cursor is inside `#image(...)`, open the image picker; otherwise Places RAG lookup |
| `editor.lookup_characters` | `Ctrl+B C` | Run the selection through the Characters book |
| `editor.lookup_notes` | `Ctrl+B G` | Run the selection through the Notes book |
| `editor.lookup_artefacts` | `Ctrl+B Y` | Run the selection through the Artefacts book |
| `editor.open_quickref` | `Ctrl+B H` | Pane-aware quick-reference overlay (also in tree / AI) |

### Global (any pane)

| Action | Default chord | Effect |
|--------|---------------|--------|
| `global.open_credits` | `Ctrl+B V` | Version / authors / dependency credits |
| `global.open_book_info` | `Ctrl+B I` | Per-book stats: paths, word counts, PDF status |
| `global.open_llm_picker` | `Ctrl+B L` | Switch the active LLM provider |
| `global.toggle_sound` | `Ctrl+B E` | Toggle typewriter SFX |
| `global.schedule_assemble` | `Ctrl+B A` | Assemble the current book under `<artefacts>/<book>/` |
| `global.schedule_build` | `Ctrl+B B` *(global, beats tree's plain-B add-book)* | Assemble + run `typst compile` |
| `global.schedule_take` | `Ctrl+B O` | Build, then copy the resulting PDF into the launch cwd |
| `global.toggle_typewriter` | `Ctrl+B W` | Full-screen typewriter mode |
| `global.toggle_ai_fullscreen` | `Ctrl+B K` | Full-screen AI mode (chat history + prompt + scope) |
| `global.status_filter_ready` | `Ctrl+B 1` | Filter modal: paragraphs with status `Ready` |
| `global.status_filter_final` | `Ctrl+B 2` | … `Final` |
| `global.status_filter_third` | `Ctrl+B 3` | … `Third` |
| `global.status_filter_second` | `Ctrl+B 4` | … `Second` |
| `global.status_filter_first` | `Ctrl+B 5` | … `First` |
| `global.status_filter_napkin` | `Ctrl+B 6` | … `Napkin` |
| `global.status_filter_none` | `Ctrl+B 7` | … (no status) |
| `global.tag_paragraph` | `Ctrl+B ]` | (1.2.5) Open the project-wide tag picker scoped to the open paragraph — Space selects, T applies, A adds, D deletes (project-wide). |
| `global.tag_search` | `Ctrl+B }` | (1.2.5) Open the tag picker in search mode — Enter on a tag lists paragraphs that carry it; Enter on a paragraph opens it. |

### AI pane

| Action | Default chord | Effect |
|--------|---------------|--------|
| `ai.clear_chat` | `Ctrl+B C` | Stop the current streaming inference and discard chat history |

### Bund sub-chords (Ctrl+Z prefix)

| Action | Default chord | Effect |
|--------|---------------|--------|
| `bund.run_buffer` | `Ctrl+Z R` | Eval the open `.bund` Script buffer against Adam |
| `bund.new_script` | `Ctrl+Z N` | Open Add modal under the Scripts system book for a new `.bund` node |
| `bund.open_eval_modal` | `Ctrl+Z E` | Pop a one-shot Bund expression modal |
| `bund.open_script_picker` | `Ctrl+Z ?` | Pick + eval a `.bund` Script under the branch scope |

### View sub-chords (Ctrl+V prefix, 1.2.4+)

Layer name: `view_sub`. Rebind via HJSON or `ink.key.bind_view_sub`.

| Action | Default chord | Effect |
|--------|---------------|--------|
| `view.export_markdown_buffer` | `Ctrl+V 1` (editor) | Save-as-picker — write open paragraph's buffer as markdown |
| `view.export_markdown_subchapter` | `Ctrl+V 2` (editor) | Save-as-picker — write subchapter subtree as markdown |
| `view.export_markdown_subtree` | `Ctrl+V 1` (tree) | Save-as-picker — write cursor's subtree as markdown |
| `view.toggle_similar_mode` | `Ctrl+V S` | Open / close the similar-paragraph picker + secondary editor |
| `view.open_progress` | `Ctrl+V G` | Open the writing-progress modal |
| `view.open_paragraph_target` | `Ctrl+V T` | Set / clear the per-paragraph word-count target |
| `view.add_link` | `Ctrl+V A` | Add outgoing wiki-link (tree picks target) |
| `view.add_incoming_link` | `Ctrl+V I` | Add incoming wiki-link (tree picks source) |
| `view.list_links` | `Ctrl+V L` | Open the outgoing-links picker |
| `view.list_backlinks` | `Ctrl+V K` | Open the backlinks picker |
| `view.toggle_bookmark` | `Ctrl+V B` | Toggle bookmark on open paragraph |
| `view.list_bookmarks` | `Ctrl+V M` | Open the bookmark picker |
| `view.fuzzy_paragraph_picker` | `Ctrl+V P` | Open the fuzzy paragraph picker |
| `view.render_paragraph` | `Ctrl+V R` | (1.2.5) Save the open paragraph, render via `typst-render`, float a PNG preview (S = save full-DPI PNG, Esc = close) |
| `view.next_diagnostic` | `Ctrl+V N` | (1.2.5) Jump editor cursor to the next typst diagnostic in the buffer (parse or semantic). Wraps at the end. |
| `view.story_graph` | `Ctrl+V W` | (1.2.5) Story view — twopi radial graph of the current book (hierarchy + wiki-links + lexicon mentions), rasterised via `resvg`. S saves the PNG. |

### Top-level keys (1.2.4+)

Layer name: `top_level`. F-keys migrated from hardcoded
matches into the bindings table — overlays accept single-token
chords (`f7`, `shift+f5`, …).

| Action | Default chord | Effect |
|--------|---------------|--------|
| `help_query` | `F1` | RAG over the Help book |
| `rename_node` | `F2` | Open the rename modal |
| `file_picker_tree_import` | `F3` (tree focus) | File-picker: import a file or directory |
| `file_picker_editor_load` | `F3` (editor focus) | File-picker: replace open buffer |
| `toggle_split` | `F4` | Toggle split-edit historical view |
| `accept_split_snapshot` | `F5` | Save a versioned snapshot |
| `open_snapshot_picker` | `F6` | Open the snapshot picker |
| `grammar_check` | `F7` | Grammar correction (default; rebindable) |
| `cycle_ai_mode` | `F9` | Cycle AI scope (Selection / Paragraph / Chapter / Book) |
| `toggle_inference_mode` | `F10` | Toggle inference mode (one-shot / chat) |

### Special

| Action | Default chord | Effect |
|--------|---------------|--------|
| `none` | *(no chord)* | "Do nothing" target — use in overlay to disable a default |

## Notes on the action table

- **Scope discrimination**: several letters mean different things in
  different panes. `Ctrl+B P` is `tree.add_paragraph` in the Tree
  pane but `editor.lookup_places_or_image` in the Editor pane. The
  binding entries carry `scope` so the dispatcher picks the right
  one based on focus.
- **Default-chord conflicts**: `B` is `global.schedule_build` AND
  the plain (non-meta) shortcut for "add book" in the Tree pane.
  The latter is handled outside the meta machinery, so the meta
  table only carries `schedule_build`.
- **Pane-agnostic chords** (`V`, `I`, `L`, `E`, `A`, `B`, `O`, `W`,
  `K`, `H`, status digits) live in the `Any` scope so they fire
  from every pane.

## Discovering bindings at runtime

- The **status bar** updates the moment you press the meta- or bund-
  prefix. It's now auto-generated from the live binding table, so
  any HJSON or `ink.key.*` change shows up there immediately.
- `inkhaven bund "ink.key.list"` (with `keymap` opted in) dumps the
  whole table as JSON. Use it to verify an overlay applied
  correctly, or to grep for "which chord runs X?"

## Hard-blocked chords

The following are intercepted before the chord dispatcher, so
they can't be overridden:

| Chord | Why |
|-------|-----|
| `Ctrl+Q` | Hard quit — the safety net |
| Whatever `meta_prefix` is set to (default `Ctrl+B`) | Setting yields prefix recursion |
| Whatever `bund_prefix` is set to (default `Ctrl+Z`) | Same |

You can change the prefix chords themselves via top-level
`keys.meta_prefix` / `keys.bund_prefix` in HJSON; that's not the
overlay channel, so the rule about "can't be in `bindings:`" still
applies.

## Examples

### Disable the typewriter chord

```hjson
keys: {
  bindings: [
    { chord: "Ctrl+b w", action: "none" }
  ]
}
```

### Add a global "open credits on F12"

Not yet supported — `bindings` requires the `<prefix> <suffix>`
two-token form. Direct chord rebinding (single-key) lands in a
later phase.

### Save-time lint via Bund

```hjson
scripting: {
  enabled_categories: ["keymap"]
  bootstrap: '''
    "hook.on_save" {
      drop
      "(saved)" println
    } register
    "Ctrl+b j" { "jot!" println } ink.key.bind_lambda
  '''
}
```

After project open: every save prints `(saved)` to the TUI log;
`Ctrl+B J` runs the inline lambda and prints `jot!`.

## Failure modes

- **Unknown action**: parse error at startup with line context.
  Inkhaven refuses to launch with a malformed `bindings:`.
- **Prefix mismatch**: an overlay entry whose first token isn't
  `meta_prefix` or `bund_prefix` errors at startup.
- **Suffix == prefix**: rejected — would cause infinite recursion.
- **Bund-side bind without `keymap` enabled**: the `ink.key.*` word
  runs the deny-stub from the sandbox; you'll see "script denied
  by inkhaven policy" in your status bar / log.
