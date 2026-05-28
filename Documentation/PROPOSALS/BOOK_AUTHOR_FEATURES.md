# Research — Book-author feature roadmap for 1.2.11

Status: **brainstorm / pre-proposal**.  Use this
doc to pick the next batch.  Each candidate has
an effort estimate, the existing inkhaven plumbing
it reuses, and the author-facing value it
unlocks.  The user picks which to promote into
focused proposals.

Two halves:

  * **§A — Deferred items.**  Already promised in
    `RELEASE_NOTES/1.2.11.md` as carryovers from
    1.2.10 / 1.2.9.  Clearing these first gives
    1.2.11 a coherent "follow-through on last
    cycle" identity.
  * **§B — New low-hanging features for working
    novelists.**  Fresh candidates, ranked by
    effort.

Reading order: §A is "cheap finish-line work, do
this first."  §B is "what's the next ambitious
thing."

---

## §A — Deferred items (cheap wins from prior cycles)

### A1.  AI-driven sentence-rhythm rewrite

**What it does.**  `Ctrl+B Shift+H` already opens
the sentence-rhythm gauge that flags MONOTONE
paragraphs.  Add a sibling chord that sends the
paragraph to the LLM with a prompt asking it to
break the monotony — same pattern as the
existing AI show-don't-tell scan
(`Ctrl+B Shift+T`).

**Effort.** Half a day.

**Reuse.**  `spawn_chat_stream` + `Inference` +
the AI pane.  Embedded fallback prompt sits next
to `show_dont_tell_default_prompt` in
`src/tui/app.rs`.

**Author value.**  When the gauge says MONOTONE,
the author wants a concrete rewrite.  Today they
have to leave the gauge, copy the paragraph, ask
the AI manually.

**Chord.**  `Ctrl+B Shift+M` ("Mix it up" /
"Modulate") — `M` is free in the meta_sub table.

---

### A2.  Concordance jump-to-occurrence

**What it does.**  `Ctrl+B Shift+L` opens the
concordance.  Today it's read-only: even the
KWIC samples show breadcrumbs like
`aerin/chapter-2:l8` but you can't navigate to
them.  `Enter` on a row closes the modal +
opens the source paragraph + jumps the cursor
to the line of the first sample.

**Effort.** Half a day.

**Reuse.**  Navigation-history ring already
records paragraph opens.  Paragraph lookup by
slug-path is already in `Hierarchy::find_by_path`.

**Author value.**  The concordance becomes a
*navigation tool*, not just a *report*.  See a
word overused 47 times → Enter → land at the
first instance → fix.

---

### A3.  Per-language emotion + linking-verb lists for SDT

**What it does.**  The show-don't-tell detector
(1.2.9) ships English defaults; Russian / French
/ German / Spanish scaffolds are empty.  Curate
those.

**Effort.**  ~2 hours per language for the basic
emotion-adjective + linking-verb lists.  More
for native-quality manner adverbs + cognition
verbs.  Total: a day for all four to get
"baseline useful".

**Reuse.**  Existing built-in lists in
`src/config.rs` — same shape, just new language
arms.  Existing Snowball stemmer pipeline.

**Author value.**  Non-English writers get the
1.2.9 prose-craft tools without typing 200
words of emotion vocabulary into their HJSON.

**Caveat.**  These lists encode genre + register
assumptions.  Need a writer / dogfooder per
target language to validate.  Acceptable to ship
"common-noir-ish defaults" with a CONFIGURATION.md
note that genres differ.

---

### A4.  Build-time doc-comment extractor polish

**What it does.**  The extractor handles
`Option<T>` / `Vec<T>` / `HashMap<K, V>` descents.
Nested shapes like `Option<HashMap<String, V>>`
don't yet recurse correctly.  Audit + fix.

**Effort.**  2 hours.

**Reuse.**  Existing `config_help_extract.rs`.

**Author value.**  Low — direct value goes to the
config TUI's help pane, indirect to authors.
Worth doing for completeness.

---

### A5.  Apply prompts-editor critique back into the main TUI's `Ctrl+B 0`

**What it does.**  The prompts-editor TUI lets
the user iterate on prompts.  The same review-
as-you-edit pattern would help on `inkhaven.hjson`
edits in the main app's HJSON editor.  Less
ambitious version: a `Ctrl+...` chord that pings
the LLM with the current HJSON + asks for
review.

**Effort.**  1 day for a basic "send buffer for
review" chord; 2-3 days for full in-TUI AI
pane.

**Reuse.**  AI plumbing.

**Author value.**  Indirect — helps authors who
fiddle with their config but mostly benefits
debugging tooling.  Lower priority than A1-A3.

---

### A6.  Richer pickers (HSL slider for colour, F3 file picker for path)

