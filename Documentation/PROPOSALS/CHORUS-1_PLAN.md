# CHORUS-1 — Voice & Style at Book Scale (RFC)

*The 2.1.0 flagship. Status: RFC. Nothing built.*

## Summary

Inkhaven already measures the **narrator's** voice book-wide (NARR-1: `inkhaven
prose` — sentence rhythm, lexical diversity, hedging, interiority, sensory
balance, active/passive, plus `prose drift` across chapters). What it has never
measured is the **cast** — whether each character sounds like themselves and
distinct from the others — nor whether the manuscript keeps its **discipline**
(a single POV per scene, a consistent tense, a stable register). CHORUS is that
layer.

> **NARR-1 profiles the narrator. CHORUS profiles the cast, enforces the rules of
> the game, and coaches.**

Three measurement pillars over one new advisory reader:

- **A — Character Voice.** Each character's dialogue, profiled with the *same*
  metric engine NARR-1 uses for the narrator; a **distinctiveness matrix** that
  flags characters who read identically; per-character voice **drift** across the
  arc.
- **B — POV & Tense Discipline.** Per-scene point-of-view with **head-hop**
  detection (interiority attributed to a non-POV character) and **tense-slip**
  detection — the classic craft errors no tool in-tree catches.
- **C — Register & Diction.** A register axis (formality / contraction /
  latinate-vs-germanic / archaism) tracked per chapter, plus anachronistic-diction
  consistency for period settings.

- **The Inner Stylist** — a new seventh inner-family reader that reads all three
  and returns advisory, grounded coaching (Praise / Note / Concern), never a
  rewrite; plus a book-scale **Style Report**.

CHORUS is overwhelmingly **assembly over existing substrate** — the metric core,
the dialogue attribution, the POV heuristic, the interiority marker lists, the
anachronism-detector shape, the review-pass rails, and the inner-family reader
template all exist. It adds **no new runtime crates**.

---

## What already exists (so CHORUS builds up, not sideways)

Being honest about the substrate is the whole reason this RFC is grounded — the
naive version of "voice at book scale" is mostly already shipped:

| Capability | Where | CHORUS relationship |
| ---------- | ----- | ------------------- |
| Narrator voice profile (CV, burstiness, MATTR, modal/interiority/sensory/passive) | NARR-1, `src/prose/` (`VoiceProfile`, `compute_profile_with`) | **Reuse the metric core per-character.** |
| Narrator voice **drift** across chapters | `inkhaven prose drift`, `src/cli/prose.rs` | **Reuse the delta+threshold shape per-speaker.** |
| Dialogue **attribution** (speaker → line) | DIALOG-1, `src/dialogue/` (`DialogueStore::certain_spans`) | **The character-line corpus Pillar A groups over.** |
| A lightweight **character fingerprint** (utterance length, MATTR, question/exclamation/hedge) | `CharacterDialogueFingerprint`, `src/dialogue/fingerprint.rs` — *already reuses NARR-1's `mattr`/`modal_unigrams`* | **Extend to the full profile + add the distinctiveness matrix + drift it can't do today.** |
| Entity-description consistency ("cramped" vs "airy" tavern) | `src/drift.rs` (WORLD-2) | Adjacent; CHORUS does *voice*, not *description*. |
| POV chip (most-mentioned character, per paragraph) | `src/tui/pov_tracker.rs` | **The only POV signal; Pillar B lifts it to per-scene + violation detection.** |
| Interiority / free-indirect markers per language | `src/prose/lexicon.rs` (EN/RU/DE/FR/ES) | **Reuse for head-hop detection.** |
| Anachronism detector (config-year term list) | `src/tui/style_warnings.rs` (`AnachronismDetector`) | **Clone the shape for register/diction word-lists.** |
| Per-paragraph craft (echo, filter, show-tell, pacing) | `src/editorial.rs`, `src/tui/style_warnings.rs` | Sibling detectors; CHORUS is *book-scale voice*, they are *local craft*. |
| Inner-family reader template (fast/slow/ambient/`.db`/`Ctrl+B J`) | `src/inner_poet/`, `src/inner_theologian/` | **The Inner Stylist is a copy of this shape.** |

The load-bearing fact: **`compute_profile_with(text, scope, lang, lexicon,
deep, mattr_window)` takes arbitrary text** — it is not hardwired to the
hierarchy — so "profile one character's aggregated dialogue" is a re-export and a
`group_by(speaker)` away.

---

## The model

CHORUS speaks in one vocabulary:

- A **voice** is a `VoiceProfile` (the NARR-1 struct) computed over some text.
  The narrator's voice is the whole book (exists). A **character's voice** is
  their aggregated attributed dialogue (new). A voice can be sliced per chapter
  to observe **drift**.
- A **distinctiveness** is the distance between two voices in a normalized
  feature space. Two characters are *indistinguishable* when their distance falls
  below a (configurable, genre-relative) threshold **and** both have enough
  dialogue to be measured with confidence.
- A **discipline finding** is a violation of a rule the manuscript is presumed to
  keep: a head-hop (interiority attributed to a non-POV character in a
  single-POV scene), a tense slip (a narrative sentence whose tense breaks the
  established one), a register break.
- A **coaching observation** is what the Inner Stylist says about all of the
  above — advisory, grounded in the measured numbers, in the book's language,
  and never a rewrite.

Everything is **advisory**: CHORUS measures and reports; it never edits prose,
and (per the AI-advisory principle) the LLM coaching stream writes to the
Thoughts pane, never to the manuscript.

---

## Pillar A — Character Voice

**Goal.** Answer three questions no in-tree tool answers: *Does each character
have a distinct voice? Do any two sound identical? Does a character's voice hold
across their arc?*

**How.**
1. **Corpus.** Reuse DIALOG-1 attribution: group every attributed dialogue line
   by speaker across the book (`DialogueStore` + a new unfiltered book-wide
   getter). Each character → one text blob (and per-chapter sub-blobs for drift).
2. **Fingerprint.** Run `compute_profile_with` — the *same* engine as the
   narrator — over each character's blob, yielding a full `VoiceProfile` (rhythm,
   diversity, hedging, sentence-type mix, characteristic tics). This supersedes
   the lightweight `CharacterDialogueFingerprint` with the real metric set while
   keeping its cheap fields as a fast summary.
3. **Distinctiveness matrix.** Build a normalized feature vector per character,
   compute pairwise distance, and surface: the most- and least-distinct
   characters, and any pair below the indistinguishable threshold. This is the
   headline finding — *"Mara and Joren read identically"* — a top revision-stage
   flaw with no existing detector.
4. **Per-character drift.** Reuse the `prose drift` delta+threshold shape, sliced
   by speaker: does Mara's voice in Act I match Act III?

**Honest limits.** Dialogue is short and sparse; a character with a handful of
lines cannot be fingerprinted. CHORUS reports a **confidence** (from utterance
count / word count) and refuses to flag low-confidence voices. Statistical
distance is a *prompt to look*, not a verdict — twins, siblings, or a
deliberately-uniform chorus may read close on purpose.

---

## Pillar B — POV & Tense Discipline

**Goal.** Catch the two classic discipline errors — **head-hopping** and **tense
slips** — that survive line edits because they are structural, not local.

**The honest constraint (decided up front).** There is **no morphological, POS,
or tense analysis anywhere in the tree** — the only parser is a Typst
tree-sitter grammar for markup. Every prose check in inkhaven is
lexicon/heuristic. So Pillar B is **heuristic by necessity**, and it must say so.

**POV.**
- Introduce a **per-scene POV** notion — a scene's POV is either **declared**
  (a `pov:CharacterName` scene tag, opt-in and authoritative) or **inferred**
  (the per-scene extension of the existing mention-count POV heuristic). First
  person is handled by a `pov:first` / `pov:I` declaration since the narrator
  isn't in the character lexicon.
- **Head-hop detection.** Reuse the per-language interiority marker lists. When
  an interiority marker (`X thought`, `ей казалось`) resolves to a character who
  is *not* the scene's POV, flag it. The new work is the **marker → subject →
  name** linkage the tree doesn't do today (nearest-name resolution within the
  marker's clause, with confidence). Advisory; deterministic fast track +
  optional AI adjudication for ambiguous clauses (the `drift.rs` pattern).

**Tense.**
- A **heuristic verb-surface** tense-slip detector: establish the scene's
  dominant narrative tense, then flag sentences whose finite-verb surface breaks
  it. Cloned in spirit from the passive-voice and anachronism heuristics.
- **The Russian gate.** Russian narrative tense is governed by *aspect*, and the
  historical present + perfective/imperfective interleaving are legitimate
  devices, not errors — a naive past→present flag is *wrong* for Russian, and
  nothing in-tree models aspect. The tense check therefore **gates to English**
  (and, cautiously, DE/FR/ES), and **explicitly excludes Russian**, exactly as
  NARR-1's language-sensitive metrics return `None` for unsupported languages.
  This is stated in the UI, not hidden. (See "Multilingual", below.)

---

## Pillar C — Register & Diction

**Goal.** Track the narrator's **register** so drift becomes visible — "the prose
gets casual in Act III" — and flag diction that breaks a period setting.

**How.** Extend NARR-1 with a **register axis** computed per chapter: contraction
rate, formality (a lexical formal/informal ratio), latinate-vs-germanic diction
balance, archaism density — cheap, language-keyed word-list metrics folded into
the existing `prose refresh`. For period settings, reuse the config-driven
`AnachronismDetector` shape with **register word-lists** (archaic / anachronistic
diction) rather than a single year. Register drift rides the existing `prose
drift` threshold machinery.

**Honest limit.** Register is the fuzziest axis; it ships as a *trend signal*
(is it moving?) not an absolute judgment, and it inherits NARR-1's "what it is
not" humility.

---

## The Inner Stylist (the seventh reader)

The three pillars **measure**; the Inner Stylist **coaches** — it is the member
of the inner family (`inner_editor`, `inner_socrates`, `inner_theologian`,
`inner_poet`, `inner_rigor`, `inner_grounding`) that owns voice & style.

- **Fast track (deterministic, offline).** Reads the character distinctiveness
  matrix, the discipline findings, and the register track, and emits structured
  **Praise / Note / Concern** to the Output pane (kind `stylist`) on the unified
  review-pass rails (`Ctrl+B Shift+C`) and the `inkhaven style scan` CLI.
- **Slow track (engage, LLM).** `Ctrl+B J → Y → E` composes the measured numbers
  into a grounded coaching prompt and streams observations into the Thoughts
  pane — *"Six distinct voices except Mara ≈ Joren; the narrator's rhythm
  flattened ~30% across Act II; two POV leaks in ch.14."* It **observes, never
  rewrites** (the poet/theologian system-prompt discipline).
- **Ambient.** Opt-in auto-scan of the open scene's discipline as you write
  (the poet ambient pattern).
- **Persistence.** Its own `inner_stylist.db` (findings + suppressions), per the
  one-db-per-reader convention.

Plus a book-scale **Style Report** — `inkhaven style report` and a TUI modal —
one dashboard unifying the narrator profile, the distinctiveness matrix, the
POV/tense discipline findings, and the register track: the book's voice health
at a glance.

---

## Multilingual

CHORUS keys everything off the project language, and is honest about where a
language isn't supported — the "does it work in Russian?" test is applied to
every pillar:

- **Pillar A** rides NARR-1's metric core, which already computes Tier-1
  (rhythm/diversity) for *all* languages and returns `None` for the
  language-sensitive metrics on unsupported languages. Character voice therefore
  works in every language (rhythm/diversity), with the language-sensitive axes
  filling in for EN/RU/DE/FR/ES. **Russian character voice works.**
- **Pillar B / POV & head-hop** reuses the per-language interiority marker lists
  (EN/RU/DE/FR/ES all present) — **head-hop works in Russian.**
- **Pillar B / tense** is the one honest exclusion: it **gates to English**
  (cautiously DE/FR/ES) and **excludes Russian** by design, because Russian tense
  is aspect and nothing models it. The UI says so plainly rather than
  false-flagging.
- **Pillar C** uses language-keyed register word-lists (EN first; others as the
  lists are built), degrading to "not available for this language" rather than
  guessing.
- **The Inner Stylist** follows the family idiom: an English prompt template with
  a "write your observations in {language}" directive, and deterministic
  word-lists selected by `lists_for(lang)`.

---

## Principles

- **Advisory, never editing.** CHORUS measures and coaches; it never touches the
  manuscript. The LLM stream writes to the Thoughts pane. (The AI-advisory
  principle.)
- **Cost informs, never blocks.** The Inner Stylist's slow track carries a
  per-session budget that is *informative* — the permissive principle. The fast
  track and all measurement are free and offline.
- **Humble by construction.** Statistical voice ≠ literary voice. Every surface
  states its confidence and its limits; CHORUS inherits NARR-1's "what it is
  not."
- **No new crates; warning-free; the 1.2.15 bar.**

---

## What CHORUS is *not*

- Not a style *corrector* — it flags, it never rewrites.
- Not a grammar checker — there is no parser; the tense check is an honest
  heuristic, English-gated.
- Not a plagiarism/AI-detector or a "good writing" score — it measures
  *consistency and distinctiveness*, not quality.
- Not a replacement for NARR-1 — it is its cast-and-discipline complement.

---

## Phases

The grounded, file-by-file plan is in
[`CHORUS-1_IMPL.md`](CHORUS-1_IMPL.md): **CH-P0** substrate seams → **P1**
character fingerprints → **P2** distinctiveness matrix → **P3** per-character
drift → **P4** POV/head-hop → **P5** tense (English-gated) → **P6**
register/diction → **P7** the Inner Stylist reader → **P8** the Style Report →
**P9** capstone (Bund/config/cost/docs/book). CH-P1+P2 are the value core; the
critical-path risk is CH-P5's language gate, resolved here in the RFC.
