# Tutorial 63 — The Facts book and AI fact-checking

*Inkhaven 1.2.21+*

Early on you collect loose material in the **Notes** and **Research**
system books — plot ideas, source clippings. But the *invariants* of your
world are a different kind of thing: it's winter in the southern reach; the
capital is three days' ride inland from Port Sael; the war ended a
generation ago. These are the ground truth every chapter must stay
consistent with — and 1.2.21 gives them a dedicated home: the **Facts**
system book, plus two AI tools that treat it as authoritative.

## 1. Collect your facts

Every project now has a **Facts** book in the tree, alongside Notes and
Research. It's free-form prose — add a paragraph per fact (or per topic),
the same way you'd add a Note:

```
Tree
├─ My Novel            (your manuscript)
├─ Notes
├─ Research
├─ Facts            ← new in 1.2.21
│   ├─ Climate
│   ├─ Geography
│   └─ Chronology
└─ …
```

Write them however reads naturally — one fact per paragraph or a themed
cluster:

```
= Climate

The Sael basin is equatorial: hot and wet year-round, no winter.
The monsoon runs June–September. Snow exists only on the Tagh peaks,
two weeks' travel north.
```

There's no schema and no special syntax — if you can write a Note, you can
write a Fact.

## 2. Interrogate your world — the Facts scope (`F9`)

The AI assistant's scope cycles with **F9**:
`None → Selection → Paragraph → Subchapter → Chapter → Book → Facts → None`.

Cycle to **Facts** and the AI pane seeds a fact-analysis session: every
paragraph of your Facts book is loaded as a visible chat prologue, and the
model is framed as a fact-checker grounded in them.

```
AI · scope=Facts · infer=Local
────────────────────────────────
⟦Facts⟧ Established world facts (3 entries):
 ── Facts / Climate ──
 The Sael basin is equatorial…
 ── Facts / Geography ──
 The capital sits three days' ride inland…
 ── Facts / Chronology ──
 The war ended in 1411…
────────────────────────────────
AI: Loaded the world facts. Ask me to check any
    passage against them, or paste prose to
    fact-check.
```

Now just talk to it: *"If my hero rides from Port Sael to the capital
overnight, is that plausible?"* — the model answers grounded in your facts
(*"No — you've established three days' ride; overnight contradicts that"*).

The Facts scope is **sticky**: unlike the other scopes (which reset to
`None` after one prompt), it stays active across follow-up questions so the
whole conversation keeps the facts in view. Cycle **F9** away when you're
done. The seed is loaded once — re-entering or reloading the project never
duplicates it.

## 3. Check a paragraph — the fact-check chord (`Ctrl+B Shift+X`)

When you're writing and want a quick check of the paragraph in front of
you, press **`Ctrl+B Shift+X`**. It:

- **locks the scope to the local paragraph** — the check is about *this*
  prose, never a whole-book send; and
- **grounds against the Facts book** — every established fact is sent as
  reference, so the model flags any claim that contradicts your world.

```
$ … your open paragraph …
  "Helena pulled her furs tight against the
   capital's first snow, two days after leaving
   the coast."

Ctrl+B Shift+X →

AI: 2 contradictions found.
    • "the capital's first snow" — contradicts
      Facts/Climate: the basin is equatorial, no
      winter; snow only on the Tagh peaks.
    • "two days after leaving the coast" —
      contradicts Facts/Geography: the capital is
      three days' ride inland.
    Verdict: 2 contradictions
```

The verdict streams into the AI pane — read it, fix the prose, move on. If
the Facts book is empty, the chord still runs a generic local fact-check
(no grounding).

## 4. Scan the whole manuscript — `inkhaven facts scan`

`Ctrl+B Shift+X` checks one paragraph; when you want to sweep the *whole*
book, run the CLI scan. It walks every chapter, retrieves the facts
relevant to each (semantic search, so a 300-fact book stays cheap), asks
the model for contradictions, and writes them to a sidecar:

```text
$ inkhaven facts scan
inkhaven facts scan · language: English · model: gemini-2.5-flash · 18 chapter(s) · 24 fact(s)
  [1/18] Arrivals → 0 contradiction(s)
  [7/18] The Crossing → 2 contradiction(s)
  …
facts scan: 3 contradiction(s) across 18 chapter(s) → .inkhaven/facts_scan.json
```

- `inkhaven facts list` re-prints the last scan without re-running it
  (so you don't pay for the LLM calls twice).
- `inkhaven facts scan --json` (or `facts list --json`) emits the report
  for a CI gate — fail the build if `findings` is non-empty.
- `--provider <name>` overrides the LLM for the run.

It's the same grounding as the chord and the F9 scope — the established
facts are the reference, the model flags what the prose contradicts. This
is an AI pass (it costs LLM calls), so it's a CLI command rather than one
of the offline `doctor --scan` classes.

## Multilingual

Both tools are multilingual. The fact-analysis system prompt, the seed
framing, and the fact-check instruction ship in English, Russian, German,
French, and Spanish, selected by the project's `language` field (or the
per-paragraph detection mode — see
[Tutorial 47](47-multilingual-prompts.md)). As with every AI flow, you can
override the prompt by adding a `fact-check` paragraph to your Prompts book
or a `fact-check` entry to `prompts.hjson`.

## See also

- [Tutorial 07 — Places and characters](07-places-and-characters.md): the
  *entity* lexicon books (Characters / Places / Artefacts), which highlight
  words in the manuscript — Facts is deliberately different: free-form
  prose, no overlay.
- [Tutorial 59 — Revision & continuity](59-revision-and-continuity.md): the
  `continuity-drift` scan, which tracks *character* facts across chapters;
  Facts is for *world* invariants.
- [Tutorial 05 — AI writing assistant](05-ai-writing-assistant.md): scopes
  (F9), inference modes (F10), and the chat pane this builds on.
