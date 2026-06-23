# Inner Socrates reference (examined authorship)

The Socratic interrogator (RFC INNER_SOCRATES-1), introduced in the 1.3.x tree.
It reads alongside you and surfaces **questions** about your prose — the
assumptions it treats as given, the framings it presupposes, the tensions inside
it, what each scene does for the work. It is **non-prescriptive by structural
commitment**: every finding is a question, never a correction. You examine your
prose; Inner Socrates never edits it.

This is the third piece of Inkhaven's *examined authorship* triad, alongside the
[ConLang Suite](CONLANG.md) (languages examined) and
[the world simulator](WORLDBUILDING.md) (a world examined). It shares their
infrastructure — the Output pane, the multilingual baseline, the ledger pattern,
the Fast/Slow split — so a user fluent in one finds this familiar.

Design rationale: [`PROPOSALS/INNER_SOCRATES-1_PLAN.md`](PROPOSALS/INNER_SOCRATES-1_PLAN.md).

> **The spine.** If a surface would say "the prose should be X", it does not ship.
> Inner Socrates makes one claim: *you have written something; here are questions
> a careful reader would ask about it.*

## The two tracks

| | Fast track | Slow track |
|---|---|---|
| **Engine** | deterministic patterns + the language detector (no LLM) | the configured LLM |
| **Trigger** | a writing pause (ambient, opt-in) or a chord | manual (`--slow`) or idle, cost-capped |
| **Scope** | one paragraph | a paragraph, or the whole timeline |
| **Provider** | none required | required |
| **Languages** | EN/RU/ES/FR/DE | the paragraph's language + English fallback |

Both consult the active **Reader Persona** and the **intent ledger** before
emitting, and both surface to the **Output pane** as `socratic_inquiry` messages.

## The categories

**Fast (7, deterministic):**

| Category | Asks about |
|---|---|
| Asserted Necessity (`modal_claims`) | an outcome treated as inevitable |
| Hedging (`hedged_uncertainty`) | authorial hedging |
| Pattern (`structural_patterns`) | a run of same-opening or same-length sentences |
| Speaker (`unattributed_dialogue`) | a run of dialogue with no speaker tag |
| Length (`sentence_length_anomalies`) | a very long sentence |
| Tense Shift (`tense_voice_shifts`) | a slip between past and present (EN) |
| Reference (`pronoun_ambiguity`) | a pronoun with two possible antecedents (EN) |

**Slow — prose (5, LLM):** Hidden Assumption · Internal Tension · Framing ·
Significance · Echo.

**Slow — timeline (3, LLM):** Dramatization Gap · Implication · Temporal Density
— read the prose against the project's timeline of events.

## Severity

Three levels, mapped onto the Output pane's `info` / `warning` / `contradiction`:

- **Notice** — a surface observation; **hidden by default**.
- **Inquiry** — a question that invites reflection; the bulk of output, visible.
- **Probe** — a structural question about the work; rare, always visible.

The default visible threshold is **Inquiry** — quiet by default.

## Reader Personas

A persona is a distinct careful-reader perspective. Its per-category **emphasis
weights** scale salience (`0.0` mutes a category). Five ship bundled:

| Persona | Voice |
|---|---|
| **Inner Socrates** (default) | "Every question opens what the prose has closed." |
| **The Careful Editor** | "Notice what the prose is doing — to itself and the reader." |
| **The Skeptical Reader** | "What's not being said is often louder than what is." |
| **The First-Time Reader** | "Pretend you've read nothing of this book before this scene." |
| **The Slow Reader** | "The rhythm of prose is doing something. What?" |

Author your own as an HJSON file in `~/.config/inkhaven/personas/` (cross-project)
or `<project>/books/intent/01-personas/` (project-only); project wins over user
wins over bundled. A persona file:

