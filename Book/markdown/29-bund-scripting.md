# 29 — Bund scripting

Bund is the embedded scripting language. Stack-based, terse, lambda-friendly. Inkhaven embeds it because the alternative — writing a YAML config language that tries to express everything from a Python — always collapses under its own weight.

You don't have to learn Bund to use inkhaven. You learn it when you want to react to a hook, automate a chord, or write a workflow that's bigger than the chord chart can express.

## Where Bund lives

Four touchpoints:

| Touchpoint | Description |
|------------|-------------|
| `scripting.bootstrap` (HJSON) | Single string evaluated once at VM init. Natural home for hook lambdas. |
| Scripts system book | Tree nodes with `.bund` extension. Inkhaven `bund.eval`s every Script node at startup, after the bootstrap. |
| Ctrl+Z (Bund prefix) | Inside the TUI: Ctrl+Z then `N` creates a Script node; `Ctrl+Z R` runs the current buffer; `Ctrl+Z ?` opens the script picker. |
| `inkhaven bund "..."` | CLI: one-shot eval against the project's VM. |

## Stack model

Values flow through the VM's stack. A literal pushes; a word consumes from the top and pushes back its result. So:

```bund
40 2 +              # pushes 40, pushes 2, + pops two and pushes 42
"Hello, " "world" cat println   # cat concats, println pops + prints
```

Lambdas are wrapped in `{ ... }` and assigned with `define` or registered as hooks with `register`:

```bund
{ dup * } "square" define
3 square println             # → 9

"hook.on_save" { drop "saved" println } register
```

The full language tour is in `Documentation/Bund/BUND_TUTORIAL.md`.

## The `ink.*` stdlib

Inkhaven exposes the project state through a stdlib of `ink.*` words. The full catalog is in Appendix C. The families:

| Family | Role |
|--------|------|
| `ink.node.*` | Read the hierarchy: list, get, children. |
| `ink.paragraph.*` | Read + write paragraph bodies, status, target, save. |
| `ink.tag.*` | List / search / add / remove tags (Chapter 14). |
| `ink.event.*` | Add / list / link events (Chapter 17). |
| `ink.story.render` | Render the book story view to PNG (Chapter 16). |
| `ink.editor.*` | Editor surface: insert, set_cursor, replace, scroll. |
| `ink.key.*` | Rebind chords at runtime (Chapter 28). |
| `ink.search.text` | Project-wide semantic search (Chapter 10). |
| `ink.fs.*` | File-system access — gated by fs_read / fs_write. |
| `ink.ai.*` | Trigger AI inference; set system prompts. |
| `ink.input` | Pop a modal prompt for the user. |
| `ink.pane.*` | Open / write to / close a floating output pane. |

Every word has a stack signature documented in `BUND_TUTORIAL.md` and Appendix C.

## Hooks

Inkhaven fires named hooks at well-defined points. Register a lambda with the hook's name to react:

| Hook | When |
|------|------|
| `hook.on_create   ( uuid kind -- )` | Node created in the tree. |
| `hook.on_save     ( uuid -- )` | Paragraph saved. |
| `hook.on_rename   ( uuid new_title -- )` | Node renamed. |
| `hook.on_snapshot ( parent_uuid snap_uuid -- )` | Snapshot taken. |
| `hook.on_delete   ( uuid -- )` | Node deleted (once per id in a subtree). |
| `hook.on_status_promoted ( uuid new_status -- )` | Status ladder bumped up. |
| `hook.on_goal_hit ( word_count_today -- )` | Daily goal reached. |
| `hook.on_diagnostic ( uuid count first-message -- )` | Typst diagnostic state changed (1.2.6+). |
| `hook.on_event_added ( uuid -- )` | Timeline event created (1.2.7+). |
| `hook.on_event_orphaned ( uuid -- )` | Event lost its last link (1.2.7+). |

## Sandbox policy

Bund runs inside a policy sandbox. Categories default-deny the destructive parts:

| Category | Default | Used by |
|----------|---------|---------|
| store_read | Allowed | ink.node.*, ink.search.*, etc. |
| store_write | Denied | ink.paragraph.save, ink.tree.delete, etc. |
| fs_read | Allowed | |
| fs_write | Denied | ink.fs.write, ink.story.render to disk. |
| net | Denied | ink.net.*. |
| shell | Denied | ink.shell.*. |
| code_eval | Denied | Bund's own `eval` word. |
| keymap | Denied | ink.key.bind at runtime. |
| ai_write | Denied | ink.ai.set_system_prompt. |
| editor_write | Allowed | ink.editor.* (the editor is yours). |
| theme_write | Denied | ink.theme.*. |

Opt in via HJSON:

```hjson
scripting: {
  enabled_categories: ["store_write", "fs_write"]
  no_default_deny:    false
}
```

`no_default_deny: true` clears the entire deny list (use sparingly — you're trusting every Bund script you run).

## A first hook

A typical bootstrap that auto-snapshots when a paragraph promotes to Final:

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

## Where to learn more

- `Documentation/Bund/BUND_TUTORIAL.md` — language tour + every stdlib word.
- Appendix C — quick reference card for the `ink.*` stdlib + hooks.
- Real example scripts ship in `Documentation/Bund/`.

## Recap

- Four touchpoints: `scripting.bootstrap`, Scripts book, `Ctrl+Z` chord, `inkhaven bund`.
- Stack-based, terse, lambdas with `{ ... }`.
- `ink.*` stdlib exposes node / paragraph / tag / event / story / editor / search / AI / fs / input / pane.
- Hooks: `on_save`, `on_rename`, `on_status_promoted`, `on_diagnostic`, `on_event_added`, etc.
- Policy categories default-deny destructive surfaces; opt in via `enabled_categories`.
