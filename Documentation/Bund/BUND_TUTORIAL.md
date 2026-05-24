# Bund — a tutorial for inkhaven writers

Bund is a small stack-based scripting language inkhaven embeds for
writing custom rules, hook scripts, and AI prompt templates. It's
[Forth][forth]-shaped — postfix operators, two stacks, curly-brace
lambdas — and small enough to learn in an afternoon.

[forth]: https://en.wikipedia.org/wiki/Forth_(programming_language)

This tutorial assumes you've used inkhaven before (see
[FIRST_STEPS.md](../FIRST_STEPS.md)) but no prior Bund or Forth
experience.

## What Bund is for, inside inkhaven

Three roles, in order of how often you'll write them:

1. **Hook lambdas** — code that runs when you save a paragraph,
   create a snapshot, or delete a node. The natural home for
   "warn me when…" rules.
2. **AI prompt helpers** — small scripts that pull a paragraph's
   content, walk surrounding nodes, and assemble a custom prompt.
3. **Custom chord bindings** — through `ink.key.*`, Bund can rewrite
   the keymap at runtime. Sandbox-gated.

You don't need Bund for everyday writing — inkhaven's chord set
already covers the common cases. Bund is the escape hatch for
authors who want their workflow to scale.

## Running a script

Two ways:

```text
$ inkhaven bund "40 2 +"           # one-liner from the shell
42

$ inkhaven --project ~/my-book bund 'ink.search.text "morning" 5'
[ … five JSON-formatted hits … ]
```

…or inside the TUI:

- Press `Ctrl+Z E`, type an expression, Enter. Result on the status bar.
- Open a `.bund` Script node in the editor, then `Ctrl+Z R` runs the
  buffer against the Adam VM.

## The two stacks

Bund's VM keeps **two stacks**:

- **The data stack** — where values sit. Most words operate here.
- **The workbench** — a scratch area for intermediate values. Less
  commonly used; most beginner code never touches it.

In this tutorial "the stack" always means the data stack.

## Numbers go on. Operators pull off.

Bund is **postfix**. You push operands first, then the operator
pulls them off:

```bund
2 3 +          // push 2, push 3, + pulls both, pushes 5
```

After this runs, the stack contains a single value, `5`. The CLI's
`inkhaven bund` prints the top of the stack at exit (with
`pretty-print` via JSON):

```text
$ inkhaven bund "2 3 +"
5
```

Bigger expressions chain naturally:

```bund
2 3 +  4 *      // = (2+3) * 4 = 20
40 2 +          // = 42
10 3 /          // integer division — note Forth-style pop order
```

(One Bund/Forth gotcha: `a b /` does `b ÷ a` because operands pop
in stack-order. So `10 3 /` gives `0` — the top of the stack `3`
pops first as the divisor of `10`. Trust the stack diagram, not
your infix instincts.)

## Strings

Double quotes:

```bund
"Hello, inkhaven!" println
```

The `println` word pulls one value off the stack, prints it
(converting non-strings to their string form), and adds a newline.

```text
$ inkhaven bund '"hello" println "world" println'
hello
world
```

Without `println`, the value just sits on the stack and gets
shown when the CLI exits:

```text
$ inkhaven bund '"hello"'
hello
```

## Stack words

A handful of words shuffle stack contents:

| Word   | Effect on stack    | Notes |
|--------|--------------------|-------|
| `dup`  | `( a -- a a )`     | duplicate the top |
| `drop` | `( a -- )`         | discard the top |
| `swap` | `( a b -- b a )`   | swap the top two |
| `over` | `( a b -- a b a )` | copy the second to the top |
| `rot`  | `( a b c -- b c a )` | rotate three |

Example — square the top of stack:

```bund
5 dup *         // → 25
```

`dup` copies 5, then `*` multiplies the pair.

## Lambdas

Curly braces wrap a block of code into a **lambda** value. You can
push a lambda onto the stack and store it under a name with
`register`:

```bund
"square" { dup * } register
```

Now the word `square` invokes the body:

```bund
5 square        // → 25
9 square        // → 81
```

Lambdas are first-class — they're values like numbers and strings.

## Comments

`// to end of line`:

```bund
// This is a comment.
2 3 +           // adds two numbers
```

## What inkhaven adds

Bundcore (the language itself) gives you arithmetic, strings,
lambdas, stack ops. Inkhaven layers an **`ink.*` stdlib** on top:
words that reach into the project store.

### Read-only `ink.*` words (default-allowed)

