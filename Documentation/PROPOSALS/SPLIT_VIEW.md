# Split view — design proposal

Status: Phase 0 planning (drafted during 1.2.12).
Owner: vulogov.
Target release: 1.2.12 if the cycle stays quiet; 1.3 if it grows.

## 2026-05-31 revision — refined spec

After review, the design collapses to a single
fullscreen-split layout with pane-targeted pickers.
Specifically:

  * **One new layout, not three.**  Drop the
    intermediate `SideBySide` (three-column)
    layout from the original proposal.  The
    fullscreen split is the only new mode.
    Standard layout remains the default.
  * **`Shift+F4` is the toggle.**  Enters / exits
    fullscreen split.  Left pane is the current
    buffer; right pane starts empty (or with
    whatever was last in the `secondary` slot)
    and is filled via a per-pane picker.
  * **Existing F4 + Ctrl+F4 bindings stay
    untouched.**  `F4` continues to toggle
    same-paragraph split-edit; `Ctrl+F4`
    continues to accept the snapshot.  No
    rebinding, no risk to existing muscle
    memory.
  * **Each editor opens its own pickers.**  The
    pickers (tree, recent, similar, bookmarks,
    fuzzy paragraph) are no longer App-global —
    they pop up in the *focused* pane and load
    the chosen paragraph into *that pane*.
    `Tab` swaps focus between left and right.
  * **Each editor is full-featured.**  Same chord
    set per pane, independent dirty bits,
    independent autosave loop, independent
    diagnostic checks.

The rest of the proposal below is mostly intact;
read the original §1-§9 for context, then the
revised state model in the Phase 0 plan that
follows.

## Problem

Two "two editor panes" features ship in inkhaven today.
Neither is what the writer needs for the workflows the
1.2.11 multilingual-prompts cycle made obvious:
translation, reference-while-writing, draft comparison
across time, cross-book lookup.

### F4 split-edit (since 1.2.4)

Same paragraph in both halves of the editor area.
Upper editable; lower is a frozen snapshot captured
when `F4` was pressed.  Two-pane layout *inside* the
editor area — tree + AI panes stay visible.  F12
critique flips to `critique-changes` mode when split
is active.  Per-paragraph; ends on `F4` again or
`Ctrl+F4` accepts the changes.

State shape: `OpenedDoc.split: Option<SplitView>` where
`SplitView { snapshot_lines, scroll_row }`.  The lower
half is a view, not a doc; you can't type into it.

### Ctrl+V S similar-mode (since 1.2.4)

Two *different* paragraphs side-by-side.  Both
editable.  The secondary replaces the AI pane —
right column of the screen is now a second editor
instead of the AI pane.  `Tab` swaps focus.
`Ctrl+V S` exits.  Secondary is chosen via the
vector-similarity picker only.

State shape: `App.secondary: Option<OpenedDoc>` plus
`App.secondary_focused: bool`.  Both are full
`OpenedDoc`s — independent textareas, independent
scroll, independent dirty bits.

### The gap

Three workflows neither feature handles cleanly:

  * **Translation work.**  Author writes in
    Russian (book `manuscript-ru`) and maintains an
    English version (book `manuscript-en`).  They
    want left = `manuscript-en/03-rain`, right =
    `manuscript-ru/03-rain`, both editable.  Today
    Ctrl+V S can get them two panes — but the
    similarity picker is the wrong way to find the
    sibling paragraph in the other book.
  * **Reference-while-writing.**  Author is writing
    chapter 7 and wants chapter 6 visible for
    continuity.  Wants both paragraphs side-by-
    side AND the AI pane available (so they can
    ask "does the tone match?").  Today Ctrl+V S
    sacrifices the AI pane.
  * **Draft comparison across time.**  Author
    snapshotted a paragraph two weeks ago and
    wants to A/B against the current draft.
    Today F4 only goes back to "opened-state"
    (the buffer at the moment F4 was pressed),
    not to an arbitrary F6 snapshot.  F6's
    snapshot diff is a render-time view; it isn't
    a side-by-side editor.

## Requirements (negotiated)

