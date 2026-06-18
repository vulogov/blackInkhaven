# EDITORIAL-3 — Finish the Editorial Pass (1.3.9)

_The Editorial Pass shipped its **navigator** (1.3.6) and its **actor**
(1.3.7 — `f` streams an AI rewrite of the located paragraph). Two cuts left
four deferrals on the table. 1.3.9 closes them, taking the cockpit to its
final 1.3 form before the 1.4 Whole-Book AI Editor swing._

## The four open deferrals

From the 1.3.7 / 1.3.8 release notes, all explicitly held back:

1. **Span-scoped rewrite** — `f` rewrites the *whole* paragraph; rewriting
   only the flagged phrase (and splicing it back) is the refinement. Every
   localized finding already carries a `location.char_range`.
2. **Filter-words** as an editorial category — surfaced the same way
   show-don't-tell was (1.3.7 P1), held back "to keep the worklist's
   signal-to-noise high."
3. **Live anachronism overlay** — the `AnachronismDetector` feeds `inkhaven
   edit`; wiring it into the *live* editor underline (theme colour + the
   style-warning pass) is the refinement. `highlight.rs` already has the
   `StyleWarningKind::Anachronism` arm (borrowing the show-tell colour).
4. **Batch "fix all of category X"** — `f` is one finding at a time.

## Guiding constraints

- **Zero new dependencies.** Everything reuses shipped machinery: the
  `rewrite → diff → snapshot` flow (`start_editorial_rewrite`,
  `pending_rewrite_diff`, `Modal::AiDiffReview`), the `style_warnings`
  detector substrate, the `char_range` already on every prose-style finding.
- **No panic surfaces; atomic writes; snapshot-gated.** Every applied
  rewrite still snapshots the paragraph first (the original is one `F6`
  away). Batch never blind-applies — each step is its own diff review.
- **Signal-to-noise first.** Filter-words land at `Info` severity (they sort
  last) and only when the detector is configured on.

---

## P0 — Span-scoped rewrite (the keystone)

Today `start_editorial_rewrite(category)` sends the whole paragraph body and
replaces the whole paragraph from the AI's reply. For a localized finding
(show-tell / anachronism / filter), rewriting one phrase is sharper and far
less likely to disturb good prose around it.

**Mechanism (reusing the existing diff flow end-to-end):**

- Generalize the pending-rewrite state: alongside `pending_rewrite_diff:
  Option<String>` (the snapshot annotation), add `pending_rewrite_span:
  Option<(usize, usize)>` — the paragraph-relative char range to splice into.
  `None` = the existing whole-paragraph replace; `Some(range)` = span splice.
- A `fix_spec` gains a `scope: FixScope { Paragraph, Span }` field (or a
  parallel `span_builtin`): for span categories the prompt sends the
  surrounding sentence/paragraph as **context** but marks the target phrase
  with sentinels (`«…»`) and asks for ONLY the replacement phrase back,
  markup-preserving, no preamble.
- On inference completion (`pump_inference`), when `pending_rewrite_span` is
  `Some((a, b))`, splice: `chars[..a] + reply.trim() + chars[b..]`, then hand
  the **whole new paragraph** to `Modal::AiDiffReview` exactly as today — so
  `a` (snapshot + replace) / `r` (reject) are unchanged.
- Pure, unit-tested `splice_span(original, range, replacement) -> String`
  (char-indexed, clamps out-of-range, preserves trailing newline).

The cockpit's `f` already resolves `(category, pid)`; it now also forwards
the selected finding's `char_range`. Echo / pacing stay paragraph-scope
(their `FixScope` is `Paragraph`) — there's no phrase to isolate.

**Deliverable:** `f` on a show-tell or anachronism finding rewrites just the
phrase; the diff still shows the full paragraph for context. Tests for
`splice_span` + `FixScope` resolution.

---

## P1 — Filter-words joins the worklist

The last live-overlay detector not yet in `inkhaven edit`.

- `paragraph_filter_word_findings(text, pid, chapter, det)` in
  `cli/editorial.rs` — the exact sibling of `paragraph_show_tell_findings`:
  one finding per flagged word, with its paragraph-relative `char_range`.
  Category **`filter`**, **`Severity::Info`** (sorts after errors/warnings —
  protects signal-to-noise), source `"style"`, message `filter word: "very"
  — consider cutting`.