| Word | Stack | What it does |
|------|-------|--------------|
| `ink.node.list` | `( -- list )` | every node in the project |
| `ink.node.get` | `( uuid -- hash \| NODATA )` | full metadata for one node |
| `ink.node.children` | `( uuid \| "" -- list )` | children of a parent (or root) |
| `ink.paragraph.text` | `( uuid -- string \| NODATA )` | body of a Paragraph |
| `ink.search.text` | `( query limit -- list )` | semantic search hits |
| `ink.snapshot.list` | `( paragraph_uuid -- list )` | snapshots of a paragraph |
| `ink.pane.show` | `( title -- )` | open the floating Bund output pane; `print` / `println` reroute there |
| `ink.pane.close` | `( -- )` | close the pane (no-op when not open) |
| `ink.pane.clear` | `( -- cleared )` | empty the pane buffer; returns false if no pane is open |
| `ink.pane.line` | `( text -- routed )` | append one line; returns false if no pane is open |
| `ink.input` | `( prompt hookname -- )` | open the input modal; on Enter pushes the typed string and fires `hookname` |
| `ink.paragraph.target` | `( path -- int \| NODATA )` | 1.2.4+: read the per-paragraph word-count goal (NODATA when unset) |
| `ink.paragraph.set_target` | `( path target -- )` | 1.2.4+: set / clear the per-paragraph goal. `target ≤ 0` clears. `store_write` — opt in via `enabled_categories: ["store_write"]` |
| `ink.search.load` | `( query -- )` | 1.2.4+: run semantic search and load the top hit into the editor (autosaves the previous buffer). `editor_write` |
| `ink.editor.replace_all` | `( old new -- count )` | 1.2.4+: in-buffer find/replace on the open editor; returns the number of replacements. `editor_write` |
| `ink.ai.send_blocking` | `( prompt -- response )` | 1.2.4+: synchronous AI send. Blocks the script (UI keeps repainting). `ai_write` |
| `ink.ai.poll` | `( -- string )` | 1.2.4+: non-blocking poll of the async AI response slot. Empty string when none ready. `ai_read` |
| `ink.fs.read` | `( path -- string )` | 1.2.4+: read a file. `fs_read` (default-allowed). |
| `ink.fs.write` | `( path content -- )` | 1.2.4+: write a file. `fs_write` — **default-denied**. Opt in: `enabled_categories: ["fs_write"]`. |

The pane + input words live under the `editor_read` policy
category (non-destructive UI), so they're allowed by default.
See [`../Tutorials/18-bund-pane-and-script-picker.md`](../Tutorials/18-bund-pane-and-script-picker.md)
for the full pane + input + script-picker workflow.

Example — print the title of every system book:

```bund
"" ink.node.children            // push root children
```

`inkhaven --project ~/my-book bund '"" ink.node.children'` returns
a JSON list of every root node (typically eight system books +
your own user books).

### Hooks (the "code that runs on…" pattern)

Hook points fire after the matching store mutation or pipeline
step succeeds:

| Hook name | Stack on entry | Fires after |
|-----------|----------------|-------------|
| `hook.on_create` | `( uuid kind -- )` | new node |
| `hook.on_save` | `( uuid -- )` | paragraph content saved |
| `hook.on_rename` | `( uuid new_title -- )` | node renamed (or auto-renamed; 1.2.4+ also rename `.typ` on disk) |
| `hook.on_snapshot` | `( parent_uuid snap_uuid -- )` | snapshot taken |
| `hook.on_delete` | `( uuid -- )` | each id in a deleted subtree |
| `hook.on_status_promoted` | `( uuid from_status to_status -- )` | 1.2.4+: paragraph status changes (`Ctrl+B R` cycle OR auto-promote on goal hit). Status strings are lowercased (`napkin`, `first`, …, `ready`, `none`). |
| `hook.on_goal_hit` | `( today_words daily_goal -- )` | 1.2.4+: project-wide `today_words` crosses `goals.daily_words` on the current day. Doesn't re-fire while still above the line — self-resets if the user dips back below. |
| `hook.on_streak_break` | `( prev_streak_days -- )` | 1.2.4+: writing streak transitions from positive to zero (grace exhausted). Arg is the streak length at the moment of the break. |
| `hook.on_assemble` | `( uuid slug root_typ_path files_written -- )` | 1.2.4+: successful `Ctrl+B A` Book assembly. `root_typ_path` is the absolute path the user feeds to `typst compile`. |
| `hook.on_take` | `( uuid slug pdf_dest -- )` | 1.2.4+: PDF copied to launch cwd by `Ctrl+B O` (extra formats may or may not have succeeded; `on_take` fires once the PDF lands). |

