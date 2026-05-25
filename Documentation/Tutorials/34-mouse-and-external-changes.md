# 34 — Mouse capture toggle + external-change auto-reload

Two small 1.2.7 ergonomics features that make the TUI play
better with other tools (terminal-native text selection, CLI
edits on the same project):

```
Ctrl+Shift+M    toggle mouse capture on / off
(none — passive) external-change auto-reload on the open paragraph
```

## Mouse capture toggle

By default Inkhaven captures every mouse event in the terminal
— click sets focus, scroll-wheel scrolls the active pane,
drag-select highlights a region in the editor. That's the
right default for in-TUI navigation, but it means the
terminal's **native** copy-paste (highlight with the mouse,
right-click → Copy on most terminals; Cmd+C on macOS;
Ctrl+Shift+C on Linux) doesn't work while Inkhaven is
running.

`Ctrl+Shift+M` flips the capture off. The status line reads:

```
mouse capture OFF · native terminal select + Cmd/Ctrl+Shift+C copies
```

Now the mouse goes through to the terminal:

- Click-and-drag highlights raw cells (no syntax-awareness,
  no per-pane behaviour).
- The terminal's own copy keystroke copies the highlight to
  the system clipboard.
- Scroll-wheel scrolls the terminal's scrollback buffer (not
  the active pane).
- Click-to-focus stops working until you toggle back.

Press `Ctrl+Shift+M` again to restore capture:

```
mouse capture ON · click-to-focus + wheel-scroll active
```

The setting is **session-only** — not persisted. Every fresh
TUI launch starts with capture ON. The HJSON has no
`mouse_captured` knob today; if you want the toggle starting
state to flip, that's a future config addition.

### Why it's a toggle, not a permanent setting

The two modes serve different workflows:

- **Capture ON** is right for "I'm writing in the TUI"
  — every chord, every pane interaction, scroll wheel for
  long paragraphs.
- **Capture OFF** is right for "I want to grab a snippet to
  paste somewhere else" or "I need to scroll back through my
  terminal scrollback to see what `inkhaven export` printed
  earlier".

The toggle is also the only way to copy from the
**AI pane** without going through `Ctrl+C` selection mode
(which only copies whole turns). For "paste this paragraph
of the model's response into my notes app", flip capture off,
mouse-select, terminal-copy.

## External-change auto-reload

Inkhaven 1.2.7 watches the on-disk mtime of the currently-open
paragraph. Once per autosave tick (every keystroke, plus the
idle-autosave timer firing) the editor checks whether the file
changed underneath it. Three cases:

```
mtime unchanged       no-op (the usual case)

mtime newer, buffer    silent reload from disk; status bar reads:
CLEAN                    ↻ reloaded `morning` — file changed on disk
                       cursor jumps to (0, 0) since the previous
                       position may not survive an external rewrite.

mtime newer, buffer    red warning, NOT reloaded:
DIRTY                    ⚠ `morning` changed on disk while you have
                         unsaved edits — Ctrl+S to overwrite the
                         external change
                       your buffer is untouched; you decide.
```

### When this fires

- `inkhaven event add` writes to a paragraph file (events live
  as paragraphs under the Timeline chapter). If you have that
  event paragraph open, the TUI picks up the CLI edit on the
  next tick.
- `sed -i` / direct text-editor on a paragraph file from
  another terminal.
- `git pull` lands a remote edit on a paragraph file.
- The TUI's own save path. **Save bumps loaded_mtime
  internally**, so saving never triggers a spurious external-
  change reload — that was a 1.2.7 cycle bug that's now
  fixed. If a Ctrl+S ever moves your cursor home, file a bug.

### Resolving a dirty-buffer warning

Two paths:

1. **Keep my edits**: Ctrl+S. Your buffer overwrites the
   external change. The status moves from the warning to
   the normal `saved …` line.
2. **Take the external change**: copy any prose you can't
   afford to lose elsewhere first, then close the paragraph
   without saving (Ctrl+B M → pick a different paragraph;
   or Esc → tree → Enter on the same row to reopen). On
   reopen the buffer reflects the disk contents.

There's no "merge" — the warning is binary. For genuine
conflict-resolution flows, lean on git or a side-by-side
diff tool against the snapshot history (F6).

### Disabling the watch

There's no config knob to turn off the watcher. It's one
syscall per tick (`std::fs::metadata().modified()`) which is
cheap even at autosave cadence. If you have a workload where
this is actually expensive, file an issue — the right fix is
to widen the tick budget, not to skip the check.

## Use cases combined

- **Hybrid workflow**: edit prose in the TUI; pop out to a
  terminal to grep across paragraph files; flip mouse capture
  off and copy a hit; flip back to keep writing. The
  external-change watcher catches any unintended edits.
- **CLI event automation**: a Bund script under `Ctrl+Z ?`
  runs `inkhaven event add …`, which rewrites the timeline-
  index paragraph. If you have that paragraph open, you'll
  see the silent reload one tick later.
- **Pair editing**: two laptops on the same project over a
  syncthing share. As soon as the other laptop's save hits
  disk, your TUI picks it up — or, if you had local edits,
  the warning gives you a chance to merge by hand.

## See also

- [`10-backups-and-recovery.md`](10-backups-and-recovery.md) —
  project-level backups; the safety net when external changes
  and local edits collide.
- [`13-ai-full-screen-mode.md`](13-ai-full-screen-mode.md) —
  `Ctrl+C` chat-selection mode (the chord-driven AI-pane copy
  flow, alternative to mouse-select).
- [`03-the-editor.md`](03-the-editor.md) — editor pane
  behaviour overview (Ctrl+S save, cursor model).
