# Tutorial 58 — Reading pace: the chip + the preview

*Inkhaven 1.2.18+*

Two small 1.2.18 features help you feel how a *reader*
experiences your prose, not just how it looks on the
editing screen: a status-bar **reading-time chip** and a
**reader-pace preview** teleprompter.

Both read at `editor.reading_wpm` (default 200 — the
common silent-reading average; ~150 is a typical
audiobook narration pace).

```hjson
{
  editor: {
    reading_time_chip: true   // default false
    reading_wpm: 200          // default 200
  }
}
```

## The reading-time chip

Turn it on with `editor.reading_time_chip: true`.  A
chip appears in the status bar showing the current
book's read-aloud length + the time remaining from the
open paragraph to the book's end:

```text
… [POV: Helena] [12C·8P·3A] 📖 18m / 4h18m  …
                            └─ remaining ─┘ └ total ┘
```

* **total** — the whole book at `reading_wpm`.  A quick
  proxy for audiobook length before you commit to a
  full `inkhaven audiobook` synthesis run.
* **remaining** — from the open paragraph (inclusive) to
  the book's end.  Walk through the manuscript and watch
  it count down.

The chip is off by default — the status bar is already
busy — so opt in when the metric is useful (e.g. when
you're targeting a specific audiobook length or a
magazine word budget).

It's cheap to compute: one linear walk of the current
book's paragraphs, summing the cached per-paragraph word
counts (the 1.2.18 hierarchy index keeps it O(n), so it
stays snappy even on a 10K-paragraph project).

## The reader-pace preview

`Ctrl+B Shift+E` (in the editor) opens a teleprompter
that advances a highlight word-by-word through the open
paragraph at `reading_wpm`:

```text
 ┌ Reader pace ──────────────────────────────────────┐
 │                                                    │
 │  Helena paused at the threshold, listening for     │
 │  the sound of [footsteps] below.  Marcus said      │
 │  nothing for a long moment, then nodded once…      │
 │                                                    │
 │ 14/210 · 58s left @ 200 wpm · Space pause · ←→     │
 │ step · r restart · Esc close                       │
 └────────────────────────────────────────────────────┘
```

Already-read words dim, the current word is highlighted,
upcoming words are normal.  The highlight moves at
reading speed — so you *experience* the prose the way a
reader will, instead of skimming it at editing-glance
speed.

### Why bother?

Pacing problems hide when you read your own work fast.
A sentence that drags, a beat that lands too abruptly, a
paragraph that's secretly twice as long as it feels —
these surface when the words arrive at a reader's
tempo.  It's the prose equivalent of reading aloud,
without needing to read aloud.

### Controls

| Key | Action |
|-----|--------|
| `Space` | Pause / resume |
| `→` | Step the highlight forward one word |
| `←` | Step the highlight back one word |
| `r` | Restart from the top |
| `Esc` | Close |

The elapsed time carries across pause/resume cycles, so
you can stop to fix a clunky sentence and pick up where
you left off.

The preview reads clean prose — the same markup-stripping
pass the audiobook export uses, so headings, emphasis
markers, and footnotes don't interrupt the flow.

## Tuning the pace

`reading_wpm` drives both features (and the per-chapter
timing in the audiobook export).  Pick the rate that
matches what you're checking:

* **200** — silent reading (e-book, print).
* **150** — audiobook narration.
* **300** — a skim-reader / fast reader.

## See also

* [Tutorial 57 — Reader-experience exports](57-reader-experience-exports.md)
  — the ePub + audiobook exports the reading-time
  estimates pair with.
* `Documentation/RELEASE_NOTES/1.2.18.md` — R.3 + R.4
  implementation log.
