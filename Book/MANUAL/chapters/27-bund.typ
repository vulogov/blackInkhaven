#import "../design.typ": *

#chapter(number: 27, title: "Scripting with Bund")

Everything in this manual so far has been a command someone at Inkhaven
already wrote for you — a chord, a subcommand, a scan. This chapter is the
one place the tool hands you the keys and lets you write commands of your
own. *Bund* is a small scripting language embedded inside Inkhaven: you can
run a line of it against your project, bind it to a chord, or — most
usefully — have it fire automatically the moment you save a paragraph, take
a snapshot, or hit your daily goal. It is the escape hatch for the author
whose workflow has outgrown the built-in chord set and wants the tool to do
something nobody at Inkhaven thought to build.

You do not need Bund to write a book. The whole of Parts I through VII
assumes you never touch it, and most authors never will. Reach for this
chapter when you catch yourself doing the same small chore by hand every
day — "warn me whenever a scene mentions a character I haven't introduced,"
"tag every paragraph in this chapter as needing a second pass," "keep a
fresh story-graph PNG on my desktop" — and want it to happen on its own.

#section("What Bund is")

Bund is a stack-based language Vladimir Ulogov wrote and open-sourced (it
lives at `github.com/vulogov/bundcore`). Inkhaven embeds three of its
crates — `bundcore`, `bund_language_parser`, and `rust_multistackvm` —
and layers its own vocabulary on top. The language itself is
#link("https://en.wikipedia.org/wiki/Forth_(programming_language)")[Forth]-shaped:
postfix operators, two stacks, curly-brace lambdas. It is small enough to
learn in an afternoon, and this section is enough to read every example in
the chapter.

#subsection("Numbers go on, operators pull off")

Bund is *postfix*. You push operands first; the operator pulls them back
off. Where you would write `2 + 3` elsewhere, in Bund you write:

```bund
2 3 +          // push 2, push 3, + pops both, pushes 5
```

After that line the stack holds a single value, `5`. Longer expressions
chain the same way, left to right:

```bund
2 3 +  4 *      // (2 + 3) then times 4  →  20
40 2 +          // →  42
```

There is one Forth gotcha worth naming up front: operands pop in
stack order, so `a b /` computes `b ÷ a`, not `a ÷ b`. Trust the stack,
not your infix instinct. Strings use double quotes, and `println` pops one
value and prints it with a newline:

```bund
"Hello, Inkhaven!" println
```

Comments run from `//` to end of line, exactly as above.

#subsection("Shuffling the stack")

A handful of words rearrange what is already on the stack. You will meet
all of them in real hooks, because a hook is handed its arguments on the
stack and has to move them into position.

#screen(caption: "The five stack-shuffling words")[```
  dup    ( a -- a a )       duplicate the top
  drop   ( a -- )           discard the top
  swap   ( a b -- b a )     swap the top two
  over   ( a b -- a b a )   copy the second up
  rot    ( a b c -- b c a ) rotate three
