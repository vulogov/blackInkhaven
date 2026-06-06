pub mod add;
pub mod ai;
pub mod backup;
pub mod build;
pub mod bund;
pub mod delete;
pub mod recover;
pub mod export;
pub mod export_concordance;
pub mod export_timeline;
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
pub mod doctor_scan;
pub mod event;
pub mod comments;
pub mod language;
pub mod templates;
pub mod thread;
pub mod tts;
pub mod gen_fixture;
pub mod bench_load;
pub mod bench_report;
pub mod epub;
pub mod audiobook;
pub mod continuity;
pub mod facts_scan;
pub mod tension;
pub mod manuscript;
pub mod prompts;
pub mod show_dont_tell;
pub mod stats;
pub(crate) mod book_walk;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};

use crate::store::NodeKind;

/// Resolve the target user book for a CLI command.
///
/// User books are top-level `Book` nodes without a
/// `system_tag` (Characters / Places / Threads etc. carry
/// one).  With `--book-name`, match by title or slug
/// (case-insensitive); without it, succeed only when the
/// project has exactly one user book.
///
/// Returns a borrowed node — callers `.clone()` when they
/// need an owned `Node`.  The `Err` is a ready-to-display
/// message prefixed with `context` (the command name);
/// callers map it into their module's error type
/// (`Error::Store` or `anyhow!`).
///
/// The single home for user-book resolution, shared by
/// build / epub / audiobook / manuscript / event /
/// export-timeline.
pub(crate) fn resolve_user_book<'a>(
    h: &'a crate::store::hierarchy::Hierarchy,
    book_name: Option<&str>,
    context: &str,
) -> std::result::Result<&'a crate::store::node::Node, String> {
    use crate::store::node::Node;
    let user_books: Vec<&Node> = h
        .children_of(None)
        .into_iter()
        .filter(|n| n.kind == NodeKind::Book && n.system_tag.is_none())
        .collect();

    match book_name {
        Some(name) => {
            let needle = name.trim().to_ascii_lowercase();
            user_books
                .iter()
                .copied()
                .find(|b| {
                    b.title.to_ascii_lowercase() == needle
                        || b.slug.to_ascii_lowercase() == needle
                })
                .ok_or_else(|| {
                    let listing = user_books
                        .iter()
                        .map(|b| format!("`{}` (slug: {})", b.title, b.slug))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let listing = if listing.is_empty() {
                        "no user books in this project".to_string()
                    } else {
                        listing
                    };
                    format!(
                        "{context}: no book matches `--book-name {name}`. Available: {listing}"
                    )
                })
        }
        None => match user_books.as_slice() {
            [book] => Ok(*book),
            [] => Err(format!(
                "{context}: project has no user books — add one with `inkhaven add book <title>`"
            )),
            _ => Err(format!(
                "{context}: project has {} user books — pass --book-name",
                user_books.len()
            )),
        },
    }
}

#[derive(Debug, Parser)]
#[command(name = "inkhaven", version, about = "TUI literary work editor for Typst books")]
pub struct Cli {
    /// Path to a project root. For `init`, this is the project to create. For
    /// every other subcommand, defaults to the current directory.  Accepts
    /// `--project`, the longer alias `--project-directory`, and the short
    /// form `-p` (1.2.10+).
    #[arg(long, short = 'p', alias = "project-directory", global = true)]
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
        /// Project template to scaffold the
        /// manuscript book + chapters + system-book
        /// seed entries.  Accepts:
        /// `empty` (default, current behavior),
        /// `novel` (three-act manuscript + Characters
        /// stubs), `nonfiction` (intro/parts/
        /// conclusion + Research methodology),
        /// `rpg-sourcebook` (Setting/Rules/
        /// Adventures/Appendices + Places +
        /// Artefacts + Threads seeds), `technical`
        /// (Overview/Reference/Tutorials/Index),
        /// `nanowrimo` (like `novel` but with a
        /// 50000-word goal + next-November pacing).
        /// Run `inkhaven template list` to see all
        /// available templates with descriptions.
        #[arg(long, default_value = "empty")]
        template: String,
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

    /// 1.2.12+ — export the project-wide concordance (every
    /// distinct lexical stem with count + KWIC samples) to a file
    /// for use in spreadsheets / analysis pipelines.  Same data the
    /// `Ctrl+B Shift+L` modal shows: stop-words / single-char
    /// tokens / pure digits filtered out; Snowball-stemmed so
    /// `walk` / `walked` / `walking` collapse to one row.  System
    /// books (Prompts / Characters / Places / Lore / Help / Notes /
    /// Artefacts) excluded — same scope as the in-TUI view.
    /// Multilingual via the project's `language` field.
    ExportConcordance {
        /// Output format.  CSV is one row per stem with semicolon-
        /// separated sample slug-paths; JSON is the structured form
        /// for downstream tooling.
        #[arg(value_enum, default_value_t = ConcordanceExportFormat::Csv)]
        format: ConcordanceExportFormat,
        /// Output path.  Required.
        #[arg(short, long)]
        output: PathBuf,
        /// Minimum count threshold.  Stems occurring fewer than
        /// this many times across the project are dropped from
        /// the export.  Default: 1 (everything).
        #[arg(long, default_value_t = 1)]
        min_count: usize,
    },

    /// Print a health report for the inkhaven install (1.2.5+).
    /// Three sections: binary (version + typst engine + font
    /// counts + package cache), project (when run inside an
    /// initialised project: hierarchy shape + word counts), and
    /// notes (actionable warnings like "typst not on PATH"). No
    /// questions asked, pipe-friendly plain-text output.
    ///
    /// 1.2.9+ — `--voices` swaps the default report for a
    /// pipe-friendly list of TTS voices visible to the host
    /// OS (`tts-rs`).  Useful for picking a value for
    /// `editor.tts.voice` in HJSON without leaving the
    /// terminal.
    Doctor {
        /// List every TTS voice the host OS exposes through
        /// `tts-rs`, one per line: `<name>  ·  <locale>`.
        /// Skips the rest of the health report when set.
        #[arg(long)]
        voices: bool,
        /// 1.2.9+ — diagnostic: init the TTS engine, set
        /// the configured voice + rate, speak the given
        /// text synchronously (block until audio drains),
        /// then exit.  Use when `Ctrl+B S` shows the
        /// modal but no audio plays — isolates the engine
        /// path from the rest of inkhaven's runtime.
        #[arg(long, value_name = "TEXT")]
        tts_test: Option<String>,
        /// 1.2.9+ — emit a copy-paste-ready HJSON
        /// snippet of every built-in filter-word list
        /// (English, Russian, French, German, Spanish).
        /// Paste under `editor.style_warnings.filter_words`
        /// to see and edit them in your project HJSON.
        #[arg(long)]
        filter_words_snippet: bool,
        /// 1.2.15+ — run the project scan only,
        /// skipping the dep-version / typst-engine
        /// dump.  Walks the hierarchy + on-disk
        /// files looking for zero-byte paragraphs,
        /// orphan DB rows, missing referenced files,
        /// and corrupt comment sidecars.
        #[arg(long)]
        scan: bool,
        /// 1.2.15+ — emit the scan results as
        /// JSON instead of human prose.  Implies
        /// `--scan`.  Useful for CI gates: `inkhaven
        /// doctor --json | jq -e '.findings == []'`.
        #[arg(long)]
        json: bool,
        /// 1.2.15+ — limit the scan to a single
        /// class.  Accepts: `zero-byte-file`,
        /// `orphan-paragraph-row`, `missing-
        /// referenced-file`, `corrupt-comments-
        /// sidecar`.
        #[arg(long, value_name = "CLASS")]
        class: Option<String>,
        /// 1.2.15+ Phase D.2 — apply per-class
        /// repairs.  Prompts `y/N` per finding
        /// unless `--yes` is also passed.  Every
        /// repair is logged to
        /// `<project>/.inkhaven/doctor.log` with
        /// before/after for audit.
        #[arg(long)]
        autofix: bool,
        /// Pair with `--autofix` to skip the per-
        /// finding prompt.  Intended for CI gates
        /// + scripted cleanup; refuses to start
        /// without `--autofix`.
        #[arg(long)]
        yes: bool,
    },