```hjson
{
    id: "my-skeptical-grandmother"
    name: "My Skeptical Grandmother"
    voice_summary: "She has read everything and believes none of it."
    voice_notes: "You are warm but unfooled. You ask about what the prose hopes you won't notice."
    emphasis: {
        framing_interrogation: 1.5
        assumption_surfacing: 1.3
        hedged_uncertainty: 0.0     // mute this category
    }
}
```

`inner-socrates persona list | show <id> | activate <id>`; in the TUI,
`Ctrl+B J → S` cycles the active persona.

## The intent ledger

The author's declared deliberate choices — the prose counterpart of the world
simulator's *magic ledger*, with the same vocabulary (Entry, Kind, Coverage,
Scope, lazy Consultation, Suppression, Promotion). A matching entry **suppresses**
a would-be finding instead of nagging.

- **Kinds:** `deliberate_ambiguity`, `framing_choice`, `structural_echo`,
  `stylistic_choice`, `deliberate_temporal_ambiguity`.
- **Scopes:** project, chapter, paragraph range, character, scene, **timeline
  range**.
- **Coverage:** which Socratic categories the entry may suppress.

Entries accumulate two ways: you declare them, or the **promotion mechanism**
suggests one after you dismiss the same kind of finding repeatedly. `inner-socrates
suggestions list` shows the patterns; `suggestions promote <category> [--chapter
<id>]` turns one into an entry that then suppresses it.

Carry series-level intentions to the next book with the **`.isl` bundle**:
`inner-socrates bundle export [--scope-level series] [--out …]` and `bundle import
<path>`.

## The Slow track — cost & coherence

`inner-socrates check --slow` (and the idle auto-check, `Ctrl+B J → S`… see the
overview) call the configured provider. Reusing the world simulator's machinery,
each call prints a **cost preflight** (estimated tokens + the day's tally),
refuses a call over the per-call soft cap (`--max-cost`, default 6000; `--force`
overrides), enforces a daily ceiling, and retries transient errors with backoff —
all tracked under a separate `inner_socrates_llm_usage` sub-budget. A missing
provider degrades to a notice; the Fast track still surfaces.

`inner-socrates timeline` runs the three timeline categories over the project's
events — silently doing nothing when there's no timeline.

## Surfaces

### CLI

```
inkhaven inner-socrates check [--text "…" | --paragraph <id>] [--slow] [--max-cost <n>] [--force]
inkhaven inner-socrates timeline [--max-cost <n>] [--force]
inkhaven inner-socrates ledger
inkhaven inner-socrates persona list | show <id> | activate <id>
inkhaven inner-socrates suggestions list | promote <category> [--chapter <id>] | dismiss <category>
inkhaven inner-socrates bundle export [--scope-level series|project|all] [--out <path>]
inkhaven inner-socrates bundle import <path> [--conflict skip|override]
```

### TUI — `Ctrl+B J`

| Key | Action |
|---|---|
| `Ctrl+B J` | the Inner Socrates overview (active persona, recent questions, ledger) |
| → `F` | fast-check the open paragraph → Output |
| → `L` | view the intent ledger |
| → `S` | cycle the active persona |
| → `A` | toggle the ambient auto-check (off by default — it runs on a writing pause) |

(`Ctrl+B I` is book-info; the Socratic family lives on `J`.) Findings reach the
**Output pane**; dismissing one there (`d`) feeds the promotion mechanism.

## Compatibility

**Zero new dependencies** — everything is inherited from the Output pane and the
world simulator (the language detector, the cost/retry helpers, the ledger
pattern, the background-job harness). Non-breaking and opt-in: nothing runs unless
you invoke it (or enable the ambient check). The Intent system book (the 13th)
seeds on project init.

## See also

- [`WORLDBUILDING.md`](WORLDBUILDING.md) — the world simulator (the sibling
  examined-authorship pillar, and the magic ledger this ledger mirrors).
- Design: [`PROPOSALS/INNER_SOCRATES-1_PLAN.md`](PROPOSALS/INNER_SOCRATES-1_PLAN.md).
