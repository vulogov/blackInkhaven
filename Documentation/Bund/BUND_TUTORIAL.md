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

Five hook points fire after the matching store mutation succeeds:

| Hook name | Stack on entry | Fires after |
|-----------|----------------|-------------|
| `hook.on_create` | `( uuid kind -- )` | new node |
| `hook.on_save` | `( uuid -- )` | paragraph content saved |
| `hook.on_rename` | `( uuid new_title -- )` | node renamed |
| `hook.on_snapshot` | `( parent_uuid snap_uuid -- )` | snapshot taken |
| `hook.on_delete` | `( uuid -- )` | each id in a deleted subtree |

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