    /// 1.2.6+ — story-timeline event management. Requires
    /// `timeline.enabled: true` in HJSON.
    #[command(subcommand)]
    Event(EventCommand),

    /// 1.2.8+ — export a book's timeline (events grouped
    /// chronologically per track) to a file. Three formats:
    /// `typst` (a text listing typst users `#include`),
    /// `svg` (a self-contained swim-lane render — circles
    /// for instant events, bars for duration events, a
    /// date axis at the top), and `png` (the same SVG
    /// rasterised through resvg + tiny-skia).
    ExportTimeline {
        /// User-book name (case-insensitive title or slug).
        /// Optional when the project has exactly one user
        /// book; required otherwise. The book's Timeline
        /// chapter is read.
        #[arg(long)]
        book_name: Option<String>,
        /// Output format. Choose one of `typst` (text
        /// listing, default), `svg` (vector swim lane),
        /// or `png` (rasterised SVG).
        #[arg(value_enum, default_value_t = TimelineExportFormat::Typst)]
        format: TimelineExportFormat,
        /// Output path. Required.
        #[arg(short, long)]
        output: PathBuf,
        /// Optional track filter (case-insensitive). When
        /// set, only events on that track land in the
        /// output. Omit to include every track.
        #[arg(long)]
        track: Option<String>,
    },

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

    /// 1.2.19+ X.1 — export a user book to a
    /// submission-ready Shunn standard manuscript format
    /// typst document (monospace, double-spaced, title
    /// page with rounded word count, running
    /// `Surname / KEYWORD / page` header, scene breaks as
    /// `#`).  The finishing-line companion to the
    /// reader-facing `epub` + `audiobook` exports.
    /// Compile to PDF with `typst compile <out>.typ`.
    Manuscript {
        /// User-book name (case-insensitive title or
        /// slug).  Optional when the project has exactly
        /// one user book.
        #[arg(long)]
        book_name: Option<String>,
        /// Output path.  Defaults to
        /// `<project>/<book-slug>-manuscript.typ`.
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
        /// Override the title (default: the book's title).
        #[arg(long)]
        title: Option<String>,
        /// Author / byline (default:
        /// `editor.comment_author`).
        #[arg(long)]
        author: Option<String>,
        /// Title-page contact block; use `\n` for line
        /// breaks (default: the author name).
        #[arg(long)]
        contact: Option<String>,
    },

    /// 1.2.18+ R.1 — export a user book to a
    /// standards-compliant EPUB 3 file.  Walks the
    /// book's chapters in order, converts the typst
    /// prose to XHTML, and assembles the container.
    /// The reader-facing companion to `inkhaven build`
    /// (which targets typst → PDF).
    Epub {
        /// User-book name (case-insensitive title or
        /// slug).  Optional when the project has exactly
        /// one user book; required otherwise.
        #[arg(long)]
        book_name: Option<String>,
        /// Output path.  Defaults to
        /// `<project>/<book-slug>.epub`.
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
        /// Override the EPUB title (default: the book's
        /// title).
        #[arg(long)]
        title: Option<String>,
        /// Override the author (default:
        /// `editor.comment_author`, else "Unknown
        /// Author").
        #[arg(long)]
        author: Option<String>,
    },

    /// 1.2.18+ R.2 — synthesise a user book to a
    /// single `.m4b` audiobook with a chapter marker per
    /// Chapter node.  Drives the TTS engine (Piper or
    /// macOS `say`) for per-chapter synthesis, then
    /// `ffmpeg` for the concat + chapter-metadata mux.
    /// Requires ffmpeg + ffprobe on PATH and
    /// `editor.tts.enabled = true`.  Synthesis is
    /// roughly real-time — a batch export, not
    /// interactive.
    Audiobook {
        /// User-book name (case-insensitive title or
        /// slug).  Optional when the project has exactly
        /// one user book.
        #[arg(long)]
        book_name: Option<String>,
        /// Output path.  Defaults to
        /// `<project>/<book-slug>.m4b`.
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
        /// Override the audiobook title (default: the
        /// book's title).
        #[arg(long)]
        title: Option<String>,
        /// Override the author (default:
        /// `editor.comment_author`).
        #[arg(long)]
        author: Option<String>,
    },

    /// 1.2.19+ C.3 — `inkhaven continuity
    /// <subcommand>`.  Build + inspect the continuity
    /// bible: `extract` runs the AI fact-extraction pass
    /// over the manuscript into
    /// `<project>/.inkhaven/continuity.json`; `list`
    /// dumps it.  The `continuity-drift` doctor scan then
    /// flags attributes that change across chapters.
    #[command(subcommand)]
    Continuity(ContinuityCommand),

    /// 1.2.19+ C.4 — `inkhaven tension <subcommand>`.
    /// `scan` runs the AI pass that tags each chapter's
    /// introduced + resolved tensions into
    /// `<project>/.inkhaven/tensions.json`; `list` dumps
    /// it.  Then `doctor --scan --class
    /// unresolved-tension` (opt-in) flags introduced
    /// tensions with no downstream payoff.
    #[command(subcommand)]
    Tension(TensionCommand),

    /// 1.2.21+ FF.2 — `inkhaven facts <subcommand>`.
    /// `scan` runs the AI pass that checks every chapter's
    /// prose against the Facts book (semantically-retrieved
    /// relevant facts per chapter) and records contradictions
    /// to `<project>/.inkhaven/facts_scan.json`; `list` dumps
    /// them.  `--json` emits the report for CI gates.
    #[command(subcommand)]
    Facts(FactsCommand),

    /// Launch the TUI editor (default if no subcommand is given).
    Tui,