```]

The parenthesised note is a *stack diagram*: what the word expects on the
left of `--`, what it leaves on the right. The whole of Bund's vocabulary
is documented this way, and so is every hook in this chapter. To square the
top of the stack you duplicate it and multiply:

```bund
5 dup *         // →  25
```

#subsection("Lambdas and named words")

Curly braces wrap a block of code into a *lambda* — a first-class value you
can push around like a number. Give one a name with `register` and it
becomes a word you can call:

```bund
"square" { dup * } register
5 square        // →  25
9 square        // →  81
```

This is the mechanism behind every hook in the chapter: a hook is simply a
lambda you have registered under a well-known name like `hook.on_save`.
When the matching thing happens, Inkhaven calls that name for you.

#term("The Adam VM")[
  Every line of Bund in your Inkhaven session runs against a single,
  process-wide virtual machine that bundcore calls *Adam* — the first VM
  constructed in the process. The CLI `inkhaven bund`, every `Ctrl+Z R`
  buffer run, every hook fire, and every keymap mutation all share it.
  Because Adam is one persistent instance, state carries across calls: a
  word you `register` once is available everywhere for the rest of the
  session. Adam is built lazily on the first evaluation, then frozen —
  stdlib loaded, policy applied, your scripts run — for the life of the
  process.
]

#subsection("What Inkhaven adds on top")

Bundcore gives you the language: arithmetic, strings, conditionals,
lambdas, math, time, the stack words above. Inkhaven adds two namespaces of
its own on top of that base:

- The `ink.*` standard library — words that reach into *your project*:
  read a paragraph's text, search the manuscript, add a tag, run a
  continuity sweep, write a file. This is the bulk of the chapter.
- The `hook.*` names — the well-known lambda names Inkhaven calls when
  something happens in the editor.

Everything else — how the language parses, how `if` and `while` work, how
you build a list — is vanilla bundcore, documented upstream at
`docs.rs/bundcore`.

#section("The four ways to run a script")

There are four surfaces from which Bund reaches Adam, and they differ only
in where the code comes from and when it runs.

#chord_table((
  ("inkhaven bund", "One-shot from the shell — evaluate, print the top of the stack, exit."),
  ("Ctrl+Z E", "Eval modal — type an expression, Enter, result on the status bar."),
  ("Ctrl+Z R", "Run the whole open .bund buffer against Adam."),
  ("Ctrl+Z N", "Create a new .bund Script node under the Scripts book."),
))

Beyond those four, two surfaces run code *for* you without a keystroke: the
`scripting.bootstrap` string in `inkhaven.hjson`, which runs once when the
project opens, and every `.bund` Script node in the tree, which is
evaluated at project open right after the bootstrap. Those two are where
real hooks live, and they are the subject of the next two sections.

The quickest way to see Bund work at all is the shell:

#screen(caption: "One-shot evaluation from the terminal")[```
  $ inkhaven bund "40 2 +"
  42

  $ inkhaven bund '"hello" println "world" println'
  hello
  world
```]

Inside the TUI, `Ctrl+Z E` pops a one-line prompt, runs what you type
against Adam, and puts the result on the status bar — the closest thing to
a read-eval-print loop. Pair it with `println` when you want to watch a
list go by:

```bund
"" ink.node.children dup println
```

The `dup` keeps the list on the stack so the status bar still shows it
after `println` has already printed it once.

#section("The .bund Script node")

A one-liner in the eval modal vanishes when you press Enter. Anything you
want to *keep* — a hook, a custom word, a small automation — lives in a
*Script node*: a first-class node in your project tree, stored on disk as a
`.bund` file, edited in the normal editor, and backed up with the rest of
the manuscript.

#term("Script node")[
  A Script node is a tree node of kind `Script`, holding Bund source
  instead of Typst prose. It sits alongside your paragraphs (`.typ`) and
  data nodes (`.hjson`) as a first-class citizen: it round-trips through
  backups, `inkhaven reindex` picks up edits made outside the TUI, and
  the editor knows to run it as Bund rather than typeset it. Its natural
  home is the `Scripts` system book, but it can live inside any user Book
  whose workflow it belongs to — nothing forces it under `Scripts`.
]

Press `Ctrl+Z N` from any pane and the Add modal opens pre-pointed at the
`Scripts` system book. Give the node a title, and it opens in the editor as
an empty Bund buffer. Write your script, save it like any paragraph, and
run it on demand with `Ctrl+Z R`. If you started a node as prose and want
to convert it — or the reverse — the tree's *morph* operation turns a
`Paragraph` into a `Script` and back without losing the body.

The important behaviour is what happens at *project open*. When Inkhaven
builds Adam, it walks the whole tree, finds every Script node in tree
order, and evaluates each one. That is the mechanism that makes hooks
persistent: you register `hook.on_save` inside a Script node once, and from
then on every project open re-installs it before you have typed a word. A
syntax error in one script logs a warning and is skipped; the others still
load, and Adam still finishes building.

#callout(label: "Order of loading")[
  At project open Adam is assembled in a fixed order: bundcore's stdlib,
  then Inkhaven's `ink.*` words, then the sandbox policy is applied, then
  the inline `scripting.bootstrap` runs, and *finally* every Script node is
  evaluated in tree order. So a Script node can rely on a word the
  bootstrap defined, but not the other way around.
]

#section("The trust gate")

Auto-running code that ships inside a project is a real risk: open a
project someone emailed you and, without a gate, its Script nodes would
execute the instant you opened it. Inkhaven closes that door with a
*trust decision*, set by `scripting.trust_decision` in `inkhaven.hjson`,
with three values:

#screen(caption: "scripting.trust_decision — three values")[```
  "ask"    (default) run scripts ONLY when the file
           <project>/.inkhaven/trust exists and holds
           the marker line `trust`.  Otherwise the
           scripts are pending your opt-in.
  "trust"  run scripts unconditionally.  Use only on
           projects you authored or audited.
  "deny"   never run scripts, trust file or not.
           Good for opening a project to read only.