**What it does.**  Phase 6B in 1.2.10 shipped
basic versions: hex input + RGB swatch for
colour; text input + existence check for path.
Add an HSL slider modal on the colour widget +
wire the main TUI's F3 file picker into the
path widget.

**Effort.**  1 day for HSL slider; 1 day for
F3 integration (the F3 picker code already
exists; mostly plumbing).

**Reuse.**  Existing colour widget + F3 picker.

**Author value.**  Low to moderate — improves
the config TUI UX.  Authors don't tune theme
colours daily.

---

## §B — New low-hanging features for working novelists

Ranked by **effort × value**.  All under one day
each unless noted.

### B1.  Said-bookism detector

**What it does.**  Inline overlay (joins the
`Ctrl+B Shift+F` style-warnings toggle) that flags
overused dialogue tags besides `said`: `muttered`,
`exclaimed`, `whispered`, `shouted`, `growled`,
`hissed`, `barked`, `chuckled`, `snarled`, etc.

The convention in modern fiction is that *said*
disappears for the reader; everything else
demands attention.  Catching the noise inline lets
the author choose deliberately.

**Effort.**  Half a day.

**Reuse.**  Plug into the existing
`style_warnings` family — copy the
`FilterWordsDetector` shape, swap the word list.
Multilingual via per-language config (same
machinery).  New theme colour
`style_warning_said_bookism_fg`.

**Author value.**  High.  Universally cited
craft guidance.  Quick to ship, immediate
impact on prose.

**Recommendation.**  Pick this if §B has time
for one item.

---

### B2.  Adverb-tag detection ("said angrily")

**What it does.**  Specific case of said-bookism:
flag dialogue tags followed (or preceded) by an
`-ly` adverb.  `she said angrily`, `he whispered
nervously`, etc.  Different from the SDT
manner-of-emotion adverbs (which catch the
adverb alone) — this catches *tags + adverbs*
specifically.

**Effort.**  Half a day.

**Reuse.**  Same style-warnings family.  2-gram
detector: `(speech_verb)(adverb_ending_in_ly)`.
Multilingual.

**Author value.**  High.  Strunk-White-canonical
red flag.  Pairs naturally with B1.

---

### B3.  Per-chapter reading time + word count chip

**What it does.**  Status-bar chip when an editor
is open on a chapter: "ch 3 · 4,212 words · ~17
min read".

**Effort.**  2 hours.

**Reuse.**  Hierarchy walk + existing
`count_words`.  Reading-speed constant
configurable (250 wpm default, conservative for
fiction).

**Author value.**  Authors track chapter length
manually now.  This kills that friction.

**Chord / config.**  No chord — passive chip.
Configurable on/off via `editor.reading_chip_enabled`.

---

### B4.  Reading-level (Flesch-Kincaid) modal

**What it does.**  `Ctrl+B Shift+K` ("Kincaid")
opens a modal showing Flesch-Kincaid Grade Level
per chapter / per book.  Authors writing for
middle-grade vs adult genre want this metric
specifically.

**Effort.**  Half a day.

**Reuse.**  Hierarchy walk + sentence splitter
already in `sentence_rhythm.rs` + new syllable-
counter (well-defined hand-roll, ~50 LOC).

**Author value.**  Moderate-to-high.  Genre fit
metric, used heavily by editors.

---

### B5.  Outline view

**What it does.**  `Ctrl+V O` opens a compact
view of the current book showing only typst
headings + paragraph titles.  Like a
table-of-contents you can navigate.  Enter on a
row jumps to that paragraph.

**Effort.**  1 day.

**Reuse.**  Hierarchy + existing tree-pane
rendering.

**Author value.**  High when working on long
books.  The main tree pane already shows the
structure but doesn't surface chapter / scene
headings *within* a paragraph.

---

### B6.  Pomodoro timer

**What it does.**  `Ctrl+B T` ("Timer") starts a
25-minute writing sprint.  Status-bar chip ticks
down.  Soft beep + modal at the end with words
written during the sprint.

**Effort.**  2 hours.

**Reuse.**  Status-bar + progress-cache + sound
hook (1.2.6+).

**Author value.**  Authors love sprints.  Builtin
beats Alt-Tab to a phone timer.

---

### B7.  Word-of-the-day chip

**What it does.**  Pick a low-frequency word
from the project's concordance once per day.
Display on the status bar as a writing-prompt
nudge: "today's word: ·tessellate· (used once)".

**Effort.**  2 hours.

**Reuse.**  Existing concordance.  Cached daily.

**Author value.**  Low-stakes whimsy.  Maybe.
Skip if unclear demand.

---

### B8.  "What did I write today?" diff modal