    /// 1.2.10+ — launch the standalone TUI configuration
    /// editor for `<project>/inkhaven.hjson`.  Tree-pane
    /// hierarchy on the left, schema-aware widgets on the
    /// right.  Read-only walk-through in Phase 1; typed
    /// editing + save + versioned backups + rollback in
    /// subsequent phases.  See
    /// `Documentation/PROPOSALS/CONFIG_TUI.md`.
    ///
    /// The existing `Ctrl+B 0` in-app HJSON editor stays
    /// as the power-user fallback for raw text editing.
    Config,

    /// 1.2.11+ — launch the standalone TUI prompts
    /// editor for `<project>/prompts.hjson`.  Four-pane
    /// workbench: prompts list (left), prompt editor
    /// (centre, same chord set as the main inkhaven
    /// editor), AI response (right), AI prompt input
    /// (bottom).  Phase 1 ships read-only; editing +
    /// save + AI integration in subsequent phases.
    /// See `Documentation/PROPOSALS/PROMPTS_EDITOR_TUI.md`.
    PromptsEditor,

    /// 1.2.11+ — show-don't-tell tooling.  Currently
    /// hosts `bootstrap`, which uses the configured LLM
    /// to generate the four per-language word lists
    /// (linking_verbs / emotion_adjectives /
    /// manner_adverbs / cognition_verbs) for the
    /// show-don't-tell overlay.  Output is an HJSON
    /// snippet on stdout — never writes to your
    /// `inkhaven.hjson` automatically; review and paste
    /// what you like.  Pattern mirrors
    /// `doctor --filter-words-snippet`.
    #[command(subcommand, name = "show-dont-tell")]
    ShowDontTell(ShowDontTellCommand),

    /// 1.2.12+ Phase B — prompts tooling.  Currently
    /// hosts `bootstrap <lang>`, which uses the
    /// configured LLM to generate per-language
    /// variants of the seven inkhaven embedded
    /// prompts (`grammar-check`, `show-don't-tell`,
    /// `sentence-rhythm-rewrite`, `critique-edit`,
    /// `critique-changes`, `explain-diagnostic`,
    /// `timeline-health`).  Output is an HJSON
    /// snippet ready to paste under
    /// `prompts.hjson`; with `--update` it merges
    /// into the live file in place via the same
    /// `apply_in_place_edits` helper the SDT
    /// bootstrap uses.  See
    /// `Documentation/PROPOSALS/MULTILINGUAL_PROMPTS.md`.
    #[command(subcommand)]
    Prompts(PromptsCommand),

    /// invented-language tooling.
    /// Scaffolds the per-language sub-books inside
    /// the top-level `Language` system book.  See
    /// `Documentation/PROPOSALS/LANGUAGE_BOOK.md`
    /// for the full design (dictionary entry HJSON
    /// schema, grammar-rule schema, phonology,
    /// sample-text, AI translation flow).  Phase A
    /// ships `init` only; phases B-D add lexicon
    /// overlay, AI translation, export, doctor.
    #[command(subcommand)]
    Language(LanguageCommand),
    /// 1.2.14+ — `inkhaven thread <subcommand>`.
    /// Plot-thread management surface — add /
    /// list narrative arcs stored as HJSON-fronted
    /// paragraphs under the `Threads` system book.
    /// See `Documentation/PROPOSALS/1.2.14_PLAN.md`.
    #[command(subcommand)]
    Thread(ThreadCommand),
    /// `inkhaven template
    /// <subcommand>`.  Surfaces information about
    /// the project templates available to
    /// `inkhaven init --template <name>`.
    #[command(subcommand)]
    Template(TemplateCommand),
    /// `inkhaven comments
    /// <subcommand>`.  Headless manipulation of
    /// the per-paragraph sidecar comment files
    /// (`.comments.json`).  Mirrors the in-TUI
    /// `Ctrl+V Shift+C` panel.
    #[command(subcommand)]
    Comments(CommentsCommand),

    /// 1.2.18+ I.1.1 — `inkhaven gen-fixture
    /// <path>` (hidden).  Generates a deterministic
    /// 10K-paragraph synthetic project for the
    /// criterion bench harness.  See
    /// `Documentation/PROPOSALS/1.2.18_PLAN.md`.
    /// Hidden from `--help` because end-user projects
    /// should not run this by accident.
    #[command(hide = true, name = "gen-fixture")]
    GenFixture {
        /// Target directory.  Wiped + recreated; use
        /// `--force` to skip the confirmation prompt.
        path: PathBuf,
        #[arg(long, default_value_t = 5)]
        books: usize,
        #[arg(long, default_value_t = 20)]
        chapters: usize,
        #[arg(long, default_value_t = 100)]
        paragraphs: usize,
        #[arg(long, default_value_t = 450)]
        target_words: u32,
        #[arg(long, default_value_t = 0xC0FFEE_DEAD_BEEFu64)]
        seed: u64,
        #[arg(long)]
        force: bool,
    },

    /// 1.2.18+ I.1.3 — `inkhaven _bench-load`
    /// (hidden).  Opens the project with phase timers
    /// + reports per-phase millis on stdout.  The
    /// in-process bench hook the I.1.2.b plan
    /// anticipated; doubles as the I.1.3 profiling
    /// instrument (a true sampling flamegraph needs
    /// dtrace, which SIP blocks without sudo).  Pair
    /// with `INKHAVEN_PERF_TRACE=1` for the
    /// sub-phase store-open + hierarchy-load breakdown.
    #[command(hide = true, name = "_bench-load")]
    BenchLoad {
        /// Search query for the search-phase timing.
        #[arg(long, default_value = "the harbor")]
        query: String,
        /// Iterations to average flatten + search over.
        #[arg(long, default_value_t = 20)]
        iterations: usize,
    },

    /// 1.2.18+ I.1.7 — `inkhaven _bench-report`
    /// (hidden).  Compares two criterion output trees +
    /// emits a markdown/plain delta table + exits 2 on
    /// any regression past `--threshold`.  Drives the
    /// CI bench gate (`.github/workflows/bench.yml`).
    #[command(hide = true, name = "_bench-report")]
    BenchReport {
        /// Baseline criterion dir (restored from the
        /// main branch's last run).  When absent /
        /// missing, every bench is treated as new (no
        /// regression possible).
        #[arg(long)]
        baseline: Option<PathBuf>,
        /// Current-run criterion dir.  Defaults to
        /// `<target>/criterion`.
        #[arg(long)]
        current: Option<PathBuf>,
        /// Regression threshold as a fraction (0.20 =
        /// 20%).
        #[arg(long, default_value_t = 0.20)]
        threshold: f64,
        /// Emit GitHub-flavoured markdown (for a PR
        /// comment) instead of plain text.
        #[arg(long)]
        markdown: bool,
    },

