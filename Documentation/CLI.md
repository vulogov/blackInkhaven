# CLI conventions

Rules every `inkhaven` command follows. Where an older spelling is still
accepted, it is a hidden alias: scripts keep working, `--help` shows one form.
Since 3.20 the whole surface is checked by tests (`src/cli/surface.rs`):
the command tree builds under clap's debug assertions (so two flags can never
claim one short letter — `ai --provider` lost its `-p`, which is the global
project flag), every output-path flag is spelled `-o/--output`, and every
`inkhaven …` command line in the README, the guides, the Bund reference and
the tutorials names a command and flags that exist (`docs_cli_surface`).

## Output paths: `-o, --output <PATH>`

A command that writes **one file the user names** takes `-o, --output <PATH>`
(a `PathBuf`), never `--out`, `--file` or a format-specific name such as
`--svg`. When the command can also print, omitting the flag prints to stdout;
when it cannot (a binary or a paginated document), it derives a default under
the project's `exports/` folder and prints the path it chose. Input paths keep
a descriptive name (`--svg <FILE>` on `glyph-lint` *reads* an SVG).

The whole `inkhaven language` family follows this since 3.18 (LANG-IMPROVE-1
LI-6): `export`, `sketch`, `scaffold`, `dictionary`, `grammar-book`,
`tutorial`, `ecology`, `font-build`, `font-compose`, `spatial-typst`,
`glyph-draft` and `export-translation` all take `-o/--output`, with `--out`
(and `--svg` on `ecology`) kept as hidden aliases. The book-export family
(`export`, `manuscript`, `docx`, `epub`, `audiobook`) always used
`-o/--output`.

Since 3.20 (CLI-1) the rest of the surface follows too, each with `--out`
kept as a hidden alias: `index`, `index-locorum`, `index-verborum`,
`backup` (`-o/--output <DIR>`), `package-help-corpus`,
`research --export-thread` / `--bibliography`, `sources export`,
`inner-socrates bundle export`, and every `pdf` operation that writes a file
(`impose`, `booklet`, `extract`, `delete`, `rotate`, `reorder`, `merge`,
`metadata`, `outline`, `barcode`, `cover`, `grayscale`, `optimize`,
`watermark`, `sample`); `pdf split` writes a directory and takes
`--output-dir` (`--out-dir` stays as an alias). A unit test pins that no
visible flag is spelled `--out` any more.

## Machine output: `--json`

A command whose result a script might read takes `--json`. With the flag the
result is one pretty-printed JSON document on stdout and nothing else is
written there; confirmations and counts stay on stderr, so `cmd --json | jq`
never sees prose. Without the flag the same data is rendered as text.

The shapes follow one rule (LANG-IMPROVE-1 LI-9): where a Bund word
(`ink.lang.*`) already returns a structure, the CLI emits the same keys, so a
script reads the same thing from either side — `list`, `generate-word`,
`syllabify`, `paradigm`, `sentence`, `memory`, `varieties` and `cognates` match
`ink.lang.list`, `generate_words`, `syllabify`, `paradigm`, `sentence`,
`memory`, `varieties` and `cognates`. Where the Bund word returns a single
string, the CLI emits an object carrying the fields its text output shows
(`ipa` → `{word, underlying, surface, romanized}`, `stress` →
`{word, syllables, stressed, marked}`, `tone` → `{input, surface}`,
`sound-change` → `{proto, form, result, rules}`, `family-tree` →
`{languages, contact}`, `gloss` → `{words, matched}`); `romanize`, `scan` and
`dialects` have no Bund twin and emit `{input, scheme, direction, output}`,
`{lines, dominant_meter}` and `{language, varieties, rows}`. The shapes live in
`src/cli/language/shapes.rs`; each derives `Deserialize` and a round-trip test
pins its keys.