**What it does.**  `Ctrl+V D` opens a per-paragraph
view showing added/removed lines today (vs the
backup at start-of-day, or a configurable
window).  Like `git diff --since=yesterday` but
author-friendly.

**Effort.**  1 day.

**Reuse.**  Snapshot + progress.db + existing
diff-rendering (`diff_utils.rs`).

**Author value.**  High end-of-session value.
Authors want to see what they actually got done.

---

### B9.  Dialogue density per chapter

**What it does.**  Bar chart per chapter showing
the percentage of lines that begin with a
dialogue marker (`"`, `'`, `«`).  Modal accessed
via `Ctrl+B Shift+D`.  Useful for novelists who
want to balance dialogue / narration.

**Effort.**  1 day.

**Reuse.**  Hierarchy walk + the heatmap
renderer.

**Author value.**  Moderate.  Less universally
useful than B1-B5 but valuable for specific
genres (commercial fiction tends to favour
high dialogue density; literary fiction less so).

---

### B10.  Synonym suggester

**What it does.**  At the cursor inside a word,
`Ctrl+B Shift+S` asks the LLM for three
alternatives.  Result lands as a small modal
near the cursor; arrow keys pick; Enter
replaces.

**Effort.**  1 day.

**Reuse.**  AI plumbing.  Small text-position-
aware modal pattern (new).

**Author value.**  Moderate.  Frequent use-
case (everyone reaches for a thesaurus); but
many writers already have a workflow.

---

## Effort × value summary

| #   | Feature                                  | Effort | Author value | Recommend? |
|-----|------------------------------------------|--------|--------------|------------|
| A1  | AI sentence-rhythm rewrite                | 0.5d   | High         | **YES**     |
| A2  | Concordance jump-to-occurrence            | 0.5d   | High         | **YES**     |
| A3  | Per-language SDT lists                    | 1d     | High (non-EN authors) | **YES** |
| A4  | Doc-comment extractor polish              | 0.25d  | Low          | yes, low cost |
| A5  | Critique-in-main-TUI                      | 1–3d   | Low-medium   | defer       |
| A6  | HSL slider + F3 file picker integration   | 2d     | Low          | defer       |
| B1  | Said-bookism detector                     | 0.5d   | High         | **YES**     |
| B2  | Adverb-tag detection                      | 0.5d   | High         | **YES**     |
| B3  | Per-chapter reading-time chip             | 0.25d  | Moderate     | **YES**     |
| B4  | Flesch-Kincaid modal                      | 0.5d   | Moderate-high| yes         |
| B5  | Outline view                              | 1d     | High         | yes         |
| B6  | Pomodoro timer                            | 0.25d  | Moderate     | yes if quick |
| B7  | Word-of-the-day chip                      | 0.25d  | Low          | skip / later |
| B8  | "What did I write today?" diff modal      | 1d     | High         | **YES**     |
| B9  | Dialogue density chart                    | 1d     | Moderate     | maybe       |
| B10 | Synonym suggester                         | 1d     | Moderate     | maybe       |

## Recommended 1.2.11 batch

Pick the four-or-five highest-value, fastest-to-
ship items.  Recommended slate:

  1. **A1** — AI sentence-rhythm rewrite (0.5d).
     Caps off the 1.2.9 sentence-rhythm work.
  2. **A2** — Concordance jump-to-occurrence
     (0.5d).  Caps off the 1.2.9 concordance
     work.
  3. **B1** — Said-bookism detector (0.5d).
     New always-on style overlay.
  4. **B2** — Adverb-tag detection (0.5d).
     Pairs with B1.
  5. **B3** — Per-chapter reading-time chip
     (0.25d).  Tiny touch with broad value.
  6. **B8** — "What did I write today?" diff
     modal (1d).  End-of-session big-win.

Total: ~3.25 focused days.  Mirrors 1.2.10's
shape (one cycle, six features, mix of "finish
deferred" + "new value").

A3 (per-language SDT lists) is high-value but
needs native-speaker validation — flag as
"contribution welcome" rather than core slate.

## Open questions for the user

  * **Pick the slate.**  Confirm the six above
    or swap items.
  * **Per-language SDT** — happy to ship "common
    register" defaults for Russian / French /
    German / Spanish, or wait for community
    contributions?
  * **B5 outline view scope** — minimum: list
    of paragraph titles per book, ↑↓ navigate,
    Enter to open.  Richer: heading-level
    nesting from typst `= Title` / `== Subtitle`
    / `=== Section` lines extracted from
    paragraph bodies.  Which?
  * **B8 diff window** — "today" = since
    midnight local time?  Since the project
    opened?  Since the last commit?  Configurable?

Pick whichever questions need answers and I'll
draft focused proposals (CONFIG_TUI / PROMPTS_EDITOR_TUI
style) for each chosen item.