    /// 1.2.17+ T.7 — `inkhaven tts <subcommand>`.
    /// Headless management of the Piper TTS stack:
    /// engine status, binary install/inspect, voice
    /// list/download/remove, catalog refresh, and
    /// cross-platform synthesis test.  Mirrors the
    /// in-TUI `Ctrl+B Shift+V` voice picker for
    /// scripts, CI gates, and remote-shell users.
    #[command(subcommand)]
    Tts(TtsCommand),

    /// `inkhaven recover <crash-report.hjson>` —
    /// pick up an inkhaven crash report and walk the
    /// rescued-buffer manifest, optionally applying
    /// each rescue back to its on-disk paragraph file
    /// after writing a `.pre-recover-<UTC>` rollback
    /// backup.  Default behaviour prompts y/N/diff
    /// per buffer; `--yes` bypasses the prompt and
    /// applies every rescue.  `--keep` leaves the
    /// report + rescue files in place; default moves
    /// them into `<project>/.inkhaven/recovered/`.
    Recover {
        /// Path to the `inkhaven-crash-<ts>.hjson`
        /// report written by the panic hook.
        report: PathBuf,
        /// Skip prompts; apply every rescue.
        #[arg(long)]
        yes: bool,
        /// Leave the report + rescue files in place
        /// after the walk.  Default moves them into
        /// `<project>/.inkhaven/recovered/`.
        #[arg(long)]
        keep: bool,
    },
}

/// sub-subcommands under
/// `inkhaven comments …`.
#[derive(Debug, Subcommand)]
pub enum CommentsCommand {
    /// Print every comment in the project (or
    /// filtered to a paragraph slug).  Default
    /// shows all; pass `--unresolved-only` to
    /// hide resolved comments.
    List {
        /// Limit to comments under this paragraph
        /// slug-path.
        #[arg(long)]
        paragraph: Option<String>,
        /// Hide resolved comments.
        #[arg(long)]
        unresolved_only: bool,
    },
    /// Mark a comment as resolved.  Identifies
    /// the comment by its UUID.
    Resolve {
        /// Comment UUID.
        id: String,
    },
    /// Re-open (un-resolve) a comment.
    Reopen {
        id: String,
    },
    /// Delete a comment.  Immediate; no undo.
    Delete {
        id: String,
    },
    /// Export every comment in the project as
    /// structured JSON.  Streams to stdout when
    /// `--output` is omitted.
    Export {
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
    },
}

