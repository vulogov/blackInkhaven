# Tutorial 106 — Voice & Style at Book Scale

*Inkhaven 2.1 (CHORUS)*

Tutorial 95 profiled your **narrator's** voice. CHORUS profiles the rest: does
each *character* sound like themselves and distinct from the others, and does the
manuscript keep its discipline — one POV per scene, a consistent tense, a stable
register? All deterministic and advisory — CHORUS measures and reports, it never
edits your prose.

## Are your characters distinct?

```sh
inkhaven chorus voices
```

```
Character voices — "The Drowned City" [en]
◆ Mara             confidence high · 214 utterance(s)
    sentence length (median)   7 words
    rhythm variety (CV)        0.42   (cast +0.05)
    lexical diversity (MATTR)  0.78   (cast +0.03)
◆ Joren            confidence high · 190 utterance(s)
    …
Distinctiveness (4 comparable voices)
  ⚠ Mara ≈ Joren  (distance 0.31) — these read alike
    closest pair:  Mara ↔ Joren  (0.31)
```

Each character's dialogue is profiled with the *same* metric engine as the
narrator, then the **distinctiveness matrix** flags any two voices that read
alike — the classic revision-stage flaw. Sparse speakers are profiled but never
flagged (CHORUS won't judge a voice it can't measure). Deliberate look-alikes
(twins, a chorus) are silenced with `chorus.distinct_ignore_pairs: ["Mara|Joren"]`.

## Discipline — POV, head-hops, tense, register

```sh
inkhaven chorus scan
```

```
POV / head-hop (advisory)
  ch.3 · scene 2 (POV Mara)
      ⚠ Joren's interiority leaks — not the scene's POV
Tense discipline (advisory; EN/DE/FR/ES, Russian excluded)
  ch.5 · scene 1 (dominant: past)
      ⚠ present-tense slip: "She is at the door now, waiting."
Register & diction (advisory, vs. chapter 1)
  ⚠ ch.7  contraction_rate rose to 0.061 (ch.1 0.012, Δ +0.049)
```

A **head-hop** is a named character other than the scene's POV shown accessing
their own inner life. Declare a scene's POV with a paragraph tag to silence false
positives:

| Tag | Meaning |
| --- | ------- |
| `pov:Mara` | single POV — anyone else's interiority is a leak |
| `pov:first` | first person — any *named* character's interiority leaks |
| `pov:omniscient` | deliberately multi-POV — head-hop off |

**Tense covers EN/DE/FR/ES; Russian is excluded by design.** Russian narrative tense is aspect (the historical
present and perfective/imperfective interleaving are legitimate), so CHORUS says
"not analysed" rather than false-flagging a Russian manuscript. Character voice
and head-hop *do* work in Russian.

## The Inner Stylist — the coach

The seventh inner-family reader synthesises all of the above into a few grounded
observations:

```sh
inkhaven chorus stylist
```

```
Inner Stylist — "The Drowned City" [en]
  ✓ [distinctiveness] 4 comparable voices, all distinct — nobody reads like anybody else.
  ⚠ [pov] ch.3 scene 2: Joren's interiority leaks — not the scene's POV Mara.
  · [register] ch.7: register drifts — contraction_rate rose to 0.061 (ch.1 0.012).
        key: register:contraction_rate:7  ·  silence with `chorus stylist --suppress register:contraction_rate:7`
```

- **`chorus stylist --coach`** turns the findings into grounded LLM coaching
  (*"I notice…"*, never a rewrite).
- **`chorus stylist --suppress <key>`** silences a finding for good (persisted in
  `inner_stylist.db`).
- **`chorus report`** is the one-screen dashboard: narrator + cast + distinctiveness + Stylist.

## In the editor

The Inner Stylist rides the **`Ctrl+B Shift+C`** review pass (its observations
land in the Output pane), and the family hub **`Ctrl+B J → Y`** runs it on demand.

---

**See also:** [CHORUS.md](../CHORUS.md) · Tutorial 95 (narrative voice) ·
Tutorial 97 (dialogue quality) · `inkhaven chorus --help`.
