#import "../design.typ": *

#chapter(number: 1, title: "Installing Inkhaven")

Inkhaven is a single program. When it is installed you have one command,
`inkhaven`, and everything else in this book happens inside it or through it —
the editor, the world, the facts, the assistant, the finished PDF. There is no
server to run, no account to create, no companion app to keep open. This chapter
gets that one command onto your machine, explains what happens the first time you
run it, and tells you how to check that all is well before you write a word.

It assumes you can open a terminal and type into it, and nothing more. If you
have never compiled a Rust program, that is fine — you will not have to
understand Rust, only to install its toolchain and let it work for a few
minutes. Read this chapter once, top to bottom; you will not need it again until
you set Inkhaven up on a second machine.

#section("What you are installing")

Inkhaven ships as one self-contained binary. It carries its own database engine,
its own semantic-search index, and — after the first run — its own local
embedding model. It does *not* depend on any external program being present on
your system: no Node, no Python, no Docker, no database server, no paid AI
account. The one genuinely optional companion is the Typst compiler, and only if
you want to render your manuscript to PDF from inside the editor; everything
else, including the plain-text and single-`.typ` exports, works with Inkhaven
alone.

#term("Binary")[
  A single executable file — here, one called `inkhaven`. "Installing" it means
  getting that file onto your machine and onto your `PATH` (the list of
  directories your shell searches for commands), so that typing `inkhaven`
  anywhere runs it. Everything Inkhaven does lives in that one file plus the
  per-user model cache it downloads once.
]

There are two ways to get the binary: let Cargo *compile* it for you (from
crates.io or from source), or download a *prebuilt* one. Compiling is the path
this chapter treats as canonical, because it always produces a binary matched to
your exact machine and needs nothing but a Rust toolchain. The prebuilt paths are
faster and are covered too.

#section("Prerequisites")

You need three things: a Rust toolchain, a terminal, and a little disk space. Only
the first requires any action.

#subsection("A Rust toolchain")

Inkhaven is written in Rust, edition 2024, and requires a compiler of at least
*version 1.85*. That minimum is not arbitrary — the 2024 edition and several
language features Inkhaven relies on landed in that release. Anything newer is
fine; Rust is strongly backward-compatible.

The official way to install Rust is #term("rustup")[the standard Rust toolchain
installer — a small script that places `rustc` (the compiler) and `cargo`
(Rust's build tool and package manager) under your home directory and adds them
to your shell's `PATH`. It needs no administrator rights and installs entirely
inside your own account.]. On macOS and Linux one line does it:

#screen(caption: "Installing the Rust toolchain via rustup")[```
  $ curl --proto '=https' --tlsv1.2 -sSf \
        https://sh.rustup.rs | sh
```]

Accept the default when it prompts (option 1 is correct). When it finishes,
either open a fresh terminal or reload your shell so that `cargo` is on the
`PATH`:

#screen(caption: "Making cargo available in the current shell")[```
  $ source "$HOME/.cargo/env"
```]

Then confirm both tools are present and new enough:

#screen(caption: "Verifying the toolchain")[```
  $ rustc --version
  rustc 1.85.0 (a028ae42f 2026-01-09)
  $ cargo --version
  cargo 1.85.0 (d73d2caf9 2026-01-06)
```]

If either prints "command not found", your shell has not yet picked up
`$HOME/.cargo/bin` — close and reopen the terminal, or add that directory to your
`PATH` by hand.

#callout(label: "Note")[
  The Rust toolchain is a one-time, whole-system install of roughly 600 MB. It is
  not part of Inkhaven and is reused by every Rust program you ever build. You
  install it once and forget it exists.
]

#subsection("A terminal")

Inkhaven is a terminal application. It runs in whatever terminal emulator you
already have — the built-in Terminal on macOS, or any of the common Linux ones
(GNOME Terminal, Konsole, Alacritty, kitty, WezTerm, xterm). It also runs
perfectly over SSH, inside `tmux`, and on a tiling window manager, because it is
text the whole way down and never reaches for a browser or a graphical window.
Any terminal from the last decade with 256-colour support will do; the richer
ones (kitty, iTerm2, WezTerm) additionally give you inline image previews, but
those are a luxury, not a requirement.

#subsection("Disk space")

A fresh, complete install occupies a few hundred megabytes, most of it the
one-time Rust toolchain. The parts that are actually Inkhaven are modest:

#screen(caption: "Disk footprint of a fresh install")[```
  Rust toolchain ...... ~600 MB  one-time, shared by all Rust
  Inkhaven binary ..... ~90 MB   the compiled `inkhaven`
  Embedding model ..... ~120 MB  downloaded once, per-user cache
  A new empty project . ~5 MB    database + index scaffolding