/// sub-subcommands under
/// `inkhaven template …`.
#[derive(Debug, Subcommand)]
pub enum TemplateCommand {
    /// List every available project template with
    /// a one-line description.  Use the `name`
    /// column as the `--template <name>` value for
    /// `inkhaven init`.
    List,
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

/// 1.2.12+ Phase B — sub-subcommands under
/// `inkhaven prompts …`.
#[derive(Debug, Subcommand)]
pub enum PromptsCommand {
    /// Generate per-language variants of inkhaven's
    /// seven embedded prompts using the configured
    /// LLM.  Emits an HJSON snippet on stdout (default)
    /// or, with `--update`, merges into
    /// `<project>/prompts.hjson` in place — versioned
    /// backup + atomic write + comment preservation
    /// via the shared `apply_in_place_edits` helper.
    /// Mirrors `inkhaven show-dont-tell bootstrap`.
    Bootstrap {
        /// Target language.  One of: english, russian,
        /// french, german, spanish.  Mapped to ISO 639-1
        /// (`en`/`ru`/`fr`/`de`/`es`) for the
        /// `language:` field on each generated prompt
        /// entry — that's the value the prompt resolver
        /// compares against.
        language: String,
        /// Optional genre / register hint folded into
        /// the prompt so the model picks vocabulary at
        /// the right reading level ("literary fiction",
        /// "thriller", "YA fantasy", …).
        #[arg(long)]
        genre: Option<String>,
        /// Override the default LLM provider for this
        /// invocation.  Same semantics as
        /// `inkhaven ai --provider`.
        #[arg(long)]
        provider: Option<String>,
        /// Apply the LLM-generated prompts **in place**
        /// to `prompts.hjson`, merging with any
        /// existing same-name entries (case-insensitive
        /// name match + `language` field match — only
        /// overwrites the exact `(name, language)`
        /// pair, leaves every other entry untouched).
        /// A versioned backup of the pre-patch file
        /// lands under `<project>/.config-backups/`
        /// first.  Without `--update`, prints the
        /// snippet to stdout and touches nothing.
        #[arg(long)]
        update: bool,
    },
}

/// 1.2.19+ C.3 — sub-subcommands under
/// `inkhaven continuity …`.
#[derive(Debug, Subcommand)]
pub enum ContinuityCommand {
    /// Run the AI fact-extraction pass over the
    /// manuscript, writing the continuity bible to
    /// `<project>/.inkhaven/continuity.json`.  Uses the
    /// configured LLM + a per-language prompt.
    Extract {
        /// LLM provider override (defaults to
        /// `llm.default`).
        #[arg(long)]
        provider: Option<String>,
    },
    /// Dump the extracted continuity bible — each
    /// character's facts, by attribute + chapter.
    List,
}

/// 1.2.19+ C.4 — sub-subcommands under
/// `inkhaven tension …`.
#[derive(Debug, Subcommand)]
pub enum TensionCommand {
    /// Run the AI pass that tags each chapter's
    /// introduced + resolved tensions into
    /// `<project>/.inkhaven/tensions.json`.
    Scan {
        /// LLM provider override (defaults to
        /// `llm.default`).
        #[arg(long)]
        provider: Option<String>,
    },
    /// Dump the tension ledger.
    List,
}

/// 1.2.21+ FF.2 — sub-subcommands under
/// `inkhaven facts …`.
#[derive(Debug, Subcommand)]
pub enum FactsCommand {
    /// Run the AI fact-check pass: check every chapter's
    /// prose against the Facts book (relevant facts
    /// retrieved per chapter via semantic search) and write
    /// contradictions to `<project>/.inkhaven/facts_scan.json`.
    Scan {
        /// LLM provider override (defaults to `llm.default`).
        #[arg(long)]
        provider: Option<String>,
        /// Emit the report as JSON (for CI gates).
        #[arg(long)]
        json: bool,
    },
    /// Dump the last scan's findings (re-runs nothing).
    List {
        /// Emit the report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Propose world-facts from the manuscript prose and
    /// (after an interactive y/N/a/q review) add the
    /// accepted ones to the Facts book.  Deduped against
    /// existing entries.  Solves the cold-start of an empty
    /// Facts book.
    Extract {
        /// LLM provider override (defaults to `llm.default`).
        #[arg(long)]
        provider: Option<String>,
        /// Accept every proposed fact without prompting.
        #[arg(long)]
        yes: bool,
        /// List the proposed facts without adding any.
        #[arg(long)]
        dry_run: bool,
    },
    /// Scaffold the starter category paragraphs (Climate,
    /// Geography, Seasons, Chronology, Culture, Rules) in
    /// the Facts book — fill-in-the-blanks for a fresh
    /// project.  Idempotent: existing categories are kept.
    /// `--genre` appends genre-specific categories
    /// (fantasy / scifi / mystery / historical).
    Init {
        /// Add the skeleton even if categories already exist.
        #[arg(long)]
        force: bool,
        /// Append genre-specific categories: general (default),
        /// fantasy, scifi, mystery, historical.
        #[arg(long)]
        genre: Option<String>,
    },
}

/// 1.2.17+ T.7 — sub-subcommands under
/// `inkhaven tts …`.  Mirrors the in-TUI
/// `Ctrl+B Shift+V` voice picker + adds engine
/// + binary diagnostics + cross-platform synth
/// testing.
#[derive(Debug, Subcommand)]
pub enum TtsCommand {
    /// `inkhaven tts engine` — print which TTS
    /// backend is active for the project (System
    /// `say` vs. Piper) and why.  Honours
    /// `tts.engine` (`auto` | `piper` | `system`).
    Engine,
    /// `inkhaven tts binary <action>` — Piper
    /// binary management.
    #[command(subcommand)]
    Binary(TtsBinarySubcommand),
    /// `inkhaven tts voice <action>` — voice
    /// catalog browsing + per-voice download /
    /// remove.
    #[command(subcommand)]
    Voice(TtsVoiceSubcommand),
    /// `inkhaven tts catalog <action>` — voice
    /// catalog cache management.
    #[command(subcommand)]
    Catalog(TtsCatalogSubcommand),
    /// `inkhaven tts test "<phrase>" [--voice
    /// <name>]` — cross-platform synthesis test
    /// routed through `TtsEngine::resolve`.
    /// Synthesises to a temp WAV + plays via the
    /// platform default (or `tts.play_command`).
    /// Reports the active backend + binary path
    /// + voice path + bytes-out.  Exits 0 on
    /// success, non-zero on synth or playback
    /// failure.
    Test {
        /// Phrase to speak.  Required.
        phrase: String,
        /// Override `editor.tts.voice` for this
        /// invocation only.  Useful for A/B-ing
        /// voices without editing HJSON.
        #[arg(long)]
        voice: Option<String>,
        /// Synthesise to a file instead of
        /// playing.  Suppresses the playback step.
        #[arg(long, value_name = "PATH")]
        output: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
pub enum TtsBinarySubcommand {
    /// Print where the Piper binary is on disk
    /// (or "not installed"), plus the resolved
    /// platform identifier + cache root.
    Status,
    /// Explicitly download the platform-
    /// appropriate Piper binary into the user
    /// cache.  Idempotent — re-downloads if the
    /// existing binary is corrupt or missing.
    Download,
}

#[derive(Debug, Subcommand)]
pub enum TtsVoiceSubcommand {
    /// Print every voice in the catalog with a
    /// downloaded / available chip + size.  Falls
    /// back to "voices on disk only" when the
    /// catalog can't be loaded.
    List {
        /// Filter to voices whose canonical key or
        /// language code contains <NEEDLE>.  Case-
        /// insensitive.
        #[arg(long)]
        filter: Option<String>,
        /// Show only voices that are already
        /// downloaded into the project.
        #[arg(long)]
        downloaded: bool,
    },
    /// Download a specific voice by canonical key
    /// (e.g. `en_US-lessac-medium`) or alias.
    /// Atomic install via `crate::io_atomic`;
    /// .gitignore updated per `tts.auto_gitignore`.
    Download {
        /// Voice key or alias.
        name: String,
    },
    /// Delete a downloaded voice + drop it from
    /// the LRU index.  Safe — the catalog itself
    /// is untouched, so the voice can be re-
    /// downloaded.
    Remove {
        /// Voice key as stored on disk.
        name: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum TtsCatalogSubcommand {
    /// Force a catalog refresh — deletes the
    /// cached `voices.json` so the next operation
    /// fetches from `tts.catalog_url`.
    Refresh,
}

/// sub-subcommands under
/// `inkhaven language …`.
#[derive(Debug, Subcommand)]
pub enum LanguageCommand {
    /// Scaffold a new language sub-book under the
    /// top-level `Language` system book.  Creates
    /// the per-language `<Name>` book plus the five
    /// standard chapters (`Meta`, `Dictionary`,
    /// `Grammar`, `Phonology`, `Sample texts`) and
    /// seeds `Meta/overview.typ` with an empty
    /// HJSON config the author fills in.  No
    /// alphabet subchapters are created yet — they
    /// auto-spawn on the first dictionary entry
    /// once `add-word` lands in Phase B.
    Init {
        /// Display name for the language.  Becomes
        /// the per-language book title — `Quenya`,
        /// `Drow`, `Klingon`, etc.  Title-case
        /// recommended; the slug is auto-derived.
        name: String,
    },
    /// add a dictionary entry to
    /// a language's `Dictionary` chapter.  Auto-
    /// creates the alphabet subchapter from the
    /// language's `Meta/overview.alphabet` field
    /// (or A-Z fallback) if it doesn't yet exist.
    /// Seeds the entry paragraph with the four
    /// core HJSON fields (`word`, `type`,
    /// `translation`, `example`) — author edits
    /// to add optional fields (`pronunciation`,
    /// `etymology`, `related`, `inflection`,
    /// `notes`).  Rejects duplicate words under
    /// the same language.
    AddWord {
        /// Target language name (case-insensitive
        /// match against existing Language sub-book
        /// titles).
        language: String,
        /// The word being defined.  Title-case as
        /// the author prefers; the slug is
        /// auto-derived.  Required UNLESS --import
        /// is set, in which case this positional is
        /// ignored.
        word: Option<String>,
        /// Part of speech.  Free-form string; the
        /// proposal §3 suggests `noun | verb |
        /// adjective | adverb | pronoun |
        /// preposition | conjunction |
        /// interjection | particle` but the field
        /// is open so the author can use language-
        /// specific categories.  Required unless
        /// --import is set.
        #[arg(long, short = 't')]
        r#type: Option<String>,
        /// Translation into the project's working
        /// language.  Required unless --import is
        /// set.
        #[arg(long)]
        translation: Option<String>,
        /// Optional canonical sample sentence the
        /// author wants frozen into the entry.
        #[arg(long)]
        example: Option<String>,
        /// bulk-import a CSV
        /// dictionary.  When set, the positional
        /// <word> + the --type / --translation /
        /// --example flags are ignored; every row
        /// of the CSV becomes an entry.
        ///
        /// CSV format: header row drives column
        /// mapping (any subset / order accepted).
        /// Required columns: `word`, `type`,
        /// `translation`.  Optional: `example`,
        /// `pronunciation`, `etymology`, `related`
        /// (`;`-separated), `inflection`
        /// (`;`-separated `key=value` pairs),
        /// `examples` (`|`-separated additional
        /// sentences), `register`, `era`, `notes`.
        /// Comment rows: `word` starting with `#`.
        /// Empty `word` rows: skipped silently.
        /// Duplicate words: skipped with warning.
        /// Tally printed at end.
        #[arg(long, value_name = "PATH")]
        import: Option<PathBuf>,
        /// when used with --import,
        /// WIPE the language's existing Dictionary
        /// chapter (every bucket subchapter + every
        /// entry paragraph) before importing the CSV.
        /// The Dictionary chapter itself is preserved
        /// — only its contents are cleared.  Without
        /// --new, existing entries are kept and the
        /// import is "update / add" semantics (duplicate
        /// words skipped, new rows added).
        #[arg(long, requires = "import")]
        new: bool,
        /// skip the pre-flight
        /// alphabet + phonology validation that
        /// normally aborts an import when any word
        /// uses characters outside the language's
        /// declared alphabet OR uses phonemes
        /// outside the declared phoneme inventories.
        /// Use when intentionally importing words
        /// that exceed the current Meta/overview
        /// declaration (e.g. you're seeding the
        /// alphabet from the CSV itself).
        #[arg(long, requires = "import")]
        force: bool,
    },
    /// health report for a language
    /// sub-book.  Counts dictionary entries, entries
    /// with examples, entries with inflection
    /// paradigms, grammar / phonology rule counts,
    /// sample-text count, and (when the project has
    /// authored prose) the manuscript words that
    /// appear as translations in the dictionary versus
    /// the working-language words in the manuscript
    /// that have no dictionary coverage.  Exit code
    /// 0 always — the report is informational, not a
    /// pass/fail gate.  See the proposal §13.
    Doctor {
        /// Language to inspect (case-insensitive
        /// match against existing Language sub-book
        /// titles).
        language: String,
        /// Emit the report as structured JSON instead
        /// of the human-readable text format.  Useful
        /// for CI gates and shell pipelines that want
        /// to grep for `coverage.with_example_pct <
        /// 80` etc.
        #[arg(long)]
        json: bool,
    },
    /// list every defined
    /// language with summary counts (dictionary
    /// entries, grammar / phonology rules, sample
    /// texts).  Companion to `inkhaven language
    /// doctor` for a quick at-a-glance overview of
    /// every language in the project.
    List,
    /// remove a dictionary entry
    /// from a language.  Mirror of `add-word`:
    /// resolves the language sub-book by case-
    /// insensitive title; finds the Dictionary
    /// chapter + bucket subchapter (derived from the
    /// word's first character); deletes the entry
    /// paragraph.  Errors when the entry doesn't
    /// exist rather than silently no-op-ing.
    RemoveWord {
        /// Target language name.
        language: String,
        /// The word to remove (case-insensitive
        /// title match).
        word: String,
    },
    /// 1.2.16+ Phase P.5 — define-or-edit a
    /// grammar or phonology rule in `$EDITOR`.
    /// Opens the rule's HJSON template in the
    /// user's `$EDITOR` (or `vi` if unset), then
    /// — on save — writes the resulting body
    /// back into a new or existing rule paragraph
    /// under the chosen category.  Pairs with the
    /// `--format grammar` exporter.
    DefineRule {
        /// Target language name (case-insensitive).
        language: String,
        /// Unique rule identifier (kebab-case
        /// recommended) — used as the paragraph
        /// slug and the `rule_id` field in the
        /// HJSON body.
        rule_id: String,
        /// Which chapter the rule lives under.
        /// `grammar` or `phonology`.
        #[arg(long, default_value = "grammar")]
        category: String,
    },
    /// export a language's content
    /// to a portable artefact.  See the proposal §12.
    /// Three formats land in Phase D; the remaining
    /// two (grammar reference + phrasebook) are
    /// Phase D.2.
    Export {
        /// Language to export (case-insensitive
        /// match against existing Language sub-book
        /// titles).
        language: String,
        /// Output format.  `json` is structured data
        /// for downstream tooling; `anki` is a CSV
        /// flash-card deck; `dictionary-twocol` is a
        /// printable two-column Typst dictionary.
        #[arg(long, short = 'f', default_value = "json")]
        format: LanguageExportFormat,
        /// Output path.  Defaults to stdout when
        /// omitted (json + anki only — typst always
        /// needs a path because the renderer doesn't
        /// stream).
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
    },
}

/// output format selector for
/// `inkhaven language export`.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum LanguageExportFormat {
    /// Full structured dump — overview, dictionary,
    /// grammar, phonology, sample-text content.
    Json,
    /// CSV deck importable by Anki / SuperMemo /
    /// Mochi.  Columns: `word`, `translation`,
    /// `type`, `example`, `inflection`.
    Anki,
    /// Two-column printable Typst dictionary.
    /// Alphabet headers between sections; entries
    /// formatted as: bold headword + POS italic +
    /// translation + examples indented.
    DictionaryTwocol,
    /// 1.2.16+ Phase P.5 — round-trip-compatible
    /// CSV that the `--import` path can re-ingest.
    /// 12-column format matching the dictionary
    /// importer; closes the import/export loop.
    Csv,
    /// 1.2.16+ Phase P.5 — typst-rendered grammar
    /// reference.  TOC + chapter per rule
    /// category (case marking, verb conjugation,
    /// etc.) + examples table + cross-references.
    /// Always needs `--output <path.typ>`.
    Grammar,
    /// 1.2.16+ Phase P.5 — typst-rendered
    /// phrasebook from the Sample-texts chapter.
    /// Two-column layout: working-language gloss
    /// on the left, invented-language sample on
    /// the right.  Always needs `--output
    /// <path.typ>`.
    Phrasebook,
}

/// output format selector for
/// `inkhaven thread export`.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ThreadExportFormat {
    /// Full structured dump.
    Json,
    /// Flat CSV with paradigm fields collapsed
    /// (characters / places / artefacts joined by
    /// `;`).
    Csv,
    /// Printable markdown summary — one section
    /// per thread with arc-shape + connections +
    /// notes.
    Markdown,
}

/// 1.2.14+ — sub-subcommands under
/// `inkhaven thread …`.  Manages plot-thread
/// paragraphs under the `Threads` system book.
/// See `Documentation/PROPOSALS/1.2.14_PLAN.md`.
#[derive(Debug, Subcommand)]
pub enum ThreadCommand {
    /// print a health report
    /// for every thread under the `Threads` system
    /// book.  Same shape as
    /// `inkhaven language doctor`: status
    /// distribution, weight distribution, link
    /// coverage statistics, blind-spot detector
    /// passes (dormant threads, payoff-marked-but-
    /// unfired, status-vs-evidence mismatches).
    /// Exit code 0 always — informational, not a
    /// pass/fail gate.  Add `--json` for CI-
    /// friendly structured output.
    Doctor {
        /// Emit the report as structured JSON
        /// instead of the human-readable text.
        #[arg(long)]
        json: bool,
    },
    /// export every thread's
    /// data to a portable artefact.  See
    /// `Documentation/PROPOSALS/1.2.14_PLAN.md`
    /// §3 for the field shape.
    Export {
        /// Output format.  `json` is structured
        /// data for downstream tooling; `csv` is
        /// a flat table (paradigm fields
        /// flattened to `key=value` pairs);
        /// `markdown` is a printable summary
        /// document.
        #[arg(long, short = 'f', default_value = "json")]
        format: ThreadExportFormat,
        /// Output path.  Defaults to stdout when
        /// omitted.
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
    },
    /// Add a new thread paragraph under the
    /// `Threads` system book.  Seeds the body
    /// with the full commented HJSON template;
    /// the author opens the paragraph to fill in
    /// arc shape, character / place links,
    /// tension, etc.
    Add {
        /// Thread title — becomes the paragraph
        /// slug + lemma.  Free-form; the
        /// underlying slug is auto-derived.
        name: String,
        /// Optional display title for the card
        /// renderer.  Falls back to `name` when
        /// not set.
        #[arg(long)]
        title: Option<String>,
        /// Arc status — `setup` | `develop` |
        /// `payoff` | `resolved` | `abandoned`.
        /// Default `setup`.
        #[arg(long, default_value = "setup")]
        status: String,
        /// Weight — `major` | `subplot` |
        /// `runner` | `bridge`.  Default `major`.
        #[arg(long, default_value = "major")]
        weight: String,
    },
    /// List every thread paragraph under the
    /// `Threads` system book with summary
    /// columns (status / weight / tension /
    /// character + place link counts).
    List {
        /// Filter to threads with this status
        /// (case-insensitive).  Omit to show all.
        #[arg(long)]
        status: Option<String>,
        /// Filter to threads with this weight
        /// (case-insensitive).  Omit to show all.
        #[arg(long)]
        weight: Option<String>,
    },
}

/// 1.2.11+ — sub-subcommands under
/// `inkhaven show-dont-tell …`.
#[derive(Debug, Subcommand)]
pub enum ShowDontTellCommand {
    /// Generate per-language word lists for the
    /// show-don't-tell overlay using the configured
    /// LLM.  Emits an HJSON snippet on stdout — never
    /// touches your `inkhaven.hjson`; review and paste
    /// what you like (same shape as
    /// `doctor --filter-words-snippet`).  The four
    /// fields produced match the
    /// `editor.style_warnings.show_dont_tell.<lang>_*`
    /// stanza: `linking_verbs`, `emotion_adjectives`,
    /// `manner_adverbs`, `cognition_verbs`.  Optional
    /// `--genre` hint biases the vocabulary toward a
    /// register (e.g. "literary fiction", "thriller",
    /// "YA fantasy") — useful when the built-in defaults
    /// sit at the wrong reading level for your corpus.
    Bootstrap {
        /// Target language.  One of: english, russian,
        /// french, german, spanish.  Other values are
        /// passed through verbatim — the LLM will try,
        /// but per-language stop-word + stemmer plumbing
        /// only ships for the five above.
        language: String,
        /// Optional genre / register hint.  Folded into
        /// the prompt so the model picks vocabulary at
        /// the right reading level.
        #[arg(long)]
        genre: Option<String>,
        /// Override the default LLM provider for this
        /// invocation.  Same semantics as `inkhaven ai
        /// --provider` (no short alias here because
        /// `-p` is reserved by the global
        /// `--project`).
        #[arg(long)]
        provider: Option<String>,
        /// 1.2.11+ — apply the LLM-discovered lists
        /// **in place** to `inkhaven.hjson`, merging
        /// with any existing per-language entries
        /// (union, case-insensitive dedup, existing
        /// entries first then new arrivals).  A
        /// versioned backup of the pre-patch file
        /// lands under `<project>/.config-backups/`
        /// before the rewrite, so rolling back is a
        /// single `cp`.  Default (without `--update`)
        /// stays as today: print the HJSON snippet to
        /// stdout and touch nothing.  The two modes
        /// are mutually compatible — `--update` also
        /// prints the merged snippet to stdout so the
        /// user can see what landed.
        #[arg(long)]
        update: bool,
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

/// 1.2.12+ — output formats for
/// `inkhaven export-concordance`.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ConcordanceExportFormat {
    /// CSV — one row per stem: headword, stem, count,
    /// variants (comma-separated), and the slug-path
    /// of each sample (semicolon-separated).  Drops
    /// the KWIC text since spreadsheet tools handle
    /// quotes poorly.  Easiest for pivoting.
    Csv,
    /// JSON — full structured form including KWIC
    /// snippets, line numbers, variants list.
    /// Use for downstream tooling.
    Json,
}

/// 1.2.8+ — output formats for `inkhaven export-timeline`.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum TimelineExportFormat {
    /// Typst-source listing — chronological events per track,
    /// calendar-formatted, ready to `#include` in a longer
    /// document. Compile through `typst compile <file>` to
    /// get PDF / SVG / PNG via typst's own pipeline.
    Typst,
    /// Vector swim-lane render — one row per track, events
    /// positioned by start tick (instant = circle, duration
    /// = bar), date axis at the top. Self-contained SVG;
    /// drop directly into an HTML page or open in any
    /// browser.
    Svg,
    /// Same swim-lane render as SVG, then rasterised through
    /// `resvg` + `tiny-skia` to a PNG. Pixel-density follows
    /// the SVG's intrinsic size (no extra DPI flag in 1.2.8 —
    /// add `--width` to taste in a follow-up).
    Png,
}

impl Cli {
    pub fn run(self) -> Result<()> {
        let project = self
            .project
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        match self.command.unwrap_or(Command::Tui) {
            Command::Init { path, force, template } => {
                init::run(&path, force, &template).map_err(Into::into)
            }
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
            Command::ExportConcordance { format, output, min_count } => {
                export_concordance::run(&project, format, &output, min_count)
                    .map_err(Into::into)
            }
            Command::Stats { book_name } => {
                stats::run(&project, book_name.as_deref()).map_err(Into::into)
            }
            Command::Doctor { voices, tts_test, filter_words_snippet, scan, json, class, autofix, yes } => {
                if filter_words_snippet {
                    doctor::run_filter_words_snippet().map_err(Into::into)
                } else if let Some(text) = tts_test {
                    doctor::run_tts_test(&project, &text).map_err(Into::into)
                } else if voices {
                    doctor::run_voices().map_err(Into::into)
                } else if scan || json || class.is_some() || autofix {
                    // 1.2.15+ Phase D.1 + D.2 —
                    // project scan path.  `--json`,
                    // `--class`, `--autofix` all
                    // imply `--scan`.
                    if yes && !autofix {
                        return Err(crate::error::Error::Config(
                            "doctor: --yes requires --autofix".into()
                        ).into());
                    }
                    let selected = match class.as_deref() {
                        None => None,
                        Some(s) => match doctor_scan::ScanClass::from_slug(s) {
                            Some(c) => Some(c),
                            None => {
                                return Err(crate::error::Error::Config(format!(
                                    "doctor: unknown scan class `{s}` — try one of: {}",
                                    doctor_scan::ScanClass::ALL
                                        .iter()
                                        .map(|c| c.slug())
                                        .collect::<Vec<_>>()
                                        .join(", "),
                                ))
                                .into());
                            }
                        },
                    };
                    let report = doctor_scan::scan_project(&project, selected)?;
                    if json {
                        let rendered = serde_json::to_string_pretty(&report)
                            .map_err(|e| crate::error::Error::Store(format!("doctor JSON: {e}")))?;
                        println!("{rendered}");
                    } else {
                        doctor_scan::print_human(&report);
                    }
                    if autofix && !report.findings.is_empty() {
                        run_autofix(&project, &report, yes)?;
                    }
                    // Exit code 2 when any finding
                    // at Warning or above shipped —
                    // matches conventional doctor /
                    // linter behaviour.  Autofix
                    // doesn't suppress this: the
                    // user may have skipped fixes,
                    // and re-running shows whether
                    // the project is now clean.
                    if report.count_at_or_above(doctor_scan::ScanSeverity::Warning) > 0 {
                        std::process::exit(2);
                    }
                    Ok(())
                } else {
                    doctor::run(&project).map_err(Into::into)
                }
            }
            Command::Build { book_name, compile } => {
                build::run(&project, book_name.as_deref(), compile).map_err(Into::into)
            }
            Command::Epub {
                book_name,
                output,
                title,
                author,
            } => epub::run(
                &project,
                book_name.as_deref(),
                output.as_deref(),
                title.as_deref(),
                author.as_deref(),
            )
            .map_err(Into::into),
            Command::Audiobook {
                book_name,
                output,
                title,
                author,
            } => audiobook::run(
                &project,
                book_name.as_deref(),
                output.as_deref(),
                title.as_deref(),
                author.as_deref(),
            )
            .map_err(Into::into),
            Command::Event(cmd) => event::run(&project, cmd).map_err(Into::into),
            Command::ExportTimeline {
                book_name,
                format,
                output,
                track,
            } => export_timeline::run(
                &project,
                book_name.as_deref(),
                format,
                &output,
                track.as_deref(),
            ).map_err(Into::into),
            Command::Tui => crate::tui::run(Some(&project)).map_err(Into::into),
            Command::Config => crate::config_tui::run(&project).map_err(Into::into),
            Command::PromptsEditor => crate::prompts_tui::run(&project).map_err(Into::into),
            Command::ShowDontTell(cmd) => {
                show_dont_tell::run(&project, cmd).map_err(Into::into)
            }
            Command::Prompts(cmd) => {
                prompts::run(&project, cmd).map_err(Into::into)
            }
            Command::Language(cmd) => {
                language::run(&project, cmd).map_err(Into::into)
            }
            Command::Thread(cmd) => {
                thread::run(&project, cmd).map_err(Into::into)
            }
            Command::Template(TemplateCommand::List) => {
                templates::list_templates();
                Ok(())
            }
            Command::Comments(cmd) => {
                comments::run(&project, cmd).map_err(Into::into)
            }
            Command::Tts(cmd) => {
                tts::run(&project, cmd).map_err(Into::into)
            }
            Command::Continuity(cmd) => {
                continuity::run(&project, cmd).map_err(Into::into)
            }
            Command::Tension(cmd) => {
                tension::run(&project, cmd).map_err(Into::into)
            }
            Command::Facts(cmd) => {
                facts_scan::run(&project, cmd).map_err(Into::into)
            }
            Command::Manuscript {
                book_name,
                output,
                title,
                author,
                contact,
            } => manuscript::run(
                &project,
                book_name.as_deref(),
                output.as_deref(),
                title.as_deref(),
                author.as_deref(),
                contact.as_deref(),
            )
            .map_err(Into::into),
            Command::BenchLoad { query, iterations } => {
                bench_load::run(&project, &query, iterations)
                    .map_err(Into::into)
            }
            Command::BenchReport {
                baseline,
                current,
                threshold,
                markdown,
            } => {
                let current = current
                    .unwrap_or_else(bench_report::default_criterion_dir);
                bench_report::run(
                    baseline.as_deref(),
                    &current,
                    threshold,
                    markdown,
                )
            }
            Command::GenFixture {
                path,
                books,
                chapters,
                paragraphs,
                target_words,
                seed,
                force,
            } => {
                let spec = gen_fixture::FixtureSpec {
                    books,
                    chapters_per_book: chapters,
                    paragraphs_per_chapter: paragraphs,
                    target_words_per_paragraph: target_words,
                    seed,
                    force,
                    ..gen_fixture::FixtureSpec::default()
                };
                let stats = gen_fixture::run(&path, spec)?;
                eprintln!(
                    "gen-fixture: {} books · {} chapters · {} paragraphs at {}",
                    stats.books_created,
                    stats.chapters_created,
                    stats.paragraphs_created,
                    path.display(),
                );
                Ok(())
            }
            Command::Recover { report, yes, keep } => {
                recover::run(&report, yes, keep)
            }
        }
    }
}

/// 1.2.15+ Phase D.2 — interactive autofix walk.
/// Iterates the scan report, prompts the user per
/// finding (or auto-accepts when `yes`), calls
/// `doctor_scan::apply_fix` for each accepted
/// repair, logs the outcome.  Halts on the first
/// fatal error (e.g. Store open failure) but
/// continues past per-finding apply errors.
fn run_autofix(project: &std::path::Path, report: &doctor_scan::ScanReport, yes: bool) -> Result<()> {
    use std::io::{BufRead, Write};
    println!();
    println!("Autofix — applying repairs.");
    let stdin = std::io::stdin();
    let mut applied = 0usize;
    let mut skipped = 0usize;
    let mut errors = 0usize;

    for (i, f) in report.findings.iter().enumerate() {
        println!(
            "\n  [{n}/{total}] {sev} · {class}",
            n = i + 1,
            total = report.findings.len(),
            sev = f.severity.slug(),
            class = f.class.slug(),
        );
        if let Some(p) = &f.path {
            println!("        path: {p}");
        }
        println!("        {}", f.detail);
        let accept = if yes {
            true
        } else {
            print!("        apply repair? [y/N]: ");
            std::io::stdout().flush().ok();
            let mut line = String::new();
            stdin
                .lock()
                .read_line(&mut line)
                .map_err(crate::error::Error::Io)?;
            matches!(line.trim(), "y" | "Y")
        };
        if !accept {
            println!("        skipped.");
            skipped += 1;
            continue;
        }
        let outcome = doctor_scan::apply_fix(project, f);
        doctor_scan::log_fix(project, f, &outcome);
        match outcome {
            Ok(note) => {
                println!("        applied: {note}");
                applied += 1;
            }
            Err(e) => {
                eprintln!("        ERROR: {e:#}");
                errors += 1;
            }
        }
    }
    println!(
        "\nAutofix done: {applied} applied, {skipped} skipped, {errors} error(s).",
    );
    Ok(())
}
