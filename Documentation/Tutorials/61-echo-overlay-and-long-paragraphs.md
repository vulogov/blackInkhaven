# Tutorial 61 — Live echo overlay and over-long paragraphs

*Inkhaven 1.2.20+*

[Tutorial 59](59-revision-and-continuity.md) introduced the
`echo-repetition` doctor scan — a distinctive word reused too close
together (*she **walked** … he **walked** … they **walked***). That scan
runs on demand and reports at the chapter level. This tutorial covers the
two 1.2.20 additions that move the same craft signal *into the editor, as
you write*:

- the **live echo overlay** (`Ctrl+B Shift+K`), and
- the **`paragraph-too-long`** doctor class.

## The live echo overlay

The overlay is the inline companion to the `echo-repetition` scan, in the
same family as the style-warning overlays from
[Tutorial 43](43-show-dont-tell.md). Toggle it with **`Ctrl+B Shift+K`**:

```
Echo overlay ON
```

With it on, every occurrence in the *open paragraph* of a word that echoes
across nearby paragraphs of the chapter is underlined — so you catch a
reused distinctive word at the keystroke, not at scan time.

```
The harbor was       The harbor was
quiet. She        →  quiet. She
walked to the        w̲a̲l̲k̲e̲d̲ to the
rail and walked      rail and w̲a̲l̲k̲e̲d̲
back.                back.
```

### How it works

The hard part of an echo signal is that it's *cross-paragraph* — unlike a
filter word, you can't decide it from the current line alone. The overlay:

1. finds the chapter the open paragraph belongs to,
2. gathers that chapter's paragraphs (the open one read **live**, so your
   unsaved edits count),
3. runs the same `echo::detect_echoes` pass the scan uses, and
4. underlines the words whose echo window covers the open paragraph.

It's cheap to leave on: sibling paragraphs are re-read only when you
navigate to a different paragraph, and the echo pass is skipped entirely
while the open text is unchanged (a content-hash check) — so there's **no
per-keystroke disk or store I/O**.

### It has its own colour

The echo overlay paints in `theme.style_warning_echo_fg` — a muted purple
(`#b48ead`) by default, deliberately **distinct from the repeated-phrase
magenta** so a *cross-paragraph echo* and a *within-paragraph repeated
phrase* never read as the same finding. Change it like any theme colour:

```hjson
{
  theme: {
    style_warning_echo_fg:       "#7aa2f7"   // any #rrggbb
    style_warning_echo_modifier: "bold"      // or underline / dim / italic / none / "underline+bold"
  }
}
```

### Tunables (shared with the scan)

The overlay reuses the same three knobs as the `echo-repetition` scan, so
what you see while writing matches what the scan reports:

| Key | Meaning |
| --- | --- |
| `editor.echo_window` | How many paragraphs apart still count as "close". |
| `editor.echo_min_repeats` | How many occurrences before it's flagged. |
| `editor.echo_max_global` | A word common enough project-wide is ignored (not a craft echo). |

It's multilingual: matching runs through the project's Snowball stemmer, so
inflected forms group (English *walk/walked/walking*; Russian folds `ё`→`е`
before stemming so `пошёл`/`пошла` group correctly).

### Starting state

`Ctrl+B Shift+K` is a session-local toggle. To start every session with it
on, set the master switch:

```hjson
{
  editor: {
    echo_overlay: true
  }
}
```

## The `paragraph-too-long` doctor class

The second 1.2.20 addition is structural rather than lexical. A paragraph
that takes too long to read at your configured reading speed is often a
wall of text a reader will bounce off — so `inkhaven doctor --scan` now
flags it:

```bash
$ inkhaven doctor --scan
…
[info] paragraph-too-long  books/my-novel/03-the-wharf/004.typ
       ~214s to read at 200 wpm (713 words) — consider a break
```

It's **Info** severity with **no autofix** — a long run-on can be a
deliberate breathless effect, so this points and you decide, like the other
revision scans. The threshold and speed are config:

| Key | Default | Meaning |
| --- | --- | --- |
| `editor.paragraph_long_secs` | `180` | Read-time (seconds) above which a paragraph is flagged. |
| `editor.reading_wpm` | `200` | Words per minute — the same speed the reading-pace chip ([Tutorial 58](58-reading-pace.md)) uses. |

So 180s at 200 wpm ≈ 600 words. Read time is computed exactly the way the
editor's reading-pace chip computes it (`words × 60 / wpm`), so a finding
matches what the reading-pace preview shows you. Set
`paragraph_long_secs: 0` to disable the class.

## See also

- [Tutorial 59 — Revision & continuity](59-revision-and-continuity.md): the
  `echo-repetition` scan and the other three multilingual detectors.
- [Tutorial 43 — Show, don't tell](43-show-dont-tell.md): the other inline
  style-warning overlays under `Ctrl+B Shift+F`.
- [Tutorial 58 — Reading pace](58-reading-pace.md): the reading-time chip
  and the `Ctrl+B Shift+E` reader-pace preview.
- [Tutorial 52 — Health and doctor](52-health-and-doctor.md): the full
  `doctor --scan` flow and the `Ctrl+B Shift+0` modal.
