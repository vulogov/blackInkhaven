# Tutorial 64 — Project-wide find & replace

*Inkhaven 1.2.22+*

In-buffer find/replace ([§3.10](../KEYBINDING.md)) works on the open
paragraph. But the edit you most often need is *across the whole book* —
renaming a character, fixing a place name, swapping a coined term in every
chapter. 1.2.22 adds that, **safely**: nothing is changed without review,
and every touched paragraph is snapshotted first so you can undo.

It's deliberately **lexical, not semantic** — literal / whole-word / regex
matching over a linear scan of your paragraphs (the same `regex` engine the
in-buffer search uses). Semantic search finds paragraphs *about* a topic;
that's the wrong tool for finding every literal occurrence of a string.

## In the editor

Open Find & Replace as usual with **`Ctrl+R`**, type your pattern and
replacement, then press **`Ctrl+B`** to switch the scope from *this
paragraph* to *the whole book* (the chip under the fields shows which).
**Enter** runs the scan and opens a review:

```
┌─ Replace: Anne → Anna  (37/37 kept) ──────────────┐
│ book-1/03-the-wharf/004-arrival                   │
│   [x]   3:18  …the door when »Anne« looked up,…   │
│   [x]   7:2   …"»Anne«," he said, not looking…    │
│ book-1/05-the-letter/012-reply                    │
│   [ ]  11:40  …old »Anne«liese from the mill…  ←  │  (skipped)
│ …                                                 │
│ ↑↓ move · Space skip · a keep all · n skip none · │
│ Enter apply · Esc cancel                          │
└───────────────────────────────────────────────────┘
```

Every match is shown **in context** with the hit highlighted. Each is
individually toggleable — **`Space`** skips (or un-skips) the one under the
cursor, **`a`** keeps all, **`n`** skips all. **`Enter`** applies only the
kept matches; **`Esc`** cancels.

The review starts in **whole-word** mode, so `Anne` won't touch `Anneliese`
— but you can change the matching mode without leaving the review *(1.2.23)*:
**`w`** toggles whole-word, **`i`** ignore-case, **`x`** regex. Each re-runs
the scan and the header shows the active mode (`[whole-word]`, `[substring]`,
`[regex]`, `…, ignore-case`) — the same modes the CLI flags below reach. The
review always shows you everything, so you decide. When you apply, each
changed paragraph gets a snapshot annotated `replace: Anne → Anna` first —
**`F6`** restores any of them.

## From the command line

For scripted or bulk renames, `inkhaven replace` does the same thing with a
preview-then-apply gate:

```bash
# Preview — lists every match, changes nothing
$ inkhaven replace "Anne" "Anna" --dry-run
book-1/03-the-wharf/004-arrival
  3:18  …the door when »Anne« looked up,…
  7:2   …"»Anne«," he said, not looking…
replace: 37 matches in 24 paragraphs — dry run, nothing changed

# Apply (snapshots each touched paragraph first)
$ inkhaven replace "Anne" "Anna" --yes
replace: applied 37 replacements in 24 paragraphs; 24 snapshots taken (restore via F6 in the editor)
```

Without `--yes` it refuses to write and just reports the count, so a bare
`inkhaven replace …` is always safe. Flags:

| Flag | Effect |
| ---- | ------ |
| `--dry-run` | Preview matches, change nothing (CI / script-safe). |
| `--yes` | Apply (required to write). |
| `--regex` | Treat the pattern as a regex (`$1`… captures in the replacement). |
| `--substring` | Match substrings too (default is whole-word). |
| `--ignore-case` | Case-insensitive match. |
| `--book <name>` | Limit to one book (default: all user books). |
| `--include-system` | Also scan the system books (Notes / Facts / …). |

## Scope and safety notes

- **User books only by default** — a manuscript rename won't silently
  rewrite your Notes, Research, or Facts. Add `--include-system` (CLI) if
  you want those too.
- **Whole-word by default** — kills the classic `Will`/`will`,
  `Mark`/`market` footgun. Use `--substring` (CLI) for raw substring
  matching.
- **Always reversible** — every touched paragraph is snapshotted before
  the write, so `F6` is your undo.
- **Per-match review is the editor's** — the CLI is preview + apply-all
  (the scripted path); the in-editor flow is where you skip individual
  matches.

## See also

- [`KEYBINDING.md` §3.10](../KEYBINDING.md) — in-buffer find & replace.
- [Tutorial 20 — Snapshot diff](20-snapshot-diff.md) and `F6` — the
  snapshots that back the undo.
- [Tutorial 40 — Concordance](40-concordance.md) — read-only project-wide
  word index (the complement to replace).