```]

The embedding model is a single shared download, not a per-project cost: a
hundred projects reuse the same cached model. A project's own footprint grows
only with your prose and its snapshots.

#section("Supported platforms")

Inkhaven is released and supported on two platforms:

#screen(caption: "Supported release targets")[```
  Linux   x86_64-unknown-linux-gnu
  macOS   aarch64-apple-darwin    (Apple Silicon)
```]

These are the targets the release pipeline builds, tests, and ships prebuilt
binaries for. Compiling from source with `cargo` works on any host Rust and the
dependencies support — an Intel Mac, for instance, compiles cleanly even though
there is no prebuilt asset for it — but the two above are the tested, first-class
combination.

#subsection("Why not Windows")

Windows is *deferred*: there is no released Windows binary, and you should not
expect one yet. The reason is specific and worth stating plainly, because it is
not a matter of effort or interest.

Inkhaven computes its embeddings locally through `fastembed`, which in turn runs
models through the ONNX Runtime by way of the `ort` crate. `ort` ships prebuilt
runtime binaries for the Windows *MSVC* toolchain but not for the Windows *GNU*
toolchain, and Inkhaven's build has historically targeted the GNU toolchain
(alongside the DuckDB and font-handling C++ that MSYS2 builds cleanly). The build
therefore fails at the ONNX Runtime bindings — upstream, in a dependency, not in
Inkhaven's own code.

#callout(label: "Why it is deferred, not abandoned")[
  The fix is real and known; it is simply not on the critical path. Any one of
  three routes unblocks Windows: `ort` shipping a Windows-GNU prebuilt (the CI job
  is kept ready and would simply go green), adopting the Windows-MSVC target so
  that `ort`'s existing prebuilts apply, or bundling a build-time-verified
  runtime library in the release archive. The one route deliberately *rejected* is
  fetching and loading a runtime library at startup — that would run unsandboxed
  native code pulled over the network, a far larger attack surface than the model
  data Inkhaven does download, and against the project's no-external-binary
  principle. Until one of the sound routes is taken, Windows users should build
  under WSL (a Linux environment) and treat it as the Linux target.
]

#section("Installing with Cargo")

If you have the Rust toolchain, the shortest path to a working `inkhaven` is to
let Cargo compile the published crate.

#subsection("cargo install inkhaven — compile from crates.io")

Inkhaven is published on crates.io; every release tag pushes a new version. This
one command downloads the crate and its dependencies and compiles the lot:

#screen(caption: "Installing the published crate")[```
  $ cargo install inkhaven
```]

Be patient the first time. The build pulls roughly a hundred crates and then
compiles the heavy ones — DuckDB, `fastembed`, and the ONNX Runtime bindings are
large C and C++ projects — so a first build takes on the order of *ten minutes* on
a modern laptop. This is a one-off: Cargo caches the compiled dependencies, and
you will not pay that cost again.

When it finishes, the binary lands in Cargo's binary directory, which is already
on your `PATH` if rustup set your shell up:

#screen(caption: "Where cargo install puts the binary")[```
  ~/.cargo/bin/inkhaven
```]

That is the whole install. Skip ahead to "The first run" once `inkhaven
--version` prints a version.

#subsection("cargo binstall — the prebuilt fast path")

If you would rather not wait for a compile and you have
#term("cargo-binstall")[a small Cargo extension that downloads a *prebuilt*
binary from a project's GitHub releases instead of compiling it. Install it once
with `cargo install cargo-binstall`; thereafter `cargo binstall <crate>` fetches
the right prebuilt asset for your platform.] installed, one command fetches the
prebuilt binary and drops it into `~/.cargo/bin`:

#screen(caption: "Installing a prebuilt binary")[```
  $ cargo binstall inkhaven
