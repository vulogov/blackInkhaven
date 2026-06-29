# Tutorial 100 — The Startup Haiku

*Inkhaven 1.4.17*

Every time Inkhaven opens, you sit down to make something. The blank-page moment is real. HAIKU-1 puts
a small, perfectly-formed poem in the Output pane to meet it — and gives the pane its first piece of
*good news* (until now it spoke only in warnings, findings, and task completions).

It is deliberately the simplest feature in Inkhaven: **no AI, no network, no generation.** All 25
poems — five per language, EN/RU/DE/FR/ES — are hand-written and baked into the binary. The haiku is
there in the first millisecond of startup, before the database journal replays, even on an airgapped
machine.

## Three moments

- **Startup.** One haiku lands in the Output pane when the project opens.
- **A new manuscript paragraph.** The instant a blank paragraph appears in a *user book* is the nakedest
  creative moment of the session — a fresh haiku meets it there. (Only real prose paragraphs: not
  Characters/Places/Notes entries, not Dictionary/Sources/Glossary/Snippets/Threads, not structural
  or template paragraphs.)
- **On demand — `Ctrl+Z p`.** When you want the pocket of stillness mid-session without making a
  paragraph, press `Ctrl+Z p` (P for Poem). The status bar confirms `✦ haiku written to Output`; your
  focus doesn't move.

The poem is in the book's language (`editor.language`); an unsupported language falls back to English.
A rotation counter advances on every trigger, so startup shows one poem, the first new paragraph the
next, and so on — you rarely see the same one twice in a session.

## What you'll see

In the Output pane, a `✦ haiku` row with the three lines beneath it:

```
✦ haiku
   Blank page catches light —
   the sentence that wants writing
   waits inside the pen.
```

`Lifetime::Session(1)` means only the most recent haiku ever lives in the pane — a new one quietly
replaces the last, so they never pile up.

## Two tiers of theme

Each language carries three **Tier A** poems (the manuscript itself — ink, the blank page, the first
sentence, a draft saved at midnight) and two **Tier B** poems (the writer's surroundings — rain on the
glass, a lamp's small circle, the pause between two words). The Tier A poems sit in the first rotation
slots, so your first encounters are always about the writing; the quieter, atmospheric ones emerge
later. A haiku about frost on the window isn't about writing — but it puts you in exactly the state
good writing requires: present, awake, noticing.

## Turning it off

If you'd rather open to silence, set it off:

```hjson
{ editor: { startup_haiku: false } }
```

That silences the startup and new-paragraph moments. The `Ctrl+Z p` chord still works whenever you
want one.

A small thing. But a writing space should feel like one from the first second.
