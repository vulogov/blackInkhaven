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
pub mod import_epub;
pub mod import_scrivener;
pub mod import_typst_help;
pub mod init;
pub mod list;
pub mod mv;
pub mod outline;
pub mod paragraph;
pub mod reindex;
pub mod restore;
pub mod search;
pub mod doctor;
pub mod doctor_scan;
pub mod check;
pub mod cost;
pub mod goals;
pub mod event;
pub mod event_critique;
pub mod comments;
pub mod language;
pub mod inner_socrates;
pub mod inner_editor;
pub mod companions;
pub mod output;
pub mod realworld;
pub mod templates;
pub mod thread;
pub mod argue;
pub mod book_index;
pub mod index_locorum;
pub mod index_verborum;
pub mod docs;
pub mod sources;
pub mod terms;
pub mod snippets;
pub mod prose;
pub mod dialogue;
pub mod myth;
pub mod tts;
pub mod gen_fixture;
pub mod bench_load;
pub mod bench_report;
pub mod epub;
pub mod audiobook;
pub mod continuity;
pub mod facts_scan;
pub mod pdf;
pub mod replace;
pub mod tension;
pub mod manuscript;
pub mod docx;
pub mod submissions;
pub mod submission;
pub mod plan;
pub mod editorial;
pub mod drift;
pub mod world;
pub mod utopia;
pub mod character;
pub mod theologian;
pub mod rigor;
pub mod lexicon;
pub mod lang;
pub mod world_prompts;
pub mod prompts;
pub mod show_dont_tell;
pub mod style;
pub mod stats;
pub mod book_rag;
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

/// 1.3.3 — the Prompts-book tier of the CLI prompt resolver: a paragraph
/// in the Prompts system book whose slug or title matches `name` (the
/// `submission-*` / `plan-*` slug).  Gives the CLI generators the same
/// three-tier resolution the TUI has (Prompts book → `prompts.hjson` →
/// built-in).  Returns the body with a leading `= heading` stripped.
pub(crate) fn resolve_book_prompt(
    store: &crate::store::Store,
    h: &crate::store::hierarchy::Hierarchy,
    name: &str,
) -> Option<String> {
    let book = h.iter().find(|n| {
        n.kind == NodeKind::Book
            && n.system_tag.as_deref() == Some(crate::store::SYSTEM_TAG_PROMPTS)
    })?;
    let lower = name.to_lowercase();
    let spaced = lower.replace('-', " ");
    for id in h.collect_subtree(book.id) {
        let Some(node) = h.get(id) else { continue };
        if node.kind != NodeKind::Paragraph {
            continue;
        }
        let s = node.slug.to_lowercase();
        let t = node.title.to_lowercase();
        if s != lower && t != lower && t != spaced {
            continue;
        }
        let bytes = store.get_content(node.id).ok().flatten()?;
        let stripped = strip_typst_heading(&String::from_utf8_lossy(&bytes));
        if !stripped.trim().is_empty() {
            return Some(stripped);
        }
    }
    None
}

