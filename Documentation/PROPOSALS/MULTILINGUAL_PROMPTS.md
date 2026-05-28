# Multilingual prompts — design proposal

Status: research / pre-implementation (drafted during 1.2.11).
Owner: vulogov.
Target release: 1.2.12 (tentative — depends on scope cut).

## Problem

Today every AI flow in inkhaven (`Ctrl+I` critique, `F7` grammar
check, `Ctrl+B Shift+T` show-don't-tell scan, `Ctrl+B Shift+M`
rhythm rewrite, etc.) resolves its prompt through the same three-
layer chain:

1. Paragraph in the project's Prompts system book, matched by slug
   or title.
2. Entry in the project's `prompts.hjson`, matched by name or title.
3. Embedded fallback baked into the binary.

The chain is language-agnostic.  An author writing in Russian who
adds a Russian `grammar-correction` paragraph to their Prompts book
discovers it correctly; an author who hasn't customised anything
gets the embedded English prompt — which the LLM dutifully applies
to Russian prose, with predictable damage.  Mixed-language projects
(Russian manuscript with English helper notes) suffer worst: the
prompt that "wins" depends on which slug landed first, not which
language the paragraph is actually in.

The fix is to make the prompt's language a first-class attribute
across all three layers and to make the resolver prefer same-
language matches over first-match wins.

## Requirements (verbatim from the user)

* Attach a language tag to the prompt, across "Prompts" book,
  `prompts.hjson`, and embedded internal prompts for English,
  Russian, Spanish, German, French.
* Prompts editor must support language tagging.
* `/` prompt picker in the AI prompt input must favour the language
  detected in the current paragraph but let the user open prompts in
  all languages.
* Fallback hierarchy: discover the *correct-language* prompt across
  the three layers — don't stop on a first lexical match if a
  better-language match exists deeper in the chain.
* Language detection: (a) the book's `language` field
  (book-defined, static) or (b) detection on the current paragraph
  (paragraph-detected, dynamic), configurable in HJSON, in
  `inkhaven config`, and at runtime via `Ctrl+B Shift+N`.  Default:
  book-defined.
* The inference language must be visible in the AI pane's
  decoration so the user can confirm what the LLM will see.

## Design

### 1. Language as data — three storage shapes, one wire shape

The wire shape is an ISO 639-1 lowercase string: `en`, `ru`, `es`,
`de`, `fr`.  This is what the resolver compares; everything else
converts to it on entry.

| Source           | How language attaches                                    |
| ---------------- | -------------------------------------------------------- |
| Prompts book     | `lang:<code>` tag on the paragraph `Node.tags`           |
| `prompts.hjson`  | New optional `language: String` field on `Prompt`        |
| Embedded         | Per-language constant tables keyed by `(name, lang)`     |

**Why `lang:<code>` tag for Prompts book paragraphs.**  `Node.tags`
already exists, is persisted by bdslib, survives renames, is
exposed by the project-wide tag UI (`Ctrl+B ]` / `Ctrl+B }`), and
needs no schema migration.  The `lang:` prefix is a convention —
the tag system is plain strings, so we don't need a typed enum
in storage.  Lookup filters by exact tag value (`lang:ru` etc).

**Why `language: Option<String>` on `Prompt`.**  Existing
`prompts.hjson` files keep working — missing `language` means
"untagged" (more on this below).  Adding `Option<T>` to a serde
struct is forward-compatible.

**Why per-language embedded tables.**  Each embedded prompt
(`grammar_check_default_prompt`, `sentence_rhythm_rewrite_default_prompt`,
`show_dont_tell_scan_default_prompt`, etc.) becomes a 5-arm match
on language.  Hand-written English ships today; the four other
variants are seeded via a new bootstrap CLI (see §6) and
committed.  No runtime translation, no LLM call on the hot path.

### 2. The resolver

```
resolve(name, want_lang) → (template, found_lang, source)
```

The function returns *both* the template AND the language it
actually came from, because the AI pane has to display the
inference language and the inference language may not match the
target language (if the user only has an English prompt for the
chosen name, we use it and surface that fact).

Lookup order (the **"discover correct, not first-matching"**
clause from the requirements):