Register one in your `inkhaven.hjson`'s `scripting.bootstrap`:

```hjson
scripting: {
  bootstrap: '''
    "hook.on_save" { drop "saved" println } register
  '''
}
```

Or — better for anything non-trivial — put it in a Bund Script
node inside the project tree (`Ctrl+Z N` to create one). Inkhaven
`bund.eval`s every Script node at startup, after the inline
bootstrap.

A more useful save hook — warn when a paragraph mentions a
Character name not in the Characters book (sketch):

```bund
"hook.on_save" {
  // ( uuid -- )
  dup ink.paragraph.text          // ( uuid text )
  swap drop                       // ( text )
  // … parsing logic here …
  drop
} register
```

(The text-walking part is left as an exercise — `ink.search.text`
against the Characters book is one approach.)

### Sandbox

A handful of word categories are denied by default — chiefly
anything that could mutate the world outside the project store
(`fs_write`, `net`, `shell`) plus `code_eval` (`bund.eval` etc.)
and `keymap` (chord rebinding). Opt in by listing categories
under `scripting.enabled_categories`:

```hjson
scripting: {
  enabled_categories: ["keymap"]
}
```

See [KEYS_REASSIGNMENT.md](../KEYS_REASSIGNMENT.md) for the chord
rebinding API specifically, and
[CONFIGURATION.md](../CONFIGURATION.md) for the full policy knobs.

## Where to put scripts

| Location | Use case |
|----------|----------|
| `inkhaven.hjson` → `scripting.bootstrap` | Tiny one-line rules |
| `.bund` Script node in the project tree | Anything longer; lives with the manuscript, gets backed up |
| User-wide `~/.inkhaven/scripts/` (planned) | Cross-project tools — not yet implemented |

To create a Script node inside the TUI: focus any pane, press
`Ctrl+Z N`. The Add modal opens pre-pointed at the `Scripts`
system book.

## Iterating

There's no read-eval-print loop yet, but the closest thing is
`Ctrl+Z E` inside the TUI — pops a one-shot prompt, runs your
expression, shows the result on the status bar. Combine with
`println` for visible feedback:

```bund
"" ink.node.children dup println
```

The `dup` keeps the list on the stack so the CLI exit print
still shows it; `println` shows it inline first.

## Where to next

- [KEYS_REASSIGNMENT.md](../KEYS_REASSIGNMENT.md) — rebinding chords
  in HJSON and from Bund (`ink.key.*`).
- [CONFIGURATION.md](../CONFIGURATION.md) — the `scripting:` block
  in `inkhaven.hjson`.