```]

Under the default `"ask"`, a project you did not write opens with its
scripts *inert*: you get a status-bar notice that scripts are pending, and
nothing runs until you deliberately create `.inkhaven/trust` with the
marker line inside it. The marker match is case-insensitive and ignores
surrounding whitespace; a trust file that says anything else does not grant
trust. Your own projects, whose scripts you wrote, are the ones where
setting `"trust"` in the HJSON is appropriate.

#callout(label: "The honest limitation")[
  A malicious project's own HJSON could set `trust_decision: "trust"`.
  The gate is aimed at the author publishing their own work, not at a
  cryptographic guarantee against a hostile package. If you open a project
  you did not write, leave the default `"ask"` in place and read its Script
  nodes before you create the trust file. The `"deny"` value exists for
  exactly the read-only-review case.
]

#section("Hooks — code that runs when something happens")

A hook is the natural home for a "warn me when…" rule. You register a
lambda under a well-known name; Inkhaven calls that name *after* the
matching editor action has already succeeded, pushing the action's details
onto the stack for your lambda to inspect. The lambda's job is to observe
and react — print a warning, add a tag, speak a line aloud — not to undo
what happened.

Here is the complete set of hook points, each with the stack it is handed
on entry:

#screen(caption: "Store-mutation hooks")[```
  hook.on_create           ( uuid kind -- )
  hook.on_save             ( uuid -- )
  hook.on_rename           ( uuid new_title -- )
  hook.on_snapshot         ( parent_uuid snap_uuid -- )
  hook.on_delete           ( uuid -- )
  hook.on_status_promoted  ( uuid from_status to_status -- )
```]

#screen(caption: "Session and milestone hooks")[```
  hook.on_goal_hit         ( today_words daily_goal -- )
  hook.on_active_goal_hit  ( active_secs goal_secs -- )
  hook.on_streak_break     ( prev_streak_days -- )
  hook.on_streak_milestone ( milestone_days -- )
  hook.on_diagnostic       ( uuid count first_message -- )
```]

#screen(caption: "Timeline and production hooks")[```
  hook.on_event_added      ( uuid -- )
  hook.on_event_orphaned   ( uuid -- )
  hook.on_assemble ( uuid slug root_typ files_written -- )
  hook.on_take             ( uuid slug pdf_dest -- )