```
PASS 1 — strict, same language:
  a) Prompts book paragraph tagged `lang:want_lang` matching name
  b) prompts.hjson entry with language == want_lang matching name
  c) Embedded table entry (name, want_lang)
  → return on first hit

PASS 2 — soft, untagged:
  a) Prompts book paragraph WITHOUT any `lang:*` tag matching name
  b) prompts.hjson entry without `language` matching name
  → return on first hit

PASS 3 — last-resort, any language:
  a) Prompts book paragraph tagged `lang:<any>` matching name
  b) prompts.hjson entry with language == any matching name
  c) Embedded table entry (name, English)
  → return on first hit; embedded English as the floor
```

Pass 1 is the "correct language" win.  Pass 2 preserves backward
compatibility — every prompt that exists today is untagged, so a
project that ran on 1.2.11 keeps working on 1.2.12 with zero
edits.  Pass 3 ensures we never fail to produce *some* prompt;
the AI pane decoration tells the user "fell back to English" and
the user can fix the Prompts book or `prompts.hjson` to upgrade
the experience.

Within each pass, the existing "slug then title" sub-order is
preserved.

### 3. Language detection

Two modes, exposed as `PromptLanguageMode { BookDefined,
ParagraphDetected }`.

**BookDefined** (default).  Returns `cfg.language` mapped to ISO
639-1: `english → en`, `russian → ru`, `french → fr`, `german →
de`, `spanish → es`.  Trivial, deterministic, zero cost.

**ParagraphDetected.**  Adds a new dep on `whatlang` (~12k SLOC,
character n-gram detector — actively maintained, no network, no
unicode-cldr bloat).  Runs on the live paragraph body.  Caching:

  * Detection runs *lazily* on first need per paragraph load.
  * Result cached on `OpenedDoc.detected_language: Option<String>`.
  * Cache invalidated on any edit that grows or shrinks the body
    by ≥ 50 chars OR on explicit re-detection (e.g. after a
    diff-review accept that replaced the body).
  * If the paragraph has fewer than 50 chars of non-whitespace
    text (a heading-only entry, a one-liner), detection is
    skipped and the resolver falls back to the BookDefined value
    silently.

ISO 639-3 → 639-1 mapping: `eng→en`, `rus→ru`, `spa→es`, `deu→de`,
`fra→fr`.  Anything else means "unsupported" → fall back to book
language.  We don't expose the long tail because the rest of the
SDT / repeated-phrase / concordance pipeline only ships stop-word
+ stemmer plumbing for those five.

### 4. Resolving "what language is the active call?"

```
fn active_prompt_language(&self) -> &str {
    // Runtime override (Ctrl+B Shift+N) trumps HJSON.
    let mode = self.prompt_lang_mode_runtime
        .unwrap_or(self.cfg.editor.prompt_language_mode);
    match mode {
        BookDefined => iso_from_long(&self.cfg.language),
        ParagraphDetected => self
            .opened
            .as_ref()
            .and_then(|d| d.detected_language.as_deref())
            .unwrap_or(iso_from_long(&self.cfg.language)),
    }
}
```

The fall-through in the ParagraphDetected arm is intentional —
when detection is unreliable (short paragraph, no opened doc), we
quietly use the book setting rather than guessing.

### 5. HJSON additions

```hjson
editor: {
  // ... existing fields ...

  // 1.2.12+ — prompt-language resolution.  "book_defined"
  // uses the top-level `language` field; "paragraph_detected"
  // runs whatlang on the live paragraph and falls back to
  // book_defined when the paragraph is too short to be
  // reliable.  Ctrl+B Shift+N toggles at runtime
  // (session-local).
  prompt_language_mode: "book_defined"

  // 1.2.12+ — minimum non-whitespace character count for
  // whatlang to even try.  Below this, paragraph_detected
  // silently falls back to book_defined.  whatlang is
  // unreliable on < 50 chars.
  prompt_language_detection_min_chars: 50
}
```

`Config::editor::prompt_language_mode` becomes an `enum` widget
in the config TUI (`PromptLanguageMode` ValueEnum).  The TUI's
schema metadata table already routes `String` fields to enum
widgets when the type is registered there (same mechanism as
`typst_compile.engine`, `embeddings.model`) — one entry added.

### 6. Embedded prompt content

Each embedded prompt becomes:

```rust
pub fn grammar_check_default_prompt(lang: &str) -> &'static str {
    match lang {
        "ru" => "Проверь грамматику и стиль...",
        "es" => "Revisa la gramática y el estilo...",
        "de" => "Prüfe Grammatik und Stil...",
        "fr" => "Vérifie la grammaire et le style...",
        _ => "Check grammar and style...",  // English floor
    }
}
```

