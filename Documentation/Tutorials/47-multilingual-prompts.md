# 47 — Multilingual prompts

Inkhaven's AI flows — grammar check, critique,
rhythm rewrite, show-don't-tell scan, timeline
health audit — all resolve their prompts through
the same chain, and as of 1.2.11 that chain is
language-aware.  If you write in Russian, the
grammar prompt the model receives is *in* Russian
and asks for *Russian* grammar; if you switch to a
French paragraph in the same project, the prompt
swaps automatically.

This tutorial covers the full picture: the
resolver's three-pass cascade, the configuration
knobs, the runtime toggle, the `lang:<code>` tag
convention on prompts, and the bootstrap CLI.

## The five supported languages

Inkhaven plumbs end-to-end for **English, Russian,
French, German, and Spanish** — Snowball stemmers,
stop-word lists, show-don't-tell vocabulary, and
embedded prompt variants all ship for these five.
Other languages still work for editing (the editor
is language-agnostic), but the AI prompt resolver
falls back to English for them.

ISO 639-1 short codes are the wire format the
resolver compares against:

| Long form  | Code |
|------------|------|
| `english`  | `en` |
| `russian`  | `ru` |
| `french`   | `fr` |
| `german`   | `de` |
| `spanish`  | `es` |

## The three-pass resolver

Every AI flow that needs a prompt asks the resolver:
*give me the `<name>` prompt in `<lang>`*.  The
resolver searches three layers in order:

### Pass 1 — strict same-language

  * Prompts-book paragraph tagged `lang:<lang>` with
    a name matching `<name>` (slug *or* title).
  * `prompts.hjson` entry with
    `language: <lang>` and the matching name.
  * The embedded prompt for that language.

A hit at Pass 1 is the ideal outcome — the user
explicitly authored a prompt for this language.

### Pass 2 — untagged (back-compat)

  * Prompts-book paragraph with NO `lang:*` tag and
    a matching name.
  * `prompts.hjson` entry with NO `language` field
    and a matching name.

Pass 2 is the "respect what was there before"
layer.  Every prompt that existed in 1.2.10 is
untagged and lands here; that's how your old
projects keep working unchanged.

### Pass 3 — any-language

  * Any prompts-book paragraph or hjson entry with
    the matching name, regardless of language tag.
  * Failing that, the embedded English prompt as a
    final floor.

Pass 3 is the safety net.  The resolver never
fails to produce *some* prompt; you'll get the
floor in English if nothing else is available, and
the AI pane decoration tells you so (see below).

## What the AI pane shows

The AI pane title bar carries a `lang=` chip
right after the existing `infer=` chip:

```
 AI · llm=claude · infer=Local · lang=ru (paragraph) · 2 turn(s)
```

The chip has two parts:

  * `ru` — the ISO code the next AI call will use
    when resolving prompts.
  * `(paragraph)` or `(book)` — the active resolution
    mode (more on this below).

The chip updates immediately when you toggle the
mode or switch paragraphs.

## The two resolution modes

### `book_defined` (default)

The resolver uses the project's top-level
`language` field from `inkhaven.hjson`.  Every
AI call resolves prompts against the same code,
regardless of which paragraph you're in.

This is the right mode for monolingual
manuscripts.  Set `language: russian` once; every
AI call hits Russian prompts.

### `paragraph_detected`

The resolver runs `whatlang` on the live paragraph
body and uses the detected language.  Falls back
to the book language when the paragraph is shorter
than `editor.prompt_language_detection_min_chars`
(default 50) — whatlang is unreliable on very short
text and we'd rather use the book setting silently
than guess.

This is the right mode for mixed-language projects
— a Russian novel with English helper notes in
the Notes book, or a French translation project
where the source is English and the target is
French.

### Configuring the mode

