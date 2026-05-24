pub mod add;
pub mod ai;
pub mod backup;
pub mod build;
pub mod bund;
pub mod delete;
pub mod export;
pub mod import_help;
pub mod import_scrivener;
pub mod import_typst_help;
pub mod init;
pub mod list;
pub mod mv;
pub mod reindex;
pub mod restore;
pub mod search;
pub mod doctor;
pub mod event;
pub mod stats;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};

use crate::store::NodeKind;

#[derive(Debug, Parser)]
#[command(name = "inkhaven", version, about = "TUI literary work editor for Typst books")]
pub struct Cli {
    /// Path to a project root. For `init`, this is the project to create. For
    /// every other subcommand, defaults to the current directory.
    #[arg(long, global = true)]
    pub project: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Initialize a new inkhaven project at the given path.
    Init {
        /// Project directory to create.
        path: PathBuf,
        /// Overwrite existing configuration if present.
        #[arg(long)]
        force: bool,
    },

    /// Add a node (book / chapter / subchapter / paragraph) to the hierarchy.
    Add {
        /// Node kind.
        #[arg(value_enum)]
        kind: CliNodeKind,
        /// Display title.
        title: String,
        /// Slash-separated slug path to the parent (e.g. `my-book/01-chapter`).
        /// Required for everything except `book` when not using --after.
        #[arg(long)]
        parent: Option<String>,
        /// Override the auto-assigned slug (defaults to slugified title).
        #[arg(long)]
        slug: Option<String>,
        /// Insert the new node immediately after an existing sibling of the
        /// same kind. Pass the sibling's slug path here; --parent is then
        /// implicit (taken from the anchor's parent).
        #[arg(long)]
        after: Option<String>,
    },

    /// Print the hierarchy as a tree.
    List,

    /// Delete a node (and its descendants) by slash-separated slug path.
    Delete {
        /// e.g. `my-book/the-storm/morning-light`
        path: String,
        /// Required confirmation flag — without it we just dry-run.
        #[arg(long)]
        yes: bool,
    },

    /// Reorder a node within its siblings by swapping with the neighbor.
    Mv {
        /// Slash-separated slug path to the node.
        path: String,
        /// `up` or `down`.
        #[arg(value_enum)]
        direction: mv::Direction,
    },

    /// Run a semantic search across the project.
    Search {
        query: String,
        #[arg(short, long, default_value_t = 10)]
        limit: usize,
    },

    /// Re-index all `.typ` files from disk into the document store.
    Reindex {
        /// Remove store records whose file is missing on disk.
        #[arg(long)]
        prune: bool,
        /// Register every orphan .typ file under the deepest hierarchy
        /// branch whose filesystem path matches the orphan's parent dir.
        #[arg(long)]
        adopt: bool,
    },

    /// Export the book(s) to a target format.
    Export {
        #[arg(value_enum, default_value_t = ExportFormat::Typst)]
        format: ExportFormat,
        /// Output path (file for typst, directory for pdf builds).
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Name of the user book to export. Required when the
        /// project holds more than one user book; with a single user
        /// book it can be omitted and that book is used implicitly.
        /// System books (Help / Scripts / Typst / Prompts / Places /
        /// Characters / Notes / Artefacts / Research) are never
        /// included — they're inkhaven internals, not manuscript
        /// content. Matched case-insensitively against `Node.title`;
        /// falls back to slug match.
        #[arg(long)]
        book_name: Option<String>,
        /// Status floor (1.2.4+) — keep only paragraphs whose
        /// status sits at or above this rung on the workflow
        /// ladder. Lowercased: `napkin`, `first`, `second`,
        /// `third`, `final`, `ready`. `--status=ready` ships
        /// only Ready paragraphs (typical "submit to the agent"
        /// workflow). Unset = include every paragraph regardless
        /// of status (including paragraphs with no status set).
        #[arg(long)]
        status: Option<String>,
        /// Tag filter (1.2.6+) — keep only paragraphs that carry
        /// this tag (case-insensitive). Combines with `--status`:
        /// a paragraph must pass both predicates to be exported.
        /// Useful with the project-wide tagging surface
        /// (Ctrl+B ] / Ctrl+B }): tag a subset of paragraphs
        /// `draft`, then `inkhaven export pdf --tag draft` to
        /// ship just that slice.
        #[arg(long)]
        tag: Option<String>,
    },

    /// Run a one-shot AI inference from the command line.
    Ai {
        prompt: String,
        #[arg(short, long)]
        provider: Option<String>,
    },

    /// Import a directory tree into the Help system book. Subdirectories
    /// become chapters / subchapters / (flattened) and files become
    /// paragraphs. Filenames and directory names supply the displayed
    /// titles. Wipes Help's existing contents first.
    ImportHelp {
        /// Source directory whose contents will be ingested under the Help
        /// system book. Files at the root land as paragraphs directly under
        /// Help; subdirectories become chapters (then subchapters, etc.).
        #[arg(long)]
        documents_directory: PathBuf,
    },

