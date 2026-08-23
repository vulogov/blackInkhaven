# BONDS-1 — The Relationship Reader (a 3.1.0 flagship RFC)

*The book already watches its facts (SENTINEL), its knowledge (KEN), its voices
(CHORUS), and its reader's experience (LECTOR). It does not watch its
**relationships**. BONDS is the eighth reader: it checks whether the bonds
between characters — the alliances, rivalries, loves, betrayals — are actually
**earned on the page**, or merely asserted and then forgotten.*

*Codename open, as REDLINE's and KEN's were: BONDS / TIES / KINSHIP. "BONDS" below.*

---

## 1. The problem

Every continuity instrument inkhaven has watches a **thing about one entity or
one fact**: is this character in two places at once (SENTINEL), could this
character know this yet (KEN), does this character's voice hold (CHORUS), does
this character's arc stall (CHAR). None of them watches the **relationship
between two characters as it moves across the book** — and in ensemble,
romance, betrayal, and political fiction, the relationships *are* the plot.

The relationship is also the single easiest thing to lose across a long draft.
The writer holds it in their head — "by the end, Mara and Kell hate each other" —
and the head is a liar: it remembers the *intent* of the arc, not whether the
scenes that turn it were ever written. You can finish a draft in which two
characters are sworn allies in act one and mortal enemies in act three with **no
scene in between where they are on the page together** — and never notice,
because you *know* why they fell out. The reader doesn't. That gap is invisible
to every tool inkhaven ships today.

## 2. The value — the case, made honestly

BONDS is a **KEN sibling**, and that framing is the whole design. It is not a
relationship *visualiser* (a pretty graph is not a reader and catches nothing).
It is a **declared-then-checked** instrument, exactly like KEN's `know:`/`secret:`
contract: the writer *declares* a bond, inkhaven *derives* the on-page reality
for free, and the **mismatch is the finding**.

**a. It catches a mistake, not a vibe.** Every BONDS finding is a concrete,
locatable defect the writer can act on or dismiss — never "consider the
relationship here." Four of them:

- **`unwritten_bond`** — a declared bond with ~zero on-page co-presence. *"You've
  established Mara and Kell as old friends, but they share almost no scenes. The
  friendship is asserted, not dramatised."* (told-not-shown, for relationships)
- **`unearned_shift`** — *the flagship catch.* A declared bond's **state changes**
  across chapters (ally → enemy, stranger → lover) with **no shared scene** to
  turn it. *"Ch. 3 they're allies; ch. 9 they're enemies — and they never share
  the page in between. The reader saw the before and the after, never the pivot."*
  This is SENTINEL/KEN's unearned-transition invariant, applied to people — the
  relationship equivalent of a plot hole.
- **`dropped_bond`** — a central bond dormant for a long stretch, then resurfacing
  as if always live. *"The Mara–Kell rivalry goes dark for eight chapters, then
  drives the climax."* (THREADS dormancy, for relationships)
- **`implied_cooling`** *(opt-in `--deep` LLM only)* — the subtle one: the tone
  between two characters cools (or warms) without the text acknowledging it. The
  `implied_irony` analog — a per-pair, cost-capped LLM read, never a whole-book pass.

