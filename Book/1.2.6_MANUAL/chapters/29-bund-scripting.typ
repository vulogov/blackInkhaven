#import "../design.typ": *

#chapter(number: 29, part: "Part IX — Customisation & Scripting",
  title: "Bund scripting")

#dropcap("B")und is the embedded scripting language. Stack-
based, terse, lambda-friendly. Inkhaven embeds it
because the alternative — writing a YAML config language
that tries to express everything from a Python — always
collapses under its own weight.

You don't have to learn Bund to use inkhaven. You learn it
when you want to react to a hook, automate a chord, or write
a workflow that's bigger than the chord chart can express.

#section("Where Bund lives")

Four touchpoints:

#chord_table((
  chord_row("scripting.bootstrap (HJSON)", "Single string evaluated once at VM init. Natural home for hook lambdas."),
  chord_row("Scripts system book", "Tree nodes with `.bund` extension. Inkhaven `bund.eval`s every Script node at startup, after the bootstrap."),
  chord_row("Ctrl+Z (Bund prefix)", "Inside the TUI: Ctrl+Z then `N` creates a Script node; `Ctrl+Z R` runs the current buffer; `Ctrl+Z ?` opens the script picker."),
  chord_row("`inkhaven bund \"...\"`", "CLI: one-shot eval against the project's VM."),
))

#section("Stack model")

Values flow through the VM's stack. A literal pushes; a
word consumes from the top and pushes back its result. So:

```bund
40 2 +              # pushes 40, pushes 2, + pops two and pushes 42
"Hello, " "world" cat println   # cat concats, println pops + prints
```

Lambdas are wrapped in `{ ... }` and assigned with `define`
or registered as hooks with `register`:

```bund
{ dup * } "square" define
3 square println             # → 9

"hook.on_save" { drop "saved" println } register
```

The full language tour is in `Documentation/Bund/BUND_TUTORIAL.md`.

#section("The `ink.*` stdlib")

Inkhaven exposes the project state through a stdlib of
`ink.*` words. The full catalog is in Appendix C. The
families:

#chord_table((
  chord_row("ink.node.*", "Read the hierarchy: list, get, children."),
  chord_row("ink.paragraph.*", "Read + write paragraph bodies, status, target, save."),
  chord_row("ink.tag.*", "List / search / add / remove tags (Chapter 14)."),
  chord_row("ink.event.*", "Add / list / link events (Chapter 17)."),
  chord_row("ink.story.render", "Render the book story view to PNG (Chapter 16)."),
  chord_row("ink.editor.*", "Editor surface: insert, set_cursor, replace, scroll."),
  chord_row("ink.key.*", "Rebind chords at runtime (Chapter 28)."),
  chord_row("ink.search.text", "Project-wide semantic search (Chapter 10)."),
  chord_row("ink.fs.*", "File-system access — gated by fs_read / fs_write."),
  chord_row("ink.ai.*", "Trigger AI inference; set system prompts."),
  chord_row("ink.input", "Pop a modal prompt for the user."),
  chord_row("ink.pane.*", "Open / write to / close a floating output pane."),
))

Every word has a stack signature documented in
`BUND_TUTORIAL.md` and Appendix C.

#section("Hooks")

Inkhaven fires named hooks at well-defined points. Register
a lambda with the hook's name to react:

#chord_table((
  chord_row("hook.on_create   ( uuid kind -- )", "Node created in the tree."),
  chord_row("hook.on_save     ( uuid -- )", "Paragraph saved."),
  chord_row("hook.on_rename   ( uuid new_title -- )", "Node renamed."),
  chord_row("hook.on_snapshot ( parent_uuid snap_uuid -- )", "Snapshot taken."),
  chord_row("hook.on_delete   ( uuid -- )", "Node deleted (once per id in a subtree)."),
  chord_row("hook.on_status_promoted ( uuid new_status -- )", "Status ladder bumped up."),
  chord_row("hook.on_goal_hit ( word_count_today -- )", "Daily goal reached."),
  chord_row("hook.on_diagnostic ( uuid count first-message -- )", "Typst diagnostic state changed (1.2.6+)."),
  chord_row("hook.on_event_added ( uuid -- )", "Timeline event created (1.2.6+)."),
  chord_row("hook.on_event_orphaned ( uuid -- )", "Event lost its last link (1.2.6+)."),
))

#section("Sandbox policy")

Bund runs inside a policy sandbox. Categories default-deny
the destructive parts:

#chord_table((
  chord_row("store_read", "Default-allowed. ink.node.*, ink.search.*, etc."),
  chord_row("store_write", "Default-denied. ink.paragraph.save, ink.tree.delete, etc."),
  chord_row("fs_read", "Default-allowed."),
  chord_row("fs_write", "Default-denied. ink.fs.write, ink.story.render to disk."),
  chord_row("net", "Default-denied. ink.net.*."),
  chord_row("shell", "Default-denied. ink.shell.*."),
  chord_row("code_eval", "Default-denied. Bund's own `eval` word."),
  chord_row("keymap", "Default-denied. ink.key.bind at runtime."),
  chord_row("ai_write", "Default-denied. ink.ai.set_system_prompt."),
  chord_row("editor_write", "Default-allowed. ink.editor.* (the editor is yours)."),
  chord_row("theme_write", "Default-denied. ink.theme.*."),
))

Opt in via HJSON:

```hjson
scripting: {
  enabled_categories: ["store_write", "fs_write"]
  no_default_deny:    false
}
```

`no_default_deny: true` clears the entire deny list (use
sparingly — you're trusting every Bund script you run).

#section("A first hook")

A typical bootstrap that auto-snapshots when a paragraph
promotes to Final:

```hjson
scripting: {
  enabled_categories: ["store_write"]
  bootstrap: '''
    "hook.on_status_promoted" {
      // ( uuid new_status -- )
      "Final" =
      {
        // Find the open paragraph; snapshot.
        // (Pseudo — see BUND_TUTORIAL.md for snapshot Bund words.)
        drop "promoted to Final" println
      } { drop drop } ifelse
    } register
  '''
}
```

#section("Where to learn more")

- `Documentation/Bund/BUND_TUTORIAL.md` — language tour +
  every stdlib word.
- Appendix C — quick reference card for the `ink.*` stdlib
  + hooks.
- Real example scripts ship in `Documentation/Bund/`.

#recap((
  [Four touchpoints: `scripting.bootstrap`, Scripts book, `Ctrl+Z` chord, `inkhaven bund`.],
  [Stack-based, terse, lambdas with `{ ... }`.],
  [`ink.*` stdlib exposes node / paragraph / tag / event / story / editor / search / AI / fs / input / pane.],
  [Hooks: `on_save`, `on_rename`, `on_status_promoted`, `on_diagnostic`, `on_event_added`, etc.],
  [Policy categories default-deny destructive surfaces; opt in via `enabled_categories`.],
))