The English variants ship hand-written (already exist today).  RU
/ ES / DE / FR variants are produced by a new CLI subcommand
mirroring the SDT bootstrap:

```
inkhaven prompts bootstrap <lang> [--genre <hint>] [--update]
```

Behaviour is the same shape as `inkhaven show-dont-tell bootstrap`:

  * Without `--update`: print each embedded prompt's translated /
    adapted variant to stdout as an HJSON snippet ready to paste
    under `prompts.hjson`.
  * With `--update`: merge into `prompts.hjson` in place (same
    `config_tui::apply_in_place_edits` helper, same versioned
    backup + atomic write).

The bootstrap is run once per language by the project maintainer
and the output is committed to `assets/default_prompts.hjson` so
every user of inkhaven gets the curated multilingual set by
default.  Users can still override per-project.

Hand-write vs LLM-generate tradeoff: hand-writing five languages
of ~15 prompts apiece is ~75 prompts of effort, much of it
requiring native speakers.  Bootstrap-then-review gets to good-
enough quality in hours, not weeks, and keeps the curation
mechanism reproducible.  Quality concerns:

  * Tone matters more than literal accuracy — a grammar prompt
    needs to instruct the LLM in the target language well enough
    that the response stays on-topic.  LLMs are good at this;
    we'll review for the obvious howlers.
  * Specific terminology (e.g. "show don't tell" → "показывай, не
    рассказывай" is a well-known calque in Russian writing-craft
    circles; "monter sans dire" is less established in French and
    needs care).  Bootstrap output is the starting point, not the
    last word.

### 7. UI changes

#### 7.1 Prompts editor TUI

The four-pane prompts editor (1.2.10+) needs a language
attribute per prompt.  Smallest viable shape:

  * **List pane** (left): show a small language chip at the end
    of each row — `grammar-correction [ru]`.  Untagged prompts
    show `[—]`.
  * **Editor pane** (centre): nothing changes — the prompt body
    is language-agnostic from the editor's perspective.
  * **Metadata footer** (new, single-line below the editor):
    `Language: <picker>` — Tab cycles through `en / ru / es / de
    / fr / (untagged)`.  Hitting Enter commits the change to
    the in-memory `Prompt.language` and marks the buffer dirty
    so `Ctrl+S` writes it back to `prompts.hjson`.
  * **AI pane**: shows the prompt's language alongside the
    template-review feedback, so the reviewer-LLM critique can
    flag a Russian prompt whose body is actually in English.

#### 7.2 `/` prompt picker in the AI prompt input

Today the `/` picker is a single sorted list of all prompts from
both sources.  New shape:

  * **Section headers**: `── In current language (ru) ──` then
    the in-language prompts; `── Other languages ──` then the
    rest.
  * **Inline language chip** per row, same `[ru]` shape as the
    prompts editor.
  * **Filter input**: free-text substring match, applied to
    headword + language code, so the user can narrow with a
    single `ru` keystroke.
  * **No language-only filter mode.**  We don't need a
    dedicated keybinding to "show only Russian prompts" — the
    section split + filter input cover it.

The picker reuses the existing fuzzy-filter widget; the section
split is a render-time grouping, not a data-model change.

#### 7.3 AI pane decoration

The AI pane title bar currently shows `AI`.  Becomes:

```
 AI · ru (paragraph)
```

Pattern:

  * `AI · <lang>` always.
  * `· (book)` or `· (paragraph)` suffix indicates which mode
    produced the language.  When ParagraphDetected falls back
    to book, we show `(book)` — the user sees the mode they
    *got*, not the mode they *requested*.
  * `· English fallback` suffix replaces the mode suffix when
    the resolver landed in Pass 3 (no in-language prompt
    available).  The author sees this and knows to add a
    `grammar-correction` paragraph in their Prompts book or to
    `inkhaven prompts bootstrap <lang> --update`.

#### 7.4 Ctrl+B Shift+N

Toggles `App.prompt_lang_mode_runtime` between
`Some(BookDefined)` and `Some(ParagraphDetected)`.  Status bar
echo: `prompt language mode: paragraph-detected (was
book-defined)`.  Session-local — does NOT write to
`inkhaven.hjson`.  The HJSON value remains the persistent
default; the chord is for quick experimentation.

Mnemonic: N for "natural language" / "language picker".  Chord
is free in the current keybinding table (audited against 1.2.11
chord list).