```]

`cargo-binstall` reads the packaging metadata from Inkhaven's `Cargo.toml`, picks
the asset matching your platform off the GitHub releases, and installs it without
compiling anything. This is the quickest route on the two supported platforms.

#subsection("GitHub Releases — a direct download")

You can also bypass Cargo entirely. Each release publishes a tarball per
platform on the project's Releases page. Download the one for your platform,
unpack it, and place the `inkhaven` binary anywhere on your `PATH`:

#screen(caption: "Installing from a release tarball")[```
  $ tar xzf inkhaven-<version>-<platform>.tar.gz
  $ sudo install inkhaven /usr/local/bin/inkhaven
```]

The releases live at #link("https://github.com/vulogov/blackInkhaven/releases")[
github.com/vulogov/blackInkhaven/releases]; the asset names encode the version
and the target triple so you can pick the right one at a glance.

#subsection("cargo install --git — a specific tag or branch")

When you want a particular tagged version, a pre-release branch, or your own
fork, install straight from the git repository and name the tag:

#screen(caption: "Installing a specific tagged version")[```
  $ cargo install \
      --git https://github.com/vulogov/blackInkhaven \
      --tag v3.0.0
```]

This compiles exactly the named revision. Drop `--tag` to build the default
branch's current tip. Check the Releases page for the tag you actually want; the
examples here name `v3.0.0`, the stable edition this manual documents.

#section("Building from source")

Compiling from a clone is the path to choose if you want to read or modify the
code, track a development branch, or simply keep the whole thing under your own
eye. It needs only the Rust toolchain and `git`.

First, clone the repository and enter it:

#screen(caption: "Cloning the source")[```
  $ git clone \
      https://github.com/vulogov/blackInkhaven.git
  $ cd blackInkhaven
```]

If you have no `git`, the GitHub page offers the same source as a zip; unpack it
and `cd` into the folder. Then build an optimised binary:

#screen(caption: "Building the release binary")[```
  $ cargo build --release
```]

The first build downloads about a hundred crates and compiles them; expect
several minutes — commonly three to eight on a modern laptop, longer on older
hardware because of the same heavy C/C++ dependencies noted above. Later builds
are incremental and reuse the cache, so they finish in seconds. When it is done,
the binary is here:

#screen(caption: "Where the source build puts the binary")[```
  ./target/release/inkhaven
```]

You can run it straight from that path. To type just `inkhaven` from anywhere,
copy it onto your `PATH`:

#screen(caption: "Putting inkhaven on your PATH")[```
  # system-wide (asks for your password)
  $ sudo install target/release/inkhaven \
        /usr/local/bin/inkhaven

  # or, no sudo, if ~/.local/bin is on PATH
  $ cp target/release/inkhaven ~/.local/bin/
```]

#callout(label: "Why the --release flag matters")[
  Without `--release`, Cargo builds an unoptimised debug binary. It runs, but it
  is several times slower and a few hundred megabytes larger — embedding and
  search in particular feel sluggish. Always build `--release` for real writing;
  reserve the debug build for hacking on Inkhaven's own code.
]

#subsection("Optional: the Typst compiler")

Inkhaven writes and manages Typst, but it does not bundle the Typst compiler. If
you want `inkhaven export pdf` (and the in-editor "build to PDF" chords) to
actually produce a PDF, install Typst separately — it, too, is a single static
binary on every platform, available from its own project page. Everything else,
including the `export typst` path that emits one combined `.typ` file, works
without it.

#section("The first run")

The install is inert until the first time Inkhaven opens a project. That first
open does two one-time things worth understanding, so that a slightly longer
startup does not alarm you.

#subsection("The embedding model download")

Inkhaven's semantic search — finding "the moment the lighthouse fails" even in a
paragraph that never says *lighthouse* — is powered by a local embedding model.
That model is not shipped inside the binary; it is downloaded the first time you
initialise or open a project. The default is #term("MultilingualE5Small")[the
default embedding model — a compact, multilingual model (English, Russian,
German, French, Spanish, and more) about 120 MB in size. It is chosen as a
sensible balance of quality against footprint; the configuration lets you switch
to the Base or Large variant, at the cost of a larger download and slower
embedding.], roughly 120 MB, fetched once into a per-user cache and reused by
every project thereafter.

The cache location depends on your platform:

#screen(caption: "Where the embedding model is cached")[```
  macOS  ~/Library/Caches/
             dev.inkhaven.inkhaven/embeddings/
  Linux  $XDG_CACHE_HOME/inkhaven/embeddings/
             (defaults to ~/.cache/inkhaven/)