```]

A few of these repay a note. `hook.on_status_promoted` fires both when you
cycle a paragraph's status by hand (`Ctrl+B R`) and when it auto-promotes
on hitting its goal; the status strings arrive lowercased (`napkin`,
`first`, …, `ready`). `hook.on_goal_hit` fires the first time the day's
word count crosses your `goals.daily_words` line and self-resets if you dip
back below, so it never nags. `hook.on_streak_milestone` fires once per
upward crossing of 7 / 30 / 100 / 365 days — a celebration, never a
blocker. `hook.on_diagnostic` is debounced: it fires only when Typst's
diagnostic state actually changes, not on every idle tick.

#subsection("How a hook behaves when it misbehaves")

Three guarantees make hooks safe to write casually. They matter enough to
state plainly:

- *A hook never breaks the editor.* Every failure path — a syntax error, a
  stack underflow, even a panic inside your lambda — is caught, logged at
  WARN, and swallowed. The save (or rename, or snapshot) that fired the
  hook still completes. You find the failure in `.inkhaven.log`, not in a
  corrupted file.
- *Hooks are bounded against runaway recursion.* A hook that triggers
  another mutation that fires another hook is capped at a depth of four;
  past that the dispatch is skipped and logged. And when a hook fires from
  inside a running `bund` evaluation — because your script itself called a
  write word — it short-circuits rather than re-enter the VM's write lock.
- *Hook output goes to the log, not the void.* Anything a hook `println`s
  is drained after the fire and emitted through tracing: it lands in
  `.inkhaven.log` in the TUI and on stderr from the CLI. Pre-1.2.6 that
  output vanished silently; now it is findable.

One consequence is worth internalising: hooks fire *synchronously* on the
thread that did the save, holding Adam's write lock. A slow hook is a slow
save, felt as a pause under your fingers. Keep hook bodies cheap; push
anything expensive to a command you run on demand.

#subsection("Registering a hook")

For a one-line rule, the `scripting.bootstrap` string in `inkhaven.hjson`
is enough:

```bund
"hook.on_save" { drop "saved" println } register
```

The `drop` throws away the paragraph UUID this hook is handed — this
trivial version does not need it. For anything longer, put the same
`register` call in a `.bund` Script node so it lives with the manuscript
and gets backed up. Both are evaluated at project open, bootstrap first.

#section("The ink.* standard library")

The `ink.*` namespace is where Bund stops being a pocket calculator and
becomes a way to *reach into your book*. It is large — dozens of families —
because nearly every Inkhaven feature that has a CLI surface also has a Bund
surface built on the same core, so a script, the CLI, and the TUI all agree.
This section surveys the shape rather than enumerating every word; the
per-feature chapters and `Documentation/Bund/BUND_TUTORIAL.md` give the
exact stack diagrams.

#subsection("The families, by what they touch")

#screen(caption: "The tree, prose, and metadata")[```
  ink.node.*       list / get / children of nodes
  ink.paragraph.*  text / target / set_target / save /
                   set_status
  ink.tree.*       add / delete / rename / move / morph
  ink.tag.*        list / search / add / remove
  ink.event.*      story-timeline events + critique
  ink.snapshot.list  ink.path.to_uuid
```]

#screen(caption: "Search, retrieval, and the AI")[```
  ink.search.*     semantic search: text / load
  ink.book_rag.*   Book-scope retrieval + grounding
  ink.ai.*         history / send / send_blocking /
                   poll / set_system_prompt
```]

#screen(caption: "The reading intelligences (read-only)")[```
  ink.continuity.*  ink.knowledge.*  ink.readthrough.*
  ink.chorus.*      ink.stylist.*    ink.revise.*
  ink.chronicle.*   ink.char.*       ink.dialogue.*
  ink.prose.*       ink.graph.*      ink.myth.*
  ink.utopia.*      ink.theologian.* ink.outline.*
  ink.inner_editor.*   ink.inner_socrates.*
  ink.sources.*  ink.terms.*  ink.snippets.*
  ink.review.*   ink.thread.list
```]

#screen(caption: "Language, verse, output, and files")[```
  ink.lang.*   the full conlang suite (dozens of words)
  ink.poem.*   syllables / scansion / rhyme / findings
  ink.io.*     print / log / notify / message.*
  ink.pane.*   ink.input     the Bund output pane + modal
  ink.editor.* cursor / text / find / goto / insert /
               replace / replace_all / scroll / delete
  ink.fs.*     read / write        ink.tts.speak
  ink.pdf.*    ink.export.*  ink.typst.*  ink.db.*
  ink.story.render  ink.key.*  ink.theme.set
```]

Most of the reading-intelligence families follow one pattern: a handful of
read words that pull findings or a report and push them as dicts, plus at
most one `suppress` word that records an author decision. So
`ink.continuity.findings`, `ink.knowledge.check`, `ink.chorus.voices`,
`ink.revise.findings` all behave alike — call it, get a list of
`{id, …, observation}` dictionaries back, walk them. The deliberately-costly
parts (the LLM coherence pass, the AI editorial letter, any prose rewrite)
are *not* exposed to Bund; the deterministic core is.

#subsection("Reading the result of a word")

Every `ink.*` word that returns structured data returns a Bund list of
dicts. From the CLI those pretty-print as JSON, which makes the terminal the
fastest way to see what a word actually gives you:

#screen(caption: "Inspecting a word's output from the shell")[```
  $ inkhaven --project ~/my-book \
      bund '"" ink.node.children'
  [ … JSON list of every root node … ]

  $ inkhaven --project ~/my-book \
      bund '"manuscript" "the storm" book_rag.context'
  [ … the exact grounding block Book chat would use … ]
