# EDITORIAL-2 — Close the loop: AI rewrite-in-place (1.3.7)

_Status: planning. Target: **1.3.7**. The Editorial Pass (1.3.6) *finds*
every problem and *jumps* you to it — but the fix is still all manual.
1.3.7 turns the cockpit from a navigator into an actor: `f` on a
rewritable finding streams an AI rewrite of the located passage, which you
accept (snapshot-gated), reject, or edit._

## Why

The cockpit drops your cursor on a flat sentence, an echo cluster, an
overlong paragraph — and then you're on your own. For the prose-level,
paragraph-scoped findings, the AI can propose the fix. Inkhaven already has
the whole rewrite→diff→snapshot machinery (the 1.2.11 sentence-rhythm
rewrite, the 1.2.9 show-don't-tell rewrite): a rewrite streams, an
`AiDiffReview` modal pops on completion, and accept snapshots the paragraph
then replaces it. 1.3.7 points that machinery at the cockpit's findings.

## Builds on (already in tree)

- **The cockpit** (1.3.6) — `Modal::EditorialPass`, findings carrying a
  resolved `location.paragraph` (the jump target).
- **The rewrite→diff→snapshot substrate** —
  `start_sentence_rhythm_rewrite` (fires a rewrite over the open paragraph,
  flags `pending_rhythm_rewrite`), `pump_inference` (pops `AiDiffReview` on
  completion), `apply_ai_diff_accepted` (snapshot-annotate + replace),
  `extract_rewrite_text`. The 3-tier language-aware `resolve_prompt`.
- **The existing prose detectors** — the `style_warnings.show_dont_tell`
  regex + the filter-word list (already an editor overlay), and the
  `echo` / `paragraph-too-long` doctor classes (already editorial findings).

## What's actually rewritable

Most editorial findings are author judgment (structure, continuity, a fact
contradiction) — there's no single-paragraph rewrite for "the midpoint
sags." The **paragraph-scoped, mechanically-addressable** ones are:

- **echo** — rewrite the paragraph to vary the over-repeated word.
- **pacing / paragraph-too-long** — tighten an overlong paragraph.
- **show-don't-tell** — rewrite a telling passage to show (the marquee — but
  it isn't yet an `edit` finding; P1 surfaces it).

The rest stay jump-only. A `✎` marks the rewritable findings in the cockpit.

## Dependencies

**None.** The rewrite reuses the existing inference + diff + snapshot
machinery; the new detectors (P1) reuse regex detection that already ships
as an overlay.

## Phases

### P0 — the rewrite loop (`f` in the cockpit)

`editorial::fix_spec(category) -> Option<FixSpec>` — for the rewritable
categories (echo, pacing), a 3-tier-resolvable prompt slug
(`editorial-fix-<category>`) + a built-in rewrite instruction (multilingual,
like the rhythm rewrite). Pure + unit-tested. `EditorialFinding::rewritable()`
gates the `✎` marker + the `f` action.

In the cockpit, **`f`** on a rewritable finding: open its paragraph
(`open_paragraph_by_uuid`), then `start_editorial_rewrite(category)` — a
sibling of `start_sentence_rhythm_rewrite` that composes the category prompt
over the paragraph body, fires the inference, and flags the pending-rewrite
diff (generalized from `pending_rhythm_rewrite`). On completion the existing
`AiDiffReview` pops: **`a`** snapshots + replaces, **`r`** rejects, **`e`**
leaves you in the editor. Closes the loop for the findings already in the
worklist.

### P1 — broaden the rewritable surface

Surface the existing deterministic prose detectors that aren't yet editorial
findings, so there's more to `f`:

- **show-don't-tell** — run the `style_warnings.show_dont_tell` regex over
  each paragraph in `collect()` and emit `EditorialFinding`s
  (category `show-tell`, severity info) with a paragraph + `char_range`
  location (the span of the telling phrase). Now jumpable + rewritable.
- **filter-words** (optional) — the same, for the filter-word list
  (`realised`, `seemed`, `began to`).

No new detection logic — the regex already ships as an overlay; this exposes
it in the worklist. The `char_range` also sharpens jump-to-location (land on
the phrase, not just the chapter).

### P2 — docs + the 1.3.7 release cut

Tutorial 68 update (the `f` rewrite loop + the new categories); KEYBINDING
(the cockpit `f`/`a`/`r`/`e`); finalize `RELEASE_NOTES/1.3.7` + index +
README; version bump `1.3.7-dev → 1.3.7`; signed tag `v1.3.7`;
`cargo publish`; merge to main; open the next cycle.

## Non-goals (deferred)

- **Span-scoped rewrite** — P0 rewrites the whole paragraph; rewriting only
  the `char_range` span (and splicing it back) is a refinement for later.
- **Rewriting judgment findings** — no AI rewrite for structure / continuity
  / fact / weak-scene; those stay jump-only (the fix is yours).
- **Batch "fix all of category X"** — one finding at a time this cycle.
- **The Whole-Book AI Editor** — still the 1.4 headline.

## Test posture

P0's `fix_spec` prompt composition + `rewritable()` are pure and
unit-tested. P1's show-don't-tell → `EditorialFinding` mapping (regex hit →
finding with the right category + `char_range`) is pure and tested over
synthetic prose. The TUI rewrite wiring reuses the already-shipped
`AiDiffReview` accept/snapshot path; covered by keybind-regression +
render-smoke tests as in 1.3.6.
