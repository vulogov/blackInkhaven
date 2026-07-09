#import "../design.typ": *

#chapter(number: 2, title: "The command in full")

You have met the short form. Now let us lay out the whole command, every option it
takes, and when you would reach for each. None of these is required for a good site —
the bare `inkhaven export html -o site` is complete — but each one answers a real
question you may eventually have.

The general shape is always the same:

#run[```
inkhaven export html [options]
```]

#term("Option (or flag)")[
  An extra word on a command, beginning with two dashes, that adjusts what it does —
  like `--output`. Some options take a value after them (`--output site`); some, called
  _switches_, stand alone. The order of options does not matter.
]

#section("Where the site goes — `--output`")

Covered already, and the one option you will always use: `--output <folder>` (short
`-o <folder>`) names the destination. A fresh export overwrites the pages in that
folder, so pointing two exports at the same folder is normal — the second simply
replaces the first.

#run[```
inkhaven export html -o public
```]

#section("Which book — `--book-name`")

A project can hold more than one book. If yours holds only one, Inkhaven exports it
without being told. If it holds several, name the one you mean:

#run[```
inkhaven export html -o site --book-name "The Drowned Atlas"
```]

#note[
  The name is the book's title as it appears in your project's tree, in quotation
  marks if it contains spaces. If you have one book, you never need this.
]

#section("Which edition — `--profile`")

This is the most powerful option, and it has a chapter's worth of ideas behind it,
so here we only introduce it. A single manuscript can carry more than one _edition_ —
a short version and a full one, a beginner's path and an expert's. You mark paragraphs
for an edition inside the editor, and then at export time you ask for that edition:

#run[```
inkhaven export html -o site --profile edition=full
```]

You can ask for more than one dimension at once:

#run[```
inkhaven export html -o site --profile edition=full --profile audience=expert
```]

Chapter 6 explains how to mark the paragraphs and how the matching works. For now,
know that `--profile` is how you publish _one slice_ of a manuscript that contains
several.

#section("Custom design — `--templates` and `--eject-templates`")

By default the site is built from a set of design files bundled inside Inkhaven — you
never see them, and you do not need to. When you want to change the design beyond what
the settings allow, you work with your own copy of those files. Two options make that
possible.

The first _writes the bundled files out_ so you have something to edit:

#run[```
inkhaven export html --eject-templates my-design
```]

#term("`--eject-templates <folder>`")[
  Writes Inkhaven's built-in design files into the folder you name, then stops without
  exporting. This is how you get a starting point to customise — especially important
  if you installed Inkhaven as a finished program, because then the files exist only
  inside it until you ask for them. "Eject" because you are ejecting the built-in
  copies out to where you can reach them.
]

The second _uses_ a folder of design files when it exports:

#run[```
inkhaven export html -o site --templates my-design
```]

#term("`--templates <folder>`")[
  Tells Inkhaven to take the design from the folder you name instead of its built-in
  defaults. Any file you did not change still falls back to the default, so you only
  keep the files you actually edited.
]

The natural rhythm is: eject once, edit the few files you care about, then export with
`--templates` pointing at them. Chapters 3 and 4 are entirely about what is inside
those files.

#section("The settings file, instead of options")

Everything you can pass as an option, you can also set once in your project's settings
file, so you do not retype it every export.

#term("`inkhaven.hjson`")[
  Your project's settings file, sitting in the project folder. It records your
  preferences in a friendly, readable format (the next chapters lean on it heavily).
  Options you pass on the command line always win over what is written here, so the
  file sets your usual habit and an option overrides it for one run.
]

For example, to make `my-design` your permanent template folder without typing
`--templates` every time:

#config("inkhaven.hjson", [```hjson
docs: {
  html: {
    template_dir: "my-design"
  }
}
```])

With that saved, a plain `inkhaven export html -o site` uses your custom design. We
will add more settings under `docs: { html: { … } }` as the book goes on.

#recap((
  [`--output <folder>` (or `-o`) names where the site is written; you always use it.],
  [`--book-name` picks a book when a project holds several.],
  [`--profile dimension=value` publishes one edition of a manuscript that carries many (Chapter 6).],
  [`--eject-templates <folder>` writes the design files out to edit; `--templates <folder>` exports using them.],
  [Anything you can pass as an option can live permanently under `docs: { html: … }` in `inkhaven.hjson`.],
))