* Don't break F4 or Ctrl+V S.  Both have established
  behaviour and at least one of us has muscle memory
  on each.  The new model **adds**; it doesn't
  replace.
* The secondary pane can be populated from any
  paragraph source — similarity picker (today's
  path), tree walk, F6 snapshot, bookmark, recent-
  paragraphs ring, sibling-book lookup.  The picker
  that fills it is chord-selectable, not feature-
  fixed.
* Three layouts, user-cycled with a single chord:
  - **Off** — full editor + AI pane (today's
    default).
  - **Side-by-side** — two paragraphs in the
    editor area; AI pane still on the right.
    Three-column layout.
  - **Full-screen split** — two paragraphs filling
    the whole window; tree + AI hidden.  Mirrors
    the existing `Ctrl+B K` AI-fullscreen
    plumbing.
* The split persists across primary-paragraph
  switches (writer scrolling chapter-by-chapter
  doesn't re-pick the secondary every time).
* The secondary slot is cleared either explicitly
  (`Ctrl+V Shift+S` cycle through to Off, or a
  dedicated "drop secondary" chord) or implicitly
  when the user picks a new primary that overlaps
  the secondary (don't show the same paragraph
  twice).
* F12 critique remains mode-aware: same-paragraph
  split (F4) → `critique-changes`; different-
  paragraph split (any of the new layouts) → a
  new `compare-paragraphs` prompt route, or the
  existing `critique-edit` against the
  *focused* paragraph alone (TBD — see §8).

## Design

### 1. Layout model

Replace the implicit "if `secondary.is_some()` then
right half = secondary, else right half = AI" with
an explicit `Layout` enum on `App`:

```rust
pub enum Layout {
    /// Today's default: editor takes the left
    /// two-thirds, AI pane takes the right third.
    Standard,
    /// Three-column: editor (1/3) · secondary (1/3)
    /// · AI (1/3).  Secondary slot must be Some;
    /// rendering falls back to Standard when it's
    /// None.
    SideBySide,
    /// Two-column: editor (1/2) · secondary (1/2).
    /// Tree + AI panes hidden.  Mirrors the
    /// fullscreen-AI mode's layout discipline.
    FullScreenSplit,
}
```

`App.layout: Layout` is session state; default `Standard`.

The renderer at `src/tui/app/render.rs` already
dispatches per-layout for the fullscreen-AI case
(`is_ai_fullscreen()`).  We add a similar branch
for `Layout::SideBySide` and `Layout::FullScreenSplit`.

### 2. The secondary slot

`App.secondary: Option<OpenedDoc>` stays as today —
the field already exists with the right shape.  What
changes is *who can write to it*.

Today only `load_secondary_paragraph` (called from
the similarity-picker accept path) writes to
`secondary`.  We generalise:

```rust
impl App {
    /// 1.2.12+ — populate the secondary slot from
    /// any paragraph node.  Replaces whatever was
    /// there before (no stack / history — that's
    /// what Alt+← / Alt+→ already do for primary).
    /// Returns Err when the node isn't a paragraph
    /// or when it equals the current primary
    /// (don't show the same paragraph twice).
    pub(super) fn pin_secondary(&mut self, id: Uuid)
        -> Result<(), String>
    { ... }

    pub(super) fn drop_secondary(&mut self) { ... }
}
```

Every existing picker that already opens a paragraph
into the primary gains a "pin to secondary instead"
variant.  Concretely:

| Picker                | Open-primary chord | Pin-to-secondary chord |
|-----------------------|--------------------|------------------------|
| Tree pane             | `Enter`            | `Shift+Enter`          |
| Fuzzy paragraph picker | `Enter`           | `Shift+Enter`          |
| Bookmark picker       | `Enter`            | `Shift+Enter`          |
| Recent-paragraphs picker | `Enter`         | `Shift+Enter`          |
| Similar-paragraph picker | `Enter` (today: opens secondary) | unchanged |
| F6 snapshot picker     | `Enter` (restores) | `Shift+Enter` (new: pin pre-restore state) |

`Shift+Enter` as the universal "pin to secondary"
modifier mirrors how `Shift+...` modifiers extend
other chords (Ctrl+V w → Ctrl+V W; selection mode
in tree).  The exception is the similarity picker
where the default open-into-secondary behaviour
*is* the muscle memory we want to preserve.

### 3. Layout cycling

A single chord cycles `Layout`:

```
Off → SideBySide → FullScreenSplit → Off
```

Mnemonic candidate: `Ctrl+V Shift+S` for "split
view".  Conflicts: `Ctrl+V S` is the
similar-mode toggle today.  We keep `Ctrl+V S` as
"toggle secondary with similarity-picker" but
*also* route it through the new layout state — if
the secondary is None when `Ctrl+V S` is pressed,
fall back to today's behaviour (open the similarity
picker).  `Ctrl+V Shift+S` is the explicit layout-
cycle that doesn't touch the secondary slot.