Persistent: edit `editor.prompt_language_mode` in
`inkhaven.hjson` — either through `inkhaven
config -p .` (it's a picker), `Ctrl+B 0` raw edit,
or by hand:

```hjson
editor: {
  prompt_language_mode: paragraph_detected
}
```

Session-local: `Ctrl+B Shift+N` cycles through
three states:

  * **No override** — defer to `inkhaven.hjson`.
  * **Override → `book_defined`** — force book mode
    for this session.
  * **Override → `paragraph_detected`** — force
    paragraph mode for this session.

The cycle ends back at "no override", so you can
get back to your HJSON default without restarting.
The status bar echoes the new mode on each press:

```
prompt language mode: paragraph_detected · resolving as `ru` · session override
```

## Tagging Prompts-book paragraphs

A Prompts-book paragraph "belongs" to a language
when its `tags` list contains `lang:<code>`.  Add
the tag through the standard project tag UI
(`Ctrl+B ]` from inside the paragraph, or `g` on a
tree row).  Multiple `lang:*` tags on one paragraph
is a user error; the resolver picks the first one.

Example: a paragraph named `grammar-check` with
`tags: ["lang:ru", "draft"]` wins Pass 1 for
Russian.  An untagged `grammar-check` paragraph
wins Pass 2 — handy as a fallback for any language
your project hasn't tagged a prompt for.

## Tagging `prompts.hjson` entries

The `language` field on each prompt entry, ISO 639-1
short code:

```hjson
{
  prompts: [
    {
      name: tighten
      language: ru
      description: Сжать прозу
      template: '''
        Сожми следующий фрагмент. Сохрани голос…
      '''
    }
    {
      name: tighten
      language: en
      description: Tighten prose
      template: '''
        Tighten the following prose. Preserve voice…
      '''
    }
    {
      name: tighten
      // no language field — Pass 2 fallback
      description: Generic tighten
      template: '''
        …
      '''
    }
  ]
}
```

The resolver picks the right entry by `(name,
language)` tuple.

## The `/` prompt picker

When you type `/` in the AI prompt input, the picker
groups matches into three buckets that mirror the
resolver:

```
── In active language (ru) ──
  [system] [ru] /grammar-check
            Грамматическая проверка
  [book]   [ru] /tighten-russian
            Сжать русскую прозу

── Untagged ──
  [system] [—]  /tighten
            Tighten prose; remove flab without …
  [book]   [—]  /continue
            Continue the scene in the same voice …

── Other languages ──
  [system] [fr] /grammar-check
            Vérification grammaticale
```

Section headers + inline `[ru]` / `[—]` chips
make each prompt's language visible at a glance.
The active-language bucket is always on top; within
each bucket the same prefix-then-substring scoring
that's always driven the picker still applies.

## The Prompts editor's language picker

Inside `inkhaven prompts-editor`, each list-pane
row carries a yellow-dim `[lang]` chip.  Pressing
`l` (lowercase L, list pane only) on the focused
prompt cycles its language tag through:

```
None → en → ru → es → de → fr → None
```

The prompt is marked dirty; `Ctrl+S` persists.

## Embedded fallbacks

For all seven of inkhaven's named flows
(`grammar-check`, `explain-diagnostic`,
`critique-edit`, `critique-changes`,
`show-dont-tell`, `sentence-rhythm-rewrite`,
`timeline-health`), the binary ships a hand-
written prompt variant in each of the five
supported languages.  These are the floor — the
resolver returns one of them when nothing else
matches.

This means a fresh project with `language:
russian` in `inkhaven.hjson` and no custom prompts
already gets correct-language behaviour for every
named flow.  No setup required.

## The bootstrap CLI

`inkhaven prompts bootstrap <lang>` asks the
configured LLM to produce per-language variants of
the seven embedded prompts and emits either an
HJSON snippet on stdout or merges into
`prompts.hjson` in place.

Default — print the snippet to stdout:

```
$ inkhaven prompts bootstrap russian
inkhaven prompts bootstrap · language: russian (ru) · model: claude-opus-4-7
......................................

// Paste under the `prompts` array in prompts.hjson.
{
  name: grammar-check
  language: ru
  description: grammar-check (ru)
  template: '''
    Сделай корректорскую вычитку русского абзаца ниже…
  '''
}
{
  name: critique-edit
  language: ru
  description: critique-edit (ru)
  template: '''
    Прочти абзац ниже как черновик…
  '''
}
…
```

In-place — merge into `prompts.hjson`:

```
$ inkhaven prompts bootstrap russian --update
…
patched <project>/prompts.hjson
  (pre-patch backup: <project>/.config-backups/prompts_20260601_143027.hjson)
```

Merge semantics: for each generated `(name, ru)`
pair, replace any existing same-`(name, language)`
entry; otherwise append.  Existing entries with a
*different* language are left untouched — they're
how the resolver's Pass 1 cascade works.  The
pre-patch backup is written first, so a roll-back
is one `cp` away.

Optional `--genre <hint>`: folded into the prompt
so the model picks vocabulary at the right register
("literary fiction", "thriller", "YA fantasy", …).
Useful when the genre-neutral defaults sit at the
wrong reading level for your corpus.

## Why we don't translate at runtime

The bootstrap is a one-shot curator, not a runtime
translator.  Three reasons:

  * **Latency.**  An always-on LLM overlay running
    on every keystroke would be unusable —
    1-3 second tail latency per call, hundreds
    of calls per editing session.
  * **Cost.**  Authors edit for hours.  Hours ×
    per-paragraph LLM calls = a meaningful API
    bill for no obvious win.
  * **Offline use.**  A core value of a terminal
    editor is that it works on a plane.  Hard-
    coding an LLM into the AI prompt resolver
    breaks that.

The bootstrap CLI is the *opt-in* path that
benefits from the LLM: run it once, review the
output, paste / merge.  Everything after that is
pure lookup — instant, deterministic, offline.

## Edge cases

- **Paragraph shorter than 50 chars.** Whatlang is
  unreliable below that.  `paragraph_detected` mode
  silently falls back to the book language.  Tune
  `editor.prompt_language_detection_min_chars` if
  your project's typical paragraphs are smaller.
- **Mixed-language paragraphs.** Whatlang returns
  one *dominant* language.  A 200-word English
  paragraph with one line of Russian dialogue
  resolves as English.  We don't sentence-split
  inside the resolver; if this matters for your
  project, switch to `book_defined`.
- **Unsupported language.** If `cfg.language` is
  set to `italian`, the resolver maps it to `en`
  (English embedded floor).  Stemmer / stop-word /
  SDT plumbing only ships for the five supported
  languages; other languages still work for editing
  but lose the AI-prompt-language affordance.

## See also

- [05 — AI writing assistant](05-ai-writing-assistant.md) — the broader AI flow this hooks into.
- [25 — tag workflows](25-tag-workflows.md) — how the `lang:<code>` tag fits with the project-wide tag system.
- [44 — prompts editor](44-prompts-editor.md) — the standalone editor for `prompts.hjson`, including the `l` chord that cycles language tags.
- `Documentation/PROPOSALS/MULTILINGUAL_PROMPTS.md` — the design proposal with the resolver's three-pass cascade spelled out formally.