### 8. Out of scope (deliberately)

  * **Mixed-language paragraphs.**  whatlang returns one
    dominant language; for a paragraph with three lines of
    English narration around two lines of Russian dialogue, it
    picks whichever wins on token count.  We don't try to
    sentence-split or re-detect mid-flow.  The author can flip
    to BookDefined for the project if this matters.
  * **Per-prompt language fallback chains.**  We don't allow a
    prompt to declare "use Russian, but fall back to English if
    Russian is missing".  The resolver's three passes are the
    fallback chain; per-prompt declarations would multiply
    complexity without obvious win.
  * **Translation.**  We never translate prompts at runtime.
    The bootstrap CLI is the only LLM-touching path; everything
    else is pure lookup.
  * **More than five languages.**  Adding a sixth language is a
    code change (embedded table + ISO mapping + stop-word /
    stemmer plumbing).  Not a runtime config.

### 9. Implementation phases

A clean cut into shippable chunks so the feature can land
incrementally without leaving the editor broken on `main`:

**Phase A — foundation.**  No UI-visible change.

  * `Prompt::language: Option<String>` field on the HJSON struct.
  * `lang:<code>` tag convention recognised by
    `lookup_book_prompt_template`.
  * `PromptLanguageMode` enum + `Config::editor::prompt_language_mode`
    field + config-TUI schema entry.
  * `iso_from_long` / `iso_to_long` helpers in `src/ai/prompts.rs`.
  * Resolver rewrite: `resolve_prompt_template_or` → new
    `resolve_prompt(name, want_lang)` returning `(template,
    found_lang, source)`.  Every existing call site updated.
  * Wire whatlang dep; `OpenedDoc::detected_language` field;
    detection on paragraph load.
  * Unit tests: all three passes, untagged-as-fallback,
    paragraph-too-short skip path.

**Phase B — embedded content + bootstrap CLI.**

  * Promote every existing embedded prompt to a 5-arm match.
  * `inkhaven prompts bootstrap <lang> [--update]` subcommand.
  * Run bootstrap for ru / es / de / fr against a working model;
    commit results to `assets/default_prompts.hjson`.

**Phase C — UI.**

  * Prompts editor TUI: language chip in list, picker in footer.
  * `/` picker: sectioned list with inline language chips.
  * AI pane title decoration.
  * `Ctrl+B Shift+N` runtime toggle.

**Phase D — polish.**

  * Surface `prompt_language_mode` in the config TUI's enum
    table (auto-flows from the Phase A schema entry).
  * Cache-invalidation hooks for `OpenedDoc.detected_language`
    on edit / diff-apply / TTS re-read.
  * Documentation: `KEYBINDING.md` row for `Ctrl+B Shift+N`,
    release notes section, brief author-facing note in the
    Help system book.

Each phase is its own PR / commit; main stays green between
them.

### 10. Risks + open questions

  * **whatlang accuracy on author prose.**  Most reliable
    on 100+ char paragraphs.  Mitigation: 50-char floor (§3),
    book-language fallback when below.  Open: test against a
    sample of real Russian / French manuscripts before
    finalising the floor.
  * **Tag pollution.**  Adding `lang:<code>` tags grows the tag
    cloud.  Mitigation: the existing tag-filter UI already
    handles 100+ distinct tags fine.  Open: should the
    project-wide tag picker hide `lang:*` tags as "system"
    tags?  Probably yes — they're an implementation detail of
    the prompt resolver, not author-facing metadata.
  * **Embedded prompt size in binary.**  Five languages × ~15
    prompts × ~500 chars ≈ 40 KB.  Negligible for a TUI
    binary that already includes typst-compile.
  * **`prompts.hjson` migration.**  The `language` field is
    `Option<String>` so loading a 1.2.11 file works untouched.
    Saving via the prompts editor would emit the new field
    only when set.  No on-disk migration needed.
  * **Embedded prompt quality.**  See §6 — bootstrap output is
    the starting point.  Need a review pass before commit; the
    SDT bootstrap experience suggests this is doable.

### 11. Recommendation

Phases A + B are the load-bearing parts: they unblock everything
else and ship value (correct-language prompts for non-English
projects) on their own.  Phases C + D are polish that the user
will notice but can wait if the cycle runs short.

Suggest Phase A lands first in a quiet 1.2.12 cycle; Phase B
follows once the bootstrap CLI is verified against a real
project; C + D land together in the same release as B since the
UX changes are most useful when there are real per-language
prompts to choose between.
