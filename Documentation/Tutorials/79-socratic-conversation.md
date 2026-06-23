# Tutorial 79 — A revision pass with Inner Socrates: the conversation and its outcomes

*Inkhaven 1.3.29+ (RFC INNER_SOCRATES-1)*

[Tutorial 78](78-inner-socrates.md) introduced Inner Socrates — the interrogator
that surfaces **questions** about your prose. This one is a workflow: how to *use*
those questions in a revision pass. You'll talk a paragraph through with a Reader
Persona, then resolve each question deliberately — declare it intended, write it
down, or mark it handled — so your Output pane reflects decisions, not a pile of
unread notices.

The whole loop is non-prescriptive: the persona discusses *with* you; it never
drafts your prose, and nothing here changes a word you didn't change yourself.

## 1. Surface the questions

Open a paragraph you're revising. Fast-check it from the Inner Socrates hub —
**`Ctrl+B J`**, then **`F`** — and the questions appear in the **Output pane**
(`Ctrl+B Tab` to look):

```
◇ Inquiry [Asserted Necessity] This passage treats an outcome as inevitable
   ("had to"). What alternatives did you decide to leave out?
◇ Inquiry [Framing] The scene is told entirely from the regent's vantage. What
   does that framing emphasize, and what does it leave unseen?
```

(For the deeper questions — hidden assumptions, internal tensions — add `--slow`
on the CLI, or let the Slow track run; see Tutorial 78.)

## 2. Talk it through — the conversation

When a question deserves more than a glance, open a **conversation**. Two ways in:

- **`Ctrl+B J → C`** from the hub, or
- cycle the AI pane's scope with **F9** until it reads **Socrates**.

Either seeds the AI pane with the active persona's voice and the paragraph's
questions, and hands you the prompt:

```
┌─ AI · Scope: Socrates · Inner Socrates ────────────────────────────┐
│ Inner Socrates: A few things to consider here. The regent's        │
│ declaration is presented as inevitable. What alternatives existed   │
│ that you decided to leave out?                                      │
│                                                                     │
│ > _                                                                 │
└─────────────────────────────────────────────────────────────────────┘
```

Type back. The persona is a careful interlocutor — it presses, clarifies, and
asks; it will not write the scene for you. Switch the reader with **`Ctrl+B J →
S`** and re-open the conversation to hear the same paragraph through the Skeptical
Reader or the Slow Reader. **F9** again cycles the scope back out when you're done.

## 3. Resolve each question — the outcomes

A question you've thought about shouldn't linger. Select it in the Output pane and
press one key:

| Key | Outcome | What it does |
|-----|---------|--------------|
| **`i`** | record as intent | Declares the category a deliberate choice in this chapter — writes an intent-ledger entry and **stops asking** that question here. |
| **`m`** | make note | Turns the question into a **Socratic Notes** entry (quoting the question and the passage, with room for your response). |
| **`x`** | mark addressed | You've revised the prose; clear it. |
| **`d`** | dismiss | Set it aside — and after five dismissals of the same kind, Inkhaven offers to declare it for you. |

Each outcome removes the message from the queue once it's handled.

### Worked example

You decide the regent's fatalism is a deliberate motif. Select the *Asserted
Necessity* finding and press **`i`**:

```
declared intent — Asserted Necessity won't be re-asked here
```

Inkhaven has written a `stylistic_choice` entry to the intent ledger, scoped to
this chapter. Re-checking the paragraph (or any other in the chapter) no longer
raises that question — the interrogator now respects your declared choice, the same
way the world fact-checker respects the magic ledger.

The framing question, though, you want to keep thinking about. Press **`m`**:

```
made a note in Notes / Socratic Notes
```

A new entry now lives under **Notes → Socratic Notes**, quoting the question and
the passage, with an empty *My response:* section waiting for you. The thinking is
captured where you'll find it on the next pass.

## 4. Let the ledger build itself

You don't have to declare every intention by hand. As you dismiss findings, the
**promotion mechanism** watches for patterns. Dismiss five *Framing* questions in a
chapter and Inkhaven suggests declaring it:

```
$ inkhaven inner-socrates suggestions list
  framing_interrogation (5×, chapter ch12) → propose `framing_choice`

$ inkhaven inner-socrates suggestions promote framing_interrogation --chapter ch12
declared intent … covering framing_interrogation — the interrogator will respect it
```

Over a draft, the ledger fills with the choices you actually made — and Inner
Socrates grows quieter about exactly the things you've already examined.

## What you learned

- **`Ctrl+B J → F`** surfaces a paragraph's questions into the Output pane.
- **`Ctrl+B J → C`** (or **F9 → Socrates**) opens a conversation with the active
  persona — it discusses, never rewrites; **`Ctrl+B J → S`** changes the reader.
- In Output, resolve each question deliberately: **`i`** record as intent · **`m`**
  make a Socratic note · **`x`** mark addressed · **`d`** dismiss.
- Recording intent suppresses the question from then on; the promotion mechanism
  builds the ledger from your own dismissals.

Back to: [Tutorial 78 — Examined authorship with Inner Socrates](78-inner-socrates.md).