    /// Import inkhaven's curated Typst reference into the Help system
    /// book. Creates / refreshes a `Typst reference` chapter so F1
    /// (RAG over Help) can answer typst questions from grounded
    /// context. Offline — the reference is bundled with the binary.
    ImportTypstHelp,

    /// Import a Scrivener (.scriv) project into the current
    /// inkhaven project (1.2.4+). Walks the binder, converts
    /// every Text document's RTF body to Typst, and
    /// materialises the hierarchy as inkhaven nodes. Single-
    /// binary — no Scrivener / pandoc / textutil required.
    ImportScrivener {
        /// Path to the `.scriv` package directory.
        scriv_path: PathBuf,
        /// Override the title used for the user book created
        /// from the Scrivener Draft folder. None → use the
        /// Draft folder's own title.
        #[arg(long)]
        draft_as_book: Option<String>,
        /// Skip everything outside the Draft (Research,
        /// Characters, Places folders Scrivener defaults to).
        #[arg(long)]
        skip_research: bool,
        /// Parse + report without creating any nodes.
        #[arg(long)]
        dry_run: bool,
    },

    /// Zip the project into a dated backup archive
    /// (`blackinkhaven_YYYYDDMM_HHMMSS.zip`).
    Backup {
        /// Output directory for the archive. Created if missing.
        /// Omit to use the project-relative default
        /// (`<parent-of-project>/inkhaven-backups/<project-basename>/`)
        /// — same location the TUI's exit hook writes to.
        #[arg(long)]
        out: Option<PathBuf>,
    },

    /// Restore a backup archive into a fresh directory.
    Restore {
        /// Path to the `.zip` backup file.
        archive: PathBuf,
        /// Destination directory. Must not already contain
        /// `inkhaven.hjson` — pick a fresh directory or wipe the old one
        /// first.
        #[arg(long)]
        to: PathBuf,
    },

    /// Evaluate a Bund expression against the Adam VM and print the
    /// top of the workbench. Phase-0 smoke command — does not open
    /// the project store. Use this to verify the scripting layer
    /// works on your install and to experiment with Bund syntax.
    Bund {
        /// The Bund script to run, e.g. `"40 2 + ."`.
        code: String,
    },

    /// Print a per-paragraph stats table (1.2.4+). Title, slug,
    /// status, word count, target %, last modified. System
    /// books are excluded; `--book-name` scopes to one user book
    /// the same way `inkhaven export` does.
    Stats {
        /// Name of the user book to report on. Required when the
        /// project holds more than one user book; with a single
        /// user book it can be omitted.
        #[arg(long)]
        book_name: Option<String>,
    },

    /// Print a health report for the inkhaven install (1.2.5+).
    /// Three sections: binary (version + typst engine + font
    /// counts + package cache), project (when run inside an
    /// initialised project: hierarchy shape + word counts), and
    /// notes (actionable warnings like "typst not on PATH"). No
    /// questions asked, pipe-friendly plain-text output.
    Doctor,

    /// 1.2.6+ — story-timeline event management. Requires
    /// `timeline.enabled: true` in HJSON.
    #[command(subcommand)]
    Event(EventCommand),

    /// 1.2.6+ — run the same flow as the TUI's Ctrl+B B
    /// without launching the TUI. Assembles the named user
    /// book into the artefacts directory and (with
    /// `--compile`) runs `typst compile` on the produced root
    /// `.typ`. Pipe-friendly progress on stderr; only the
    /// final PDF path lands on stdout. Useful for CI, batch
    /// builds, and end-to-end verification of the
    /// HJSON-driven `settings.typ`.
    Build {
        /// User-book name (case-insensitive title or slug).
        /// Optional when the project has exactly one user
        /// book; required otherwise.
        #[arg(long)]
        book_name: Option<String>,
        /// Also invoke `typst compile` on the assembled root
        /// `.typ`. Without it the command stops after
        /// writing the artefacts tree.
        #[arg(long)]
        compile: bool,
    },

    /// Launch the TUI editor (default if no subcommand is given).
    Tui,
}