```]

That second line is genuinely useful on its own: it prints the grounding
block Book-scope chat would feed the model for that query, straight from
the terminal, no TUI needed.

#section("The sandbox policy")

The `ink.*` surface includes words that write files, mutate your tree,
rebind your chords, and call the LLM. Left ungated, a save-hook you pasted
from a tutorial could quietly do any of those. So every `ink.*` word is
sorted into a *category*, and a conservative subset of categories is denied
by default. You opt in explicitly, in HJSON, category by category.

#subsection("The categories")

#screen(caption: "Default-ALLOWED — safe, non-destructive")[```
  store_read    read nodes, tags, findings, search
  fs_read       read a file
  editor_read   query the buffer / cursor / pane
  ai_read       read chat history
  audio         TTS playback (also gated by HJSON)
```]

#screen(caption: "Default-DENIED — opt in per family")[```
  store_write   mutate tree, tags, events, status
  editor_write  insert / replace / scroll the buffer
  ai_write      send a prompt, set the system prompt
  fs_write      write a file, export, render a PNG
  theme_write   recolour the interface
  keymap        rebind chords via ink.key.*
  net  shell  code_eval   reach outside the project
```]

The rule of thumb is exactly what it looks like: *reads are open, writes are
shut.* A word like `ink.continuity.findings` is `store_read` and works out
of the box; `ink.tag.add` is `store_write` and errors until you enable it.
You turn a family on with one line:

```hjson
scripting: {
  enabled_categories: ["store_write", "fs_write"]
}
```

That grants every `store_write` word (tag and event mutations, tree edits,
status changes) and every `fs_write` word (file writes, exports,
`ink.story.render`) at once.

#subsection("Finer control, and the resolution order")

Four knobs sit under `scripting` for cases where a whole category is too
coarse. `enabled_categories` and `disabled_categories` flip a family on or
off; `enabled_words` allows a single word even when its category is denied
(grant `ink.fs.read` without opening all of `fs_read`); `disabled_words`
denies a single word regardless of its category. When a word is looked up,
the order is fixed: an explicit `enabled_words` entry wins, then
`disabled_words`, then a denied category, otherwise allow. A power-user
`no_default_deny: true` clears the built-in baseline entirely — off by
default, and rarely what you want.

When a denied word runs, it does not silently do nothing — it returns a
clean error, `script denied by inkhaven policy`, and the specific word name
is written to `.inkhaven.log` at the moment the policy was applied. If a
script mysteriously stops, the log names the offending word.

#subsection("Filesystem confinement")

Enabling `fs_read` or `fs_write` answers "may this script touch the
filesystem at all?" — a separate question from "*where*?" By default,
paths handed to `ink.fs.read` and `ink.fs.write` are confined to the
project root: a script cannot read your home directory or write outside the
book, even with the category enabled. Setting `fs_unsandboxed: true`
collapses that confinement to "anywhere this user can reach" — the
pre-1.2.15 behaviour, recommended only for trusted projects that genuinely
need a shared location outside the tree.

#callout(label: "The every-word-classified guard")[
  A word registered but *forgotten* from the category table would be
  silently allowed — escaping `disabled_categories` entirely. Inkhaven
  forbids that with a test, `every_registered_word_is_classified`, that
  walks every `ink.*` word Adam registers and fails the build unless each
  one is either given a category or listed in an explicit
  "pure — touches nothing protected" allowlist (the in-memory PDF ops, the
  `ink.lang.dict` constructor). A new word cannot ship un-gated by
  accident.
]

#section("Worked examples")

Two complete scripts, each small enough to type, show the pieces working
together.

#subsection("A save-hook that watches for unintroduced names")

This registers `hook.on_save` to pull the just-saved paragraph's text and
check it — the sketch below keeps the parsing trivial and leaves the
name-matching to `ink.search.text` against your Characters book:

```bund
"hook.on_save" {
  // ( uuid -- )   fired after a paragraph saves
  dup ink.paragraph.text     // ( uuid text )
  swap drop                  // ( text )   keep the body
  // … scan `text` for names, search Characters,
  //   ink.io.notify when one is missing …
  drop
} register
```

Registered in a Script node, this reinstalls itself at every project open
(once the project is trusted). It reads only — `ink.paragraph.text` and
`ink.search.text` are both `store_read` — so it needs no category opt-in.

#subsection("Tagging scenes for a pass")

This one *does* write, so it needs `store_write` enabled. `ink.tag.add` takes
a paragraph's *slug path* — the bracketed path `inkhaven list` prints — and a
tag name, and adds the tag to that paragraph. Marking the scenes you mean to
revisit is the kind of chore worth scripting:

```bund
// ( path tag -- )  — root-anchored slug path, then the tag.
"the-ninth-lantern/03-the-mole/opening"  "editing-pass-2" ink.tag.add
"the-ninth-lantern/03-the-mole/the-turn" "editing-pass-2" ink.tag.add
```

Each line pushes a path and a tag and calls `ink.tag.add`, which resolves the
path to a paragraph and records the tag. (To *discover* paths programmatically,
`"" ink.node.children` walks the top-level books and each node dict carries its
`id`, `kind`, `title`, and `slug`.) Run it once from `Ctrl+Z R`, or wrap it in
a word you call when you start a pass.

#section("The inkhaven bund CLI")

The `bund` subcommand runs a script headless, with no TUI, and prints the
top of the stack when it exits:

#screen(caption: "inkhaven bund — headless evaluation")[```
  $ inkhaven bund "2 3 + 4 *"
  20

  $ inkhaven --project ~/my-book \
      bund '"morning" 5 ink.search.text'
  [ … five JSON search hits … ]