```]

On a normal connection the download completes in well under two minutes, behind a
splash screen that shows the elapsed time. It happens once per model: switching
the configured model later downloads the new one on the next open and leaves the
old one on disk until you remove the cache directory yourself. If you ever change
`embeddings.model` in a project's configuration, Inkhaven re-embeds every
paragraph against the new model the next time it opens.

#subsection("Model init and the project lock")

After the model is present, Inkhaven initialises it into memory and takes an
*advisory* lock on the project so that two sessions do not write the same
database at once. The lock is a small file, `.inkhaven.lock`, in the project
root, held for as long as the session runs and released automatically by the
operating system when the process exits — even on a crash or a `kill -9`, so
there is never a stale lock to clean up by hand.

#callout(label: "The lock informs, it never blocks")[
  In keeping with Inkhaven's permissive character, opening a project that another
  session already holds does not fail. The launcher tells you who holds it — a
  process id, host, and time — and lets you open it anyway. Only genuine
  data-safety is at stake, and the choice stays yours. This is the same principle
  you will meet throughout Inkhaven: it warns, it does not forbid.
]

Once the model is cached and warm, subsequent launches are quick — there is no
download and the lock is instantaneous.

#section("Verifying the install")

Two commands confirm a healthy install without touching any project. First, the
version:

#screen(caption: "Confirming the binary runs")[```
  $ inkhaven --version
  inkhaven 3.0.0
```]

A version number means the binary is on your `PATH` and runs. If instead you see
"command not found", the binary is not on your `PATH` — invoke it by its full
path (`./target/release/inkhaven` from a source build, or `~/.cargo/bin/inkhaven`
from a Cargo install), or copy it to a directory that is on your `PATH`.

Second, the help surface, which lists every top-level subcommand and the global
flags:

#screen(caption: "Listing the command surface")[```
  $ inkhaven --help
  TUI literary work editor for Typst books

  Usage: inkhaven [OPTIONS] [COMMAND]

  Commands:
    init      Initialize a new project
    add       Add a node to the hierarchy
    list      Print the hierarchy as a tree
    search    Run a semantic search
    export    Export the book(s)
    backup    Zip the whole project
    ...

  Options:
    -p, --project <PROJECT>   Path to a project root
    -h, --help                Print help
    -V, --version             Print version
```]

Any subcommand takes `--help` of its own — `inkhaven export --help`,
`inkhaven init --help` — which prints that command's full set of flags. This is
the authoritative reference for the exact surface of the version you have
installed; when this book and the binary ever disagree, the binary's `--help` is
right.

#section("Troubleshooting the install")

Most installs are uneventful. The handful of things that can go wrong have short
answers.

#subsection("command not found: inkhaven")

Your shell cannot find the binary on its `PATH`. From a source build it lives at
`./target/release/inkhaven`; from `cargo install` at `~/.cargo/bin/inkhaven`.
Either call it by that full path, copy it into a directory that is on your `PATH`
(`/usr/local/bin` or `~/.local/bin`), or alias it. Confirm your `PATH` contains
`$HOME/.cargo/bin` if you expected Cargo's install to be found automatically.

#subsection("The first-run download is slow or stalls")

The model download is the one part of startup that reaches the network. If the
splash screen's elapsed counter climbs past a couple of minutes on a connection
you know is working, something upstream is slow rather than broken. You can:

#screen(caption: "Recovering from a stalled first-run download")[```
  - Check the machine actually has connectivity.
  - Watch the splash's elapsed timer — under ~2 min
    is normal for MultilingualE5Small.
  - Press Ctrl+Q to abort startup, then retry later.