**b. It's a NATIVE finding — one a generic editor-AI cannot produce.** The
unearned-shift catch requires holding, at once: the declared relationship states
(from the writer's tags), the timeline of *which characters shared which scenes*
(from `TlEvent.characters` + KEN's scene walk), and the chapter ordering. No
paste-a-chapter-into-ChatGPT workflow has that whole-book, structured substrate.
This is the moat, and it exists **only because the 2.x arc already built the
timeline, the roster, and the scene walk** — BONDS is almost pure reuse.

**c. It's CHEAP — ≈ $0 at any book size.** The deterministic core
(unwritten/unearned/dropped) sends **nothing** to a model: it's set-matching over
declared tags and derived co-presence, both already local. Cost scales with the
number of *declared bonds* (a handful), not pages. The only LLM touchpoint is the
opt-in `--deep` `implied_cooling` pass — one small per-pair call, under the daily
cost cap — and even that is off by default.

**d. It's SILENT when it should be.** On a solo-protagonist survival story with no
declared bonds, BONDS finds nothing and says so. It earns its keep on the books
where relationships carry the weight and stays out of the way on the others.

**The honest limitation (stated, not buried).** BONDS's *sharp* catches depend on
the writer **declaring** relationships with `rel:` tags — the same discipline KEN
asks with `know:`/`secret:`. Without tags, the deterministic findings go quiet and
only weak co-presence observations remain. This is the declared-then-checked
contract by design: it is what makes the findings precise instead of vibes, and it
is the price of admission. BONDS is for the writer who will spend two minutes
tagging the bonds that matter — and gets, in return, a guarantee that every one of
them is paid off on the page.

## 3. The mechanism (declared · derived · checked)

- **Declared** — `rel:<kind>:<A>:<B>` tags, placed in the manuscript the way
  `know:` tags are. `<kind>` is the bond's current state (`ally`, `rival`,
  `lover`, `enemy`, `kin`, `mentor`, …); `<A>`/`<B>` are the two characters. A tag
  placed in ch. 3 declares the state *there*; a differently-`kind`ed tag later
  declares a **transition**. The Characters/bible layer may declare a baseline
  bond once; per-chapter tags declare its evolution.
- **Derived** — for every scene, the set of characters present, straight from
  `TlEvent.characters` + the KEN scene walk (`src/ken/walk.rs`). Free — no new
  corpus, no embeddings, no LLM.
- **Checked** — order the declared states by reading position; for each declared
  bond and each transition, ask the deterministic question (enough shared scenes?
  a shared scene at the pivot? a long silent gap?) and emit the finding on mismatch.

## 4. How it plugs into the reader family (not a silo)

BONDS reuses four pieces of shared machinery, so from the writer's chair it is
"one more instrument on the dashboards you already use," not a new app:

1. **The declared substrate** — `rel:` tags parse through the same prefixed-tag
   path as `know:`/`secret:`; free-form strings, no schema change.
2. **The evidence substrate** — co-presence comes from the same
   `TlEvent.characters` + scene walk that KEN uses for presence and SENTINEL for
   co-location. BONDS builds nothing new.
3. **The one worklist** — a confirmed BONDS finding becomes an `EditorialFinding`
   (`source: "bonds"`) that flows through `collect()` into the **`Ctrl+V Shift+R`
   Editorial Pass / REDLINE** queue, getting the same **Rewrite / Decision / Brief**
   treatment as any other reader's finding, through the same confirmed-diff
   contract. `unearned_shift` promotes to a **Decision** (a real authorial call);
   the rest to a **Brief**. When a BONDS finding lands at the same chapter as a
   SENTINEL or LECTOR finding, the writer sees them together — instruments
   converging on one spot.
4. **The same surfaces** — a `Ctrl+V Shift+O` dashboard modal (mirroring KEN's
   ledger and CHRONICLE's), `ink.bonds.{findings,check,ties}` Bund words (like
   `ink.knowledge.*`), a `bonds:` config stanza, and the CLI `inkhaven bonds
   check [--json] [--deep]`.

**It complements every existing reader without overlapping any:** CHAR watches one
character's arc, BONDS the dyad between two; KEN tracks *knowledge*, BONDS *bonds*
(same engine, different declared axis); SENTINEL uses co-location as a *violation*
check ("not in two places at once"), BONDS uses co-presence as *evidence* ("they
shared a scene to earn the turn"); the Inner readers stay paragraph-level craft,
BONDS is book-scale structure.

## 5. Cost invariant (recorded, load-bearing)

The deterministic core (import tags, derive co-presence, check, promote) is **$0
API at any book size** — it never sends the manuscript to a model. The only LLM
call is the opt-in `--deep` `implied_cooling` read, which is **per declared pair**,
cost-capped under the daily budget, and off by default. **Design invariant: BONDS
must never run a whole-book "judge the relationships" pass.** Any future
semantic corroboration stays per-pair and cost-capped — never book-wide. (This is
the same invariant KEN records for its `--deep` pass.)

## 6. Boundaries / non-goals

- **Not a visualiser.** No relationship graph render as the deliverable — a graph
  catches nothing. (A read-only graph *view* could come later, but the value is
  the findings.)
- **Not a generator.** BONDS observes and reconciles; it never writes prose. It
  proposes findings; the writer confirms fixes through REDLINE's existing
  confirmed-diff contract. (`feedback_ai_advisory` holds.)
- **Not inference of undeclared bonds.** BONDS checks the bonds the writer
  *declares*; it does not guess relationships from prose (that's the fuzzy,
  low-moat trap). The opt-in `--deep` pass is the only place it reads tone, and
  only for already-declared pairs.
- **Silent without tags** — by design.

## 7. Surfaces at a glance

| Surface | Handle | What |
|---|---|---|
| CLI | `inkhaven bonds check [--json] [--deep] [--strict]` | run the reader; `--strict` = non-zero exit on any finding (CI gate) |
| Dashboard | `Ctrl+V Shift+O` | the relationship ledger — findings + jump-to-anchor, mirroring KEN's `Ctrl+B Shift+Z` |
| Worklist | `Ctrl+V Shift+R` | confirmed findings join the unified Editorial Pass |
| Bund | `ink.bonds.findings` / `.check` / `.ties` | read-only (`store_read`); the `--deep` LLM pass stays CLI-only |
| Config | `bonds:` stanza | thresholds (min shared scenes, dormancy window, cost cap) + `rel:` kinds |

## 8. Phasing

Mirrors the KEN arc (BONDS is smaller given the reuse). Value core = P1+P2+P3.
See `BONDS-1_IMPL.md` for the file-by-file, grounded plan.

- **BD-P0** — model + `rel:` tag grammar (`src/bonds/mod.rs`; `RelTag`, `Bond`, `BondFinding`).
- **BD-P1** — gather: parse `rel:` tags (declared) + derive per-scene co-presence (reuse KEN walk + `TlEvent.characters`).
- **BD-P2** — check (THE CORE): the deterministic `unwritten_bond` / `unearned_shift` / `dropped_bond` invariants → `BondFinding`s. Pure, testable.
- **BD-P3** — promote: `from_bonds_finding` + a `source = "bonds"` block in `collect`; `response_kind` routing (unearned_shift → Decision, rest → Brief).
- **BD-P4** — surfaces: CLI `bonds`, the `Ctrl+V Shift+O` dashboard modal, a keymap-shadow-guard-clean binding.
- **BD-P5** — Bund `ink.bonds.*` (+ policy row + `WORD_REFERENCE.md` row for the doc-rot guard) + `bonds:` config.
- **BD-P6** — `--deep` `implied_cooling` LLM pass (opt-in, per-pair, cost-capped).
- **BD-P7** — capstone: `BONDS.md`, a tutorial, KEYBINDING/CONFIGURATION/FEATURE_INDEX, README, `RELEASE_NOTES/3.1.0.md`, e2e. Multilingual: `rel:` kinds + roster names are language-agnostic; the `--deep` prompt keys off project language.

---

*Links: KEN [[ken-1-rfc]] (the template — walk, gather, check, deep, dashboard,
Bund, collect bridge), REDLINE [[redline-1-rfc]] (the promotion target), SENTINEL
[[continuity-intelligence-2.2]] + CHAR (the sibling continuity readers it
complements), the dismantled READERS-1 [[readers-1-rfc]] (unrelated — human
beta-feedback, do-not-build). Feature gate [[project_blackinkhaven]];
observe-never-generate [[feedback_ai_advisory]]; multilingual bar
[[feedback_multilingual]].*