/// Sub-subcommands under `inkhaven event …`.
#[derive(Debug, Subcommand)]
pub enum EventCommand {
    /// Create a new event under the named book's Timeline
    /// chapter (created lazily on first use).
    Add {
        /// Event title (free-form). Becomes the paragraph's
        /// display name + slug seed.
        title: String,
        /// Calendar-formatted start time. See
        /// `timeline.calendar` in HJSON for the syntax
        /// (defaults: sols `Sol N`; gregorian `Y.M.D`;
        /// custom `1A.3.15`).
        #[arg(long)]
        start: String,
        /// Calendar-formatted end time. Omit for an instant
        /// event.
        #[arg(long)]
        end: Option<String>,
        /// Precision override. When unset, inferred from the
        /// shape of `--start` (no day segment → month; no
        /// month → year; season name → season).
        #[arg(long)]
        precision: Option<String>,
        /// Track / POV / parallel-storyline label. Defaults
        /// to `timeline.default_track`.
        #[arg(long)]
        track: Option<String>,
        /// Book slug or title (case-insensitive). Required
        /// when the project holds more than one user book.
        #[arg(long)]
        book_name: Option<String>,
    },
    /// List events in chronological order.
    List {
        /// Filter to a single book.
        #[arg(long)]
        book_name: Option<String>,
        /// Track filter (case-insensitive exact match).
        #[arg(long)]
        track: Option<String>,
    },
    /// Show details for one event by slug-path.
    Show {
        /// Slug-path of the event paragraph.
        path: String,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliNodeKind {
    Book,
    Chapter,
    Subchapter,
    Paragraph,
    /// Bund script — a `.bund` file `bund.eval`'d into Adam at
    /// project open. Default home is the `Scripts` system book,
    /// but Scripts can also live inside any user Book.
    Script,
}

impl From<CliNodeKind> for NodeKind {
    fn from(k: CliNodeKind) -> Self {
        match k {
            CliNodeKind::Book => NodeKind::Book,
            CliNodeKind::Chapter => NodeKind::Chapter,
            CliNodeKind::Subchapter => NodeKind::Subchapter,
            CliNodeKind::Paragraph => NodeKind::Paragraph,
            CliNodeKind::Script => NodeKind::Script,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ExportFormat {
    /// Concatenated `.typ` source.
    Typst,
    /// PDF via the `typst` CLI (must be on PATH).
    Pdf,
    /// Markdown via the in-process typst→markdown converter
    /// (`src/export/markdown.rs`).
    Markdown,
    /// LaTeX via the `tylax` crate. No external `pdflatex` needed
    /// for emit — but the user wants `pdflatex` / `xelatex` if they
    /// later compile the result.
    Tex,
    /// EPUB3 zip — markdown intermediate, written via the bundled
    /// `zip` crate.
    Epub,
}

impl Cli {
    pub fn run(self) -> Result<()> {
        let project = self
            .project
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        match self.command.unwrap_or(Command::Tui) {
            Command::Init { path, force } => init::run(&path, force).map_err(Into::into),
            Command::Add {
                kind,
                title,
                parent,
                slug,
                after,
            } => add::run(
                &project,
                kind.into(),
                &title,
                parent.as_deref(),
                slug.as_deref(),
                after.as_deref(),
            )
            .map_err(Into::into),
            Command::List => list::run(&project).map_err(Into::into),
            Command::Delete { path, yes } => delete::run(&project, &path, yes).map_err(Into::into),
            Command::Mv { path, direction } => {
                mv::run(&project, &path, direction).map_err(Into::into)
            }
            Command::Search { query, limit } => {
                search::run(&project, &query, limit).map_err(Into::into)
            }
            Command::Reindex { prune, adopt } => {
                reindex::run(&project, prune, adopt).map_err(Into::into)
            }
            Command::Export {
                format,
                output,
                book_name,
                status,
                tag,
            } => export::run(
                &project,
                format,
                output.as_deref(),
                book_name.as_deref(),
                status.as_deref(),
                tag.as_deref(),
            )
            .map_err(Into::into),
            Command::Ai { prompt, provider } => {
                ai::run(&project, &prompt, provider.as_deref()).map_err(Into::into)
            }
            Command::ImportHelp {
                documents_directory,
            } => import_help::run(&project, &documents_directory).map_err(Into::into),
            Command::ImportTypstHelp => {
                import_typst_help::run(&project).map_err(Into::into)
            }
            Command::ImportScrivener {
                scriv_path,
                draft_as_book,
                skip_research,
                dry_run,
            } => import_scrivener::run(
                &project,
                &scriv_path,
                draft_as_book.as_deref(),
                skip_research,
                dry_run,
            )
            .map_err(Into::into),
            Command::Backup { out } => backup::run(&project, out.as_deref()).map_err(Into::into),
            Command::Restore { archive, to } => {
                restore::run(&archive, &to).map_err(Into::into)
            }
            Command::Bund { code } => bund::run(&code, &project),
            Command::Stats { book_name } => {
                stats::run(&project, book_name.as_deref()).map_err(Into::into)
            }
            Command::Doctor => doctor::run(&project).map_err(Into::into),
            Command::Build { book_name, compile } => {
                build::run(&project, book_name.as_deref(), compile).map_err(Into::into)
            }
            Command::Event(cmd) => event::run(&project, cmd).map_err(Into::into),
            Command::Tui => crate::tui::run(Some(&project)).map_err(Into::into),
        }
    }
}