```]

Aborting is safe: nothing is half-written into the cache in a way that a later
retry cannot replace.

#subsection("Offline, air-gapped, or behind a proxy")

Inkhaven never needs the network *except* for that one first-run model download
(and, separately, for any external AI provider you choose to configure — which is
entirely optional). Two consequences follow.

On a *proxy*, make sure the standard `HTTPS_PROXY` environment variable is set
before the first run so the download can reach out. On a genuinely *air-gapped*
machine, you cannot download at all — so seed the cache from a networked machine
instead: install and run Inkhaven once on a connected computer, then copy the
populated cache directory (the platform paths listed under "The first run") onto
the air-gapped machine before its first launch. With the model already present,
Inkhaven starts fully offline and stays offline for the whole of writing,
worldbuilding, search, and snapshots — every subsystem but the external providers
is on-device.

#subsection("A configuration error on open")

If opening a project reports a missing configuration field, you are opening a
project whose `inkhaven.hjson` was written by an older release than the binary
you just installed. Add the missing field by hand — the error names it, and
Appendix C lists every key with its default. Do *not* reach for
`inkhaven init --force` to "refresh" the config: `--force` deletes and recreates
the *entire project directory* — every book, database, and word of prose — not
just `inkhaven.hjson`. New projects created by the version you installed never
hit this.

#section("The CLI and the TUI")

One binary, two shapes. Understanding the split now will save confusion in every
later chapter, because this manual moves freely between them.

Run `inkhaven` with *no subcommand* — inside a project directory, or with a
`--project` path — and it launches the full-screen editor: the #term("TUI")[the
Text User Interface — Inkhaven's full-screen, keyboard-driven editor, with its
tree, editor, search, AI, and output panes. It takes over the terminal, and you
leave it with `Ctrl+Q`. This is where you actually write.], the interactive
program that fills the terminal and is the subject of most of this book:

#screen(caption: "Opening the editor")[```
  $ cd ~/Books/my-novel
  $ inkhaven                 # opens the TUI here
  # …or from anywhere:
  $ inkhaven --project ~/Books/my-novel
```]

Run `inkhaven` *with* a subcommand and it does one job and exits, printing to the
terminal without ever entering full-screen mode. These headless commands are what
you script, pipe, and run in continuous integration:

#screen(caption: "Headless, one-shot commands")[```
  $ inkhaven init ~/Books/my-novel
  $ inkhaven --project ~/Books/my-novel list
  $ inkhaven --project ~/Books/my-novel \
        search "the lighthouse fails"
  $ inkhaven --project ~/Books/my-novel \
        export pdf --status ready
```]

The global `--project` flag (also `-p`, or the long alias
`--project-directory`) tells any command which project to act on, and defaults to
the current directory — so `cd` into a project first and you can drop it
entirely. The top-level command families you will meet across the book group
roughly like this:

#chord_table((
  chord_row("init · add · mv", "Create and shape the hierarchy"),
  chord_row("list · outline", "Inspect the tree from the shell"),
  chord_row("search · book-rag", "Semantic search and retrieval"),
  chord_row("export · index", "Produce PDF, Typst, EPUB, indexes"),
  chord_row("backup · restore", "Project-level disaster recovery"),
  chord_row("reindex · doctor", "Reconcile store with disk; health"),
  chord_row("ai", "One-shot inference from the shell"),
))

A few subcommands — the standalone configuration editor, the research and
worldbuilder and linguistic workspaces — are themselves full-screen TUIs rather
than one-shot commands; the book flags each where it introduces it. Everything
else you type after `inkhaven` runs, prints, and returns you to your prompt.

#two_track(
  [If you write *fiction*, your usual entry is the bare `inkhaven` — you live in
  the editor, with the tree, the world, and the readers a keystroke away. The
  headless commands matter mainly at the edges: `init` to begin, `backup` to be
  safe, `export` to ship.],
  [If you write *non-fiction* or documentation, you will lean on the headless side
  far more — `search`, `export`, `index`, and the check commands slot naturally
  into build scripts and continuous integration, where an editor cannot go.],
)

With `inkhaven` installed, verified, and its two shapes clear, you are ready to
create a project — which is where the next chapter begins.

#recap((
  [Inkhaven is *one self-contained binary*; the only prerequisite you must install
  yourself is a *Rust toolchain of version 1.85 or newer* via rustup.],
  [Install it by compiling — `cargo install inkhaven` (a ~10-minute first build)
  or `cargo build --release` from a clone — or grab a prebuilt binary with
  `cargo binstall` or from GitHub Releases.],
  [The supported targets are *Linux x86_64* and *macOS Apple Silicon*; Windows is
  deferred because the ONNX Runtime bindings `fastembed` needs have no
  Windows-GNU prebuilt.],
  [The *first run* downloads a ~120 MB embedding model once into a per-user cache,
  then takes an advisory project lock that *informs but never blocks*.],
  [Verify with `inkhaven --version` and `inkhaven --help`; running `inkhaven` bare
  opens the *TUI*, while `inkhaven <subcommand>` runs *headless* and exits.],
))
