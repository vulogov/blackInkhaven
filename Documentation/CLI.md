# CLI conventions

Rules every new `inkhaven` command follows. Existing commands outside the
`language` family are not renamed to match; where an older spelling is still
accepted, it is a hidden alias.

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
(and `--svg` on `ecology`) kept as hidden aliases so existing scripts keep
working. A unit test pins that inventory. The book-export family (`export`,
`manuscript`, `docx`, `epub`, `audiobook`) already uses `-o/--output`;
tooling commands that take `--out` (`pdf *`, `backup`,
`package-help-corpus`) are unchanged.