- Run the `FilterWordsDetector` in `prose_style_findings` (it's already
  built from `cfg.editor.style_warnings.filter_words`); skip when
  `det.is_empty()`. No new detection — the detector already ships.
- `fix_spec("filter")` returns a **span-scoped** `FixSpec` ("cut or replace
  the filter word; if cutting leaves the sentence intact, return it without
  the word") → rewritable via P0. Because it's span-scoped, the fix touches
  only the offending word.

**Deliverable:** `inkhaven edit` surfaces filter words (Info), jumpable and
`f`-fixable; `--only filter` narrows to them. Pure mapper test.

---

## P2 — Live anachronism overlay

The detector feeds the worklist; now it underlines as you type, like
filter-words / show-don't-tell already do.

- Add a theme colour `style_warning_anachronism_fg` (config + `theme`
  table + manual `Default`), a distinct hue from show-tell's `#94e2d5`
  (proposed `#f9e2af` amber — "wrong era" reads as caution).
- Run the `AnachronismDetector` in the live editor style-warning pass
  (wherever `FilterWordsDetector` / `ShowDontTellDetector` already run
  per visible line), emitting `StyleWarningKind::Anachronism` hits.
- `highlight.rs`: point the existing `Anachronism` arm at the new colour
  (it currently borrows show-tell's).
- Off until `anachronism.year` is set (the detector is already `is_empty()`
  without a year) — a contemporary novel underlines nothing.

**Deliverable:** with a setting `year`, "wristwatch" underlines amber live in
the editor; toggles with the existing style-warnings master switch.

---

## P3 — Batch fix-all of a category

`f` is one finding at a time. Add `F` (Shift+F) in the cockpit: enqueue every
**rewritable** finding in the current filter and walk them.

- A small queue on the app (`editorial_fix_queue: Vec<(Uuid, String,
  Option<(usize,usize)>)>` — pid, category, span). `F` fills it from the
  filtered, rewritable findings.
- Drive it through the **existing** per-item diff review — **no blind
  apply**: pop one → open paragraph → `start_editorial_rewrite` (span-aware
  from P0) → on `a`/`r` in `AiDiffReview`, auto-advance to the next in queue
  (or close when drained). Each step still snapshots.
- Status line shows progress (`editorial batch: 3 of 7 · a accept / r skip /
  Esc stop`); `Esc` in the diff review abandons the rest of the queue.

**Deliverable:** `F` on a filtered category (e.g. all show-tell) walks each
rewrite with the normal accept/reject gate. Queue-drain logic unit-tested
where it's pure.

---

## P4 — Docs + 1.3.9 release cut

- Refresh **Tutorial 68** (the Editorial Pass) with span-scoped `f`,
  filter-words, batch `F`, and the live anachronism underline; note the new
  theme key.
- **KEYBINDING.md**: `F` batch-fix in the cockpit.
- **CONFIGURATION.md**: `theme.style_warning_anachronism_fg`.
- **RELEASE_NOTES/1.3.9.md** + index row; top **README** "Latest release".
- Version `1.3.9-dev` → `1.3.9` (+ Cargo.lock); full `cargo test`.
- Signed commit + tag `v1.3.9` (verify "Good signature"); `cargo publish`;
  merge `--no-ff` into `main`; open the next dev cycle.

---

## Out of scope (carryovers)

- **The Whole-Book AI Editor** (RAG over the manuscript) — the 1.4 headline.
- **Embedding-based divergent-description / drift** check (the semantic
  consistency layer) — strong 1.4-adjacent candidate.
- PDF N-up / booklet impose presets; CMYK-JPEG grayscale; ePub inline
  images + popup footnotes; sixth supported language; TUI `edit --deep`
  trigger.

## Phase order

P0 (span rewrite) is the keystone — P1 (filter) and P3 (batch) both lean on
it; P2 (overlay) is independent and can land any time. Sequence: **P0 → P1 →
P2 → P3 → P4**.