/// Drop a single leading `= heading` line (and the blank after it).
fn strip_typst_heading(body: &str) -> String {
    let mut lines = body.lines().peekable();
    if lines.peek().is_some_and(|l| l.trim_start().starts_with("= ")) {
        lines.next();
        if lines.peek().is_some_and(|l| l.trim().is_empty()) {
            lines.next();
        }
    }
    lines.collect::<Vec<_>>().join("\n").trim().to_string()
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

    /// OUTLINE-1 — print the manuscript outline as an indented text tree
    /// (terminal counterpart to the `Ctrl+2` Outline pane).
    Outline {
        /// Only show nodes whose title or slug-path matches (case-insensitive).
        #[arg(long)]
        filter: Option<String>,
    },

    /// OUTLINE-1 — copy or move a paragraph across parents.
    #[command(subcommand)]
    Paragraph(ParagraphCommand),

    /// Run a semantic search across the project.
    Search {
        query: String,
        #[arg(short, long, default_value_t = 10)]
        limit: usize,
    },

    /// 1.4.1+ BOOK_RAG-1 — "Chat with Your Book" retrieval from the
    /// terminal: inspect the passages Book-scope chat would ground on.
    #[command(subcommand)]
    BookRag(BookRagCommand),

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
        /// TDOC-3 — conditional-content profile as `dimension=value`, repeatable
        /// (e.g. `--profile edition=enterprise --profile audience=expert`). A
        /// paragraph tagged `profile:dimension:v` is emitted only when a matching
        /// value is requested; untagged paragraphs are always emitted.
        #[arg(long = "profile")]
        profiles: Vec<String>,
        /// TDOC-4 (HTML) — directory of custom templates (`functional/` and/or
        /// `theme/`); a file present there overrides the bundled default. Overrides
        /// `docs.html.template_dir`. Absolute or project-relative.
        #[arg(long)]
        templates: Option<PathBuf>,
        /// TDOC-4 (HTML) — write the bundled default templates to this directory
        /// (a starting point for customisation) and exit. Use `export html
        /// --eject-templates <dir>`.
        #[arg(long)]
        eject_templates: Option<PathBuf>,
        /// Double-blind submission (1.6.15+) — omit identifying front matter
        /// (authors, affiliations, ORCID, corresponding author, funding) from the
        /// rendered title block, keeping title, abstract, keywords, and the
        /// data/code-availability statements. For a `pdf`/`tex`/`typst` export you
        /// send to a double-blind review. No effect when no `frontmatter` is set.
        #[arg(long)]
        blind: bool,
        /// arXiv / preprint bundle (1.6.16+) — write a self-contained LaTeX
        /// submission to this path: the `.tex`, `sources.bib`, every referenced
        /// figure (copied with paths rewritten), and a `MANIFEST.txt`. A `.zip`
        /// extension writes a single archive; otherwise a directory. Implies the
        /// `tex` format (composes with `--blind`).
        #[arg(long)]
        bundle: Option<PathBuf>,
    },

    /// INDEX-1 — generate a back-of-book index (terms → the chapters they appear in)
    /// from the Glossary's canonical terms and `docs.index.terms`.
    Index {
        /// Limit to one user book (default: all user books).
        #[arg(long)]
        book_name: Option<String>,
        /// Output format: `md` (default), `typst`, or `json`.
        #[arg(long, default_value = "md")]
        format: String,
        /// Write to this file instead of stdout.
        #[arg(short, long)]
        out: Option<PathBuf>,
    },

    /// LOCI — generate an Index Locorum: every `@key[locus]` cited across the
    /// manuscript, grouped by source and sorted by passage. For scripture,
    /// classics, and law (`@bible[John 3:16]`, `@kant[A51/B75]`).
    IndexLocorum {
        /// Limit to one user book (default: all user books).
        #[arg(long)]
        book_name: Option<String>,
        /// Output format: `md` (default), `typst`, or `json`.
        #[arg(long, default_value = "md")]
        format: String,
        /// Write to this file instead of stdout.
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// Exit non-zero if any locus is malformed (fails its source's reference
        /// scheme) — fits a continuous-integration step.
        #[arg(long)]
        strict: bool,
    },

    /// LEXICON — generate an Index Verborum: every scholarly-lexicon term used in
    /// the manuscript, with its original-language form, its distinct senses, and
    /// the chapters that use it. For theology / philosophy / classics.
    IndexVerborum {
        /// Limit to one user book (default: all user books).
        #[arg(long)]
        book_name: Option<String>,
        /// Output format: `md` (default), `typst`, or `json`.
        #[arg(long, default_value = "md")]
        format: String,
        /// Write to this file instead of stdout.
        #[arg(short, long)]
        out: Option<PathBuf>,
    },

    /// ARG-1 — extract each chapter's central claims and their support (an argument
    /// outline), flagging unsupported claims and orphan citations. AI-driven; exits
    /// non-zero when any gap is found.
    Argue {
        /// Limit to one user book (default: all user books).
        #[arg(long)]
        book_name: Option<String>,
        /// LLM provider override.
        #[arg(long)]
        provider: Option<String>,
        /// Machine-readable JSON report.
        #[arg(long)]
        json: bool,
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

    /// Import a `.epub` as a user book — one chapter per spine
    /// document, the converted prose as paragraphs, images
    /// extracted to a sidecar folder. The inverse of `inkhaven epub`.
    ImportEpub {
        /// Path to the `.epub` file.
        epub_path: PathBuf,
        /// Override the title of the created book. None → the EPUB's
        /// `dc:title`.
        #[arg(long)]
        book_name: Option<String>,
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

    /// Run the deterministic style-warning detectors (filter-words,
    /// repeated-phrase, show-don't-tell, anachronism) over the manuscript and
    /// print a report — CLI/CI parity for the in-editor overlay (`Ctrl+V w`).
    Style {
        /// Scope to one user book (default: all non-system books).
        #[arg(long)]
        book_name: Option<String>,
        /// Detector language (default: the project's top-level `language`).
        #[arg(long)]
        language: Option<String>,
        /// Emit the report as JSON (per-kind totals + per-paragraph counts) for CI.
        #[arg(long)]
        json: bool,
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

    /// 1.3.1+ SUBMISSION-1 — export a user book to a
    /// Shunn standard-manuscript-format **Word** document
    /// (`.docx`) — the format agents actually require.
    /// Same layout as `manuscript` (double-spaced 12 pt,
    /// title page, `Surname / KEYWORD / page` header from
    /// page 2, scene breaks as `#`), emitted as OOXML.
    Docx {
        /// User-book name (case-insensitive title or
        /// slug).  Optional when the project has exactly
        /// one user book.
        #[arg(long)]
        book_name: Option<String>,
        /// Output path.  Defaults to
        /// `<project>/<book-slug>-manuscript.docx`.
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
        /// Body typeface: `times` (default) or `courier`.
        #[arg(long)]
        font: Option<String>,
    },

    /// 1.3.1+ SUBMISSION-1 — the submission tracker:
    /// record where the manuscript went, when, and what
    /// came back (the `.inkhaven/submissions.json`
    /// sidecar).  The generated drafts live in the
    /// `Submissions` system book.
    #[command(subcommand)]
    Submissions(SubmissionsCommand),

    /// 1.3.1+ SUBMISSION-1 — build the submission package
    /// (singular): the AI book `digest` now, the query /
    /// synopsis / comp / logline generators next.
    #[command(subcommand)]
    Submission(SubmissionCommand),

    /// 1.3.2+ PLANNING-1 — the Planning Board (story
    /// structure): `plan init` scaffolds a framework's
    /// beats; coverage/pacing + AI analyze follow.
    #[command(subcommand)]
    Plan(PlanCommand),

    /// 1.3.6 EDITORIAL-1 — **The Editorial Pass**: one ranked revision
    /// worklist unifying every detector (the editorial `doctor` classes +
    /// `plan check`'s structural findings + the Facts-scan sidecar). Reads
    /// what's already computed — no live AI.
    Edit {
        /// Machine-readable output for a CI gate.
        #[arg(long)]
        json: bool,
        /// Restrict to these categories (comma-separated), e.g.
        /// `echo,pacing,structure`.
        #[arg(long, value_delimiter = ',')]
        only: Option<Vec<String>>,
        /// Which book's structure findings to include (defaults to the sole
        /// user book).
        #[arg(long)]
        book_name: Option<String>,
        /// Include findings you've deferred in the cockpit (hidden by
        /// default).
        #[arg(long)]
        show_deferred: bool,
        /// Run the AI scans first (Facts / tension / continuity) to refresh
        /// their sidecars, then aggregate — the semantic tier. Needs a
        /// provider; not combinable with `--json`.
        #[arg(long)]
        deep: bool,
        /// LLM provider override for `--deep`.
        #[arg(long)]
        provider: Option<String>,
    },

    /// 1.3.11 WORLD-3 — `inkhaven world`: a consolidated world-consistency
    /// snapshot — established facts + internal/prose contradictions + drift +
    /// continuity coverage + anachronisms, with a health summary.  Reads the
    /// computed sidecars (deterministic, `--json`-gateable); `--deep` refreshes
    /// the AI scans first.  Complements `inkhaven edit`: `edit` is a walkable
    /// worklist of everything; `world` is a consistency snapshot of the world
    /// layer, grouped by entity / fact.
    World {
        /// Machine-readable output for a CI gate.
        #[arg(long)]
        json: bool,
        /// Refresh the AI scans first (facts check / facts scan / drift /
        /// continuity), then aggregate. Needs a provider; not with `--json`.
        #[arg(long)]
        deep: bool,
        /// LLM provider override for `--deep`.
        #[arg(long)]
        provider: Option<String>,
        /// Focus on one entity: its drift conflicts, description trail,
        /// tracked attributes, and whether it's named in the prose.
        #[arg(long)]
        entity: Option<String>,
        /// WORLD-6 — sub-checks under `inkhaven world …`. With no subcommand,
        /// `inkhaven world` prints the consistency snapshot as before.
        #[command(subcommand)]
        sub: Option<WorldCommand>,
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

    /// 1.3.13 BREADTH-1 — `inkhaven lang <subcommand>`.  Multilingual coverage:
    /// `status` prints what works in the project (or `--language`) language —
    /// stemming, detector word-lists, prompts, embeddings.
    #[command(subcommand)]
    Lang(LangCommand),

    /// 1.3.10 WORLD-2 — `inkhaven drift <subcommand>`.  Semantic drift:
    /// descriptions of the same entity (character / place / artefact) that
    /// diverge across the manuscript without a hard factual clash.  `list`
    /// prints the description snippets the retriever found per entity
    /// (deterministic, no AI).
    #[command(subcommand)]
    Drift(DriftCommand),

    /// 1.4.16 CHAR-1 — `inkhaven character <subcommand>`.  Character arc
    /// tracking: a chapter-ordered observable-state chain (LLM), a deterministic
    /// agency score, stall detection, completeness checks against the author's
    /// declared arc, and Planning-Board coverage gaps.  `arc <name>` is a
    /// read-only report; `check`/`plan` gate via exit codes.
    #[command(subcommand)]
    Character(CharacterCommand),

    /// 1.4.18 INNER-THEOLOGIAN-1 — `inkhaven theologian <subcommand>`. The
    /// tradition-neutral moral/theological reader. `scan` runs the deterministic
    /// fast-track ethical-signal detector (exit 1 on any unsuppressed signal);
    /// `session` runs the slow-track LLM over a chapter / the book and prints its
    /// questions; `suppress` mutes a signal. It asks, never judges.
    #[command(subcommand)]
    Theologian(TheologianCommand),

    /// RIGOR — the deterministic reasoning-rigor reader. `scan` flags argument-rigor
    /// signals (false dichotomy, question-begging, straw man, overgeneralization,
    /// non-sequitur) via language-keyed cue markers. Advisory; the argument-side
    /// complement to `theologian`. Zero-AI.
    #[command(subcommand)]
    Rigor(RigorCommand),

    /// LEXICON — the scholarly lexicon: `list` the terms with their
    /// original-language forms and distinct senses. A term with ≥2 senses marked
    /// `watch_equivocation` in the Glossary is policed by the rigor reader.
    #[command(subcommand)]
    Lexicon(LexiconCommand),

    /// 1.4.19 MYTH-1 — `inkhaven myth <subcommand>`. The mythological & symbolic
    /// pattern library over the **declared** Mythology book. `scan` prints the
    /// symbol/motif/archetype heatmap + deterministic findings; `check` runs the
    /// LLM consistency / completeness / role passes (exit 1 on findings);
    /// `profile` prints the declared inventory; `refresh` recomputes the
    /// deterministic caches; `suppress` mutes a finding. Reads declarations only,
    /// never interprets, never edits prose.
    #[command(subcommand)]
    Myth(MythCommand),

    /// 1.3.0 PDF-1 — `inkhaven pdf <subcommand>`.  Page operations
    /// (extract / split / merge / rotate / reorder / delete), metadata,
    /// and outline over an existing PDF.  Writes are atomic and never
    /// silently overwrite the input.
    #[command(subcommand)]
    Pdf(PdfCommand),

    /// 1.2.22 R.5 — `inkhaven replace <pattern> <replacement>`.
    /// Project-wide find & replace: literal + whole-word by default
    /// (`--substring` opts out, `--regex` for a regex), optional
    /// `--ignore-case`.  `--dry-run` previews; `--yes` applies,
    /// snapshotting each touched paragraph first.  System books are
    /// excluded unless `--include-system`; `--book <name>` narrows to
    /// one.
    Replace {
        /// Text to find (or a regex with `--regex`).
        pattern: String,
        /// Replacement (regex captures `$1`… with `--regex`).
        replacement: String,
        /// Treat the pattern as a regular expression.
        #[arg(long)]
        regex: bool,
        /// Match substrings too (default: whole-word only).
        #[arg(long)]
        substring: bool,
        /// Case-insensitive match.
        #[arg(long)]
        ignore_case: bool,
        /// Limit to one book by name (default: all user books).
        #[arg(long)]
        book: Option<String>,
        /// Include system books (Notes / Facts / …) in the scan.
        #[arg(long)]
        include_system: bool,
        /// Preview matches without changing anything.
        #[arg(long)]
        dry_run: bool,
        /// Apply the replacements (required to write).
        #[arg(long)]
        yes: bool,
    },

    /// Launch the TUI editor (default if no subcommand is given).
    Tui,

    /// 1.5.0 RESRCH-1 — launch the Research Assistant (`inkhaven research`): a
    /// separate TUI screen for AI-assisted research that transfers verified
    /// findings into the Facts / Notes corpus with a mandatory confirmation
    /// step. `--thread` opens (or creates) a named, resumable session;
    /// `--list-threads` and `--export-thread` are non-interactive.
    Research {
        /// Open (or create) a named research thread. Without it: the thread
        /// picker (>1 thread) or the `default` thread (0–1).
        #[arg(long)]
        thread: Option<String>,
        /// List all research threads (name, last-active, turn count, cost) and
        /// exit. Honours `--format table|json`.
        #[arg(long)]
        list_threads: bool,
        /// Export a named thread's history and exit. Honours `--format md|json`
        /// and `--out <path>` (default stdout).
        #[arg(long, value_name = "NAME")]
        export_thread: Option<String>,
        /// Output format for `--list-threads` (table|json) / `--export-thread`
        /// (md|json).
        #[arg(long)]
        format: Option<String>,
        /// Destination file for `--export-thread` (default: stdout).
        #[arg(long)]
        out: Option<String>,
        /// 1.5.1 RESRCH-2 — ingest a document (md / txt / pdf) as a research
        /// source and exit (non-interactive). A `.bib` file imports its
        /// citations into the Sources book.
        #[arg(long, value_name = "PATH")]
        import: Option<String>,
        /// 1.5.6 RESRCH-3 (R3-D) — register a folder for re-import-on-change
        /// (import it now, and re-import at each launch when its files change).
        #[arg(long, value_name = "FOLDER")]
        sync: Option<String>,
        /// 1.5.6 RESRCH-2 (R2-F) — research a question list headlessly (one
        /// question per line; `#` comments ignored) and write a Markdown report
        /// (`--out`, default stdout).
        #[arg(long, value_name = "FILE")]
        batch: Option<String>,
        /// R2-F — with `--batch`, insert facts that clear `--confidence`
        /// (otherwise the report lists candidates only).
        #[arg(long)]
        auto_confirm: bool,
        /// R2-F — auto-insert confidence threshold, 0..1 (default 0.7).
        #[arg(long, value_name = "0..1")]
        confidence: Option<f64>,
        /// 1.5.8 RESRCH-5 (R5-D) — emit the Sources Research chapter as BibTeX
        /// (`--out` file, else stdout) and exit.
        #[arg(long)]
        bibliography: bool,
        /// 1.5.9 RESRCH-GUTENBERG — ingest a public-domain Project Gutenberg book
        /// (search query or bare PG id; accepts a leading `--chapter N`) and exit.
        #[arg(long, value_name = "QUERY")]
        gutenberg: Option<String>,
        /// 1.6.16 RESRCH-ARCHIVE — ingest a public-domain Internet Archive text
        /// (search query) and exit.
        #[arg(long, value_name = "QUERY")]
        archive: Option<String>,
        /// 1.6.16 RESRCH-WIKISOURCE — ingest a public-domain Wikisource page
        /// (search query, in the book's language) and exit.
        #[arg(long, value_name = "QUERY")]
        wikisource: Option<String>,
        /// 1.6.18 RESRCH-SCRIPTURE — ingest a public-domain Bible passage
        /// (`<book> <chapter>`, by project language) and exit.
        #[arg(long, value_name = "REF")]
        bible: Option<String>,
        /// 1.6.18 RESRCH-SCRIPTURE — ingest a public-domain Qur'an surah
        /// (`<surah number or name>`, by project language) and exit.
        #[arg(long, value_name = "SURAH")]
        quran: Option<String>,
        /// 1.6.18 RESRCH-SCRIPTURE — ingest a public-domain Book of Mormon passage
        /// (`<book> <chapter>`) and exit.
        #[arg(long, value_name = "REF")]
        bookofmormon: Option<String>,
        /// 1.6.16 SCHOLAR P1 — scan the Facts book for source-attributed
        /// contradictions (cross-source vs within-source) and exit.
        #[arg(long)]
        contradict: bool,
        /// 1.6.18 SCHOLAR — scan the Facts book for converging (triangulated)
        /// evidence across independent sources and exit.
        #[arg(long)]
        converge: bool,
        /// 1.6.17 SCHOLAR — the Dialectician's Socratic questions over the Facts
        /// corpus (nearest facts for the given topic) and exit.
        #[arg(long, value_name = "TOPIC")]
        socrates: Option<String>,
        /// 1.6.18 SCHOLAR P3 — print the persisted, topic-clustered report of the
        /// accumulated contradiction / convergence / relation findings and exit.
        #[arg(long)]
        report: bool,
    },

    /// 1.7 LING-1 — launch the Linguistic companion (`inkhaven linguistic`): a
    /// full-screen TUI for developing, verifying, analysing and researching the
    /// project's constructed languages, over the `Language` system book. The
    /// non-interactive `inkhaven language …` family hosts the same operations
    /// as one-shot commands.
    Linguistic {
        /// Open with this language selected (case-insensitive). Without it, the
        /// tree opens at the Languages book root.
        #[arg(long, value_name = "NAME")]
        language: Option<String>,
        /// Open (or create) a named chat session. Defaults to `default`.
        #[arg(long, value_name = "NAME")]
        session: Option<String>,
    },

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

    /// 1.4.5+ SOURCES-1 — `inkhaven sources <subcommand>`.
    /// Bibliography & citation surface: validate `@key`
    /// references, list defined entries, import a `.bib`
    /// file into the `Sources` system book.
    #[command(subcommand)]
    Sources(SourcesCommand),

    /// Technical-documentation tooling (TDOC-1): verify the manuscript's
    /// `verify`-marked code blocks against configured runners.
    #[command(subcommand)]
    Docs(DocsCommand),

    /// 1.4.8+ TERMS-1 — `inkhaven terms <subcommand>`.
    /// Terminology governance: scan prose for banned synonyms
    /// of canonical terms defined in the `Glossary` system book.
    #[command(subcommand)]
    Terms(TermsCommand),

    /// 1.4.9+ REUSE-1 — `inkhaven snippets <subcommand>`.
    /// Reusable content blocks: list snippets + reference counts,
    /// validate `#include` references against the `Snippets` book.
    #[command(subcommand)]
    Snippets(SnippetsCommand),

    /// 1.4.12+ NARR-1 — `inkhaven prose <subcommand>`. Narrative-voice
    /// profiling: deterministic, zero-AI voice metrics per chapter (rhythm,
    /// lexical diversity, epistemic hedging, interiority, sensory balance,
    /// passive ratio) stored in `.inkhaven/prose.duckdb`.
    #[command(subcommand)]
    Prose(ProseCommand),

    /// 1.4.14 DIALOG-1 — dialogue quality & attribution: detect speech spans,
    /// flag zero-attribution / said-bookism / talking-head findings, and build
    /// per-character dialogue fingerprints. Stored in `.inkhaven/dialogue.duckdb`.
    #[command(subcommand)]
    Dialogue(DialogueCommand),

    /// 1.3.24 PANE-1 — the Output message channel (CLI surface; the pane is TUI).
    #[command(subcommand)]
    Output(OutputCommand),

    /// WORLD-4 — the world-simulation compiler. P0 ships the astronomy layer:
    /// scaffold / validate / compile a `world.hjson`. See
    /// `Documentation/PROPOSALS/WORLD-4_PLAN.md`.
    #[command(subcommand)]
    Realworld(RealworldCommand),

    /// INNER_SOCRATES-1 — examined authorship: run the Socratic interrogator over
    /// prose (questions, never corrections). See
    /// `Documentation/PROPOSALS/INNER_SOCRATES-1_PLAN.md`.
    #[command(subcommand)]
    InnerSocrates(InnerSocratesCommand),

    /// 1.4.2+ INNER_EDITOR-1 — the Inner Editor literary/stylistic companion
    /// (the second Inner-family member). Engage on a paragraph, inspect
    /// findings, config, and usage. See
    /// `Documentation/PROPOSALS/INNER_EDITOR-1_PLAN.md`.
    #[command(subcommand)]
    InnerEditor(InnerEditorCommand),

    /// 1.4.4+ COMPANIONS-1 — the examined-authorship cockpit: open findings
    /// across the Inner family, the shared intent ledger + pending promotions,
    /// and today's LLM cost per companion, in one view.
    Companions,

    /// Road to 1.4.0 — the unified review pass: run every applicable fast,
    /// deterministic checker (fact-check + Inner Socrates + timeline critique)
    /// over a scope and print a consolidated summary.
    Check {
        /// Check one paragraph by id (reads its content from the store).
        #[arg(long)]
        paragraph: Option<String>,
        /// Restrict to a single book (slug or title). Default: the whole project.
        #[arg(long)]
        book_name: Option<String>,
        /// Skip the world fact-checker.
        #[arg(long = "no-fact")]
        no_fact: bool,
        /// Skip the Inner Socrates fast track.
        #[arg(long = "no-socrates")]
        no_socrates: bool,
        /// Skip the timeline critique.
        #[arg(long = "no-timeline")]
        no_timeline: bool,
    },
    /// Road to 1.4.0 — the unified AI cost dashboard: today's LLM call tallies for
    /// each capped subsystem (world slow track, Inner Socrates slow track) vs their
    /// daily caps.
    Cost,
    /// Road to 1.4.0 — the writing-goals report: project + per-book word totals,
    /// today vs the daily goal, current streak (with grace), per-book pace +
    /// deadline, weekly status promotions, and active time. The terminal
    /// counterpart to the `Ctrl+V g` progress modal. Read-only.
    Goals,
    /// WORLD-4 — fact-check prose against the simulated world (fast track):
    /// flag implausible world-assertions (travel time, …), respecting the
    /// `magic:` ledger's declared exceptions.
    FactCheck {
        /// Check this literal text.
        #[arg(long)]
        text: Option<String>,
        /// Check a paragraph by id (reads its content from the store).
        #[arg(long)]
        paragraph: Option<String>,
        /// Also run the slow track — an LLM pass for subtle / implicit
        /// contradictions the patterns miss (needs an LLM provider; cost-capped).
        #[arg(long)]
        slow: bool,
        /// Slow-track per-call soft cap (estimated tokens). The call is skipped
        /// with a notice if the estimate exceeds this; `--force` overrides it.
        #[arg(long, default_value_t = 6000)]
        max_cost: usize,
        /// Run the slow track even if the cost estimate exceeds `--max-cost`.
        #[arg(long)]
        force: bool,
        /// WORLD-5 — timeline-aware checks: `auto` (on if the project has a
        /// timeline), `on`, or `off`.
        #[arg(long, value_parser = ["auto", "on", "off"], default_value = "auto")]
        timeline_aware: String,
        /// WORLD-5 — run *only* the timeline-aware checks (skip the world checks).
        #[arg(long)]
        timeline_only: bool,
    },
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

/// OUTLINE-1 — sub-subcommands under `inkhaven paragraph …`. `src` / `dest`
/// are slash-separated slug paths (as printed by `inkhaven outline`).
#[derive(Debug, Subcommand)]
pub enum ParagraphCommand {
    /// Duplicate a paragraph under a destination node (fresh uuid; the
    /// timeline event, if any, is not copied).
    Copy {
        /// Slug path of the paragraph to copy.
        src: String,
        /// Slug path of the destination node (a branch to nest into, or a
        /// paragraph to land alongside).
        dest: String,
    },
    /// Relocate a paragraph under a destination node.
    Move {
        /// Slug path of the paragraph to move.
        src: String,
        /// Slug path of the destination node.
        dest: String,
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
    /// TIMELINE-2-INTEGRATION — run the timeline-internal critique (orphan +
    /// fuzzy-precision overlap). Travel-time / date / pacing concerns now live in
    /// `realworld fact-check` and `inner-socrates check`.
    Critique {
        /// Restrict to one track (case-insensitive exact match).
        #[arg(long)]
        track: Option<String>,
        /// Restrict to a single book (slug or title). Default: the whole project.
        #[arg(long)]
        book_name: Option<String>,
        /// Use the deprecated original critique (the five-item AI audit). Prints a
        /// deprecation notice; slated for removal in a later release.
        #[arg(long)]
        legacy: bool,
        /// Show how the new infrastructure's coverage maps to the legacy critique.
        #[arg(long = "migration-check")]
        migration_check: bool,
        /// Show which legacy categories moved where (and to which command).
        #[arg(long)]
        diff: bool,
        /// Skip LLM elaboration of findings (pattern-only output).
        #[arg(long = "no-elaborate")]
        no_elaborate: bool,
        /// Bypass the elaboration soft confirm cap.
        #[arg(long)]
        force: bool,
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
    /// 1.3.8 — internal-consistency check: flag fact pairs that
    /// contradict each other *within* the Facts book (distinct from
    /// `scan`, which checks prose against facts). Writes
    /// `<project>/.inkhaven/facts_check.json`; surfaced in `inkhaven edit`.
    Check {
        #[arg(long)]
        provider: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// 1.3.8 — copy series-shared facts (a directory of plain-text fact
    /// files) into this project's Facts book. A hard snapshot of the shared
    /// canon, after which `scan` / fact-check see them as local facts.
    Import {
        /// The shared-facts directory (defaults to `facts.shared_path`).
        #[arg(long)]
        from: Option<String>,
        /// Actually write (otherwise prints what it would add).
        #[arg(long)]
        yes: bool,
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

/// 1.3.10 WORLD-2 — `inkhaven drift …`: semantic drift across the manuscript.
/// 1.3.13 BREADTH-1 — `inkhaven lang …`: multilingual coverage.
#[derive(clap::Subcommand, Debug)]
pub enum LangCommand {
    /// Print the coverage matrix for the project (or `--language`) language.
    Status {
        /// Report for this language instead of the project's `language`.
        #[arg(long)]
        language: Option<String>,
    },
    /// Generate the full per-language detector vocabulary (filter words,
    /// show-don't-tell, stop-words, drift pronouns) for any language via one
    /// LLM pass. Prints a paste-able HJSON snippet; `--yes` also patches
    /// `inkhaven.hjson` in place (versioned backup + atomic).
    Bootstrap {
        /// The language to bootstrap (e.g. `italian`).
        language: String,
        /// LLM provider override (defaults to `llm.default`).
        #[arg(long)]
        provider: Option<String>,
        /// Patch `inkhaven.hjson` in place (otherwise just prints the snippet).
        #[arg(long)]
        yes: bool,
    },
}

#[derive(clap::Subcommand, Debug)]
pub enum DriftCommand {
    /// Print the description snippets retrieved for each entity (Characters /
    /// Places / Artefacts): which paragraphs describe it, in chapter order.
    /// Deterministic — reuses the existing vector index, runs no AI.
    List {
        /// Emit the descriptions as JSON.
        #[arg(long)]
        json: bool,
        /// Scope to entities whose name contains this (case-insensitive).
        #[arg(long)]
        entity: Option<String>,
    },
    /// Run the AI drift pass: for each entity, judge whether its descriptions
    /// across the manuscript contradict each other, and write the
    /// contradictions to `<project>/.inkhaven/drift.json`. Surfaced in
    /// `inkhaven edit`.
    Scan {
        /// LLM provider override (defaults to `llm.default`).
        #[arg(long)]
        provider: Option<String>,
        /// Emit the report as JSON (for CI gates).
        #[arg(long)]
        json: bool,
    },
}

/// 1.3.0 PDF-1 — `inkhaven pdf …` page operations + metadata + outline
/// over an existing PDF (typically inkhaven's own `Ctrl+B B` output).
/// 1.3.2+ PLANNING-1 — `inkhaven plan …`: the Planning Board (story
/// structure).  `init` scaffolds a framework's beats in P0; `check`
/// (coverage + pacing) and `analyze` (AI) arrive in later phases.
#[derive(Debug, Subcommand)]
pub enum PlanCommand {
    /// Scaffold a story-structure framework's beats into the `Planning`
    /// system book.
    Init {
        /// `three_act` (default) | `save_the_cat` | `story_circle` |
        /// `hero_journey` | `seven_point`.
        #[arg(long)]
        framework: Option<String>,
    },
    /// Diagnose structure against a book: beat coverage (gaps), per-beat
    /// position drift, and per-act word-share pacing.  Deterministic — no
    /// AI.  Map a beat to a chapter by setting `mapped_chapter` in its
    /// Planning-book paragraph.
    Check {
        #[arg(long)]
        book_name: Option<String>,
        #[arg(long)]
        json: bool,
        /// Drift / pacing tolerance in percent (default 10).
        #[arg(long)]
        drift: Option<u32>,
    },
    /// AI structure analysis: over the book digest + the framework, map
    /// the beats to chapters and name the structural problems (the sag,
    /// missing beats).  Builds the digest if needed.
    Analyze {
        #[arg(long)]
        book_name: Option<String>,
        #[arg(long)]
        provider: Option<String>,
    },
    /// Map a beat to a chapter (set its `mapped_chapter`), optionally
    /// linking threads + setting status.  `<beat>` is the beat name or
    /// slug; `<chapter>` is a chapter slug (see `plan check`).
    Map {
        beat: String,
        chapter: String,
        /// Comma-separated thread slugs to link.
        #[arg(long, value_delimiter = ',')]
        threads: Option<Vec<String>>,
        /// `planned` | `drafted` | `done`.
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        book_name: Option<String>,
    },
    /// Clear a beat's `mapped_chapter` (turn it back into an open gap).
    Unmap { beat: String },
    /// Plan-first.  `--premise "<logline>"` expands each beat into an
    /// intention (AI); `--chapters` materializes a chapter shell per beat
    /// under the manuscript book (opt-in, refuses to clobber an existing
    /// book) and back-links each beat.  Pass either or both.  Run `plan
    /// init` first.
    Scaffold {
        /// The premise / logline to plan around (fills beat intentions).
        #[arg(long)]
        premise: Option<String>,
        /// Create a chapter shell per beat under the manuscript book.
        #[arg(long)]
        chapters: bool,
        #[arg(long)]
        book_name: Option<String>,
        #[arg(long)]
        framework: Option<String>,
        #[arg(long)]
        provider: Option<String>,
    },
    /// Scene cards (1.3.4) — a finer grain than beats: each scene's
    /// goal / conflict / disaster, with a weak-scene (no-turn) check.
    Scene {
        #[command(subcommand)]
        cmd: PlanSceneCommand,
    },
    /// Sequel cards (1.3.5) — the reactive counterpart to the proactive
    /// scene: reaction / dilemma / decision. A sequel that reaches a
    /// dilemma but never decides stalls the story.
    Sequel {
        #[command(subcommand)]
        cmd: PlanSequelCommand,
    },
    /// Tension second opinion (1.3.5) — an AI intensity reading to compare
    /// against the deterministic curve.
    Tension {
        #[command(subcommand)]
        cmd: PlanTensionCommand,
    },
}

/// `inkhaven plan tension …` — the AI intensity "second opinion".
#[derive(Debug, Subcommand)]
pub enum PlanTensionCommand {
    /// Rate every chapter's dramatic intensity (0–100) with the LLM and
    /// cache it. The `Ctrl+V Shift+K` outline + `plan check` then show it as
    /// a third line beside expected (framework) and actual (obligations).
    Rate {
        #[arg(long)]
        book_name: Option<String>,
        #[arg(long)]
        provider: Option<String>,
        /// Re-rate every chapter even if the cache is still current.
        #[arg(long)]
        refresh: bool,
    },
}

/// `inkhaven plan sequel …` — manage the reactive (reaction/dilemma/
/// decision) cards. They share the Planning book's `Scenes` chapter with
/// scene cards, tagged by kind.
#[derive(Debug, Subcommand)]
pub enum PlanSequelCommand {
    /// Add a sequel card under a chapter.
    Add {
        title: String,
        #[arg(long)]
        chapter: String,
        /// The POV character's emotional response to the prior disaster.
        #[arg(long)]
        reaction: Option<String>,
        /// The bad-options bind it forces.
        #[arg(long)]
        dilemma: Option<String>,
        /// The choice that launches the next goal.
        #[arg(long)]
        decision: Option<String>,
    },
    /// List sequel cards (grouped by chapter) with the no-decision flag.
    List,
    /// Update fields on an existing sequel (matched by title).
    Set {
        title: String,
        #[arg(long)]
        chapter: Option<String>,
        #[arg(long)]
        reaction: Option<String>,
        #[arg(long)]
        dilemma: Option<String>,
        #[arg(long)]
        decision: Option<String>,
        #[arg(long)]
        status: Option<String>,
    },
    /// Remove a sequel card (matched by title).
    Remove { title: String },
}

/// `inkhaven plan scene …` — manage the Planning book's scene cards.
#[derive(Debug, Subcommand)]
pub enum PlanSceneCommand {
    /// Add a scene card under a chapter.
    Add {
        /// Scene title (its identifier within the Planning book).
        title: String,
        /// Chapter slug the scene belongs to.
        #[arg(long)]
        chapter: String,
        /// What the POV character wants.
        #[arg(long)]
        goal: Option<String>,
        /// What stands in the way.
        #[arg(long)]
        conflict: Option<String>,
        /// The turn — how the scene ends worse / changed.
        #[arg(long)]
        disaster: Option<String>,
    },
    /// List scene cards (grouped by chapter) with weak-scene flags.
    List,
    /// Update fields on an existing scene (matched by title).
    Set {
        title: String,
        #[arg(long)]
        chapter: Option<String>,
        #[arg(long)]
        goal: Option<String>,
        #[arg(long)]
        conflict: Option<String>,
        #[arg(long)]
        disaster: Option<String>,
        #[arg(long)]
        status: Option<String>,
    },
    /// Remove a scene card (matched by title).
    Remove { title: String },
    /// AI-scaffold a scene card from a chapter's prose (goal / conflict /
    /// disaster). Pass `--chapter <slug>` for one, or `--all` for every
    /// chapter without a card yet.
    Scaffold {
        #[arg(long)]
        chapter: Option<String>,
        /// Scaffold every chapter that has no card yet.
        #[arg(long)]
        all: bool,
        #[arg(long)]
        book_name: Option<String>,
        #[arg(long)]
        provider: Option<String>,
    },
}

/// 1.3.1+ SUBMISSION-1 P3 — `inkhaven submission …` (singular): the AI
/// package-build side.  `digest` lands in P3.1; the query / synopsis /
/// comps / logline generators arrive in P3.2.
#[derive(Debug, Subcommand)]
pub enum SubmissionCommand {
    /// Build (or show the cached) book digest — the compact whole-book
    /// context the package generators consume: title / author / length /
    /// chapter one-line summaries + the Characters and Threads books.
    /// Cached in `.inkhaven/digest-<slug>.json`, rebuilt when the
    /// manuscript's structure changes or with `--refresh`.
    Digest {
        #[arg(long)]
        book_name: Option<String>,
        /// Rebuild even if the cached digest is still valid.
        #[arg(long)]
        refresh: bool,
        /// Override the LLM provider for the summary pass.
        #[arg(long)]
        provider: Option<String>,
    },
    /// Draft a query letter from the digest into the `Submissions` book.
    Query {
        #[arg(long)]
        book_name: Option<String>,
        #[arg(long)]
        provider: Option<String>,
    },
    /// Draft a synopsis (one page; `--long` for 2–3 pages). Spoils the
    /// ending by design.
    Synopsis {
        #[arg(long)]
        book_name: Option<String>,
        #[arg(long)]
        long: bool,
        #[arg(long)]
        provider: Option<String>,
    },
    /// Suggest comp titles (general-knowledge suggestions, not market
    /// data).
    Comps {
        #[arg(long)]
        book_name: Option<String>,
        #[arg(long)]
        provider: Option<String>,
    },
    /// Draft a logline + elevator pitch.
    Logline {
        #[arg(long)]
        book_name: Option<String>,
        #[arg(long)]
        provider: Option<String>,
    },
}

/// 1.4.9+ REUSE-1 — `inkhaven snippets …` sub-subcommands.
#[derive(Debug, Subcommand)]
pub enum SnippetsCommand {
    /// List the snippets defined in the Snippets book + how many times each is
    /// referenced across the project.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Validate every `#include "…/snippets/<slug>.typ"` against the defined
    /// snippets. Missing references → error (exit 1); orphaned snippets → warning.
    Check {
        /// Limit the scan to one user book (default: all).
        #[arg(long)]
        book: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

/// 1.4.12+ NARR-1 — `inkhaven prose …` sub-subcommands.
#[derive(Debug, Subcommand)]
pub enum ProseCommand {
    /// (Re)compute and print a book's narrative-voice profile.
    Profile {
        /// User book (default: the single user book).
        #[arg(long)]
        book: Option<String>,
        /// Include Tier-2 metrics (sensory balance + active/passive ratio).
        #[arg(long)]
        deep: bool,
        #[arg(long)]
        json: bool,
        /// Override the prose language (en/ru/de/fr/es).
        #[arg(long)]
        language: Option<String>,
    },
    /// Recompute stale profiles for a book (summary only).
    Refresh {
        #[arg(long)]
        book: Option<String>,
        #[arg(long)]
        deep: bool,
        #[arg(long)]
        language: Option<String>,
    },
    /// Chapter-to-chapter (or vs a `--reference` project) voice drift.
    Drift {
        #[arg(long)]
        book: Option<String>,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        language: Option<String>,
        /// `baseline` (every chapter vs chapter 1) or `rolling` (adjacent).
        #[arg(long, default_value = "baseline")]
        mode: String,
        /// Another project's root to compare against (reads its prose.duckdb).
        #[arg(long)]
        reference: Option<std::path::PathBuf>,
    },
    /// Print the per-metric interpretation guide.
    Suggest {
        #[arg(long)]
        book: Option<String>,
        #[arg(long)]
        language: Option<String>,
    },
}

/// DIALOG-1 — sub-subcommands under `inkhaven dialogue …`.
#[derive(Debug, Subcommand)]
pub enum DialogueCommand {
    /// Detect dialogue + print findings. Exits non-zero if any zero-attribution
    /// span is found (a CI pre-submission gate).
    Scan {
        #[arg(long)]
        book: Option<String>,
        /// Filter: `zero-attribution` | `said-bookism` | `talking-heads` | `all`.
        #[arg(long)]
        findings: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Print per-character dialogue fingerprints.
    Profile {
        #[arg(long)]
        book: Option<String>,
        /// A single character (default: all).
        #[arg(long)]
        character: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Force full recomputation, bypassing the content-hash cache.
    Refresh {
        #[arg(long)]
        book: Option<String>,
        /// Only this chapter ordinal (1-based).
        #[arg(long)]
        chapter: Option<u32>,
    },
    /// Deterministic, template-driven chapter dialogue summary (no LLM).
    Suggest {
        #[arg(long)]
        book: Option<String>,
        #[arg(long)]
        chapter: Option<u32>,
    },
}

/// 1.4.8+ TERMS-1 — `inkhaven terms …` sub-subcommands.
#[derive(Debug, Subcommand)]
pub enum TermsCommand {
    /// Scan prose for banned synonyms of Glossary canonical terms and report
    /// every occurrence with its location. Exits non-zero when any are found —
    /// drop it into a pre-build / CI check.
    Check {
        /// Limit the scan to one user book (default: all user books).
        #[arg(long)]
        book: Option<String>,
        /// Machine-readable JSON report.
        #[arg(long)]
        json: bool,
    },
    /// LLM-assisted canonicalisation: cluster terms appearing in multiple
    /// surface forms in a book and propose Glossary entries for the genuine
    /// terminology drift. Needs an LLM provider; cost-capped.
    Suggest {
        /// The book to analyse (defaults to the sole user book).
        #[arg(long)]
        book: Option<String>,
        /// Override the LLM provider for this call.
        #[arg(long)]
        provider: Option<String>,
        /// Per-call soft cap (estimated tokens); skipped with a notice unless
        /// `--force`.
        #[arg(long, default_value_t = 8000)]
        max_cost: usize,
        /// Run past the soft cap.
        #[arg(long)]
        force: bool,
        /// Create the proposed entries as draft paragraphs in the Glossary book.
        #[arg(long)]
        auto_create: bool,
    },
}

/// 1.4.5+ SOURCES-1 — `inkhaven sources …` sub-subcommands.
#[derive(Debug, Subcommand)]
pub enum SourcesCommand {
    /// Validate every `@key` cited in prose against the entries defined in the
    /// Sources book (honouring `sources.all` scope). Exits non-zero when any
    /// key is undefined — drop it into a pre-build check.
    Check {
        /// Limit the scan to one user book (default: all user books).
        #[arg(long)]
        book_name: Option<String>,
        /// Machine-readable JSON report.
        #[arg(long)]
        json: bool,
    },
    /// NF-CITE — the Sourcing pass: flag sentences that make a checkable factual
    /// claim (a statistic, a date, a quotation, an attributed finding) but carry no
    /// `@key` citation. A paragraph tagged `no-cite` is skipped. Exits non-zero when
    /// any uncited claim is found — a pre-publish / CI gate for nonfiction.
    Coverage {
        /// Limit the scan to one user book (default: all user books).
        #[arg(long)]
        book_name: Option<String>,
        /// Machine-readable JSON report.
        #[arg(long)]
        json: bool,
        /// AI track: also catch subtler uncited claims and check each against your
        /// Facts book for support (needs a configured model; costs tokens).
        #[arg(long)]
        ai: bool,
        /// LLM provider override for the `--ai` track.
        #[arg(long)]
        provider: Option<String>,
    },
    /// List the citation entries defined in the Sources book.
    List {
        /// Limit to the chapter named after this book.
        #[arg(long)]
        book_name: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Import a `.bib` file: each entry becomes an HJSON paragraph under the
    /// Sources chapter for the target book. Existing keys are skipped.
    Import {
        /// Path to the BibTeX `.bib` file.
        file: std::path::PathBuf,
        /// Target user book (defaults to the sole user book).
        #[arg(long)]
        book_name: Option<String>,
    },
    /// Export the Sources book's entries to `bibtex` or `csl-json` (for Zotero /
    /// other citation managers). Writes to `--out`, else stdout.
    Export {
        /// `bibtex` (default) or `csl-json`.
        #[arg(long, default_value = "bibtex")]
        format: String,
        /// Limit to the chapter named after this book.
        #[arg(long)]
        book_name: Option<String>,
        /// Write to this file instead of stdout.
        #[arg(long)]
        out: Option<std::path::PathBuf>,
    },
}

/// TDOC-1 — `inkhaven docs …` subcommands.
#[derive(Debug, Subcommand)]
pub enum DocsCommand {
    /// Run every `verify`-marked code block through its configured runner. Exits
    /// non-zero when any block fails, so it fits a pre-release / CI check. Requires
    /// `docs.verify.enabled: true` and, to actually execute, `--yes` (or use
    /// `--dry-run` to preview the commands).
    Verify {
        /// Limit to one user book (default: all user books).
        #[arg(long)]
        book_name: Option<String>,
        /// Limit to one paragraph by its slug-path.
        #[arg(long)]
        paragraph: Option<String>,
        /// List each block that would run and its resolved command; execute nothing.
        #[arg(long)]
        dry_run: bool,
        /// Confirm execution of the configured runner commands (required to run).
        #[arg(long)]
        yes: bool,
    },
    /// Check link integrity across the manuscript: internal cross-references that
    /// no longer resolve (always) and, with `--external`, `http(s)` URLs in prose
    /// for link-rot. Exits non-zero when any link is broken.
    Links {
        /// Limit to one user book (default: all user books).
        #[arg(long)]
        book_name: Option<String>,
        /// Also check external `http(s)` URLs for link-rot (network access).
        #[arg(long)]
        external: bool,
    },
    /// TDOC-5 — a review/currency dashboard: per-chapter status breakdown, the
    /// paragraphs still below a readiness floor, and — with `--since <ref>` — the
    /// paragraphs whose file changed since a git tag/commit (so they want a
    /// re-read). Exits non-zero when any paragraph is below the floor.
    Review {
        /// Limit to one user book (default: all user books).
        #[arg(long)]
        book_name: Option<String>,
        /// Readiness floor to measure against: `napkin` | `first` | `second` |
        /// `third` | `final` | `ready`. Default `ready`.
        #[arg(long, default_value = "ready")]
        floor: String,
        /// Flag paragraphs whose `.typ` file changed since this git ref (a tag or
        /// commit) — the "re-review since the last release" view.
        #[arg(long)]
        since: Option<String>,
    },
}

/// 1.4.1+ BOOK_RAG-1 — `inkhaven book-rag …` sub-subcommands. The terminal
/// counterpart to the AI pane's Book scope ("Chat with Your Book").
#[derive(Debug, Subcommand)]
pub enum BookRagCommand {
    /// Inspect what Book-scope retrieval would feed the model for a query:
    /// the semantically relevant paragraphs (expanded + token-budgeted),
    /// exactly as the TUI grounds its Book chat. No LLM call.
    Retrieve {
        /// The question to retrieve grounding passages for.
        query: String,
        /// User-book name (title or slug). Optional when the project has
        /// exactly one user book.
        #[arg(long)]
        book_name: Option<String>,
        /// Override `book_rag.top_k` for this run only (config untouched).
        #[arg(long)]
        top_k: Option<usize>,
        /// Print the composed grounding context block the model receives,
        /// rather than the human-readable passage listing.
        #[arg(long)]
        context: bool,
    },
}

/// 1.3.1+ SUBMISSION-1 — `inkhaven submissions …` sub-subcommands.
#[derive(Debug, Subcommand)]
pub enum SubmissionsCommand {
    /// Record a new submission.
    Add {
        /// Agency / publication / contest.
        #[arg(long)]
        market: String,
        #[arg(long)]
        agent: Option<String>,
        /// Paragraph slug of the draft used (in the `Submissions` book).
        #[arg(long)]
        draft: Option<String>,
        /// `drafting` (default) | `sent` | `rejected` | `offer` |
        /// `withdrawn`.
        #[arg(long)]
        status: Option<String>,
        /// ISO `YYYY-MM-DD` (defaults to today when `--status sent`).
        #[arg(long)]
        date_sent: Option<String>,
        /// Next-action date (ISO `YYYY-MM-DD`).
        #[arg(long)]
        next: Option<String>,
        #[arg(long)]
        notes: Option<String>,
    },
    /// List the log (optionally `--json`, filtered by `--status` or just
    /// the still-`--open` ones).
    List {
        #[arg(long)]
        json: bool,
        #[arg(long)]
        status: Option<String>,
        /// Only submissions still awaiting a response.
        #[arg(long)]
        open: bool,
    },
    /// Move a record to a new status (stamps a response date for
    /// `rejected` / `offer`).
    Status {
        id: String,
        status: String,
        #[arg(long)]
        response_date: Option<String>,
    },
    /// Append a timestamped note to a submission's event trail
    /// (e.g. "got a call", "requested edits", "moving to round two").
    AddNote {
        id: String,
        /// The note text.
        text: String,
    },
    /// Remove a record.
    Remove { id: String },
}

/// Mutating ops write a `<stem>-<op>.pdf` sibling unless `--out` is
/// given; writes are atomic.  Imposition / cover / barcode / preflight
/// arrive in later phases.
#[derive(Debug, Subcommand)]
pub enum PdfCommand {
    /// Print page count, page-1 size, source, title/author, outline size.
    Info {
        input: std::path::PathBuf,
    },
    /// Impose into print-ready signatures using a named `imposition:`
    /// profile (binding style / sheet size / creep / marks).  The
    /// profile comes through the config cascade (project + global);
    /// `default` and `chapbook` are built in.
    Impose {
        input: std::path::PathBuf,
        /// Imposition profile name.
        #[arg(long, default_value = "default")]
        config: String,
        #[arg(long)]
        out: Option<std::path::PathBuf>,
        /// Preview the plan (signatures / sheets / creep / first-sheet
        /// schematic) without imposing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Quick saddle-stitch booklet — zero config.  Auto-fits the press
    /// sheet to two source pages side-by-side (any page size works), in
    /// one nested signature.  The shortcut for `impose --config chapbook`
    /// when you just want a foldable booklet of whatever you have.
    Booklet {
        input: std::path::PathBuf,
        #[arg(long)]
        out: Option<std::path::PathBuf>,
        /// Center each spread on a named sheet preset (A4, A3, LETTER,
        /// TABLOID, …) instead of auto-fitting to 2× the page size.
        #[arg(long)]
        sheet: Option<String>,
        /// Add shingle creep compensation (recommended past ~40 pages so
        /// the inner leaves don't bleed past the trim after folding).
        #[arg(long)]
        creep: bool,
        /// Omit crop + fold marks for a clean already-trimmed proof.
        #[arg(long)]
        no_marks: bool,
        /// Preview the plan (signatures / sheets / first-sheet schematic)
        /// without imposing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Keep only the given pages (e.g. `--pages 2-4,7`).
    Extract {
        input: std::path::PathBuf,
        #[arg(long)]
        pages: String,
        #[arg(long)]
        out: Option<std::path::PathBuf>,
    },
    /// Delete the given pages.
    Delete {
        input: std::path::PathBuf,
        #[arg(long)]
        pages: String,
        #[arg(long)]
        out: Option<std::path::PathBuf>,
    },
    /// Rotate the given pages by 90 / 180 / 270 degrees (added to any
    /// existing rotation).
    Rotate {
        input: std::path::PathBuf,
        #[arg(long)]
        pages: String,
        #[arg(long)]
        degrees: i64,
        #[arg(long)]
        out: Option<std::path::PathBuf>,
    },
    /// Reorder pages by a comma-separated 1-based permutation
    /// (e.g. `--mapping 3,1,2`).
    Reorder {
        input: std::path::PathBuf,
        #[arg(long)]
        mapping: String,
        #[arg(long)]
        out: Option<std::path::PathBuf>,
    },
    /// Split into pieces by `--every <n>` pages or `--at <p,p,…>`
    /// (split before those 1-based pages).
    Split {
        input: std::path::PathBuf,
        #[arg(long)]
        every: Option<usize>,
        #[arg(long)]
        at: Option<String>,
        #[arg(long)]
        out_dir: Option<std::path::PathBuf>,
    },
    /// Concatenate two or more PDFs into `--out`.
    Merge {
        inputs: Vec<std::path::PathBuf>,
        #[arg(long)]
        out: std::path::PathBuf,
    },
    /// Read (no flags), set (`--title`/`--author`/`--subject`/
    /// `--keywords`), or `--strip` the document metadata.
    Metadata {
        input: std::path::PathBuf,
        #[arg(long)]
        strip: bool,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        author: Option<String>,
        #[arg(long)]
        subject: Option<String>,
        /// Comma-separated keywords.
        #[arg(long)]
        keywords: Option<String>,
        #[arg(long)]
        out: Option<std::path::PathBuf>,
    },
    /// List the document outline (bookmarks), or inject one from a TOC file
    /// with `--set` (indented `Title :: page` lines; indentation nests).
    Outline {
        input: std::path::PathBuf,
        /// A table-of-contents file to inject as bookmarks. Each line is
        /// `Title :: page` (1-based); leading spaces/tabs set the nesting.
        #[arg(long, value_name = "TOC")]
        set: Option<std::path::PathBuf>,
        /// Where to write the injected PDF (default: `<stem>-outline.pdf`).
        #[arg(long)]
        out: Option<std::path::PathBuf>,
    },
    /// Check a PDF is print-ready (RFC §8.6): effective image DPI, font
    /// embedding, page-size consistency, blank/colour pages.  Profile
    /// (`--profile hand_binding|print_shop|strict`) sets the DPI target;
    /// `--dpi` overrides it.
    Preflight {
        input: std::path::PathBuf,
        #[arg(long, default_value = "hand_binding")]
        profile: String,
        #[arg(long)]
        dpi: Option<u32>,
    },
    /// Generate a standalone EAN-13 ISBN barcode PDF (RFC §8.5).
    Barcode {
        /// 12- or 13-digit ISBN (hyphens/spaces ignored).
        isbn: String,
        #[arg(long)]
        out: std::path::PathBuf,
        /// Bar height in mm (EAN-13 nominal ≈ 22.85).
        #[arg(long)]
        height_mm: Option<f32>,
        /// X-dimension (single module width) in mm (SC2 ≈ 0.33).
        #[arg(long)]
        module_mm: Option<f32>,
        /// Omit the human-readable digits under the bars.
        #[arg(long)]
        no_text: bool,
    },
    /// Generate a full cover-and-spine PDF (RFC §8.4): one landscape page
    /// `[bleed | back | spine | front | bleed]`.  Trim/bleed/stocks come
    /// from the `cover:` config; the spine width is computed from
    /// `--pages` + stocks (or forced with `--spine-mm`).
    Cover {
        #[arg(long)]
        out: std::path::PathBuf,
        /// Interior page count (drives the computed spine width).
        #[arg(long)]
        pages: usize,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        author: Option<String>,
        /// Back-cover blurb (top-left of the back panel).
        #[arg(long)]
        back: Option<String>,
        /// Front-cover art (any format the `image` crate reads).
        #[arg(long)]
        image: Option<std::path::PathBuf>,
        /// How the front art fills its region: `cover` (default, aspect-
        /// preserving full-bleed crop), `fit`, or `stretch`.
        #[arg(long)]
        fit: Option<String>,
        /// ISBN — renders an EAN-13 barcode on the back panel.
        #[arg(long)]
        isbn: Option<String>,
        /// Override the computed spine width (mm).
        #[arg(long)]
        spine_mm: Option<f32>,
        /// Override the config trim width (mm).
        #[arg(long)]
        width_mm: Option<f32>,
        /// Override the config trim height (mm).
        #[arg(long)]
        height_mm: Option<f32>,
    },
    /// Convert to grayscale (RFC §8.7): neutralize content-stream colour
    /// + convert DeviceRGB/CMYK images to DeviceGray, including DCTDecode
    /// (JPEG) photos (re-embedded as grayscale JPEGs).  Best-effort —
    /// CMYK JPEGs / exotic colour spaces are left as-is.
    Grayscale {
        input: std::path::PathBuf,
        #[arg(long)]
        out: Option<std::path::PathBuf>,
    },
    /// Losslessly slim a PDF: prune orphan objects + Flate-compress every
    /// uncompressed stream.
    Optimize {
        input: std::path::PathBuf,
        #[arg(long)]
        out: Option<std::path::PathBuf>,
    },
    /// Stamp text and/or an image onto a page range (RFC §8.7).
    Watermark {
        input: std::path::PathBuf,
        /// Stamp text (e.g. `DRAFT`).
        #[arg(long)]
        text: Option<String>,
        /// Stamp image (logo); any format the `image` crate reads.
        #[arg(long)]
        image: Option<std::path::PathBuf>,
        /// Constant alpha 0..1 (default 0.18).
        #[arg(long)]
        opacity: Option<f32>,
        /// Text rotation in degrees (default 45).
        #[arg(long)]
        rotation: Option<f32>,
        /// Font size in pt (default 72).
        #[arg(long)]
        size: Option<f32>,
        /// Anchor: `center` | `top-left` | `top-right` | `bottom-left` |
        /// `bottom-right` (default center).
        #[arg(long, default_value = "center")]
        position: String,
        /// Limit to a page range (e.g. `1` or `2-4,7`); default all.
        #[arg(long)]
        pages: Option<String>,
        #[arg(long)]
        out: Option<std::path::PathBuf>,
    },
    /// Quick-proof subset: keep `--count` evenly-spaced pages (first +
    /// last always included).
    Sample {
        input: std::path::PathBuf,
        #[arg(long, default_value_t = 8)]
        count: usize,
        #[arg(long)]
        out: Option<std::path::PathBuf>,
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
    /// 1.3.19 LANG-1 P6 — import a dictionary from
    /// another conlang/linguistics tool.  Parses the
    /// foreign format into entries and, by default,
    /// previews them (count + a sample) without
    /// touching the book; pass `--yes` to write them
    /// into the Dictionary.  Duplicate headwords are
    /// skipped with a warning.  Complements the
    /// own-CSV path of `add-word --import`.
    Import {
        /// Language to import into (case-insensitive
        /// match; must already exist).
        language: String,
        /// Path to the source file (`.txt`/`.db`/`.sfm`
        /// for Toolbox; `.pgd`/`.xml` for PolyGlot).
        #[arg(long, value_name = "PATH")]
        file: PathBuf,
        /// Source format.
        #[arg(long, value_enum)]
        format: LanguageImportFormat,
        /// Write the parsed entries into the
        /// Dictionary.  Without this, the command only
        /// previews what it would import.
        #[arg(long)]
        r#yes: bool,
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
    /// 1.3.19 LANG-1 P6 — semantic-gap finder.
    /// Diff the lexicon against a reference concept
    /// scope and report which concepts are still
    /// missing (frequency-ranked, most-core first) —
    /// the exact list to feed `generate-lexicon`.
    Gaps {
        /// Language to analyse.
        language: String,
        /// Reference scope: the built-in
        /// `swadesh_100` core vocabulary (default),
        /// or a path to an HJSON concept list
        /// (`{ name, concepts: [...] }`).
        #[arg(long, default_value = "swadesh_100")]
        scope: String,
        /// Emit the report as JSON (covered / missing
        /// arrays + coverage percentage) for piping.
        #[arg(long)]
        json: bool,
    },
    /// 1.3.19 LANG-1 P6 — creative text generators.
    /// Deterministic, grounded surfaces that show the
    /// language alive: `names` (phonotactic),
    /// `prose` (grammatical sentences via the syntax
    /// engine), `poem` (metered verse).  The themed
    /// modes `blessing` / `curse` / `incantation` are
    /// AI-composed but constrained to the existing
    /// lexicon (need `--provider`).  Prints only —
    /// nothing is written to the book.
    Compose {
        /// Language to compose in.
        language: String,
        /// What to generate: `names` | `prose` |
        /// `poem` | `blessing` | `curse` |
        /// `incantation`.
        #[arg(long, default_value = "prose")]
        kind: String,
        /// How many items (names / sentences).
        #[arg(long, default_value_t = 5)]
        count: usize,
        /// Seed for the deterministic generators —
        /// change it for a different draw.
        #[arg(long, default_value_t = 0)]
        seed: u64,
        /// Verse meter for `poem`: comma-separated
        /// syllable counts per line (e.g. `5,7,5`).
        #[arg(long, default_value = "5,7,5")]
        meter: String,
        /// AI provider override for the themed modes
        /// (else the configured default).
        #[arg(long)]
        provider: Option<String>,
    },
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

    /// LANG-1 P1.1 — generate candidate words from a language's
    /// phonotactic templates.  Reads the typed phoneme block in the
    /// language's `Phonology` chapter (phonemes / classes / templates /
    /// constraints), samples words deterministically, and prints the
    /// ones that satisfy every declared phonotactic constraint.  Pure +
    /// deterministic: the same language + `--count` always yields the
    /// same list, so it's safe in scripts.
    GenerateWord {
        /// Target language name (case-insensitive match against the
        /// per-language sub-book titles under the `Language` system book).
        language: String,
        /// Which template role to draw from.
        #[arg(long, default_value = "root")]
        role: String,
        /// How many words to generate.
        #[arg(long, default_value_t = 20)]
        count: usize,
    },

    /// LANG-1 P1.2 — syllabify a word against a language's phonology.
    /// Segments the word into the language's phonemes (longest-grapheme
    /// match over the inventory), then breaks it into syllables using
    /// sonority peaks + the Maximal Onset Principle, printing the
    /// `CV.CVC`-style result.  An inspector for the phonotactics that the
    /// onset / coda / sonority constraints + (later) stress placement rely
    /// on.
    Syllabify {
        /// Target language name (case-insensitive).
        language: String,
        /// The word to syllabify, in the language's romanization (or raw
        /// IPA).  Segmented greedily against the phoneme inventory.
        #[arg(long)]
        word: String,
    },

    /// LANG-1 P1.3 — derive the surface pronunciation of a word by applying
    /// the language's allophony rules to its underlying form.  Segments the
    /// word into phonemes, runs the ordered allophony rewrites
    /// (underlying → surface), and prints both the surface IPA and its
    /// romanized rendering.
    Ipa {
        /// Target language name (case-insensitive).
        language: String,
        /// The word, in the language's romanization (or raw IPA).
        #[arg(long)]
        word: String,
    },

    /// LANG-1 P1.4 — place primary stress on a word per the language's
    /// stress rule.  Syllabifies the word, applies the rule (fixed initial /
    /// final / penultimate / antepenultimate, or the weight-sensitive Latin
    /// rule), and prints the syllabification with `ˈ` before the stressed
    /// syllable.
    Stress {
        /// Target language name (case-insensitive).
        language: String,
        /// The word, in the language's romanization (or raw IPA).
        #[arg(long)]
        word: String,
    },

    /// LANG-1 P1.5 — convert between IPA and a named romanization scheme.
    /// Forward (default): a space-separated IPA phoneme sequence → written
    /// text.  `--reverse`: written text → IPA, using the scheme's contextual
    /// rules to disambiguate shared graphemes.  `--scheme` selects a named
    /// scheme (defaults to the language's default / first).
    Romanize {
        /// Target language name (case-insensitive).
        language: String,
        /// Forward: space-separated IPA phonemes (`k a ʃ i`).  Reverse:
        /// the written word.
        #[arg(long)]
        text: String,
        /// Named romanization scheme (defaults to the language's default).
        #[arg(long)]
        scheme: Option<String>,
        /// Convert text → IPA instead of IPA → text.
        #[arg(long)]
        reverse: bool,
    },

    /// LANG-1 P1.6 — apply the language's tone-sandhi rules to a sequence of
    /// per-syllable tones.  Takes the underlying tones (as the lexicon would
    /// carry them), runs the ordered sandhi rewrites, and prints the surface
    /// tones (e.g. Mandarin `3 3` → `2 3`).
    Tone {
        /// Target language name (case-insensitive).
        language: String,
        /// Space-separated underlying tone labels (`3 3 3`, `H L H`).
        #[arg(long)]
        tones: String,
    },

    /// LANG-1 P2.1 — audit a language's dictionary for consistency: headwords
    /// that break the phonotactics, homophones (entries sharing a surface
    /// form), and duplicate meanings (accidental synonyms).  The deterministic
    /// half of the dedup gate the AI lexicon generator reuses.
    Audit {
        /// Target language name (case-insensitive).
        language: String,
        /// Emit the report as JSON (for CI / scripting).
        #[arg(long)]
        json: bool,
    },

    /// LANG-1 P6.1 — a descriptive profile of a language: inventory balance
    /// (consonants / vowels), phoneme frequency across the lexicon, the
    /// syllable-length distribution, which onsets/codas get used, and the
    /// part-of-speech spread.  The snapshot the grammar book / dictionary draw
    /// on (vs `audit`, which hunts for problems).
    Stats {
        /// Target language name (case-insensitive).
        language: String,
        /// Emit the profile as JSON.
        #[arg(long)]
        json: bool,
    },

    /// 1.7 LING-1 L-P2 — deterministic quantitative metrics for a language:
    /// phoneme-distribution entropy, the Zipf fit of that distribution,
    /// phonotactic saturation, and mora weight. The information-theoretic
    /// complement to `stats`. Read-only; `--json` for machine use.
    Metrics {
        /// Target language name (case-insensitive).
        language: String,
        /// Emit the metrics as JSON.
        #[arg(long)]
        json: bool,
    },

    /// 1.7 LING-1 L-P3 — judge the language's grammar block against the
    /// typological baseline: head-directionality harmony, the classic
    /// implicational universals (Greenberg 2/3/4, the OV↔GenN / OV↔RelN
    /// correlations), and a word-order + morphotype survey. A violation is a
    /// flag, not an error. Read-only; `--json` for machine use.
    Universals {
        /// Target language name (case-insensitive).
        language: String,
        /// Emit the report as JSON.
        #[arg(long)]
        json: bool,
    },

    /// 1.7 LING-1 Wave-2 — validate the typed grammar blocks (`ug_parameters`,
    /// `verb_classes`) in the Grammar chapter and check them for consistency
    /// against the WALS feature answers (e.g. `head_final` vs `word_order`).
    #[command(name = "grammar-check")]
    GrammarCheck {
        /// Target language name (case-insensitive).
        language: String,
        /// Emit the report as JSON.
        #[arg(long)]
        json: bool,
    },

    /// 1.7 LING-1 Wave-2 — find minimal pairs in the lexicon and report the
    /// distinctive feature each turns on (the functional load of the language's
    /// contrasts), via the distinctive-feature matrix. Read-only.
    Pairs {
        /// Target language name (case-insensitive).
        language: String,
        /// Maximum example pairs to list (the counts reflect all of them).
        #[arg(long, default_value_t = 30)]
        limit: usize,
        /// Emit the report as JSON.
        #[arg(long)]
        json: bool,
    },

    /// 1.7 LING-1 Wave-2 — judge the phoneme inventory against cross-linguistic
    /// tendencies (voicing-pair symmetry, place-series coverage, near-universal
    /// segments, size), via the distinctive-feature matrix. A gap is a flag, not
    /// an error. Read-only; `--json` for machine use.
    Naturalness {
        /// Target language name (case-insensitive).
        language: String,
        /// Emit the report as JSON.
        #[arg(long)]
        json: bool,
    },

    /// 1.7 LING-1 Wave-2 — detect vowel harmony (backness, rounding) by measuring
    /// how consistently the vowels within a word agree, via the feature matrix.
    /// Read-only; `--json` for machine use.
    Harmony {
        /// Target language name (case-insensitive).
        language: String,
        /// Emit the report as JSON.
        #[arg(long)]
        json: bool,
    },

    /// 1.7 LING-1 Wave-2 — where each phoneme appears (syllable onset / nucleus /
    /// coda and the word edges), and any restricted (defective) distributions —
    /// a consonant confined to codas, or barred from a word edge. Read-only.
    Distribution {
        /// Target language name (case-insensitive).
        language: String,
        /// Emit the report as JSON.
        #[arg(long)]
        json: bool,
    },

    /// 1.7 LING-1 L-P4 — a one-page grammar sketch: a deterministic prose
    /// overview of the language, assembling the phonology, typology and lexicon
    /// analyses. Instant and always current (no AI). `--out` writes to a file.
    Sketch {
        /// Target language name (case-insensitive).
        language: String,
        /// Write the sketch to this file (default: stdout).
        #[arg(long, value_name = "PATH")]
        out: Option<String>,
    },

    /// 1.7 LING-1 Wave-2 — recommend phonemes that would round out the inventory
    /// (the voiced counterpart missing beside a voiceless obstruent, a
    /// near-universal segment the inventory lacks), via the feature matrix.
    /// Advisory; changes nothing.
    #[command(name = "suggest-phonemes")]
    SuggestPhonemes {
        /// Target language name (case-insensitive).
        language: String,
        /// Emit the report as JSON.
        #[arg(long)]
        json: bool,
    },

    /// 1.7 LING-1 L-P4 — scaffold a starter phoneme inventory from a short
    /// description (AI), as pasteable Phonology-chapter HJSON. Preview-only: it
    /// prints (or `--out` writes) a validated block; it never touches an
    /// existing language.
    Scaffold {
        /// A short description of the language's desired sound (e.g. "a flowing,
        /// vowel-rich island language").
        #[arg(long, value_name = "DESCRIPTION")]
        from: String,
        /// Write the proposal to this file (default: stdout).
        #[arg(long, value_name = "PATH")]
        out: Option<String>,
        /// Override the LLM provider for this call.
        #[arg(long)]
        provider: Option<String>,
    },

    /// 1.7 LING-1 L-P5 — the morphological parser: analyse a surface word into
    /// root + affixes by reversing the morphology (strip known affixes until what
    /// remains is a dictionary root). The inverse of paradigm generation.
    Parse {
        /// Target language name (case-insensitive).
        language: String,
        /// The surface word to analyse.
        #[arg(long, value_name = "WORD")]
        word: String,
        /// Emit the analyses as JSON.
        #[arg(long)]
        json: bool,
    },

    /// 1.7 LING-1 L-P6b — build the X-bar phrase-structure tree (CP → TP → VP) of
    /// a clause from its subject / verb / object(s), placing heads and complements
    /// by the language's word order. `--word-order` overrides the declared feature.
    Tree {
        /// Target language name (case-insensitive).
        language: String,
        /// The clause's verb.
        #[arg(long, value_name = "VERB")]
        verb: String,
        /// The clause's arguments, comma-separated (subject, object, indirect).
        #[arg(long, value_name = "SUBJ,OBJ,IOBJ")]
        args: String,
        /// Override the word order (svo | sov | vso | …).
        #[arg(long, value_name = "ORDER")]
        word_order: Option<String>,
        /// Emit the tree as JSON.
        #[arg(long)]
        json: bool,
    },

    /// 1.7 LING-1 L-P6b — syntactic movement: front a constituent (wh-movement /
    /// topicalisation) over the X-bar tree, leaving a coindexed trace.
    Movement {
        /// Target language name (case-insensitive).
        language: String,
        /// The clause's verb.
        #[arg(long, value_name = "VERB")]
        verb: String,
        /// The clause's arguments, comma-separated (subject, object, indirect).
        #[arg(long, value_name = "SUBJ,OBJ,IOBJ")]
        args: String,
        /// Which role to front (subject | object | indirect).
        #[arg(long = "move", value_name = "ROLE")]
        r#move: String,
        /// Override the word order.
        #[arg(long, value_name = "ORDER")]
        word_order: Option<String>,
        /// Emit the derived tree as JSON.
        #[arg(long)]
        json: bool,
    },

    /// 1.7 LING-1 L-P6b — binding theory: decide whether one argument may refer to
    /// another, by c-command and the binding principles (a reflexive object may
    /// bind to the subject, a pronoun may not, a name is a Principle-C violation).
    Binding {
        /// Target language name (case-insensitive).
        language: String,
        /// The clause's verb.
        #[arg(long, value_name = "VERB")]
        verb: String,
        /// The clause's arguments, comma-separated (subject, object, indirect).
        #[arg(long, value_name = "SUBJ,OBJ,IOBJ")]
        args: String,
        /// The potential antecedent's role (default: subject).
        #[arg(long, default_value = "subject")]
        antecedent: String,
        /// The anaphor's role (default: object).
        #[arg(long, default_value = "object")]
        anaphor: String,
        /// The anaphor type (reflexive | pronoun | name).
        #[arg(long, value_name = "TYPE", default_value = "reflexive")]
        r#type: String,
        /// Override the word order.
        #[arg(long, value_name = "ORDER")]
        word_order: Option<String>,
        /// Emit the verdict as JSON.
        #[arg(long)]
        json: bool,
    },

    /// 1.7 LING-1 L-P6 — the Oracle: judge a candidate word for well-formedness
    /// by linguistic level — phonotactics (unknown segments, constraint
    /// violations) and morphology (does it analyse as root + affixes?). Unlike
    /// `audit`, it judges arbitrary input, not just the finished lexicon.
    Check {
        /// Target language name (case-insensitive).
        language: String,
        /// The candidate word to judge.
        #[arg(long, value_name = "WORD")]
        word: String,
        /// Emit the findings as JSON.
        #[arg(long)]
        json: bool,
    },

    /// 1.7 LING-1 L-P6 — the Oracle over a whole clause (levels 3–4): subject–verb
    /// agreement and argument structure. Checks that the verb inflects for its
    /// subject's features and that the argument count matches the verb's valence.
    CheckClause {
        /// Target language name (case-insensitive).
        language: String,
        /// The observed verb surface form.
        #[arg(long, value_name = "VERB")]
        verb: String,
        /// The clause's arguments, comma-separated (subject first).
        #[arg(long, value_name = "SUBJ,OBJ,IOBJ")]
        args: String,
        /// The verb's root, for regenerating its expected agreeing form.
        #[arg(long, value_name = "ROOT")]
        verb_root: Option<String>,
        /// The subject's features, e.g. `number=pl,person=3` (enables the
        /// agreement check).
        #[arg(long, value_name = "K=V,…")]
        subject_features: Option<String>,
        /// Override the verb's valence (else read from its verb class).
        #[arg(long)]
        valence: Option<String>,
        /// Emit the findings as JSON.
        #[arg(long)]
        json: bool,
    },

    /// 1.7 LING-1 L-P6 — the Oracle's agreement check over any head–dependent pair:
    /// does a dependent word (an adjective, a determiner, a verb) correctly inflect
    /// for its head's features, under the declared agreement rule?
    CheckAgreement {
        /// Target language name (case-insensitive).
        language: String,
        /// The dependent's category — the `dependent` of an agreement rule
        /// (`adjective`, `determiner`, `verb`, …).
        #[arg(long, value_name = "CATEGORY")]
        dependent: String,
        /// The dependent's observed surface form.
        #[arg(long, value_name = "FORM")]
        form: String,
        /// The dependent's root, for regenerating its expected agreeing form.
        #[arg(long, value_name = "ROOT")]
        root: String,
        /// The head's features, e.g. `number=pl,gender=fem`.
        #[arg(long, value_name = "K=V,…")]
        head_features: String,
        /// Emit the findings as JSON.
        #[arg(long)]
        json: bool,
    },

    /// 1.7 LING-1 L-P5 — argument linking: work out a clause's thematic roles,
    /// RRG macroroles (actor / undergoer) and grammatical relations from the
    /// verb's valence. `--valence` overrides a declared verb class.
    Link {
        /// Target language name (case-insensitive).
        language: String,
        /// The verb (looked up in the `verb_classes` block for its valence).
        #[arg(long, value_name = "VERB")]
        verb: String,
        /// The clause's arguments, comma-separated (subject first).
        #[arg(long, value_name = "A,B,C")]
        args: String,
        /// Override the valence (intransitive | transitive | ditransitive | impersonal).
        #[arg(long)]
        valence: Option<String>,
        /// Emit the linking as JSON.
        #[arg(long)]
        json: bool,
    },

    /// 1.7 LING-1 L-P1 — the Consequence Tracer. Preview a pending sound change
    /// across the current lexicon (which words shift, which distinctions merge,
    /// which new homophones appear) without committing it. The rule uses the
    /// rewrite syntax `X > Y / A _ B`, e.g. `s > ʃ / _ i` or `d > t / _ #`.
    Trace {
        /// Target language name (case-insensitive).
        language: String,
        /// The pending sound-change rule to preview.
        #[arg(long, value_name = "RULE")]
        rule: String,
        /// Maximum example changes to list (the counts reflect all of them).
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// Emit the report as JSON.
        #[arg(long)]
        json: bool,
    },

    /// LANG-1 P6.2 — render the dictionary as a document.  Markdown (`md`) or
    /// Typst (`typ`); the Typst path is a paginated, two-column book that embeds
    /// the generated conscript font and shows each headword in the native script
    /// (transliterated) beside its romanization, pronunciation, and gloss.
    Dictionary {
        /// Target language name (case-insensitive).
        language: String,
        /// Output format: `md` or `typ`.
        #[arg(long, default_value = "md")]
        format: String,
        /// Write the document here (otherwise it prints to stdout).
        #[arg(long)]
        out: Option<std::path::PathBuf>,
        /// Conscript font family for the Typst path (defaults to the `font`
        /// block's family).
        #[arg(long)]
        font: Option<String>,
    },

    /// LANG-1 P6.3 — render a reference grammar as a document.  Markdown (`md`)
    /// or Typst (`typ`); the Typst path is a paginated A5 book with an outline
    /// and numbered sections — phonology (inventory, phonotactics, allophony,
    /// stress, tone), morphology (affixes, derivation), the typology answers,
    /// idioms & metaphors, and the sample texts.  The companion volume to
    /// `dictionary`.
    GrammarBook {
        /// Target language name (case-insensitive).
        language: String,
        /// Output format: `md` or `typ`.
        #[arg(long, default_value = "md")]
        format: String,
        /// Write the document here (otherwise it prints to stdout).
        #[arg(long)]
        out: Option<std::path::PathBuf>,
        /// Conscript font family for the Typst path (defaults to the `font`
        /// block's family).
        #[arg(long)]
        font: Option<String>,
        /// Prepend an AI-written study guide that explains the linguistic terms
        /// the grammar uses (case, alignment, allophony, …) and how this
        /// language applies them.  Needs an AI provider.
        #[arg(long)]
        study: bool,
        /// Override the configured AI provider (for `--study`).
        #[arg(long)]
        provider: Option<String>,
    },

    /// LANG-1 P7 — generate a learner's textbook with the AI: a complete graded
    /// course (pronunciation guide, vocabulary, grammar lessons with worked
    /// examples, a reading, and exercises) authored by the model from the
    /// language's own data.  Markdown (`md`) or Typst (`typ`); the Typst path
    /// embeds the conscript font behind a deterministic page scaffold.
    Tutorial {
        /// Target language name (case-insensitive).
        language: String,
        /// Output format: `md` or `typ`.
        #[arg(long, default_value = "md")]
        format: String,
        /// Write the document here (otherwise it prints to stdout).
        #[arg(long)]
        out: Option<std::path::PathBuf>,
        /// Conscript font family for the Typst path (defaults to the `font`
        /// block's family).
        #[arg(long)]
        font: Option<String>,
        /// Override the configured AI provider.
        #[arg(long)]
        provider: Option<String>,
    },

    /// LANG-1 P2.6 — link a Place to a language it's spoken in.  Stored in a
    /// `.inkhaven/conlang-links.json` sidecar (the Places book is prose and is
    /// never modified).  Sets the primary language by default; `--secondary`
    /// adds a secondary one.
    LinkPlace {
        /// Place name (matched case-insensitively against the Places book).
        place: String,
        /// Language name.
        language: String,
        /// Add as a secondary language instead of setting the primary.
        #[arg(long)]
        secondary: bool,
        /// 1.3.22 LANG-2 P4 — the variety (dialect/register id) of the primary
        /// language spoken here.
        #[arg(long)]
        variety: Option<String>,
    },

    /// LANG-1 P2.6 — declare a Character's proficiency in a language (native /
    /// fluent / conversational / broken / reading_only).  Stored in the
    /// `.inkhaven/conlang-links.json` sidecar; feeds AI dialog generation.
    LinkCharacter {
        /// Character name (matched case-insensitively against the Characters book).
        character: String,
        /// Language name.
        language: String,
        /// Proficiency: native | fluent | conversational | broken | reading_only.
        proficiency: String,
        /// 1.3.22 LANG-2 P4 — the variety this character natively speaks (their
        /// idiolect base).
        #[arg(long)]
        native_variety: Option<String>,
    },

    /// LANG-1 P2.6 — list the Places and Characters linked to a language.
    Speakers {
        /// Language name (case-insensitive).
        language: String,
    },

    /// 1.3.22 LANG-2 P4 — the language **ecology**: who speaks what (and which
    /// variety) where.  Lists every place with its language + variety, every
    /// character with their commanded languages + native variety, and the
    /// contact areas.  With `--svg <path>` writes a node-link **atlas**.
    Ecology {
        /// Write the atlas as an SVG file at this path (instead of the text report).
        #[arg(long)]
        svg: Option<std::path::PathBuf>,
    },

    /// 1.3.22 LANG-2 P4 — render a form / text in a **character's idiolect** —
    /// their native variety of their primary language (from the links sidecar).
    Idiolect {
        /// Character name (case-insensitive).
        character: String,
        /// A single base form to render in the idiolect.
        #[arg(long)]
        word: Option<String>,
        /// A run of whitespace-separated base forms, rendered word by word.
        #[arg(long)]
        text: Option<String>,
    },

    /// LANG-1 P5.2 — compile a directory of glyph SVGs into a UFO font source.
    /// Each SVG's filename stem names the glyph (a single character also sets
    /// its Unicode codepoint); unsuitable glyphs (see `glyph-lint`) are
    /// skipped with a warning.  Emits a UFO source and/or a ready-to-use
    /// TrueType binary (`--format`); the TTF is compiled fully in-process
    /// (no external tool).  Source the glyphs either from a language's own
    /// `font` block (`--language`) or a loose directory (`--glyphs`).
    FontBuild {
        /// Font family name (optional with `--language`, which supplies it).
        family: Option<String>,
        /// Build from this language's `font` config block + glyph store.
        #[arg(long)]
        language: Option<String>,
        /// Directory of glyph `.svg` files (alternative to `--language`).
        #[arg(long)]
        glyphs: Option<std::path::PathBuf>,
        /// Output path (extension set by `--format`; defaults to `<family>.<ext>`).
        #[arg(long)]
        out: Option<std::path::PathBuf>,
        /// Units per em (the design grid; overrides the config value).
        #[arg(long)]
        upm: Option<f64>,
        /// Output format: `ufo` (editable source), `ttf` (binary font), or
        /// `both`.
        #[arg(long, default_value = "ufo")]
        format: String,
    },

    /// LANG-1 P5.4 — import a glyph SVG into a language's writing system:
    /// preflight it, copy it into the project glyph store, and bind it to a
    /// phoneme and/or Unicode codepoint in the language's `font` config block.
    /// `font-build --language` then compiles the script straight from the book.
    FontImportGlyph {
        /// Language to bind the glyph to.
        language: String,
        /// Path to the glyph `.svg` file.
        #[arg(long)]
        svg: std::path::PathBuf,
        /// Phoneme (or romanization grapheme) this glyph stands for.
        #[arg(long)]
        phoneme: Option<String>,
        /// Unicode codepoint: a single character (`a`) or hex (`U+E000`).
        #[arg(long)]
        codepoint: Option<String>,
        /// Glyph name (defaults: `uniXXXX` from the codepoint, else the
        /// phoneme, else the SVG filename stem).
        #[arg(long)]
        name: Option<String>,
    },

    /// LANG-1 P5.4 — show a language's `font` config: family, units-per-em,
    /// and every glyph binding with its codepoint, phoneme, and artwork status.
    FontConfig {
        /// Language to inspect.
        language: String,
        /// Emit JSON.
        #[arg(long)]
        json: bool,
    },

    /// LANG-1 P5.6 — list the spatial templates available to a language:
    /// built-in arrangements (`lr`/`tb`/`quad`/`stack3`) plus any custom
    /// `templates` defined in its `font` block.
    FontTemplates {
        /// Language to inspect.
        language: String,
    },

    /// LANG-1 P5.6 — compose component glyphs into a precomposed block (a
    /// Hangul-style syllable square, an Egyptian quadrat) per a spatial
    /// template, baking them into one glyph.  Advisory — previews the composite
    /// + preflight; `--yes` binds it like `font-import-glyph`.
    FontCompose {
        /// Language the block is for.
        language: String,
        /// Spatial template name (see `font-templates`).
        #[arg(long)]
        template: String,
        /// Name for the composed glyph.
        #[arg(long)]
        name: String,
        /// Unicode codepoint: a single character (`가`) or hex (`U+AC00`).
        #[arg(long)]
        codepoint: Option<String>,
        /// Phoneme/syllable this block stands for.
        #[arg(long)]
        phoneme: Option<String>,
        /// A component binding `SLOT=GLYPH` (repeat for each cell).
        #[arg(long = "slot", value_name = "SLOT=GLYPH")]
        slots: Vec<String>,
        /// Write the composed SVG here (otherwise it prints to stdout).
        #[arg(long)]
        out: Option<std::path::PathBuf>,
        /// Bind the composed block into the language's `font` block.
        #[arg(long)]
        yes: bool,
    },

    /// LANG-1 P5.6 — input method: transliterate romanized/phonemic text into
    /// the script's codepoints using the `font` block's glyph→phoneme bindings
    /// (longest key wins, so digraph keys like `th`/`ka` beat their prefixes).
    /// The result renders in the generated font.
    Transliterate {
        /// Language to type in.
        language: String,
        /// The romanized / phonemic text to convert.
        #[arg(long)]
        text: String,
        /// Emit JSON.
        #[arg(long)]
        json: bool,
    },

    /// LANG-1 P5.6 — binding-time B: emit a Typst quadrat that arranges
    /// component glyphs spatially at layout time (the hieroglyphic path — no
    /// precomposed font glyph).  Each component renders as a character of the
    /// language's font, so it must have a codepoint.
    SpatialTypst {
        /// Language the quadrat is for.
        language: String,
        /// Spatial template name (see `font-templates`).
        #[arg(long)]
        template: String,
        /// Name for the emitted Typst `#let` binding.
        #[arg(long)]
        name: String,
        /// A component binding `SLOT=GLYPH` (repeat for each cell).
        #[arg(long = "slot", value_name = "SLOT=GLYPH")]
        slots: Vec<String>,
        /// Quadrat side length (a Typst length).
        #[arg(long, default_value = "2em")]
        size: String,
        /// Write the Typst snippet here (otherwise it prints to stdout).
        #[arg(long)]
        out: Option<std::path::PathBuf>,
    },

    /// LANG-1 P5.5 — AI text-to-SVG glyph draft: describe a glyph and the model
    /// drafts an SVG, which is run through the suitability preflight.  Advisory
    /// — previews the SVG + verdict; `--yes` binds a usable draft into the
    /// language's `font` block (the same path as `font-import-glyph`).
    GlyphDraft {
        /// Language the glyph is for.
        language: String,
        /// What the glyph should look like (e.g. "a vertical stroke with a hook").
        #[arg(long)]
        describe: String,
        /// Phoneme (or grapheme) this glyph stands for.
        #[arg(long)]
        phoneme: Option<String>,
        /// Unicode codepoint: a single character (`a`) or hex (`U+E000`).
        #[arg(long)]
        codepoint: Option<String>,
        /// Glyph name (defaults like `font-import-glyph`).
        #[arg(long)]
        name: Option<String>,
        /// Override the configured AI provider.
        #[arg(long)]
        provider: Option<String>,
        /// Write the drafted SVG here (otherwise it prints to stdout).
        #[arg(long)]
        out: Option<std::path::PathBuf>,
        /// Bind the drafted glyph into the language's `font` block.
        #[arg(long)]
        yes: bool,
    },

    /// LANG-1 P5.1 — check whether a glyph SVG is suitable for font
    /// compilation: does it parse, does it have a fillable outline (not
    /// stroke-only / empty), is it free of raster images, is it monochrome.
    /// Run it on AI-drafted or hand-drawn artwork before binding it to a
    /// phoneme.
    GlyphLint {
        /// Path to the SVG file.
        #[arg(long)]
        svg: std::path::PathBuf,
    },

    /// LANG-1 P4.3 — AI comparative reconstruction: given cognate forms from
    /// daughter languages, propose the most plausible proto-form (with sound
    /// correspondences + reasoning).  Advisory — a proposal, nothing committed.
    Reconstruct {
        /// Cognate daughter forms (space-separated).
        #[arg(long)]
        forms: String,
        /// The shared meaning (optional context for the model).
        #[arg(long)]
        gloss: Option<String>,
        /// LLM provider override.
        #[arg(long)]
        provider: Option<String>,
    },

    /// LANG-1 P4.3 — AI genealogical-realism check: assess whether a language's
    /// diachronic sound-change chain is typologically plausible (attested) or
    /// unnatural, rule by rule.
    RealismCheck {
        /// Target language name (case-insensitive).
        language: String,
        /// LLM provider override.
        #[arg(long)]
        provider: Option<String>,
    },

    /// LANG-1 P4.2 — print the language-family tree (each language under its
    /// declared `proto`).
    FamilyTree,

    /// LANG-1 P4.2 — the cognate set of a proto-form: its reflex in every
    /// daughter language (each daughter's sound-change chain applied to the
    /// proto-form).
    Cognates {
        /// The proto-language name (case-insensitive).
        proto: String,
        /// The proto-form to trace.
        #[arg(long)]
        form: String,
    },

    /// LANG-1 P4.1 — apply a language's diachronic sound-change chain to a
    /// proto-form and print the resulting daughter form.  Reads the
    /// `{ diachronics: { proto, rules } }` block in the Phonology chapter.
    SoundChange {
        /// Target language name (case-insensitive).
        language: String,
        /// The proto-form to evolve (in the proto's romanization).
        #[arg(long)]
        form: String,
    },

    /// LANG-1 P4.1 — derive a daughter language's lexicon from its proto by
    /// applying the daughter's diachronic sound-change chain to every proto
    /// dictionary entry.  Advisory: nothing is added without `--yes`.
    DeriveLexicon {
        /// The daughter language (declares `proto` + `rules` in its Phonology
        /// chapter's diachronics block).
        language: String,
        /// Add the derived forms to the daughter's Dictionary.
        #[arg(long)]
        yes: bool,
    },

    /// LANG-1 P3.5 — add an idiom (a phrase with a literal word-by-word gloss
    /// and a separate idiomatic meaning).  Stored in the Grammar chapter; the
    /// AI translation consults it to stay idiomatic.
    IdiomAdd {
        /// Target language name (case-insensitive).
        language: String,
        /// The phrase as a whole.
        #[arg(long)]
        form: String,
        /// Word-by-word literal gloss.
        #[arg(long)]
        literal: Option<String>,
        /// What it actually means.
        #[arg(long)]
        meaning: String,
        /// Register tag (formal / vulgar / …).
        #[arg(long)]
        register: Option<String>,
    },

    /// LANG-1 P3.5 — declare a conceptual metaphor (a source→target domain
    /// mapping, e.g. LIFE is a JOURNEY).
    MetaphorAdd {
        /// Target language name (case-insensitive).
        language: String,
        /// Source domain.
        #[arg(long)]
        source: String,
        /// Target domain.
        #[arg(long)]
        target: String,
        /// An example phrase exhibiting the metaphor.
        #[arg(long)]
        example: Option<String>,
    },

    /// LANG-1 P3.5 — list a language's idioms and declared metaphors.
    Idioms {
        /// Target language name (case-insensitive).
        language: String,
    },

    /// LANG-1 P3.4 — the grammar questionnaire.  With no `--set`, lists the
    /// typological-feature catalog (WALS-aligned: word order, alignment, case,
    /// tense/aspect/mood, …) with the language's current answers + coverage.
    /// `--set <feature>=<value>` records one answer (validated against the
    /// catalog) into a `grammar` block in the Grammar chapter.
    Grammar {
        /// Target language name (case-insensitive).
        language: String,
        /// Record an answer: `word_order=sov`.
        #[arg(long)]
        set: Option<String>,
        /// Emit the current answers as JSON.
        #[arg(long)]
        json: bool,
    },

    /// LANG-1 P3.3 — propose derived lexemes for a root: apply the language's
    /// derivational rules (agent nouns, diminutives, …) to the root, with
    /// allophony, and print the new word + sense + POS for each rule that
    /// fires.  Advisory: nothing is added without `--yes`.
    Derive {
        /// Target language name (case-insensitive).
        language: String,
        /// The root word (in the language's romanization).
        #[arg(long)]
        root: String,
        /// The root's gloss (defaults to the root).
        #[arg(long)]
        gloss: Option<String>,
        /// The root's part of speech (gates which rules apply).
        #[arg(long)]
        pos: Option<String>,
        /// Add the proposed derived forms to the Dictionary.
        #[arg(long)]
        yes: bool,
    },

    /// LANG-1 P3.2 — interlinear auto-gloss of conlang text.  Builds a reverse
    /// index from the dictionary (each entry's bare form, plus the inflected
    /// forms of entries that declare a `paradigm`, with allophony applied),
    /// then prints a Leipzig-style two-line gloss for the given text.
    Gloss {
        /// Target language name (case-insensitive).
        language: String,
        /// The conlang text to gloss (whitespace-separated words).
        #[arg(long)]
        text: String,
    },

    /// 1.7 IGT-1 (Wave 4) — interlinear glossed text: auto-gloss a sentence and
    /// lay it out as an aligned Leipzig block (the sentence, the gloss, a literal
    /// translation). Reuses the auto-gloss index; `--json` emits the structured
    /// IGT.
    Igt {
        /// Target language name (case-insensitive).
        language: String,
        /// The conlang sentence to gloss.
        #[arg(long)]
        text: String,
        /// Emit the structured IGT as JSON.
        #[arg(long)]
        json: bool,
    },

    /// LANG-1 P3.1 — generate the full paradigm of a root: apply a paradigm
    /// template's morpheme sequence (from the `Morphology` chapter) to the
    /// root, run the language's allophony across the affix boundaries, and
    /// print the surface form + Leipzig gloss for every cell.
    Paradigm {
        /// Target language name (case-insensitive).
        language: String,
        /// The root word (in the language's romanization).
        #[arg(long)]
        root: String,
        /// Paradigm template name (from the Morphology chapter).
        #[arg(long)]
        template: String,
        /// Gloss for the root (defaults to the root itself).
        #[arg(long)]
        gloss: Option<String>,
    },

    /// LANG-1 syntax — assemble a **sentence** from a subject, verb, and object.
    /// Orders the words by the language's `word_order`, case-marks the nouns by
    /// its `alignment`, runs agreement (adjective↔noun, verb↔subject), and
    /// prints the clause with an interlinear gloss.  Words are `root` or
    /// `root:gloss`.
    Sentence {
        /// Target language name (case-insensitive).
        language: String,
        /// Subject noun (`kira` or `kira:bird`).
        #[arg(long)]
        subject: Option<String>,
        /// Subject's grammatical number.
        #[arg(long, default_value = "sg")]
        subject_number: String,
        /// Subject's person, for verb agreement (`1` / `2` / `3`).
        #[arg(long, default_value = "3")]
        subject_person: String,
        /// An adjective modifying the subject (`mira:bright`).
        #[arg(long)]
        subject_adj: Option<String>,
        /// The verb (`nami` or `nami:see`).
        #[arg(long)]
        verb: Option<String>,
        /// Object noun (`pata:stone`); omit for an intransitive clause.
        #[arg(long)]
        object: Option<String>,
        /// Object's grammatical number.
        #[arg(long, default_value = "sg")]
        object_number: String,
        /// An adjective modifying the object.
        #[arg(long)]
        object_adj: Option<String>,
        /// Paradigm used to inflect nouns (case marking).
        #[arg(long, default_value = "noun")]
        noun_paradigm: String,
        /// Paradigm used to inflect verbs.
        #[arg(long, default_value = "verb")]
        verb_paradigm: String,
        /// 1.3.19 — negate the clause (realized per the
        /// `negation` typology: particle / affix /
        /// auxiliary).  Supply the negative word with
        /// `--negator`, else only the gloss is marked.
        #[arg(long)]
        negate: bool,
        /// The negative word/affix (`na` or `na:not`),
        /// when the language has one.
        #[arg(long)]
        negator: Option<String>,
        /// 1.3.19 — make it a polar (yes/no) question
        /// (realized per the `question` typology:
        /// particle / inversion / intonation /
        /// morphology).
        #[arg(long)]
        question: bool,
        /// The question particle (`ka` or `ka:Q`), when
        /// the language uses one.
        #[arg(long)]
        q_particle: Option<String>,
    },

    /// 1.3.19 LANG-1 P9 — build a noun phrase with a
    /// **relative clause** ("the bird that sees the
    /// stone"), obeying the `relative_clause` typology
    /// (prenominal vs postnominal).  The head plays a
    /// role inside the embedded clause (its subject or
    /// object — the gap); the engine case-marks and
    /// agrees the rest.  Prints surface + interlinear
    /// gloss + literal.
    Relative {
        /// Target language name (case-insensitive).
        language: String,
        /// The head noun being modified (`kira` or
        /// `kira:bird`).
        #[arg(long)]
        head: String,
        /// The head's role in the embedded clause:
        /// `subject` ("the bird that sees …") or
        /// `object` ("the stone that … sees").
        #[arg(long, default_value = "subject")]
        role: String,
        /// The embedded verb (`nami:see`).
        #[arg(long)]
        verb: String,
        /// The other (non-head) argument of the
        /// embedded clause, when transitive
        /// (`pata:stone`).
        #[arg(long)]
        with: Option<String>,
        /// The relativizer word (`ya:that`), when the
        /// language uses one (glossed `REL`).
        #[arg(long)]
        relativizer: Option<String>,
        /// Paradigm used to inflect nouns.
        #[arg(long, default_value = "noun")]
        noun_paradigm: String,
        /// Paradigm used to inflect verbs.
        #[arg(long, default_value = "verb")]
        verb_paradigm: String,
    },

    /// 1.3.19 LANG-1 P9 — **coordinate** noun phrases
    /// or clauses with a conjunction ("the bird and
    /// the stone", "the bird sees and the river
    /// falls").  Give two or more `--np` (each a single
    /// `root:gloss` noun) OR two or more `--clause`
    /// (each `subj verb [obj]`, space-separated
    /// `root:gloss` words); join them with
    /// `--conjunction`.  Prints surface + interlinear +
    /// literal.
    Coordinate {
        /// Target language name (case-insensitive).
        language: String,
        /// A clause conjunct — space-separated
        /// `root:gloss` words: subject, verb, and an
        /// optional object.  Repeat for each clause.
        #[arg(long = "clause")]
        clauses: Vec<String>,
        /// A noun-phrase conjunct — a single
        /// `root:gloss` noun.  Repeat for each noun.
        #[arg(long = "np")]
        nps: Vec<String>,
        /// The conjunction (`na:and`, `or:or`); glossed
        /// by its own gloss, or `CONJ` if none given.
        #[arg(long)]
        conjunction: Option<String>,
        /// Paradigm used to inflect nouns.
        #[arg(long, default_value = "noun")]
        noun_paradigm: String,
        /// Paradigm used to inflect verbs.
        #[arg(long, default_value = "verb")]
        verb_paradigm: String,
    },

    /// 1.3.19 LANG-1 P9 — build a sentence with a
    /// **complement clause**: a whole clause serving as
    /// the object of a matrix verb of speech or
    /// cognition ("I know that the bird sees the
    /// stone").  The matrix subject + verb wrap an
    /// embedded clause (`--comp-*`) introduced by an
    /// optional complementizer; the complement fills
    /// the object slot, so word order positions it.
    /// Prints surface + interlinear + literal.
    Complement {
        /// Target language name (case-insensitive).
        language: String,
        /// The matrix subject (`mi:I`).
        #[arg(long)]
        subject: Option<String>,
        /// The matrix subject's person, for agreement.
        #[arg(long, default_value = "1")]
        subject_person: String,
        /// The matrix subject's number.
        #[arg(long, default_value = "sg")]
        subject_number: String,
        /// The matrix verb (`tira:know`).
        #[arg(long)]
        verb: String,
        /// The complementizer (`ya:that`), glossed `COMP`.
        #[arg(long)]
        complementizer: Option<String>,
        /// The embedded clause's subject (`kira:bird`).
        #[arg(long)]
        comp_subject: Option<String>,
        /// The embedded clause's verb (`nami:see`).
        #[arg(long)]
        comp_verb: String,
        /// The embedded clause's object (`pata:stone`).
        #[arg(long)]
        comp_object: Option<String>,
        /// Paradigm used to inflect nouns.
        #[arg(long, default_value = "noun")]
        noun_paradigm: String,
        /// Paradigm used to inflect verbs.
        #[arg(long, default_value = "verb")]
        verb_paradigm: String,
    },

    /// LANG-1 P3.x — apply **agreement** (concord): inflect a dependent word
    /// (an adjective, a verb) to agree with the grammatical features of its
    /// head (its noun, its subject).  Uses the `agreement` rules + paradigm in
    /// the Morphology chapter.
    Agree {
        /// Target language name (case-insensitive).
        language: String,
        /// The dependent root word to inflect.
        #[arg(long)]
        word: String,
        /// The dependent's part of speech (must match an `agreement` rule).
        #[arg(long)]
        pos: String,
        /// The head's grammatical features, e.g. `number=pl,case=dat`.
        #[arg(long)]
        features: String,
        /// Gloss for the dependent root (defaults to the root itself).
        #[arg(long)]
        gloss: Option<String>,
    },

    /// 1.3.22 LANG-2 P1 — list the **varieties** (dialects / registers /
    /// sociolects) declared for a language, with their axis, prestige, and the
    /// size of their sound-change + word-override deltas.
    Varieties {
        /// Target language name (case-insensitive).
        language: String,
    },

    /// 1.3.22 LANG-2 P1 — render a form *in a variety* (`lect`).  Applies the
    /// variety's sound changes (the same engine diachronics uses,
    /// synchronically) to a `--word` or a `--text` run, showing the base →
    /// variety diff.  A variety is a *dialect*, *register*, or *sociolect*.
    /// 1.3.23 LANG-3 P0 — **translate** English into the conlang. Tier 1 (the
    /// deterministic rule-based spine): each English word is mapped to a
    /// headword by its lexicon gloss, and the LANG-1 syntax engine orders,
    /// case-marks, inflects, and agrees the result. Pure-Rust and offline; it
    /// handles simple declarative sentences (the neural tiers, for richer
    /// parsing and fluency, arrive in later phases). Unknown words are marked
    /// `«word»` and listed so you can coin or `add-word` them.
    Translate {
        /// Source language name (case-insensitive).
        language: String,
        /// The English text to translate.
        text: String,
        /// Show the per-word decision trace.
        #[arg(long)]
        trace: bool,
        /// Emit JSON instead of the formatted display.
        #[arg(long)]
        json: bool,
    },

    /// 1.3.23 LANG-3 P0 — **reverse** a conlang sentence back into English
    /// (Tier 1 RBMT). Each surface word is un-inflected against the lexicon's
    /// paradigm forms and glossed; roles are read off the language's
    /// `word_order`. English generation is deliberately plain.
    Reverse {
        /// Source (conlang) language name (case-insensitive).
        language: String,
        /// The conlang surface text to reverse-translate.
        text: String,
        /// Emit JSON instead of the formatted display.
        #[arg(long)]
        json: bool,
    },

    /// 1.3.23 LANG-3 P0 — **cross-translate** one conlang into another by
    /// pivoting through English (reverse the source, then translate into the
    /// target). The English waypoint is shown; error compounds across the two
    /// passes.
    Cross {
        /// Source (conlang) language name (case-insensitive).
        from: String,
        /// Target (conlang) language name (case-insensitive).
        to: String,
        /// The source-language surface text.
        text: String,
        /// Emit JSON instead of the formatted display.
        #[arg(long)]
        json: bool,
    },

    /// 1.3.23 LANG-3 P1 — **remember** a confirmed English→conlang translation
    /// (the correction loop, Amendment A1). The pair is appended to the
    /// language's translation memory, so `translate` reuses it immediately —
    /// exactly on the next call, no retraining. Re-remembering an English
    /// supersedes its prior target.
    Remember {
        /// Target language name (case-insensitive).
        language: String,
        /// The English source.
        #[arg(long)]
        english: String,
        /// The confirmed conlang translation.
        #[arg(long)]
        conlang: String,
    },

    /// 1.3.23 LANG-3 P1 — list a language's **translation memory** (the
    /// remembered English→conlang pairs `translate` draws on).
    Memory {
        /// Target language name (case-insensitive).
        language: String,
    },

    /// 1.3.23 LANG-3 P1 — generate a **synthetic corpus**: translate an English
    /// source pool with the RBMT and keep the sentences the language fully
    /// covers, seeding the translation memory (Amendment A1). Previews by
    /// default; `--yes` adds the accepted pairs. The acceptance rate and the
    /// top missing words gauge how mature the lexicon is.
    Corpus {
        /// Target language name (case-insensitive).
        language: String,
        /// A custom English pool file (one sentence per line); defaults to the
        /// bundled pool.
        #[arg(long)]
        pool: Option<String>,
        /// Cap how many pool sentences to translate.
        #[arg(long)]
        limit: Option<usize>,
        /// Add the accepted pairs to the language's translation memory.
        #[arg(long)]
        yes: bool,
        /// Emit JSON instead of the formatted display.
        #[arg(long)]
        json: bool,
    },

    /// 1.3.23 LANG-3 P2 — **evaluate** translation quality without a human
    /// reference (Amendment A1 / RFC §8.8): *round-trip* semantic similarity
    /// (translate then reverse, compared to the source by embedding cosine) and
    /// *coverage* (the fraction of the test set the lexicon fully translates).
    /// Measures the rule-based engine; the test set defaults to the bundled pool.
    Eval {
        /// Target language name (case-insensitive).
        language: String,
        /// A custom English test-set file (one sentence per line); defaults to
        /// the bundled pool.
        #[arg(long)]
        test_set: Option<String>,
        /// Cap how many sentences to evaluate.
        #[arg(long)]
        limit: Option<usize>,
        /// Emit JSON instead of the formatted display.
        #[arg(long)]
        json: bool,
    },

    /// 1.3.23 LANG-3 P3 — **export** the language's translation system as a
    /// portable `.itm` bundle (Amendment A1 / RFC §8.9): a single zip of the
    /// translation memory + lexicon + manifest + README, to ship alongside a
    /// published work. Under the retrieval architecture the memory *is* the
    /// model, so the pack is small and re-importable.
    ExportTranslation {
        /// Target language name (case-insensitive).
        language: String,
        /// Output path (defaults to `<lang>-translation.itm`).
        #[arg(long)]
        out: Option<String>,
    },

    Lect {
        /// Target language name (case-insensitive).
        language: String,
        /// The variety id (e.g. `lowland`, `formal`).
        variety: String,
        /// A single base form to render in the variety.
        #[arg(long)]
        word: Option<String>,
        /// A run of whitespace-separated base forms, rendered word by word.
        #[arg(long)]
        text: Option<String>,
    },

    /// 1.3.22 LANG-2 P1 — a **dialect-comparison** table: the first `--count`
    /// dictionary headwords rendered across the base form and every declared
    /// variety (the classic dialectology display).
    Dialects {
        /// Target language name (case-insensitive).
        language: String,
        /// How many headwords to compare.
        #[arg(long, default_value_t = 12)]
        count: usize,
    },

    /// 1.3.22 LANG-2 P2 — **borrow** a word into a language: nativise a donor
    /// form to the recipient's inventory + phonotactics (perceive → repair via
    /// the `loan_phonology` block).  Shows the adaptation trace.  With `--yes`,
    /// adds the adapted word to the recipient's Dictionary, recording the donor
    /// in the etymology.  The donor form is given *phonemically* (one symbol per
    /// sound).
    Borrow {
        /// Recipient language (the borrower).
        language: String,
        /// The donor form to adapt (phonemic).
        #[arg(long)]
        form: String,
        /// The donor language name, recorded in the etymology.
        #[arg(long)]
        from: Option<String>,
        /// Working-language gloss for the loanword (needed with `--yes`).
        #[arg(long)]
        gloss: Option<String>,
        /// Part of speech for the added entry (default `noun`).
        #[arg(long, default_value = "noun")]
        r#type: String,
        /// Add the adapted word to the recipient's Dictionary.
        #[arg(long)]
        r#yes: bool,
    },

    /// 1.3.22 LANG-2 P3 — show **areal** (Sprachbund) convergence.  With a
    /// language, assesses each declared areal feature against that language's
    /// own typology — already converged, would shift, or would adopt (an
    /// advisory overlay, never rewriting the grammar).  With no language, prints
    /// the regional view: every contact area, its member languages, and the
    /// shared features.
    Areal {
        /// Language to assess (omit for the whole-world regional view).
        language: Option<String>,
    },

    /// 1.3.22 LANG-2 P6 — (AI) **propose a dialect/register**: the model suggests
    /// a coherent set of sound changes + a few lexical swaps for the requested
    /// flavour; the deterministic engine validates them (so the variety is always
    /// phonologically legal) and previews the result.  With `--yes`, writes the
    /// variety into the Grammar chapter.
    ProposeDialect {
        /// Target language name (case-insensitive).
        language: String,
        /// The flavour to design, e.g. "a coastal trading dialect" or "an
        /// archaic priestly register".
        #[arg(long)]
        describe: String,
        /// The variety id to use (default: derived from the description).
        #[arg(long)]
        id: Option<String>,
        /// AI provider override.
        #[arg(long)]
        provider: Option<String>,
        /// Write the proposed variety into the Grammar chapter.
        #[arg(long)]
        r#yes: bool,
    },

    /// 1.3.22 LANG-2 P6 — (AI) assess whether a language's declared **areal**
    /// (Sprachbund) features are typologically plausible — the contact analogue
    /// of `realism-check`.
    ArealCheck {
        /// Target language name (case-insensitive).
        language: String,
        /// AI provider override.
        #[arg(long)]
        provider: Option<String>,
    },

    /// 1.3.22 LANG-2 P6 — (AI) **propose realistic loanwords**: which concepts a
    /// language would borrow from a donor in a topic domain, each with a
    /// plausible donor form, then nativised by the deterministic adapter (P2).
    /// Advisory — add the ones you like with `borrow … --yes`.
    ProposeLoans {
        /// Recipient language (the borrower).
        language: String,
        /// The donor language name.
        #[arg(long)]
        from: String,
        /// The semantic domain to borrow in (trade, religion, seafaring …).
        #[arg(long)]
        topic: Option<String>,
        /// How many loanwords to propose.
        #[arg(long, default_value_t = 6)]
        count: usize,
        /// AI provider override.
        #[arg(long)]
        provider: Option<String>,
    },

    /// LANG-1 P2.7 — scan the manuscript for candidate **undefined** conlang
    /// words: words that look like the language (segment fully into its
    /// inventory + pass its phonotactics) but aren't in the dictionary.  Only
    /// paragraphs that already contain a known conlang word are scanned, so
    /// working-language prose is skipped.  Heuristic — review the list, then
    /// `add-word` the real ones or fix the typos.
    ScanManuscript {
        /// Language name (case-insensitive).
        language: String,
        /// Emit the report as JSON.
        #[arg(long)]
        json: bool,
    },

    /// LANG-1 P2.4 — query a language's dictionary by the rich entry fields:
    /// register, semantic domain, in-world era, part of speech, and a free
    /// substring over headword + gloss.  Filters combine (AND).
    Query {
        /// Target language name (case-insensitive).
        language: String,
        /// Register tag (formal / vulgar / archaic / sacred …).
        #[arg(long)]
        register: Option<String>,
        /// Semantic-domain tag (weapon / kinship / weather …).
        #[arg(long)]
        domain: Option<String>,
        /// In-world era tag.
        #[arg(long)]
        era: Option<String>,
        /// Part of speech.
        #[arg(long)]
        pos: Option<String>,
        /// Substring over headword + gloss (case-insensitive).
        #[arg(long)]
        text: Option<String>,
        /// Emit matches as JSON.
        #[arg(long)]
        json: bool,
    },

    /// LANG-1 P2.2 — AI-assisted dictionary generation.  The deterministic
    /// generator builds a pool of phonotactically-valid forms; the AI assigns
    /// each a concept / gloss / part-of-speech for the requested topic; then
    /// every proposal passes the dedup gate (no illegal form, no homophone of
    /// an existing word, no duplicate meaning) before it is offered.  Glosses
    /// are written in the project's working language.  Advisory: nothing is
    /// added without `--yes`.
    GenerateLexicon {
        /// Target language name (case-insensitive).
        language: String,
        /// Semantic domain to generate vocabulary for (e.g. "seafaring").
        /// Omit for general everyday vocabulary.
        #[arg(long)]
        topic: Option<String>,
        /// How many entries to propose.
        #[arg(long, default_value_t = 20)]
        count: usize,
        /// Optional in-world era tag recorded on the prompt.
        #[arg(long)]
        era: Option<String>,
        /// Optional register tag (formal / vulgar / sacred / …).
        #[arg(long)]
        register: Option<String>,
        /// LLM provider override (defaults to the configured provider).
        #[arg(long)]
        provider: Option<String>,
        /// Also reject near-synonyms (embedding cosine over glosses) — the
        /// semantic half of the dedup gate.  Loads the embedding model.
        #[arg(long)]
        semantic: bool,
        /// Cosine threshold above which two glosses count as near-synonyms
        /// (with `--semantic`).
        #[arg(long, default_value_t = 0.88)]
        semantic_threshold: f32,
        /// Add the kept proposals to the Dictionary (default is a dry run).
        #[arg(long)]
        yes: bool,
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
    /// 1.3.19 LANG-1 P6 — XLIFF 1.2 translation
    /// interchange.  Each entry becomes a
    /// `trans-unit` (working-language source →
    /// invented-word target), so the lexicon loads
    /// into CAT tools (OmegaT, memoQ, Weblate) as a
    /// translation memory.  Streams to stdout.
    Xliff,
    /// 1.3.19 LANG-1 P6 — LaTeX via the `linguex`
    /// package: bold headword + POS + gloss, with
    /// any example sentence as a numbered `\ex.`.
    /// Paste-ready for a paper or grammar sketch.
    /// Streams to stdout.
    Linguex,
    /// 1.3.19 LANG-1 P6 — Markdown IPA inventory
    /// chart: consonants and vowels grouped, each
    /// with its romanization.  Streams to stdout.
    IpaChart,
}

/// 1.3.19 LANG-1 P6 — foreign lexicon formats the
/// `inkhaven language import` command can ingest
/// (beyond the round-trippable own-CSV that
/// `add-word --import` already reads).
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum LanguageImportFormat {
    /// Toolbox / MDF Standard Format (SFM) — the
    /// `\lx … \ps … \ge …` marker database used by
    /// SIL Toolbox, FieldWorks, and **Lexique Pro**.
    Toolbox,
    /// PolyGlot dictionary.  Pass either the native
    /// `.pgd` archive (the `PGDictionary.xml` is
    /// unzipped automatically) or a raw exported
    /// `.xml`.
    Polyglot,
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

/// WORLD-6 — sub-subcommands under `inkhaven world …`: the utopian/dystopian
/// coherence checker. `inkhaven world` with no subcommand keeps the existing
/// consistency snapshot.
#[derive(Debug, Subcommand)]
pub enum WorldCommand {
    /// Run the coherence check (Stage 1 always; Stage 2/3 on demand). Exits 1 on
    /// any chain-logic finding, 2 on any entailment violation.
    UtopiaCheck {
        #[arg(long)]
        book: Option<String>,
        /// `1` | `2` | `3` | `all`. Default: Stage 1, report cached 2/3.
        #[arg(long)]
        stage: Option<String>,
        /// Restrict to one named premise group.
        #[arg(long)]
        group: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Print the extracted claim model without running any checks.
    UtopiaModel {
        #[arg(long)]
        book: Option<String>,
        #[arg(long)]
        group: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Mark a finding suppressed with a reason.
    UtopiaSuppress {
        #[arg(long)]
        finding: String,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        book: Option<String>,
    },
    /// Force recomputation (Stage 1 or Stage 3), bypassing the hash cache.
    UtopiaRefresh {
        #[arg(long)]
        book: Option<String>,
        /// `1` (re-extract) or `3` (re-scan prose). Default: 1.
        #[arg(long)]
        stage: Option<u8>,
    },
}

/// 1.4.16 CHAR-1 — sub-subcommands under `inkhaven character …`.
#[derive(Debug, Subcommand)]
pub enum CharacterCommand {
    /// Show one character's tracked arc: declaration, chapter-by-chapter state
    /// chain, agency scores, stalls, completeness checks, and planning gaps.
    /// Read-only (reads the cached `char.duckdb`; run `refresh`/`check` first to
    /// populate it).
    Arc {
        /// Character name (case-insensitive; matches the Characters-book roster).
        name: String,
        #[arg(long)]
        book: Option<String>,
    },
    /// Run arc-completeness checks for every declared arc (LLM; extracts the
    /// state chain first, lazily). Exits 1 on any gap or stall, 2 if the ending
    /// or earned-arc check fails.
    Check {
        #[arg(long)]
        book: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Recompute agency (deterministic) and re-extract the observable-state
    /// chain (LLM, content-hash lazy) for declared characters. `--name` limits
    /// it to one character.
    Refresh {
        #[arg(long)]
        book: Option<String>,
        #[arg(long)]
        name: Option<String>,
    },
    /// Detect Planning-Board arc-coverage gaps (deterministic, no LLM): a
    /// declared arc no scene card names, an arc confined to the first half, or
    /// an arc with no scene card in the final act. Exits 1 on any gap.
    Plan {
        #[arg(long)]
        book: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

/// 1.4.18 INNER-THEOLOGIAN-1 — sub-subcommands under `inkhaven theologian …`.
#[derive(Debug, Subcommand)]
pub enum TheologianCommand {
    /// Run the deterministic fast-track ethical-signal detector across the book.
    /// Exits 1 on any unsuppressed signal (a pre-submission review prompt).
    Scan {
        #[arg(long)]
        book: Option<String>,
        /// `moral-invisibility` | `consequence-gap` | `sacred-levity` | `all`.
        #[arg(long)]
        signal: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Run a slow-track theological session (LLM) over a chapter or the whole
    /// book and print the persona's questions. `--category 1-6` (default 6 —
    /// the book's implicit theology); `--lens <code>` restricts to one tradition.
    Session {
        #[arg(long)]
        book: Option<String>,
        /// Restrict to one chapter (1-based); omitted = the whole book.
        #[arg(long)]
        chapter: Option<u32>,
        /// Question category 1–6 (default 6).
        #[arg(long)]
        category: Option<u8>,
        /// Restrict to one tradition lens (e.g. `gnostic`, `buddhism`, `secular`).
        #[arg(long)]
        lens: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Suppress every fast-track signal on a paragraph (intent-ledger style).
    Suppress {
        #[arg(long)]
        para: String,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        book: Option<String>,
    },
}

/// LEXICON (1.6.21+) — sub-subcommands under `inkhaven lexicon …`.
#[derive(Debug, Subcommand)]
pub enum LexiconCommand {
    /// List the scholarly-lexicon terms — original-language forms, distinct senses,
    /// and which are watched for equivocation.
    List {
        #[arg(long)]
        book: Option<String>,
        /// Only the equivocation-watched, multi-sense terms.
        #[arg(long)]
        watched: bool,
        #[arg(long)]
        json: bool,
    },
}

/// RIGOR (1.6.20+) — sub-subcommands under `inkhaven rigor …`.
#[derive(Debug, Subcommand)]
pub enum RigorCommand {
    /// Run the deterministic reasoning-rigor reader across the book — false
    /// dichotomy, question-begging, straw man, overgeneralization, non-sequitur.
    /// Advisory (exits 0) unless `--strict`.
    Scan {
        #[arg(long)]
        book: Option<String>,
        /// `false-dichotomy` | `question-begging` | `straw-man` |
        /// `overgeneralization` | `non-sequitur` | `all`.
        #[arg(long)]
        signal: Option<String>,
        #[arg(long)]
        json: bool,
        /// Exit non-zero when any signal is found (for a CI gate).
        #[arg(long)]
        strict: bool,
    },
}

/// 1.4.19 MYTH-1 — sub-subcommands under `inkhaven myth …`.
#[derive(Debug, Subcommand)]
pub enum MythCommand {
    /// Refresh the inventory + deterministic scans and print the symbol-density /
    /// motif-presence / archetype-presence heatmap plus the deterministic
    /// findings (archetype vacant/absent, motif absent from the final act).
    /// Zero-AI.
    Scan {
        #[arg(long)]
        book: Option<String>,
        /// Recompute every chapter, bypassing the content-hash cache.
        #[arg(long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
    /// Run the LLM checks (symbol consistency, motif completeness, archetype role)
    /// plus the deterministic checks. Exits 1 on any unsuppressed finding — a
    /// pre-submission review gate.
    Check {
        #[arg(long)]
        book: Option<String>,
        /// `symbol` | `motif` | `archetype` | `deterministic` | `all` (default).
        #[arg(long)]
        kind: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Print the declared inventory (symbols / motifs / archetypes) without
    /// running any check.
    Profile {
        #[arg(long)]
        book: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Force recomputation of the deterministic caches (density + explicit
    /// motifs), bypassing the content-hash cache.
    Refresh {
        #[arg(long)]
        book: Option<String>,
    },
    /// Mark a finding suppressed by id.
    Suppress {
        #[arg(long)]
        finding: String,
        #[arg(long)]
        book: Option<String>,
    },
}

/// 1.3.24 PANE-1 — sub-subcommands under `inkhaven output …`. The minimal CLI
/// surface over the Output message store (the pane itself is a TUI feature);
/// useful for scripting and for sshing into a project without a TUI.
/// WORLD-4 — sub-subcommands under `inkhaven realworld …`. P0 = the astronomy
/// slice; the surface grows with each layer (RFC §10.1).
#[derive(Debug, Subcommand)]
pub enum RealworldCommand {
    /// Scaffold a starter `world.hjson` at the project root.
    New {
        /// The world's name.
        name: String,
        /// Overwrite an existing `world.hjson`.
        #[arg(long)]
        force: bool,
    },
    /// Parse `world.hjson` and report whether it is valid.
    Validate,
    /// Propose several candidate worlds from consecutive seeds (each row is a
    /// seed you can adopt in `world.hjson`) — the world proposes, you choose.
    Variants {
        /// How many candidates to summarize (1–24).
        #[arg(long, default_value_t = 5)]
        count: usize,
    },
    /// Show the parsed world definition.
    Show {
        /// Emit the full definition as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Compile the whole world (all layers) — or one `--layer` — and print it.
    Compile {
        /// Which layer to compile: `all` (default, the whole world) or one of
        /// astronomy · geology · climate · hydrology · demographics.
        #[arg(long)]
        layer: Option<String>,
        /// Emit the layer output as JSON.
        #[arg(long)]
        json: bool,
        /// Write the output into the World system book (requires an
        /// initialized project). Astronomy is compiler-owned and overwrites.
        #[arg(long)]
        materialize: bool,
    },
    /// Generate Place proposals from the demographics layer into the proposal
    /// queue (nothing commits until you accept). Re-running skips sites already
    /// accepted or rejected.
    Propose,
    /// WORLD-12 — generate Mythology proposals (symbols / motifs) from the
    /// world's cultures' beliefs into the proposal queue. Accept them like Place
    /// proposals to commit `para:myth-*` entries into the Mythology book.
    ProposeMyth,
    /// WORLD-12 — propose one ruler per polity (named in the world's style,
    /// rooted in the realm's culture) into the proposal queue. Accept them like
    /// Place proposals to commit Character stubs into the Characters book.
    ProposeRulers,
    /// WORLD-13 — propose one language per culture (from its language profile +
    /// naming sample) into the proposal queue. Accept to scaffold a language book
    /// in the ConLang suite, seeded with the world's design brief.
    ProposeLanguage,
    /// Work the proposal queue (list / accept / reject).
    Proposals {
        #[command(subcommand)]
        cmd: ProposalsCommand,
    },
    /// List the Place ↔ World cross-references (accepted compiler Places + their
    /// climate zone / biome / hydrology basis / coordinates).
    Places,
    /// WORLD-12 — give a Place (from the Places book) a location on the world
    /// grid so it appears on the plakat map. Pass grid cells (`--x`/`--y`) or
    /// geographic degrees (`--lat`/`--lon`); the biome under the cell is filled
    /// from the compiled climate. Works for any Place, hand-authored or compiled.
    SetCoords {
        /// The Place's name (its paragraph title in the Places book).
        name: String,
        /// Grid column (0..width-1). Use with `--y`.
        #[arg(long)]
        x: Option<usize>,
        /// Grid row (0..height-1, row 0 = north). Use with `--x`.
        #[arg(long)]
        y: Option<usize>,
        /// Latitude in degrees (−90 south .. 90 north). Use with `--lon`.
        #[arg(long, allow_hyphen_values = true)]
        lat: Option<f64>,
        /// Longitude in degrees (−180 .. 180). Use with `--lat`.
        #[arg(long, allow_hyphen_values = true)]
        lon: Option<f64>,
    },
    /// WORLD-7 — derive a story-Timeline calendar from the world's astronomy
    /// (months, season markers) and print it to adopt under `timeline.calendar`.
    Calendar,
    /// WORLD-13 — a chronicle of the world's compiled past: for each epoch, how
    /// far the world had grown by then (settlements, population, realms) alongside
    /// its events. Pure presentation of the history layer — no simulation.
    Chronicle {
        /// Emit the chronicle as JSON.
        #[arg(long)]
        json: bool,
    },
    /// WORLD-14 — propose settlement names in each realm's own phonic style
    /// (deterministic), so a realm's towns share a family sound instead of the
    /// generic placeholder names. A naming aid you adopt when you accept Places.
    Name {
        /// Emit the names as JSON.
        #[arg(long)]
        json: bool,
    },
    /// WORLD-15 — the trade network: which realms are linked (each to its nearest
    /// non-rival neighbours) and how (land road or sea lane). Connectivity, not
    /// simulated economics. Drawn on the plakat map as roads.
    Trade {
        /// Emit the routes as JSON.
        #[arg(long)]
        json: bool,
    },
    /// WORLD-7 — emit a consolidated Markdown world reference (calendar, sky,
    /// regions, landmarks, waters, settlements, economy, magic).
    Gazetteer {
        /// Write to a file instead of stdout (e.g. a manuscript appendix source).
        #[arg(long)]
        output: Option<String>,
    },
    /// WORLD-8 — derive the world's founding chronology + epochs from the
    /// demographics and print it, with an adoptable Timeline block.
    History {
        /// Emit the chronology as JSON.
        #[arg(long)]
        json: bool,
        /// Write the History chapter into the World system book.
        #[arg(long)]
        materialize: bool,
    },
    /// WORLD-10 — the local season + insolation for a day-of-year at a latitude.
    Weather {
        /// Day of the year (0-based).
        #[arg(long, default_value_t = 0.0)]
        day: f64,
        /// Latitude in degrees (negative = southern hemisphere).
        #[arg(long, default_value_t = 45.0)]
        lat: f64,
    },
    /// WORLD — flora / fauna archetypes + a keystone animal per land biome.
    Ecology,
    /// WORLD-9 — nations formed by clustering settlements around their capitals.
    Polities,
    /// WORLD-9 — one culture per polity (ethos, belief, a conlang profile).
    Culture,
    /// WORLD-10 — is a journey between two map cells plausible in the claimed
    /// time? Checks the real distance (planet + grid) against the mode's pace.
    Travel {
        /// Origin place name (an accepted world Place); overrides --from-x/-y.
        #[arg(long)]
        from: Option<String>,
        /// Destination place name; overrides --to-x/-y.
        #[arg(long)]
        to: Option<String>,
        #[arg(long, default_value_t = 0.0)]
        from_x: f64,
        #[arg(long, default_value_t = 0.0)]
        from_y: f64,
        #[arg(long, default_value_t = 0.0)]
        to_x: f64,
        #[arg(long, default_value_t = 0.0)]
        to_y: f64,
        /// Claimed journey time in days.
        #[arg(long, default_value_t = 1.0)]
        days: f64,
        /// foot (default) | horse | cart | ship.
        #[arg(long, default_value = "foot")]
        mode: String,
    },
    /// WORLD-10 — a scene brief: season + weather at a place's latitude on a
    /// day, its biome/climate, and the nearest realm's culture.
    Scene {
        /// An accepted world Place the scene is set in.
        #[arg(long)]
        place: Option<String>,
        /// Day of the year (0-based).
        #[arg(long, default_value_t = 0.0)]
        day: f64,
        /// Latitude override (else derived from the place's map row).
        #[arg(long)]
        lat: Option<f64>,
    },
    /// Show the magic ledger — the declared exceptions to physics the
    /// fact-checker will respect. Edit it in the `magic:` block of `world.hjson`.
    Magic {
        /// Also materialize the ledger into the World book.
        #[arg(long)]
        materialize: bool,
    },
    /// WORLD-5 — flag every co-location conflict in the timeline: a character whose
    /// events place them in two different places at overlapping times. Pure
    /// timeline check (no LLM); respects the `magic:` ledger.
    CoLocation,
    /// Cross-paragraph coherence pass (slow track): gather every paragraph under a
    /// node (book / chapter) and ask the LLM for contradictions *between* them — a
    /// character in two places, a fact reversed, a timeline that doesn't add up.
    Coherence {
        /// The book or chapter node id whose paragraphs to check together.
        node: String,
        /// Per-call soft cap (estimated tokens); the call is skipped with a notice
        /// if exceeded unless `--force`.
        #[arg(long, default_value_t = 8000)]
        max_cost: usize,
        /// Run even if the cost estimate exceeds `--max-cost`.
        #[arg(long)]
        force: bool,
    },
    /// WORLD-12 — an AI pass over `world.hjson`: compile + run the deterministic
    /// lints (free), then ask an LLM to critique the world's consistency and
    /// realism and recommend improvements. `--write-notes` files each
    /// recommendation into the Notes book. Cost-capped; the cap informs, never
    /// blocks.
    Critique {
        /// Per-call soft cap (estimated tokens); overrides `world.critique_max_tokens`.
        #[arg(long)]
        max_cost: Option<usize>,
        /// Run even if the cost estimate exceeds the soft cap.
        #[arg(long)]
        force: bool,
        /// File each recommendation as a paragraph in the Notes book.
        #[arg(long)]
        write_notes: bool,
        /// Run only the deterministic lints; skip the LLM call entirely.
        #[arg(long)]
        lints_only: bool,
    },
    /// Render the world map with `plakat`: compile every layer, emit a MapSpec,
    /// and write a features PNG + GeoJSON under `assets/maps/`. Resolved landmark
    /// positions are read back to refine each Place's coordinates.
    Map {
        /// Build and write the MapSpec but don't invoke `plakat` (useful for
        /// inspecting the spec or when plakat isn't installed).
        #[arg(long)]
        spec_only: bool,
        /// Don't update Place coordinates from plakat's resolved landmark
        /// positions (render the map but leave the cross-references untouched).
        #[arg(long)]
        no_ingest: bool,
    },
}

/// INNER_SOCRATES-1 — the examined-authorship CLI surface. P2 ships the Fast
/// track (`check`) and the intent ledger (`ledger`); the Slow track, personas,
/// and conversation land in later phases.
#[derive(Debug, Subcommand)]
pub enum InnerSocratesCommand {
    /// Run the deterministic Fast track over prose and print the questions it
    /// raises. Persists + emits to Output when the project is initialized.
    Check {
        /// Check this literal text.
        #[arg(long)]
        text: Option<String>,
        /// Check a paragraph by id (reads its content from the store).
        #[arg(long)]
        paragraph: Option<String>,
        /// Check a paragraph by its slug path as shown in `inkhaven list`
        /// (e.g. `essay/ch1/opening`). The convenient alternative to
        /// `--paragraph <uuid>`; `NN-` order prefixes are tolerated.
        #[arg(long)]
        path: Option<String>,
        /// Also run the Slow track — an LLM pass for the deep Socratic questions
        /// patterns miss (needs an LLM provider; cost-capped).
        #[arg(long)]
        slow: bool,
        /// Slow-track per-call soft cap (estimated tokens); skipped with a notice
        /// if exceeded unless `--force`.
        #[arg(long, default_value_t = 6000)]
        max_cost: usize,
        /// Run the Slow track even if the cost estimate exceeds `--max-cost`.
        #[arg(long)]
        force: bool,
    },
    /// Run the timeline pass — compare the project's timeline of events against
    /// the prose and ask whether what is declared is dramatized (needs an LLM
    /// provider; silently does nothing without a timeline).
    Timeline {
        #[arg(long, default_value_t = 8000)]
        max_cost: usize,
        #[arg(long)]
        force: bool,
    },
    /// Inspect persisted findings.
    #[command(subcommand)]
    Findings(FindingsCommand),
    /// List the intent ledger (the deliberate authorial choices the interrogator
    /// respects).
    Ledger,
    /// Reader Personas — the careful-reader perspectives you switch between.
    #[command(subcommand)]
    Persona(PersonaCommand),
    /// Promotion candidates — patterns of repeated dismissal the system suggests
    /// declaring as a deliberate intent.
    #[command(subcommand)]
    Suggestions(SuggestionsCommand),
    /// `.isl` ledger bundles — carry series-level intentions between books.
    #[command(subcommand)]
    Bundle(BundleCommand),
}

/// INNER_EDITOR-1 (1.4.2+) — `inkhaven inner-editor …`, the Inner Editor
/// literary/stylistic companion's terminal surface.
#[derive(Debug, Subcommand)]
pub enum InnerEditorCommand {
    /// Run one Editor pass over a paragraph and print its observations (the same
    /// engine the TUI chord uses). Needs an LLM provider; cost recorded under
    /// the `inner_editor` budget (informative).
    Engage {
        /// Observe this literal text (no preceding context).
        #[arg(long)]
        text: Option<String>,
        /// Observe a paragraph by id (reads it + its preceding context from the store).
        #[arg(long)]
        paragraph: Option<String>,
        /// Skip the informative daily-cap warning.
        #[arg(long)]
        force: bool,
    },
    /// Inspect persisted Editor findings.
    #[command(subcommand)]
    Findings(EditorFindingsCommand),
    /// Declare an observation category a deliberate choice — writes an intent
    /// ledger entry that suppresses future Editor findings of that category
    /// (project-wide, or scoped to a chapter).
    Intent {
        /// The category id (e.g. `tautology`, `dictionary_richness`, `style_instability`).
        category: String,
        /// Limit the declaration to a chapter id.
        #[arg(long)]
        chapter: Option<String>,
        /// A note explaining the choice (shown when a finding is suppressed).
        #[arg(long)]
        description: Option<String>,
    },
    /// Promotion candidates — categories you've dismissed enough that declaring
    /// them deliberate (an intent) would quiet the noise.
    #[command(subcommand)]
    Suggestions(EditorSuggestionsCommand),
    /// Inspect the Inner Editor configuration.
    #[command(subcommand)]
    Config(EditorConfigCommand),
    /// Today's Inner Editor LLM usage by sub-budget.
    Usage,
}

/// `inkhaven inner-editor suggestions …`.
#[derive(Debug, Subcommand)]
pub enum EditorSuggestionsCommand {
    /// List `(category, chapter)` dismissal patterns at or above the threshold.
    List {
        #[arg(long, default_value_t = 5)]
        threshold: i64,
    },
    /// Promote a pattern — declare the category deliberate (writes the ledger)
    /// and stop suggesting it.
    Promote {
        category: String,
        #[arg(long)]
        chapter: Option<String>,
        #[arg(long)]
        description: Option<String>,
    },
    /// Refuse a suggestion — don't propose this `(category, chapter)` again.
    Dismiss {
        category: String,
        #[arg(long)]
        chapter: Option<String>,
    },
}

/// `inkhaven inner-editor findings …`.
#[derive(Debug, Subcommand)]
pub enum EditorFindingsCommand {
    /// List persisted findings (newest first), optionally filtered by severity.
    List {
        /// `praise` | `note` | `concern`.
        #[arg(long)]
        severity: Option<String>,
    },
    /// A paragraph's findings across re-engagements (oldest first).
    History {
        #[arg(long)]
        paragraph: String,
    },
}

/// `inkhaven inner-editor config …`.
#[derive(Debug, Subcommand)]
pub enum EditorConfigCommand {
    /// Show the active Inner Editor configuration (tuning, categories, caps).
    Show,
}

/// INNER_SOCRATES-1 — the `.isl` ledger-bundle surface.
#[derive(Debug, Subcommand)]
pub enum BundleCommand {
    /// Export intent-ledger entries to an `.isl` bundle.
    Export {
        /// Which entries to include.
        #[arg(long, value_parser = ["series", "project", "all"], default_value = "series")]
        scope_level: String,
        /// Output path (default: `<project>/intent-ledger.isl`).
        #[arg(long)]
        out: Option<String>,
    },
    /// Import intent-ledger entries from an `.isl` bundle.
    Import {
        path: String,
        /// What to do when an entry id already exists.
        #[arg(long, value_parser = ["skip", "override"], default_value = "skip")]
        conflict: String,
    },
}

/// INNER_SOCRATES-1 — the promotion-candidate surface.
#[derive(Debug, Subcommand)]
pub enum SuggestionsCommand {
    /// List promotion candidates (categories dismissed ≥ threshold times).
    List {
        #[arg(long, default_value_t = 5)]
        threshold: i64,
    },
    /// Promote a candidate into an intent-ledger entry that suppresses it.
    Promote {
        /// The Socratic category id (e.g. `framing_interrogation`).
        category: String,
        /// The chapter the entry scopes to (omit for project-wide).
        #[arg(long)]
        chapter: Option<String>,
        /// The author's explanation of the deliberate choice.
        #[arg(long)]
        description: Option<String>,
    },
    /// Refuse a candidate so it won't re-suggest.
    Dismiss {
        category: String,
        #[arg(long)]
        chapter: Option<String>,
    },
}

/// INNER_SOCRATES-1 — inspecting persisted findings.
#[derive(Debug, Subcommand)]
pub enum FindingsCommand {
    /// List all persisted findings (newest first).
    List,
    /// Show a paragraph's findings in chronological order (across re-checks).
    History { paragraph: String },
}

/// INNER_SOCRATES-1 — the Reader Persona surface.
#[derive(Debug, Subcommand)]
pub enum PersonaCommand {
    /// List available personas (bundled + user + project), marking the active one.
    List,
    /// Show one persona's voice and category emphasis.
    Show { id: String },
    /// Make a persona active for this project.
    Activate { id: String },
    /// Scaffold a new persona HJSON file in the project to edit.
    New {
        id: String,
        #[arg(long)]
        name: Option<String>,
    },
}

/// WORLD-4 — the proposal queue surface (RFC §8.9). Accepting a proposal commits
/// the proposed record (a Place) to its system book; rejecting records the
/// decision so the compiler won't re-propose it.
#[derive(Debug, Subcommand)]
pub enum ProposalsCommand {
    /// List proposals, optionally filtered by status (pending / accepted / rejected).
    List {
        #[arg(long)]
        status: Option<String>,
    },
    /// Accept a proposal by id (creates the Place).
    Accept {
        id: String,
    },
    /// Reject a proposal by id (won't be re-proposed).
    Reject {
        id: String,
    },
    /// Accept every pending proposal.
    AcceptAll,
    /// Drop all still-pending proposals.
    Clear,
}

#[derive(Debug, Subcommand)]
pub enum OutputCommand {
    /// List active Output messages.
    Show {
        /// Only this kind.
        #[arg(long)]
        kind: Option<String>,
        /// Only this severity (info | warning | contradiction | progress).
        #[arg(long)]
        severity: Option<String>,
        /// Cap how many to show.
        #[arg(long)]
        limit: Option<usize>,
        /// Emit JSON instead of the formatted display.
        #[arg(long)]
        json: bool,
    },
    /// Emit a message (for testing / scripting).
    Emit {
        /// Message kind (e.g. `bund_print`).
        kind: String,
        /// Kind-specific metadata as a JSON object.
        #[arg(long, default_value = "{}")]
        metadata: String,
        /// Severity (info | warning | contradiction | progress).
        #[arg(long, default_value = "info")]
        severity: String,
    },
    /// Dismiss a message by id.
    Dismiss {
        /// The message UUID.
        id: String,
    },
    /// Clear messages — a kind, or everything with `--all`.
    Clear {
        /// Only this kind.
        #[arg(long)]
        kind: Option<String>,
        /// Dismiss all active messages.
        #[arg(long)]
        all: bool,
    },
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
    /// TDOC-4 — a self-contained HTML static site (a directory).
    /// Needs `--output <dir>`.
    Html,
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

/// Does this command mutate the project's node store? Used to decide whether
/// to take the multi-writer advisory lock. Conservative — the structural
/// writers and the store-creating importers; read-only reports are excluded so
/// they never warn while a TUI is open.
fn command_mutates(command: &Command) -> bool {
    matches!(
        command,
        Command::Add { .. }
            | Command::Delete { .. }
            | Command::Mv { .. }
            | Command::Paragraph(_)
            | Command::Reindex { .. }
            | Command::Replace { .. }
            | Command::ImportScrivener { .. }
            | Command::ImportEpub { .. }
            | Command::ImportHelp { .. }
            | Command::ImportTypstHelp
            | Command::Recover { .. }
            | Command::Event(EventCommand::Add { .. })
            | Command::Sources(SourcesCommand::Import { .. })
            | Command::Thread(ThreadCommand::Add { .. })
            | Command::Language(
                LanguageCommand::Init { .. }
                    | LanguageCommand::AddWord { .. }
                    | LanguageCommand::RemoveWord { .. }
                    | LanguageCommand::Import { .. }
            )
    )
}

/// Take the project advisory lock for a store-mutating CLI command, mirroring
/// the TUI's policy (`project_lock.enabled` / `on_conflict`). Returns the held
/// lock (kept alive for the command), or `None` when locking doesn't apply
/// (non-mutating command, not an initialized project, lock disabled, or already
/// busy and we proceed permissively). Errors only when `on_conflict = refuse`.
fn maybe_lock_for_cli(
    project: &std::path::Path,
    command: &Command,
) -> Result<Option<crate::project_lock::ProjectLock>> {
    use crate::project_lock::{acquire, LockOutcome};
    if !command_mutates(command) {
        return Ok(None);
    }
    let layout = crate::project::ProjectLayout::new(project);
    if layout.require_initialized().is_err() {
        return Ok(None); // not a project — the command reports that on its own terms
    }
    let cfg = match crate::config::Config::load_layered(&layout.config_path()) {
        Ok(c) => c,
        Err(_) => return Ok(None),
    };
    if !cfg.project_lock.enabled {
        return Ok(None);
    }
    match acquire(&layout.root) {
        Ok(LockOutcome::Acquired(lock)) => Ok(Some(lock)),
        Ok(LockOutcome::Busy(info)) => {
            eprintln!(
                "⚠ Another inkhaven session may already have this project open ({}).",
                info.describe()
            );
            eprintln!("  Concurrent writes can corrupt the project store.");
            if cfg.project_lock.on_conflict == "refuse" {
                anyhow::bail!(
                    "refusing to run: project is locked by another session (project_lock.on_conflict = refuse)"
                );
            }
            eprintln!("  Proceeding anyway.");
            Ok(None)
        }
        // Locking unsupported on this filesystem — proceed permissively.
        Err(_) => Ok(None),
    }
}

impl Cli {
    pub fn run(self) -> Result<()> {
        let project = self
            .project
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        let command = self.command.unwrap_or(Command::Tui);

        // Multi-writer safety (1.6.0): a store-mutating CLI command run while a
        // TUI — or another CLI writer — holds the project is a lost-update risk.
        // Consult the same advisory lock the TUI uses: inform (never hard-block
        // unless `project_lock.on_conflict = refuse`) and hold it for the
        // command's duration so concurrent CLI writers serialise. `_lock` lives
        // until `run` returns.
        let _lock = maybe_lock_for_cli(&project, &command)?;

        match command {
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
            Command::Outline { filter } => {
                outline::run(&project, filter.as_deref()).map_err(Into::into)
            }
            Command::Paragraph(cmd) => match cmd {
                ParagraphCommand::Copy { src, dest } => {
                    paragraph::copy(&project, &src, &dest).map_err(Into::into)
                }
                ParagraphCommand::Move { src, dest } => {
                    paragraph::move_(&project, &src, &dest).map_err(Into::into)
                }
            },
            Command::Search { query, limit } => {
                search::run(&project, &query, limit).map_err(Into::into)
            }
            Command::BookRag(cmd) => match cmd {
                BookRagCommand::Retrieve {
                    query,
                    book_name,
                    top_k,
                    context,
                } => book_rag::retrieve(
                    &project,
                    &query,
                    book_name.as_deref(),
                    top_k,
                    context,
                )
                .map_err(Into::into),
            },
            Command::Reindex { prune, adopt } => {
                reindex::run(&project, prune, adopt).map_err(Into::into)
            }
            Command::Export {
                format,
                output,
                book_name,
                status,
                tag,
                profiles,
                templates,
                eject_templates,
                blind,
                bundle,
            } => {
                // TDOC-3 — parse `--profile dim=value` pairs.
                let profile_pairs: Vec<(String, String)> = profiles
                    .iter()
                    .filter_map(|p| p.split_once('=').map(|(k, v)| (k.trim().to_string(), v.trim().to_string())))
                    .collect();
                export::run(
                    &project,
                    format,
                    output.as_deref(),
                    book_name.as_deref(),
                    status.as_deref(),
                    tag.as_deref(),
                    &profile_pairs,
                    templates.as_deref(),
                    eject_templates.as_deref(),
                    blind,
                    bundle.as_deref(),
                )
                .map_err(Into::into)
            }
            Command::Index { book_name, format, out } => {
                book_index::run(&project, book_name.as_deref(), &format, out.as_deref())
                    .map_err(Into::into)
            }
            Command::IndexLocorum { book_name, format, out, strict } => {
                index_locorum::run(&project, book_name.as_deref(), &format, out.as_deref(), strict)
                    .map_err(Into::into)
            }
            Command::IndexVerborum { book_name, format, out } => {
                index_verborum::run(&project, book_name.as_deref(), &format, out.as_deref())
                    .map_err(Into::into)
            }
            Command::Argue { book_name, provider, json } => {
                argue::run(&project, book_name.as_deref(), provider.as_deref(), json)
                    .map_err(Into::into)
            }
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
            Command::ImportEpub {
                epub_path,
                book_name,
                dry_run,
            } => import_epub::run(&project, &epub_path, book_name.as_deref(), dry_run)
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
            Command::Style { book_name, language, json } => {
                style::run(&project, book_name, language, json).map_err(Into::into)
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
            Command::Research {
                thread,
                list_threads,
                export_thread,
                format,
                out,
                import,
                sync,
                batch,
                auto_confirm,
                confidence,
                bibliography,
                gutenberg,
                archive,
                wikisource,
                bible,
                quran,
                bookofmormon,
                contradict,
                converge,
                socrates,
                report,
            } => crate::research::run(
                &project,
                crate::research::ResearchInvocation {
                    thread,
                    list_threads,
                    export_thread,
                    format,
                    out,
                    import,
                    sync,
                    batch,
                    auto_confirm,
                    confidence,
                    bibliography,
                    gutenberg,
                    archive,
                    wikisource,
                    bible,
                    quran,
                    bookofmormon,
                    contradict,
                    converge,
                    socrates,
                    report,
                },
            )
            .map_err(Into::into),
            Command::Linguistic { language, session } => crate::linguistic::run(
                &project,
                crate::linguistic::LinguisticInvocation { language, session },
            )
            .map_err(Into::into),
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
            Command::Output(cmd) => output::run(&project, cmd).map_err(Into::into),
            Command::Realworld(cmd) => realworld::run(&project, cmd).map_err(Into::into),
            Command::InnerSocrates(cmd) => inner_socrates::run(&project, cmd).map_err(Into::into),
            Command::InnerEditor(cmd) => inner_editor::run(&project, cmd).map_err(Into::into),
            Command::Companions => companions::run(&project).map_err(Into::into),
            Command::Check { paragraph, book_name, no_fact, no_socrates, no_timeline } => {
                check::run(
                    &project,
                    paragraph.as_deref(),
                    book_name.as_deref(),
                    no_fact,
                    no_socrates,
                    no_timeline,
                )
                .map_err(Into::into)
            }
            Command::Cost => cost::run(&project).map_err(Into::into),
            Command::Goals => goals::run(&project).map_err(Into::into),
            Command::FactCheck { text, paragraph, slow, max_cost, force, timeline_aware, timeline_only } => {
                realworld::fact_check(&project, text, paragraph, slow, max_cost, force, &timeline_aware, timeline_only)
                    .map_err(Into::into)
            }
            Command::Thread(cmd) => {
                thread::run(&project, cmd).map_err(Into::into)
            }
            Command::Terms(cmd) => {
                terms::run(&project, cmd).map_err(Into::into)
            }
            Command::Snippets(cmd) => {
                snippets::run(&project, cmd).map_err(Into::into)
            }
            Command::Dialogue(cmd) => {
                dialogue::run(&project, cmd).map_err(Into::into)
            }
            Command::Prose(cmd) => {
                prose::run(&project, cmd).map_err(Into::into)
            }
            Command::Sources(cmd) => {
                sources::run(&project, cmd).map_err(Into::into)
            }
            Command::Docs(cmd) => docs::run(&project, cmd).map_err(Into::into),
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
            Command::Drift(cmd) => drift::run(&project, cmd).map_err(Into::into),
            Command::Lang(cmd) => lang::run(&project, cmd).map_err(Into::into),
            Command::Character(cmd) => character::run(&project, cmd).map_err(Into::into),
            Command::Theologian(cmd) => theologian::run(&project, cmd).map_err(Into::into),
            Command::Rigor(cmd) => rigor::run(&project, cmd).map_err(Into::into),
            Command::Lexicon(cmd) => lexicon::run(&project, cmd).map_err(Into::into),
            Command::Myth(cmd) => myth::run(&project, cmd).map_err(Into::into),
            Command::World { json, deep, provider, entity, sub } => match sub {
                Some(cmd) => utopia::run(&project, cmd).map_err(Into::into),
                None => world::run(&project, json, deep, provider.as_deref(), entity.as_deref())
                    .map_err(Into::into),
            },
            Command::Pdf(cmd) => pdf::run(cmd, &project).map_err(Into::into),
            Command::Replace {
                pattern,
                replacement,
                regex,
                substring,
                ignore_case,
                book,
                include_system,
                dry_run,
                yes,
            } => replace::run(
                &project,
                &pattern,
                &replacement,
                regex,
                substring,
                ignore_case,
                book.as_deref(),
                include_system,
                dry_run,
                yes,
            )
            .map_err(Into::into),
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
            Command::Docx {
                book_name,
                output,
                title,
                author,
                contact,
                font,
            } => docx::run(
                &project,
                book_name.as_deref(),
                output.as_deref(),
                title.as_deref(),
                author.as_deref(),
                contact.as_deref(),
                font.as_deref(),
            )
            .map_err(Into::into),
            Command::Submissions(cmd) => {
                submissions::run(&project, cmd).map_err(Into::into)
            }
            Command::Submission(cmd) => {
                submission::run(&project, cmd).map_err(Into::into)
            }
            Command::Plan(cmd) => {
                plan::run(&project, cmd).map_err(Into::into)
            }
            Command::Edit { json, only, book_name, show_deferred, deep, provider } => {
                editorial::run(
                    &project,
                    json,
                    only,
                    book_name.as_deref(),
                    show_deferred,
                    deep,
                    provider.as_deref(),
                )
                .map_err(Into::into)
            }
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

#[cfg(test)]
mod prompt_resolve_tests {
    use super::strip_typst_heading;

    #[test]
    fn strips_a_single_leading_heading() {
        assert_eq!(
            strip_typst_heading("= Plan Analyze\n\nYou are an editor.\nBe terse."),
            "You are an editor.\nBe terse."
        );
        // no heading → unchanged (trimmed)
        assert_eq!(strip_typst_heading("Just a prompt.\n"), "Just a prompt.");
        // only the FIRST heading is dropped
        assert_eq!(strip_typst_heading("= Title\nbody = x"), "body = x");
    }
}