```]

Two behaviours are worth knowing. First, whether `ink.*` words work depends
on the working directory: if it (or `--project`) is an initialised Inkhaven
project, the store is opened — which arms the scripting layer automatically
— and the project words come alive. Against a plain directory the script
runs on the bare Adam VM, so pure arithmetic and strings work but `ink.*`
words error cleanly. (Opening a project loads the embedding model, a
few-seconds cost the arithmetic-only path skips.) Second, output ordering
is faithful: any `print`/`println` the script produced is emitted first, in
script order, then the top-of-stack value on its own line — so a script's
narration and its result never smear together. A script that leaves an
empty stack and printed nothing reports `(no result)`.

The CLI is the way to fold Bund into a shell pipeline, a Makefile, or a
cron job: because Adam and the store are the same as the TUI's, a headless
`inkhaven bund` sees exactly the book your editor does.

#recap((
  [*Bund* is a small, Forth-shaped, stack-based language embedded in
   Inkhaven for hooks, custom rules, and automation — postfix operators,
   two stacks, curly-brace lambdas registered as named words.],
  [Every session shares one process-wide VM, *Adam*: state persists across
   calls, and it is built once at project open — stdlib, then policy, then
   bootstrap, then every Script node in tree order.],
  [A *`.bund` Script node* keeps a script with the manuscript; auto-loading
   it at open is gated by the *trust decision* (`ask` / `trust` / `deny`
   plus the `.inkhaven/trust` marker file), so a foreign project's code
   never runs unasked.],
  [*Hooks* are lambdas you register under well-known names —
   `hook.on_save`, `hook.on_snapshot`, `hook.on_streak_milestone`, and the
   rest — fired after the matching action; failures are always logged and
   swallowed, never breaking a save.],
  [The *`ink.*` standard library* reaches into your project — tree, prose,
   tags, timeline, search, the reading intelligences, language, files, the
   AI — with reads open and writes shut by default.],
  [The *sandbox* sorts every word into a category, denies the destructive
   ones until you opt in per family in HJSON, confines `ink.fs.*` to the
   project root, and a build-time guard forbids any word from shipping
   un-classified.],
  [`inkhaven bund "<code>"` runs a script headless against the same Adam
   and store the TUI uses — the way to fold Bund into a shell pipeline.],
))