- [The bundcore docs](https://docs.rs/bundcore) — for the full
  vanilla stdlib (arithmetic, math, time, conversion). Most of it
  is auto-loaded into the Adam VM at startup.

---

## 1.2.6 — stdlib + hook additions

The 1.2.6 cycle expanded the `ink.*` stdlib substantially.
Three families landed:

- `ink.tag.*` (1.2.6+) — project-wide tag metadata.
- `ink.event.*` (1.2.6+) — story-timeline events.
- `ink.story.render` (1.2.6+) — book-graph PNG writer.

Plus one new editor word (`ink.editor.set_cursor`) and
three new hooks (`hook.on_diagnostic`, `hook.on_event_added`,
`hook.on_event_orphaned`).

### `ink.tag.*` — project-wide tags

```bund
                          ink.tag.list          ( -- list )
"intro/scene-1"           ink.tag.list_for      ( path -- list | NODATA )
"draft"                   ink.tag.search        ( tag -- list-of-paths )
"intro/scene-1" "draft"   ink.tag.add           ( path tag -- )
"intro/scene-1" "draft"   ink.tag.remove        ( path tag -- )
```

Policy:

- `list`, `list_for`, `search` → `store_read` (default
  allowed).
- `add`, `remove` → `store_write` (default denied — opt in
  via `scripting.enabled_categories`).

`ink.tag.list` returns the sorted, case-insensitively
de-duplicated set of every tag in the project.

`ink.tag.search` returns slug-paths (not UUIDs) so the
result is portable to other `ink.*` words that accept
paths.

Example — auto-tag every paragraph in a chapter as
`editing-pass-2`:

```bund
"intro" ink.node.children          // ( list )
{
  dup "kind" get
  "Paragraph" =
  {
    "path" get
    "editing-pass-2"
    ink.tag.add
  }
  { drop } ifelse
} each
```

### `ink.event.*` — story timeline (1.2.6+)

Requires `timeline.enabled: true` in HJSON. Each word
that needs the timeline checks at call time and errors
clearly if disabled.

```bund
                              ink.event.list             ( -- list )
                              ink.event.list_orphans     ( -- list )
"Aerin Saga" "Storm" "1A.2.3" ink.event.add              ( book title spec -- uuid )
event-id "1A.2.5"             ink.event.set_end          ( uuid spec -- )
event-id "season"             ink.event.set_precision    ( uuid prec -- )
event-id "main"               ink.event.set_track        ( uuid track -- )
event-id "aerin-saga/ch4/scene" ink.event.link_paragraph
                                                          ( uuid path -- )
```

Policy:

- `list`, `list_orphans` → `store_read`.
- `add`, `set_*`, `link_paragraph` → `store_write`.

`ink.event.list` returns a list of hashes with `id`,
`title`, `slug`, `path`, `start_ticks`, `end_ticks`
(NODATA when instant), `precision`, `track` (NODATA
when default), `is_orphan`, `linked_paragraphs`,
`characters`, `places`.

Calendar specs (`"1A.2.3"`, `"Sol 5"`, etc.) follow the
project's `timeline.calendar` HJSON — see
[CONFIGURATION.md](../CONFIGURATION.md).

`set_precision` accepts `hour` / `day` / `week` / `month`
/ `season` / `year`.

`set_track ""` (empty string) clears the track back to
the default.

`set_end ""` or `set_end "none"` clears the duration
back to an instant event.

Example — create a paired event + link to an existing
paragraph:

```bund
"Aerin Saga" "Marketplace scene" "1A.2.8" ink.event.add
dup "aerin-saga/ch4/marketplace-scene" ink.event.link_paragraph
```

`dup` keeps the UUID on the stack so the link can use
it without re-resolving.

### `ink.story.render` — write the book graph to disk

```bund
"Aerin Saga" "~/Desktop/aerin-story.png" ink.story.render
```

Stack: `( book-name path -- )`. Case-insensitive book
lookup against title + slug. `~/` expansion supported.
Policy: `fs_write` (default-denied).

Useful as a `hook.on_save` side effect that keeps a
fresh graph available alongside the manuscript.

### `ink.editor.set_cursor` (1.2.6+)

```bund
3 42 ink.editor.set_cursor      ( row col -- )    // 1-based
```

The existing `ink.editor.goto` is 0-based; the new
`set_cursor` is 1-based to match typst's diagnostic
line:col convention. Pairs naturally with
`hook.on_diagnostic` for auto-jump scripts.

Policy: `editor_write` (default allowed).

### New hooks

```bund
"hook.on_diagnostic" {
  // ( uuid count first-message -- )
  swap drop swap drop                  // ( count )
  dup 5 >
  { "⚠ many diagnostics: " print println }
  { drop } ifelse
} register
```

Fires on diagnostic state changes only (clean → errored,
count change, top-message change). Debounced — no fire
on every idle tick.

```bund
"hook.on_event_added" {
  // ( uuid -- )
  dup ink.node.get "path" get          // ( uuid path )
  "new-event" ink.tag.add               // ( uuid )
  drop
} register
```

Fires from every path that adds an event: CLI
`event add`, the TUI `n` chord inside the swim-lane
view, and the Bund word `ink.event.add` (the latter
only when not inside an `eval` guard).

```bund
"hook.on_event_orphaned" {
  // ( uuid -- )
  drop
  "orphan check needed" println
} register
```

Fires on the `linked → orphan` transition.
Automatically catches deletes via the delete-time scrub.

### Hook stdout

`hooks::fire` (1.2.6+) drains the Bund print buffer at
the end of each hook fire and emits the output via
`tracing::info!(target: "inkhaven::hook::out")`. So
hook `println`s land in `.inkhaven.log` from the TUI
and on stderr from the CLI. Pre-1.2.6, hook stdout
vanished silently.

### Policy categories (1.2.6 cycle additions)

No new categories were added. The existing eleven
(`store_read`, `store_write`, `fs_read`, `fs_write`,
`net`, `shell`, `code_eval`, `keymap`, `ai_write`,
`editor_write`, `theme_write`) carry every new word.

The default deny list is unchanged: `fs_write`, `net`,
`shell`, `code_eval`, `keymap`.

A common opt-in for timeline + tag scripting:

```hjson
scripting: {
  enabled_categories: ["store_write", "fs_write"]
}
```

`store_write` enables `ink.tag.add/remove` and every
`ink.event.*` writer. `fs_write` enables
`ink.story.render` writing a PNG path.
