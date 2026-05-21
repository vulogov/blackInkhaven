# Bund — scripting inside inkhaven

Bund is a stack-based scripting language Vladimir Ulogov wrote
([github.com/vulogov/bundcore](https://github.com/vulogov/bundcore)).
Inkhaven embeds the [`bundcore`](https://docs.rs/bundcore),
[`bund_language_parser`](https://docs.rs/bund_language_parser), and
[`rust_multistackvm`](https://docs.rs/rust_multistackvm) crates to
expose Bund as the editor's user-customisation surface — hooks, AI
prompt helpers, chord rebinding.

If you've never touched Bund: start with
[BUND_TUTORIAL.md](BUND_TUTORIAL.md).

## What's in this directory

| Document | Read for |
|----------|----------|
| [`BUND_TUTORIAL.md`](BUND_TUTORIAL.md) | An afternoon's read. Stack model, syntax, lambdas, inkhaven's `ink.*` stdlib, hooks, sandbox. |

## Where Bund touches inkhaven

| Surface | File / chord | What |
|---------|--------------|------|
| One-shot CLI | `inkhaven bund "<code>"` | Evaluate against Adam, print the top of stack |
| TUI eval modal | `Ctrl+Z E` | Modal prompt → eval → result on the status bar |
| TUI buffer run | `Ctrl+Z R` (on a `.bund` Script node) | Eval the whole open buffer |
| Script tree node | `Ctrl+Z N` | Create a new `.bund` Script under the Scripts system book |
| HJSON bootstrap | `inkhaven.hjson` → `scripting.bootstrap: '''…'''` | Tiny inline rules. Runs once at project open. |
| Save / rename / delete hooks | Lambda named `hook.on_save` etc. | Runs after the matching `Store::*` mutation. See tutorial. |
| Chord rebinding | `ink.key.bind`, `ink.key.bind_lambda`, … | Sandbox-gated. See [KEYS_REASSIGNMENT.md](../KEYS_REASSIGNMENT.md). |

## The "Adam" VM

The Bund VM inkhaven uses is a process-wide singleton — the same
instance handles `inkhaven bund` from the CLI, every `Ctrl+Z R`
buffer run, every hook fire, every `ink.key.*` mutation. State
persists across calls (so a script that defines `square` once
makes it available everywhere afterward).

"Adam" comes from `bundcore`'s own naming — the first VM instance
constructed in the process.

## Sandbox

Bund words are grouped into categories, and a configurable subset
is denied by default — `fs_write`, `net`, `shell`, `code_eval`,
and `keymap`. Per-category opt-in via
`scripting.enabled_categories` in `inkhaven.hjson`. The
[tutorial's "Sandbox" section](BUND_TUTORIAL.md#sandbox) walks
through the typical pattern.

## Upstream documentation

The bundcore language itself is documented at
[github.com/vulogov/bundcore](https://github.com/vulogov/bundcore).
Inkhaven only adds the `ink.*` and `hook.*` namespaces on top of
the vanilla stdlib; everything else (arithmetic, control flow,
lambdas, classes, strings, time, math) comes from bundcore.

## File on disk

Bund Script nodes are stored as `.bund` files under
`books/<containing-book>/` in the project tree, just like
paragraphs (`.typ`) and HJSON data nodes (`.hjson`). Backups
round-trip them; `inkhaven reindex` picks up edits made outside
the TUI.
