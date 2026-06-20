#import "../design.typ": *

#chapter(number: 3, title: "Getting set up")

This chapter takes you from nothing to an empty language, ready to fill in. Five
short steps: install Inkhaven, create a project, create your language, learn how
to edit its chapters, and (optionally) connect an AI provider.

#section("1. Install Inkhaven")

Inkhaven is a single program. If you have the Rust toolchain installed, the
simplest route is:

```sh
cargo install inkhaven
```

This downloads and builds the latest release and puts an `inkhaven` command on
your system. Pre-built downloads are also available from the project's releases
page. To check it worked, run:

```sh
inkhaven --version
```

You should see a version number of `1.3.17` or newer (this book's examples assume
that release). If the command is not found, make sure the install location is on
your system's `PATH`; the install step prints where it placed the program.

#section("2. Create a project")

Everything you do lives inside a *project* — a folder Inkhaven manages. Create
one with `init`, giving it a path:

```sh
inkhaven init ~/eldar-project
```

This makes the folder, sets up Inkhaven's internal storage, and prints a short
summary. From now on, run conlang commands from inside that folder:

```sh
cd ~/eldar-project
```

#callout(label: "One project, many languages")[
  A single project can hold as many languages as you like — useful when you
  build a family of related tongues later (Part V). Each is a separate language
  under the same project.
]

#section("3. Create your language")

Now create the language itself. We will call ours *Eldar*:

```sh
inkhaven language init Eldar
```

This adds an *Eldar* sub-book under the Language book and creates its five
chapters: *Meta*, *Dictionary*, *Grammar*, *Phonology*, and *Sample texts*. The
language exists now, but it is empty — no sounds, no words, no rules. Filling
those in is the rest of the book.

You can list your languages at any time:

```sh
inkhaven language list
```

#section("4. How you edit a language")

Here is the one mechanic that everything else depends on, so read it carefully.

Most of what defines a language — its sounds, its grammar rules, its history — is
written as small blocks of structured text in a format called *HJSON*, and placed
into the right chapter of the language book. HJSON is a gentle, human-friendly way
to write structured data: things in `{ … }` are collections of named fields, and
things in `[ … ]` are lists.

#term("HJSON")[
  A relaxed, human-readable data format (a friendlier cousin of JSON). You write
  fields as `name: value`, group them with curly braces `{ }`, and make lists
  with square brackets `[ ]`. Inkhaven reads these blocks to reconstruct your
  language.
]

On disk, each chapter of your language is a folder of small text files. To add a
phonology block, for example, you create a text file in the Phonology chapter's
folder and paste the block in. The chapters live under your project at:

```text
books/language/<your-language>/04-phonology/
books/language/<your-language>/03-grammar/
books/language/<your-language>/05-sample-texts/
```

(The exact numbers may differ; the names are what matter.) After you add or
change a file by hand, tell Inkhaven to notice it:

```sh
inkhaven reindex --adopt
```

This *adopts* any new files into the language so the tools can read them. You run
it once after editing files; the commands that write changes for you (like
`add-word`) do not need it.

#callout(label: "The one HJSON pitfall to remember")[
  In HJSON, a value that is not in quotes runs to the end of the line. So always
  put quotes around short word-like values such as `kind: "consonant"` or
  `position: "suffix"`. If you forget, you will get a clear error pointing at the
  line. When in doubt, quote it.
]

We will write our first real block — the sound inventory — in the very next
chapter, so this will quickly become familiar.

#section("5. Connect an AI provider (optional)")

Skip this section entirely if you do not want to use the AI features; everything
essential works without it. To enable them, you tell Inkhaven which provider to
use and give it your API key. You obtain a key by signing up with a provider
(this book's live examples use *DeepSeek*); the provider gives you a long secret
string. You make it available to Inkhaven through an *environment variable* — a
named value your terminal holds. For DeepSeek, for instance:

```sh
export DEEPSEEK_API_KEY="your-secret-key-here"
```

Then, on any command that uses AI, you select the provider with `--provider
deepseek`. Other providers work the same way with their own key names. That is
all the setup AI needs; we will use it sparingly and always say when.

#recap((
  [Install with `cargo install inkhaven`; check with `inkhaven --version`.],
  [Create a project with `inkhaven init <path>`, then `cd` into it.],
  [Create a language with `inkhaven language init <name>` — it gets five
   chapters.],
  [Define a language by placing *HJSON* blocks into chapter folders, then
   running `inkhaven reindex --adopt`. Always quote short values.],
  [AI features are optional: set an API key and pass `--provider` when you want
   them.],
))
