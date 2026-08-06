#import "../design.typ": *

#chapter(number: 29, title: "Configuration")

Every Inkhaven project keeps its settings in one plain-text file at its root:
`inkhaven.hjson`. It is the single place you tell the tool which language you
write in, which AI provider to reach for, what colours to paint, how often to
back up, and a hundred smaller things. This chapter is the guided tour of that
file — what it is, how Inkhaven reads it, how to change it without breaking it,
and the handful of blocks you are actually likely to touch. It is deliberately
*not* the exhaustive list: every field, its type, and its default lives in
*Appendix C*, and this chapter points you there again and again. Read this
for the shape of the thing; reach for the appendix when you need a specific
knob.

The reassuring news, before any detail: you can write an entire book and never
open this file. Inkhaven writes a complete, working `inkhaven.hjson` when you
first create a project, and every value in it has a sensible default. The
configuration is there for when you want to bend the tool to your hand — not a
gate you must pass through first.

#section("What inkhaven.hjson is")

When you run `inkhaven init`, the tool writes a full, annotated configuration
file into the new project's root, copied verbatim from a template baked into the
binary. That file is yours from then on — Inkhaven reads it but, with one narrow
exception we will come to, never rewrites it behind your back. It sits beside
your manuscript in the project tree, so it travels with the book: clone the
folder, and the settings come along.

#term("HJSON")[
  Inkhaven's config is written in #link("https://hjson.github.io/")[HJSON] — a
  relaxed superset of JSON meant for humans to edit. It is JSON with the sharp
  edges filed off: comments are allowed, keys need no quotes, commas are
  optional, and strings can span many lines. Any strict JSON is already valid
  HJSON, so nothing you know about JSON stops working.
]

The relaxations matter because you *will* read this file more than you write it,
and comments are how it explains itself. Here is a small but real slice:

#screen(caption: "inkhaven.hjson — the HJSON dialect at a glance")[```
// inkhaven.hjson — the whole project's configuration.
// Lines beginning with // are comments, ignored on read.
{
  language: english            // keys and values need no quotes
  genre:    literary_realism

  goals: {
    daily_words: 1500          // trailing commas are optional
    books: {
      story: { target_words: 80000 }
    }
  }

  // A multiline value uses triple-quote fences:
  note:
    '''
    Everything between the fences is kept verbatim,
    line breaks and all — handy for a long template.
    '''
}
```]

Three habits carry you through almost every edit. Comment a line out with `//`
rather than deleting it, so you can put it back. Leave keys and simple values
unquoted; quote only when a value contains characters that would confuse the
parser (a leading `#`, a stray colon) or when you want a string to stay exactly
as typed — durations like `"7d"` and hex colours like `"#1e1e2e"` are quoted for
that reason. And group related settings inside `{ … }` blocks, exactly as the
shipped file does.

#section("How Inkhaven reads it: the layered config")

Configuration is read *once*, at launch, and then handed — as an immutable,
cloned value — to every subsystem that needs it: the editor, the AI client, the
theme renderer, the backup hook. Nothing re-reads the file mid-session, which is
why a change only takes effect on the next launch. But "the file" is a small
simplification. What Inkhaven actually assembles, through `Config::load_layered`,
is a *stack* of sources, lowest priority to highest:

#screen(caption: "Precedence — each layer overrides the one above it")[```
  built-in defaults          (compiled into the binary)
    +-- project  inkhaven.hjson             (the full config)
          +-- ~/.config/inkhaven/config.hjson       (partial)
                +-- ~/.config/inkhaven/conf/*.hjson  (partial,
                                       sorted by name; last wins)
```]

Read the ladder from the top down. First come the *compiled-in defaults* — every
field the tool knows about, already filled in. On top of those, your project's
`inkhaven.hjson` is merged, key by key. Then, if they exist, your *user-global*
override files are layered on last, so a global file wins over the project. That
ordering is deliberate: because `inkhaven init` writes a complete project config,
a project-wins rule would bury any personal preference you set globally. The
global files are *partial* — you put in only the keys you want to change
everywhere, and everything else falls through to the project. It is the way to
carry one theme, or one keybinding, across every book you write without editing
each project by hand.

Two properties of this merge are worth holding onto. First, *missing fields
never break anything.* Every field is optional and falls back to its default, so
a config written by an older release keeps working when a new release adds
fields, and a partial global file is exactly that — partial. A typo'd key is
simply ignored: harmless, but also silently ineffective, so if a setting seems
not to take, check the spelling against Appendix C. Second, *a broken global file
is skipped, not fatal.* A malformed override under `~/.config/inkhaven/` is
logged as a warning and passed over — one stray brace there must never brick
every project you own. A malformed *project* `inkhaven.hjson`, by contrast, is a
hard error that stops the launch with a clear message, because that file is
load-bearing for the one project in front of you.

#callout(label: "The security floor")[
  The merge has one carve-out. Object blocks merge key by key, but a *list*
  replaces the list beneath it wholesale — so setting `shell.blocked_externals`
  to add one program would otherwise wipe the shipped block-list that keeps
  `vim`, `ssh`, `sudo` and friends out of the embedded shell. After every load
  and merge, Inkhaven unions those shipped entries back in. You can always *add*
  to the block-list; you cannot accidentally erase its floor. It is the one place
  the otherwise-permissive config refuses to let you shoot yourself in the foot.
]

#section("Editing it safely")

There are two honest ways to change the file, and Inkhaven supports both.

#subsection("From inside the editor — Ctrl+B 0")

The chord `Ctrl+B 0` opens `inkhaven.hjson` in a full-screen modal editor,
without ever leaving the app. It is a real text editor over the raw file — HJSON
syntax highlighting, and the same movement and editing chords the manuscript
editor uses (arrows, `Home` / `End`, word jumps, `Ctrl+U` undo, `Ctrl+Y` redo,
selection with `Shift`, and so on). Because you are editing the file's actual
text, your comments and formatting are preserved by definition — nothing is
re-serialised.

#screen(caption: "Ctrl+B 0 — editing the config in place")[```
+- Edit . inkhaven.hjson ------------------------------+
|  1  {                                                |
|  2    language: english                              |
|  3    llm: { default: gemini }                       |
|  4    backup: { max_age: "7d" }                      |
|  5  }                                                 |
|                                                      |
|  Ctrl+S save    Ctrl+R review    Esc close           |
+------------------------------------------------------+
```]

`Ctrl+S` saves. If the saved bytes differ from what was loaded, a *Restart
required* overlay appears, because — as we saw — the config is read only at
launch. Your edit is on disk; it simply applies the next time you start Inkhaven.

#screen(caption: "The restart-required overlay after a real change")[```
+- Restart required ----------------------+
|  Saved. Configuration is read once, at  |
|  launch. Restart inkhaven to apply.     |
|             [ press any key ]           |
+-----------------------------------------+
```]

Two more keys earn their place in this modal. `Ctrl+R` fires an *LLM review* of
the buffer you are looking at — the same "reviewer model, never an executor"
pattern the prompts editor uses. The model reads your config and streams back
notes (a suspicious value, a key that looks misspelled, a block that contradicts
another); the response lands in the AI pane, visible once you close the modal. It
advises, it never writes. And `Esc` closes the editor, warning you on the status
line if you have unsaved edits.

#callout(label: "How features write config for you")[
  A few features edit the config on your behalf — the goals editor, and CLI
  helpers like `show-dont-tell bootstrap --update`. They do not re-serialise the
  whole file (that would flatten your comments). Instead they perform a
  *surgical splice*: each targeted leaf is patched in place if it already exists,
  or appended inside its parent block if it does not, leaving every comment and
  every untouched key exactly where it was. Before the write, the pre-patch file
  is copied to `<project>/.config-backups/`, and the new file is written
  atomically — so an interrupted patch can never truncate your config, and a
  roll-back is one `cp` away.
]

#subsection("By hand, in any editor")

Because it is plain text, you can also open `inkhaven.hjson` in whatever editor
you like — including Inkhaven's own embedded shell, or the manuscript editor if
you add the file to the project. Two safety rails are worth knowing. You can
*validate* a config without launching the TUI by running any read-only command
against the project and discarding its output; if the file is malformed the CLI
prints a `config error: …` and exits non-zero. And if you ever want to start
over, rename the file and run `inkhaven init --force` to lay down a fresh
default template, then re-merge your customisations from the old copy. Inkhaven
never edits the file in place except through the surgical splice above, so what
you write is what stays.

#section("A tour of the blocks you will touch")

The full file has dozens of blocks; most you will never open. What follows is
the short list of the ones that reward a visit, each with the one or two knobs
that matter and a pointer to where the feature itself is documented. For the
complete field list, types, and defaults of any block, see *Appendix C*.

#subsection("language and genre — who the book is for")

Two top-level fields set the project's identity. `language` (default `english`)
is the primary writing language; it drives the Snowball stemmers behind the
Places and Characters highlight overlays, the default grammar-check prompt, and
the language of every style word-list. It accepts the Snowball set — `english`,
`russian`, `french`, `german`, `spanish`, and more. `genre` (default unset, i.e.
genre-blind) names the book's genre, e.g. `literary_realism` or `fantasy`; the
Inner Editor and other readers use it to tune their prompts. Change `language`
and restart, and the whole tool speaks your language. See the chapters on the
editor overlays and the inner readers; full list in Appendix C.

#subsection("llm — which AI answers")

The `llm` block lists your AI providers and picks a default. Out of the box it
ships five ready entries — `gemini` (the default), `claude`, `openai`,
`deepseek`, and `grok` — each naming a `model` and the environment variable that
holds its API key (`api_key_env`). Switching the default AI for the whole project
is a one-word edit: set `default: claude` and restart. Local providers like
Ollama need no key — omit `api_key_env` entirely. If a provider's key variable is
unset at runtime, Inkhaven declines cleanly rather than crashing, and by default
(`auto_fallback: true`) it will reach for any other provider whose key *is*
present. The AI chapters in Part III cover scopes, chat, and cost; the per-field
reference is in Appendix C.

#subsection("editor — how the writing surface behaves")

The `editor` block governs the manuscript pane's behaviour (its *colours* live
in `theme`). The knobs you are most likely to touch: `wrap` (default `true`, soft
word-wrap), `autosave_seconds` (default `5` — idle seconds before a dirty
paragraph saves; `0` disables idle autosave, though save-on-switch and
save-on-quit still fire), `tab_width` (default `2`), and `auto_close_pairs`
(default `true`). Nested under it is `style_warnings` — the inline overlays that
underline weak prose. The whole family is *off by default* (`style_warnings.
enabled: false`); turn it on and the sub-detectors (filter words, repeated
phrases, show-don't-tell, anachronism) light up, each individually tunable and
each keyed to your project `language`. The Writing chapters in Part II cover the
editor and these overlays end to end; Appendix C has every sub-knob.

#subsection("theme — every colour on screen")

The `theme` block sets every colour the TUI paints — pane backgrounds and
foregrounds, the five border states, modal windows, the syntax highlighter, the
lexicon overlays. The shipped defaults are a #link("https://catppuccin.com/palette/")[Catppuccin
Mocha] dark palette, tested across many terminals. Values are hex strings
(`"#RRGGBB"` or the short `"#RGB"`); an *empty string or an unparseable value*
falls back to the baked-in default, which is exactly what makes a partial global
theme override work — you name only the handful of colours you want to change.
This is the block most people put in a user-global file so one palette follows
them across every book. The panes chapter shows what each colour paints; the full
colour table is Appendix C.

#subsection("backup — the safety net")

The `backup` block drives both the `inkhaven backup` command and the auto-backup
that fires when you quit. `out_dir` (default empty, meaning a
project-sibling `inkhaven-backups/<project-name>/` directory next to your
project) is where `.zip` snapshots land; `max_age`
(default `"7d"`, humantime-parsed, so `"24h"` or `"12h"` also work) is how stale
the last backup may get before the exit hook makes a fresh one; `keep_last`
(default `0`, meaning keep all) prunes older archives; and `auto_backup_on_exit`
(default `true`) is the clean on/off switch. The Backup chapter earlier in this
part walks the whole cycle; the fields are in Appendix C.

#subsection("goals — writing targets and pace")

The `goals` block fuels the status-bar progress widget and the progress modal.
Every field is optional and *zero or empty disables that particular goal while
still recording history*, so the modal always has something to show. The common
ones: `daily_words` (default `0`), `active_minutes_daily`, `streak_grace_per_week`
(rest days forgiven per rolling week), and per-book `target_words` and
`deadline` under `books.<slug>`. `auto_promote_on_target` (default `true`)
nudges a paragraph's status up a rung when it crosses its word target. The
writing-goals material covers the workflow; Appendix C lists the fields.

#subsection("ai — the AI pane's behaviour")

Distinct from `llm` (which picks the *provider*), the `ai` block tunes how the AI
pane *behaves*. The one most writers care about is `diff_review_on_apply`
(default `true`): every AI edit to your prose is routed through a side-by-side
diff you must accept before a single byte changes — the tool's standing promise
that the AI never writes to your manuscript unasked. `per_paragraph_memory`
(default `false`) opts a paragraph into remembering its own chat turns. Part III
covers the AI pane; the block's fields are in Appendix C.

#subsection("embeddings — the semantic index")

The `embeddings` block controls how your prose is turned into vectors for
semantic search. `model` (default `MultilingualE5Small`) picks the fastembed
model — keep a multilingual one if you write in any non-English language. The
rest (`chunk_size` `800`, `chunk_overlap` `0.15`, `pool_size` `4`) rarely need a
hand. Changing the model triggers a one-time download and a re-index of your
prose; run `inkhaven reindex` after. The search chapter in Part II covers the
index; Appendix C has the model list.

#subsection("keys — rebinding the chords")

A short list of global chords is rebindable through `keys`: `save` (`Ctrl+s`),
`search` (`Ctrl+/`), `ai_prompt` (`Ctrl+i`), pane cycling, and — most usefully —
the three *prefix* chords `meta_prefix` (`Ctrl+b`), `bund_prefix` (`Ctrl+z`), and
`view_prefix` (`Ctrl+v`). If your terminal multiplexer eats `Ctrl+B` — `tmux`
claims it by default — set `meta_prefix: Ctrl+g` and every Meta chord follows.
Deeper sub-chord remapping lives in the `keys.bindings` overlay list. The
Keybinding chapter and Appendix A carry the full chord map; the config fields are
in Appendix C.

#section("The principle behind all of it")

One idea runs under every block above, and it is worth naming because it explains
the defaults. Inkhaven is a *permissive* tool: it informs, it does not block.
Almost everything is off, or set to its safe default, until you opt in — the
style overlays, per-paragraph memory, the health monitor, the timeline, the
embedded shell's riskier corners. Cost caps and budget settings *warn* you and
colour a chip; they never refuse to run the thing you asked for. The one place
the tool overrides you is the security floor on the shell block-list, and even
there it only *adds* protection, never removes a choice. The config file is a set
of dials you may turn, not a form you must fill in — which is exactly why you can
write a whole book without opening it, and bend the tool to your hand the day you
decide to.

#recap((
  [Every project keeps its settings in `inkhaven.hjson` at its root — written
  complete by `inkhaven init`, in the comment-friendly *HJSON* dialect, and read
  once at launch.],
  [Config is *layered*: compiled-in defaults, then the project file, then partial
  user-global overrides that win — so missing fields never break, and a broken
  global file is skipped while a broken project file is fatal.],
  [Edit it safely two ways: `Ctrl+B 0` opens a full-screen editor (`Ctrl+S` save,
  `Ctrl+R` LLM review, a *restart-required* overlay), or hand-edit any way you
  like; feature-driven edits use a comment-preserving surgical splice with a
  pre-patch backup.],
  [The blocks you will actually touch are `language` / `genre`, `llm`, `editor`
  (with `style_warnings`), `theme`, `backup`, `goals`, `ai`, `embeddings`, and
  `keys` — each covered in depth by its own chapter, with every field in
  *Appendix C*.],
  [The governing principle: *cost caps inform, never block*, and almost
  everything is off or default-safe until you opt in.],
))