### 4. Persistence + invariants

  * **Primary-switch preserves secondary.**  Today
    Ctrl+V S exits when the user switches primary.
    The new model keeps the secondary pinned —
    explicitly: opening a new paragraph into the
    primary leaves `secondary` alone unless the
    new primary's UUID == secondary's UUID, in
    which case `secondary` is cleared.
  * **Secondary outlives layout flips.**  Cycling
    `Standard → SideBySide → FullScreenSplit → Off`
    doesn't clear the secondary slot; you can
    pop the layout back to Standard knowing the
    secondary is still pinned for next time.
  * **Empty-secondary fallback.**  Rendering
    `SideBySide` or `FullScreenSplit` with
    `secondary == None` silently falls back to
    `Standard` (and the layout flips back so the
    state is honest about what's shown).

### 5. Save semantics

Both `opened` and `secondary` are full `OpenedDoc`s
with independent dirty bits.  The existing
`save_current` already saves whichever
`secondary_focused` flag points at.  We add:

  * **`Ctrl+S` saves the focused doc** (today).
  * **`Ctrl+B Ctrl+S` saves both** (new).  Mnemonic
    overload of the meta-prefix → save; reads
    "save everything".  Fires the
    paragraph-language detection re-hook on each.
  * **Idle autosave saves whichever has been
    inactive longest** — today autosave only
    touches the primary; secondary loses edits
    until the user `Ctrl+S`-es manually.

### 6. AI pane behaviour in each layout

| Layout            | AI pane | Notes |
|-------------------|---------|-------|
| `Standard`        | visible | Today's behaviour. |
| `SideBySide`      | visible | Three-column, AI pane narrower (1/3 instead of 1/3 of a 2/3 split — net ~25% reduction in AI pane width). |
| `FullScreenSplit` | hidden  | F10 / `Ctrl+I` still routes AI calls but the response only surfaces when layout returns to Standard or SideBySide.  Status bar shows "AI response pending — press Ctrl+V Shift+S to return" when an in-flight call completes. |

The streamer keeps running in `FullScreenSplit`; the
user just can't *see* it stream.  The `Done` message
plus an audible bell (when sound is on) signal
completion.

### 7. F12 critique in each shape

| Active layout       | secondary populated? | F12 routes to |
|---------------------|----------------------|---------------|
| `Standard`          | n/a                  | `critique-edit` (unchanged) |
| `Standard` + F4 split | n/a (split is same-paragraph) | `critique-changes` (unchanged) |
| `SideBySide` / `FullScreenSplit` | yes, secondary != primary | New `critique-compare` prompt — sends both bodies + asks for a comparative critique |

The new `critique-compare` prompt name needs an
embedded fallback (5-language match, same as the
other six embedded prompts).  Goes through the Phase A
resolver naturally.

### 8. Picker UI

Every picker that grows a `Shift+Enter` route gains a
one-line footer hint:

```
↑↓ navigate · Enter open · Shift+Enter pin in split · Esc cancel
```

Renders dim so it doesn't compete with content.

For the **sibling-book lookup picker** (translation
workflow), a new chord — candidate `Ctrl+V Shift+B`
("Book sibling") — walks the project tree looking
for a paragraph with the *same slug* under a
different top-level book and pins it to the
secondary.  When multiple matches exist, a small
picker lets the user choose; when zero match, a
status message names the slug it tried.

### 9. Out of scope (deliberately)

  * **N > 2 panes.**  Three- or four-way splits
    aren't on the table.  The TUI's column widths
    don't survive past two.
  * **Independent tree pane per side.**  Both
    halves share the same tree pane (which only
    affects the primary).  Want to walk the
    secondary tree → swap primary/secondary, walk,
    swap back.
  * **Per-side AI pane.**  Single AI pane at most.
    Two streaming inferences would require
    significant streamer changes for marginal
    value.
  * **Persistent layout across sessions.**  The
    layout choice is session-local; restart drops
    to `Standard`.  Saving it in `inkhaven.hjson`
    could land as polish but isn't load-bearing.
  * **Mouse-driven split-bar drag.**  The 1/3-1/3-
    1/3 and 1/2-1/2 ratios are fixed.  A draggable
    sash is a 1.3 polish item, not 1.2.12.

### 10. Implementation phases

**Phase A — foundation.**  No UI-visible change.

  * `Layout` enum on `App`; default `Standard`.
  * `pin_secondary(id) -> Result<(), String>` and
    `drop_secondary()` helpers.
  * Renderer dispatches on `Layout` (today's
    behaviour now lives explicitly under
    `Layout::Standard`).
  * Idle autosave touches both `opened` and
    `secondary`.
  * Test: layout invariant — `SideBySide` with
    `secondary == None` renders as `Standard`.

**Phase B — pickers route through.**

  * Tree pane gains `Shift+Enter` → pin secondary.
  * Fuzzy paragraph picker likewise.
  * Bookmark picker, recent-paragraphs picker
    likewise.
  * F6 snapshot picker gains `Shift+Enter` → pin
    pre-restore state.
  * Status footer hint added to each.
  * Test: pin-then-switch-primary preserves
    secondary; pin-where-id-equals-primary returns
    error.

**Phase C — layout cycling.**

  * `Ctrl+V Shift+S` cycles
    `Standard → SideBySide → FullScreenSplit → Off`.
  * `Ctrl+V S` keeps today's behaviour but routes
    through the new state machine (so the
    secondary persists across layout cycles).
  * AI pane width plumbing for the three-column
    case; status-bar "AI response pending" hint
    for `FullScreenSplit`.
  * Test: cycle preserves secondary; fallback to
    Standard when secondary cleared.

**Phase D — translation + critique-compare.**

  * `Ctrl+V Shift+B` sibling-book lookup picker.
  * `critique-compare` embedded prompt (5-language
    floor); routed through the multilingual
    resolver.
  * F12 dispatch updated for the new shape.
  * Test: sibling-book lookup with single match
    auto-pins; multi-match opens picker; zero-
    match reports a status message.

Each phase is its own commit / PR; main stays green
between them.

## Risks + open questions

  * **AI pane width in `SideBySide`.**  At 1/3 of
    an 80-column terminal that's 27 chars wide —
    AI responses wrap aggressively.  Mitigation:
    1.2.11 already added the markdown wrapping
    polish; verify behaviour at 27 cols against
    real critique responses before committing to
    the 1/3-1/3-1/3 ratio.  Fallback: 2/5-2/5-1/5
    or hide the AI pane entirely in `SideBySide`
    (collapsing it back to today's two-layout
    shape minus the `Standard` case).
  * **F4 split-edit + secondary collision.**  What
    happens when F4 is active AND a secondary is
    pinned?  The simplest answer: F4 only takes
    effect on the *focused* doc.  So same-paragraph
    snapshot lives inside whichever pane has focus;
    the other pane shows its own paragraph
    independently.  Test before committing.
  * **`secondary_focused` semantics.**  Today
    `Tab` toggles focus between `opened` and
    `secondary` when both exist.  Tabs need to
    also work in `FullScreenSplit` and skip the
    tree / AI panes there.  Plumbing only, no
    user-visible change.
  * **Open question: should F12 `critique-compare`
    take a target language?**  In translation
    workflow, the user probably wants the critique
    in their *working* language, not in the
    secondary's language.  Solution: use
    `active_prompt_language()` exactly as the
    other AI flows do; the secondary's tag
    influences resolver Pass 1 only when it
    matches.

## Recommendation

Phase A is pure plumbing — explicit `Layout` enum,
generalised `pin_secondary`, idle autosave for
secondary.  Ship it first; it can't break anyone
because nothing user-visible changes.

Phases B + C are the user-visible win — `Shift+Enter`
in every picker plus the layout cycle.  These
should ship together since `Ctrl+V Shift+S` doesn't
do anything useful without the picker plumbing to
populate the secondary.

Phase D (sibling-book + critique-compare) is the
translation-workflow payoff but it's also the
phase with the most new prompt content (the
5-language `critique-compare` floor).  Could defer
to 1.3 if the cycle stays tight.

Suggest A in this cycle for sure; B + C if the
polish work doesn't fill the cycle; D if the
multilingual prompts content was tractable enough
that another bootstrap-style content drop is on
the table.

---

## Phase 0 — planning

Phase 0 is reconnaissance + design finalisation, not
implementation.  Exit criterion: every decision below
is committed; every chord collision is resolved; every
file slated for change is named; the test surface is
mapped.  After Phase 0, Phase A's commits should be
mechanical.

### 1. The new chord

`Shift+F4` is unused in the current binding table.
We claim it for the fullscreen-split toggle.  No
rebinding of existing chords — F4 and Ctrl+F4 keep
their meanings.

| Chord     | Today                     | After 1.2.12              |
|-----------|---------------------------|---------------------------|
| `F4`      | `editor.toggle_split`     | **unchanged**             |
| `Ctrl+F4` | `editor.accept_split_snapshot` | **unchanged**        |
| `Shift+F4` | unbound                  | `editor.toggle_split_view` (new) |

The Bund-key plumbing
(`if matches!(key.code, F(4)) && ctrl` at
`app.rs:17943`) gains a sibling check for the Shift
modifier so `Shift+F4` is intercepted before
tui-textarea sees it.  No change to the existing
Ctrl-F4 path.

Mnemonic: Shift = "bigger / fuller" pairs with the
fullscreen-vs-inline distinction — F4 is the
inline split, Shift+F4 is the bigger fullscreen
split.

### 2. State-model decision

`OpenedDoc`-as-secondary already exists from the
Ctrl+V S similar-mode work (`App.secondary:
Option<OpenedDoc>`, `App.secondary_focused: bool`).
The split-view's right pane is the *same shape*.
Reuse the slot; gate the rendering on a new layout
flag rather than introducing a parallel slot.

**Decision: single secondary slot, layout-driven
render.**

  * `App.secondary` already a full `OpenedDoc`.  No
    change to its lifecycle: open via picker, edit
    independently, save via Ctrl+S when focused.
  * Add `App.split_view: bool` (or a `Layout` enum
    if we anticipate further modes — see §6).
    When `true`, the renderer goes into the new
    layout regardless of whether `secondary` is
    populated.  Empty-right-pane shows a hint.

**Why not two parallel slots (one for
similar-mode, one for split-view)?**
That would mean a paragraph the user pinned via
similar-mode disappears when they enter split-view
and vice versa.  Sharing the slot is the model that
respects "the user picked this paragraph for the
secondary — keep it there until they pick a
different one."

**Side-effect:** Ctrl+V S similar-mode and Shift+F4
fullscreen-split share the `secondary` slot.
Pressing Ctrl+V S after Shift+F4 doesn't change the
secondary contents; it just toggles the layout.
We need to be explicit about this in the status
echo so the user understands.

### 3. Picker-target plumbing

Today every picker (`Modal::FuzzyParagraphPicker`,
`Modal::BookmarkPicker`, `Modal::SimilarPicker`,
tree pane Enter, recent-paragraphs picker) loads
the chosen paragraph into `self.opened` —
implicitly the primary.

**Decision: capture target at picker-open time.**

Add `App.picker_target: PaneTarget`
(`enum PaneTarget { Primary, Secondary }`),
default `Primary`.  Every picker-open code path
sets this from the current focus before opening
the modal.  Every picker-accept code path
consumes it.

```rust
enum PaneTarget {
    Primary,
    Secondary,
}

impl App {
    fn picker_open_target(&self) -> PaneTarget {
        if self.split_view && self.secondary_focused {
            PaneTarget::Secondary
        } else {
            PaneTarget::Primary
        }
    }
}
```

Picker accept routes:

```rust
match self.picker_target {
    PaneTarget::Primary   => self.open_paragraph_by_uuid(id),
    PaneTarget::Secondary => self.pin_secondary(id),
}
```

This generalises to *every* picker without per-
picker plumbing changes — they all already call
`open_paragraph_by_uuid` somewhere; we wrap that
in a target-aware dispatch.

**Files in scope:**
  * `app.rs` — `open_fuzzy_paragraph_picker`,
    `open_bookmark_picker`,
    `open_recent_paragraph_picker`,
    `open_similar_picker`, plus tree-pane
    Enter dispatcher.
  * `app/editor_impl.rs` — `open_paragraph_by_uuid`
    becomes target-aware (or, equivalently, gets
    a sibling `pin_secondary_by_uuid`).

### 4. Renderer dispatch

`src/tui/app/render.rs` already has the
`is_ai_fullscreen` branch for `Ctrl+B K`.  We add
a sibling branch:

```rust
fn draw(&self, f: &mut Frame) {
    if self.is_ai_fullscreen() { ... return; }
    if self.split_view { self.draw_split_view(f); return; }
    self.draw_standard(f);
}
```

`draw_split_view` layout:

```
┌─────────────────────────────────────────────┐
│ status bar                                  │
├──────────────────────┬──────────────────────┤
│                      │                      │
│   Primary editor     │   Secondary editor   │
│   (focused)          │                      │
│                      │                      │
├──────────────────────┴──────────────────────┤
│ AI prompt input (always at bottom)          │
└─────────────────────────────────────────────┘
```

  * Equal-width 1:1 column split.
  * Both panes get the full editor chrome (gutter,
    line numbers, search highlights, style-warning
    overlays).
  * Tree pane, AI response pane: hidden.  Tree
    *pickers* still work (they're modals, not the
    persistent tree pane).
  * AI prompt input: still at the bottom.  AI
    responses to Ctrl+I from inside split-view
    surface as a status-bar "AI response pending,
    Shift+F4 to return" hint and are visible the
    moment layout flips back to Standard.
  * Right pane when `secondary.is_none()`: shows a
    dim hint — "press Ctrl+V P to pick a
    paragraph, Ctrl+V S for similar, or Enter
    here to copy left".  (TBD — see §7.)

### 5. Tab semantics

Today Tab cycles editor → tree → AI → editor.  In
split-view, the tree and AI panes don't exist on
screen.  Tab needs to swap left ↔ right only.

**Decision:** Tab in `split_view = true` calls
`self.secondary_focused = !self.secondary_focused`
without going through the broader pane cycle.

This is a small, contained change to the Tab
handler.  Shift+Tab (reverse-cycle) does the same
thing in a two-pane case.

### 6. Autosave + dirty tracking

`OpenedDoc.dirty` already exists per-doc; the
secondary's bit is independently tracked from
similar-mode.  The autosave loop today only walks
`self.opened` — needs to also walk `self.secondary`.

**Decision:** generalise the idle-autosave function
to iterate both slots.  Each save-current is
already a no-op on a clean doc, so the change is
the loop, not the save logic.

```rust
fn idle_autosave_tick(&mut self) {
    // Save primary (today's behaviour).
    self.save_current();

    // 1.2.12+ — also save secondary when it's
    // dirty.  Split-view makes this necessary; in
    // similar-mode the same hook fires usefully but
    // didn't exist before.
    if self.secondary.is_some() {
        self.save_secondary_current();
    }
}
```

Implementing `save_secondary_current` is the
sibling of `save_current` — same shape, different
target.  Roughly 60 lines of mirror logic.

### 7. Open questions (decide in Phase 0)

  * **Empty-right-pane behaviour.**  When the user
    presses `Shift+F4` and `secondary` is None,
    what does the right pane show?  Options:
      1. Dim placeholder + chord hint (today's
         "no AI inference yet" pattern).
      2. Auto-copy the left buffer (so the user
         starts with a duplicate they can edit
         in parallel).
      3. Auto-open the recent-paragraph picker
         targeting the right pane (jump-start
         the workflow).
    Recommendation: option 1.  Most explicit.
  * **Drop secondary on exit?**  When the user
    presses `Shift+F4` to leave split-view, what
    happens to the secondary?  Options:
      1. Keep it pinned (re-entering split-view
         returns to the same state).
      2. Clear it.
    Recommendation: option 1.  Pinning is the
    less-surprising default; the user can clear
    explicitly via a new "drop secondary" chord
    if needed.
  * **F12 critique in split-view.**  The current
    F12 dispatches off `OpenedDoc.split` (F4 mode
    → critique-changes).  In Shift+F4 split-view
    with two paragraphs, what does F12 send?
    Options:
      1. The *focused* paragraph only —
         critique-edit.  Simplest, least
         surprising.
      2. Both paragraphs + a new
         `critique-compare` prompt.  Aligns
         with the translation workflow.
    Recommendation: defer the `critique-compare`
    prompt to Phase D (per the original §10);
    Phase 0 ships option 1.
  * **Bound chords that escape split-view.**
    `Ctrl+B K` (fullscreen AI), `Ctrl+B W`
    (typewriter mode), `Ctrl+B 0` (HJSON editor)
    are full-screen modes themselves.  Pressing
    one of them from inside split-view should
    cleanly close split-view first.  Or should
    they nest?
    Recommendation: cleanly close split-view.
    Nesting fullscreen-modes inside fullscreen-
    modes is a rendering nightmare.

### 8. Code touchpoints

Files that need edits, in order of blast radius
(largest first):

  1. `src/tui/app.rs` — new fields
     (`split_view`, `picker_target`), Tab
     handler, autosave loop, picker-target
     dispatch, status-bar plumbing.
  2. `src/tui/app/render.rs` — `draw_split_view`
     function + dispatcher branch.
  3. `src/tui/app/render/panes.rs` — refactor
     `draw_editor` to take a `OpenedDoc` ref +
     a `Rect` so it can render against either
     slot.  Today it implicitly walks
     `self.opened`.
  4. `src/tui/keybind.rs` — chord rebind
     (add `Shift+F4`; do NOT touch existing
     `F4` / `Ctrl+F4`).
  5. `src/tui/app/editor_impl.rs` —
     `pin_secondary_by_uuid` (new),
     `save_secondary_current` (new),
     `load_paragraph` → split into `load_into`
     that takes a target slot.
  6. `src/tui/quickref.rs` — chord hints.
  7. `Documentation/KEYBINDING.md` — row
     updates.
  8. `Documentation/KEYS_REASSIGNMENT.md` —
     action table entries.

The biggest single edit is #3 — `draw_editor`
takes its target implicitly today.  Threading it
through is mechanical but touches every editor-
render call site (search highlights, style-warning
overlays, gutter, line numbers).

### 9. Tests to plan

Phase 0 should write the test names; Phase A
implements them as it goes.

  * `split_view_toggle_with_empty_secondary_renders_placeholder`
  * `split_view_toggle_preserves_secondary_across_exit_reenter`
  * `tab_in_split_view_swaps_focus_only_left_right`
  * `picker_target_routes_to_secondary_when_secondary_focused`
  * `idle_autosave_tick_persists_dirty_secondary`
  * `f4_split_edit_still_works_inside_split_view`
    (orthogonal feature — both should compose)
  * `existing_f4_and_ctrl_f4_bindings_remain_untouched`
  * `entering_fullscreen_ai_from_split_view_exits_split_view`

### 10. Exit criteria for Phase 0

  * This document captures the resolved chord table
    (§1), the state-model decision (§2), the
    picker-target plumbing (§3), the renderer
    branch (§4), the Tab + autosave + open-question
    answers (§5-§7).
  * The code-touchpoint list (§8) is reviewed
    against the actual files — no missing surface.
  * Phase A's commit can be opened with the
    chord-rebind + new-fields skeleton already
    in hand.

Once exit criteria are met, Phase A starts.
