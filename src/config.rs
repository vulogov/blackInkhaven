use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const DEFAULT_PROJECT_CONFIG: &str = include_str!("../assets/default_project.hjson");
pub const DEFAULT_PROMPTS: &str = include_str!("../assets/default_prompts.hjson");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub embeddings: EmbeddingsConfig,
    #[serde(default)]
    pub llm: LlmConfig,
    #[serde(default)]
    pub editor: EditorConfig,
    #[serde(default)]
    pub keys: KeyBindings,
    #[serde(default)]
    pub hierarchy: HierarchyConfig,
    #[serde(default)]
    pub theme: ThemeConfig,
    #[serde(default)]
    pub backup: BackupConfig,
    #[serde(default)]
    pub sound: SoundConfig,
    #[serde(default)]
    pub typst_templates: TypstTemplatesConfig,
    #[serde(default)]
    pub typst_compile: TypstCompileConfig,
    #[serde(default)]
    pub typst_page: TypstPageConfig,
    #[serde(default)]
    pub typst_fonts: TypstFontsConfig,
    #[serde(default)]
    pub typst_layout: TypstLayoutConfig,
    #[serde(default)]
    pub images: ImagesConfig,
    /// Multi-format export configuration — drives the Ctrl+B O
    /// extra-format pipeline. CLI `inkhaven export <fmt>` uses
    /// the same converters but ignores this list (it picks one
    /// format explicitly).
    #[serde(default)]
    pub output: OutputConfig,
    /// Writing-progress goals. Feeds the status-bar widget and
    /// the Ctrl+V G progress modal. Empty defaults disable goals
    /// + targets but still record events so the modal has data
    /// to show.
    #[serde(default)]
    pub goals: GoalsConfig,
    /// 1.2.6+ — AI-pane behaviour knobs that aren't tied to a
    /// specific provider (per-paragraph memory, future
    /// turn-history overrides, etc).
    #[serde(default)]
    pub ai: AiConfig,
    /// 1.2.6+ — story timeline configuration. Disabled by
    /// default; set `timeline.enabled: true` plus a calendar
    /// preset to turn on event tracking. See
    /// `crate::timeline::calendar::CalendarConfig`.
    #[serde(default)]
    pub timeline: TimelineConfig,
    /// Bund scripting sandbox policy. Defaults deny destructive
    /// categories (fs_write, net, shell, code_eval); writers opt
    /// in by listing the categories or words they want to allow.
    /// See `src/scripting/policy.rs`.
    #[serde(default)]
    pub scripting: crate::scripting::policy::Policy,
    /// Primary writing language of the project. Drives:
    /// * Snowball stemmers for the editor's Places/Characters highlight
    ///   overlay (overrides `editor.stemming.languages` when non-empty).
    /// * The default F7 grammar-check prompt's grammar rules.
    ///
    /// Accepts any name handled by `parse_stemmer_language` (`english`,
    /// `russian`, `french`, …). Empty string falls back to
    /// `editor.stemming.languages`.
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_prompts_path")]
    pub prompts_file: PathBuf,
    /// Where per-book artefacts (rendered PDFs, build intermediates, …)
    /// land. Each new book gets its own subdirectory under here. Created
    /// on project open if missing. Relative paths resolve against the
    /// project root; absolute paths are used verbatim.
    #[serde(default = "default_artefacts_directory")]
    pub artefacts_directory: String,
    /// Seconds between background calls to `Store::sync()`, which
    /// flushes the HNSW vector index to disk. Acts as a safety net —
    /// every explicit mutation in `src/store/` already calls
    /// `sync()` on its own. The tick is cheap when the index is
    /// clean (dirty-flag short-circuit), so the default cadence is
    /// generous. `0` disables the background task entirely.
    #[serde(default = "default_sync_interval")]
    pub sync_interval_seconds: u64,
}

fn default_view_prefix() -> String {
    "Ctrl+v".into()
}

fn default_sync_interval() -> u64 {
    600
}

fn default_prompts_path() -> PathBuf {
    PathBuf::from("prompts.hjson")
}

fn default_language() -> String {
    "english".into()
}

fn default_artefacts_directory() -> String {
    // Empty string → resolved at runtime to the OS per-user cache
    // directory (`<cache_dir>/inkhaven/artefacts/<project-basename>/`).
    // Build artefacts are ephemeral; keeping them outside the project
    // tree means `git status` / backups / shell tab completion don't
    // see them.
    String::new()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            embeddings: EmbeddingsConfig::default(),
            llm: LlmConfig::default(),
            editor: EditorConfig::default(),
            keys: KeyBindings::default(),
            hierarchy: HierarchyConfig::default(),
            theme: ThemeConfig::default(),
            backup: BackupConfig::default(),
            sound: SoundConfig::default(),
            typst_templates: TypstTemplatesConfig::default(),
            typst_compile: TypstCompileConfig::default(),
            typst_page: TypstPageConfig::default(),
            typst_fonts: TypstFontsConfig::default(),
            typst_layout: TypstLayoutConfig::default(),
            images: ImagesConfig::default(),
            output: OutputConfig::default(),
            goals: GoalsConfig::default(),
            ai: AiConfig::default(),
            timeline: TimelineConfig::default(),
            scripting: crate::scripting::policy::Policy::default(),
            language: default_language(),
            prompts_file: default_prompts_path(),
            artefacts_directory: default_artefacts_directory(),
            sync_interval_seconds: default_sync_interval(),
        }
    }
}

/// Where backups land and how often the TUI should make one on exit. Empty
/// `out_dir` disables auto-backup (manual `inkhaven backup` still works);
/// `max_age = "0s"` (or unset) means "never auto-trigger".
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BackupConfig {
    /// Directory where `.zip` snapshots are written. May be a relative path
    /// (resolved against the project root) or absolute. Created if missing.
    pub out_dir: String,
    /// Maximum age of the last backup before the TUI's exit hook creates a
    /// fresh one. Parsed via the `humantime` crate, so values like `"7d"`,
    /// `"24h"`, `"30m"` are all accepted. Empty string or `"0s"` disables.
    #[serde(with = "humantime_serde")]
    pub max_age: std::time::Duration,
    /// 1.2.6+: when a backup finishes — either the manual Ctrl+B B
    /// chord or the exit-hook auto-backup — hold the splash on
    /// screen with a "Press any key to continue…" prompt so the
    /// user can read the result before the TUI dismisses it.
    /// Default true. Set false to keep the auto-dismiss behaviour
    /// from 1.2.5 and earlier.
    #[serde(default = "default_backup_wait_for_key")]
    pub wait_for_key_after_backup: bool,
}

fn default_backup_wait_for_key() -> bool {
    true
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            // Empty string → use the OS per-user data directory
            // (`<data_dir>/inkhaven/backups/<project-basename>/`). Set
            // to an explicit path to override — see
            // `Store::resolve_backup_dir`. Keeping backups out of the
            // project tree by default avoids "snapshot contains itself"
            // recursion.
            out_dir: String::new(),
            // Roughly a week. Vladimir's books move fast enough that a
            // weekly snapshot pairs sensibly with the per-paragraph
            // snapshots the editor already supports.
            max_age: std::time::Duration::from_secs(7 * 24 * 3600),
            wait_for_key_after_backup: default_backup_wait_for_key(),
        }
    }
}

/// Typewriter sound effects (Enter key, focus-out). Synthesised at
/// runtime — no audio assets needed. `enabled` is toggled live with
/// Ctrl+B E; the chord rewrites this stanza in place so the choice
/// survives the next launch.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SoundConfig {
    pub enabled: bool,
    /// Master volume 0.0–1.0 applied uniformly to every synthesised
    /// sample. Clamped at load time.
    pub volume: f32,
}

impl Default for SoundConfig {
    fn default() -> Self {
        Self {
            // Default off so new users aren't surprised by audio at
            // launch. Ctrl+B E opts in once they're settled.
            enabled: false,
            volume: 0.6,
        }
    }
}

/// Typst function templates used during Book assembly (Ctrl+B A).
/// Each field is the raw Typst source code for a wrap function — they
/// get inlined verbatim into the per-book `globals.typ` paragraph the
/// first time a user book is created. Customise them to taste; the
/// shipped defaults are minimal "show content as-is with a heading"
/// wrappers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TypstTemplatesConfig {
    pub wrap_book: String,
    pub wrap_chapter: String,
    pub wrap_subchapter: String,
    pub wrap_paragraph: String,
    /// Frontispiece-style: page break + full-page centered image,
    /// optional caption. Called for Image nodes whose parent is a
    /// Book.
    pub wrap_image_book: String,
    /// Chapter-art: page break + 80%-width image + caption. Called
    /// for Image nodes whose parent is a Chapter.
    pub wrap_image_chapter: String,
    /// Smaller centered image + caption. Called for Image nodes
    /// whose parent is a Subchapter.
    pub wrap_image_subchapter: String,
    /// `figure(image(...), caption: ...)`. Not called by the
    /// assembler (Image nodes never sit under a Paragraph) but
    /// available as a regular function for users to call by hand
    /// from paragraph text.
    pub wrap_image_inline: String,
}

impl Default for TypstTemplatesConfig {
    fn default() -> Self {
        Self {
            wrap_book: default_wrap_book().into(),
            wrap_chapter: default_wrap_chapter().into(),
            wrap_subchapter: default_wrap_subchapter().into(),
            wrap_paragraph: default_wrap_paragraph().into(),
            wrap_image_book: default_wrap_image_book().into(),
            wrap_image_chapter: default_wrap_image_chapter().into(),
            wrap_image_subchapter: default_wrap_image_subchapter().into(),
            wrap_image_inline: default_wrap_image_inline().into(),
        }
    }
}

/// Baked-in defaults for the four wrap functions. Used both for
/// `TypstTemplatesConfig::default()` and as a fallback in the Book
/// assembly procedure when the HJSON entry is empty / missing.
pub fn default_wrap_book() -> &'static str {
    "#let wrap_book(body) = {\n  body\n}\n"
}
pub fn default_wrap_chapter() -> &'static str {
    "#let wrap_chapter(title, body) = {\n  heading(level: 1, title)\n  body\n}\n"
}
pub fn default_wrap_subchapter() -> &'static str {
    "#let wrap_subchapter(title, body) = {\n  heading(level: 2, title)\n  body\n}\n"
}
pub fn default_wrap_paragraph() -> &'static str {
    "#let wrap_paragraph(body) = {\n  body\n  parbreak()\n}\n"
}

pub fn default_wrap_image_book() -> &'static str {
    "// Frontispiece — Image directly under a Book.\n\
     #let wrap_image_book(path, title, caption, alt: none) = {\n\
     \u{20}\u{20}pagebreak(weak: true)\n\
     \u{20}\u{20}align(center + horizon, image(path, alt: alt, width: 90%))\n\
     \u{20}\u{20}if caption != none [#align(center)[#emph(caption)]]\n\
     \u{20}\u{20}pagebreak(weak: true)\n\
     }\n"
}

pub fn default_wrap_image_chapter() -> &'static str {
    "// Chapter-art — Image directly under a Chapter.\n\
     #let wrap_image_chapter(path, title, caption, alt: none) = {\n\
     \u{20}\u{20}pagebreak(weak: true)\n\
     \u{20}\u{20}align(center, image(path, alt: alt, width: 80%))\n\
     \u{20}\u{20}if caption != none [#align(center)[#emph(caption)]]\n\
     }\n"
}

pub fn default_wrap_image_subchapter() -> &'static str {
    "// Section image — Image directly under a Subchapter.\n\
     #let wrap_image_subchapter(path, title, caption, alt: none) = {\n\
     \u{20}\u{20}align(center, image(path, alt: alt, width: 60%))\n\
     \u{20}\u{20}if caption != none [#align(center)[#emph(caption)]]\n\
     }\n"
}

pub fn default_wrap_image_inline() -> &'static str {
    "// Inline figure — call from paragraph text with #wrap_image_inline(...).\n\
     #let wrap_image_inline(path, title, caption, alt: none) = figure(\n\
     \u{20}\u{20}image(path, alt: alt, width: 80%),\n\
     \u{20}\u{20}caption: caption,\n\
     )\n"
}

impl TypstTemplatesConfig {
    /// Per-template fallback to the shipped default when the user has
    /// emptied the HJSON entry. Returns owned strings so callers can
    /// stitch them into a `globals.typ` file without worrying about
    /// lifetimes.
    pub fn resolved_wrap_book(&self) -> String {
        if self.wrap_book.trim().is_empty() {
            default_wrap_book().into()
        } else {
            self.wrap_book.clone()
        }
    }
    pub fn resolved_wrap_chapter(&self) -> String {
        if self.wrap_chapter.trim().is_empty() {
            default_wrap_chapter().into()
        } else {
            self.wrap_chapter.clone()
        }
    }
    pub fn resolved_wrap_subchapter(&self) -> String {
        if self.wrap_subchapter.trim().is_empty() {
            default_wrap_subchapter().into()
        } else {
            self.wrap_subchapter.clone()
        }
    }
    pub fn resolved_wrap_paragraph(&self) -> String {
        if self.wrap_paragraph.trim().is_empty() {
            default_wrap_paragraph().into()
        } else {
            self.wrap_paragraph.clone()
        }
    }
    pub fn resolved_wrap_image_book(&self) -> String {
        if self.wrap_image_book.trim().is_empty() {
            default_wrap_image_book().into()
        } else {
            self.wrap_image_book.clone()
        }
    }
    pub fn resolved_wrap_image_chapter(&self) -> String {
        if self.wrap_image_chapter.trim().is_empty() {
            default_wrap_image_chapter().into()
        } else {
            self.wrap_image_chapter.clone()
        }
    }
    pub fn resolved_wrap_image_subchapter(&self) -> String {
        if self.wrap_image_subchapter.trim().is_empty() {
            default_wrap_image_subchapter().into()
        } else {
            self.wrap_image_subchapter.clone()
        }
    }
    pub fn resolved_wrap_image_inline(&self) -> String {
        if self.wrap_image_inline.trim().is_empty() {
            default_wrap_image_inline().into()
        } else {
            self.wrap_image_inline.clone()
        }
    }

    /// Concatenated body for the per-book `globals.typ` paragraph:
    /// the editor-chrome heading line, a brief comment header, then
    /// the eight wrap_* functions (four for prose-level wrappers,
    /// four for image-level wrappers).
    pub fn globals_typ_body(&self) -> String {
        let mut out = String::new();
        out.push_str("= globals.typ\n\n");
        out.push_str(
            "// Wrap functions used by inkhaven's `Book assembly` (Ctrl+B A).\n\
             // Each node in the manuscript tree is fed through the matching\n\
             // wrap_* call when the assembler synthesises index.typ files.\n\
             // Customise to taste — page breaks, headings, fonts, layout.\n\n",
        );
        out.push_str("// ---- Prose wrappers ----\n");
        out.push_str(&self.resolved_wrap_book());
        out.push('\n');
        out.push_str(&self.resolved_wrap_chapter());
        out.push('\n');
        out.push_str(&self.resolved_wrap_subchapter());
        out.push('\n');
        out.push_str(&self.resolved_wrap_paragraph());
        out.push_str("\n// ---- Image wrappers ----\n");
        out.push_str(&self.resolved_wrap_image_book());
        out.push('\n');
        out.push_str(&self.resolved_wrap_image_chapter());
        out.push('\n');
        out.push_str(&self.resolved_wrap_image_subchapter());
        out.push('\n');
        out.push_str(&self.resolved_wrap_image_inline());
        out
    }
}

/// Behaviour of the `typst compile` step driven by Ctrl+B B / Ctrl+B O,
/// plus the typst-as-library knobs added in 1.2.5. The stanza is its
/// own struct so new knobs (timeouts, custom typst path, extra args)
/// can land without breaking serde compatibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TypstCompileConfig {
    /// System prompt fed to the AI when `typst compile` returns
    /// non-zero. Empty → falls back to the baked-in default.
    pub error_system_prompt: String,
    /// Which engine drives Ctrl+B B / Ctrl+B O (the user-visible
    /// "Take the book → PDF" path).
    ///
    /// * `"external"` (default) — spawn the host's `typst` binary as
    ///   a child process. Pure shell-out, smallest binary footprint,
    ///   output exactly matches what the user gets typing
    ///   `typst compile` themselves.
    /// * `"inprocess"` — run the in-process typst compiler. Not yet
    ///   wired up in 1.2.5; the value is accepted today so HJSON
    ///   configs written now survive when the engine lands. Falls
    ///   back to `external` at runtime when the in-process engine
    ///   isn't compiled in.
    ///
    /// See the typst-as-library Phase plan in `Documentation/`.
    pub engine: String,
    /// Run `typst-syntax` against the open buffer on idle / save
    /// and surface parse errors in the status bar (1.2.5+). Pure
    /// parser — no eval, layout, render, fonts, or package
    /// resolution. Adds no shell-out and is independent of which
    /// `engine` is selected for PDF builds.
    pub diagnostics: bool,
    /// Minimum seconds of editor idle time before a diagnostics
    /// re-check runs. Same units as `editor.autosave_seconds` and
    /// piggy-backs on the same idle clock — set to `0` to check
    /// on every keystroke (cheap on small buffers; can stutter on
    /// chapter-sized pastes).
    pub diagnostics_idle_seconds: u64,
    /// 1.2.5+: when `engine = "inprocess"`, upgrade the idle /
    /// save diagnostic check from `typst-syntax` (parse only) to
    /// a full `typst::compile` against the open paragraph in
    /// isolation. Surfaces semantic errors (undefined functions,
    /// type errors, missing fonts) the parser can't catch. Costs
    /// 10–200 ms per check. **False positives are expected** when
    /// the paragraph references book-level definitions from the
    /// assembled preamble — turn off if your manuscript uses
    /// custom `#show` rules. Has no effect when
    /// `engine = "external"`.
    pub semantic_diagnostics: bool,
    /// 1.2.5+: ship Computer Modern and Linux Libertine inside
    /// the inkhaven binary so the in-process engine can lay out
    /// even on hosts without system fonts. Adds ~10 MB; turn off
    /// if you're confident every host inkhaven runs on has the
    /// fonts your manuscript needs. No effect when
    /// `engine = "external"`.
    pub bundle_fonts: bool,
    /// 1.2.5+: also search the host's system fonts via fontdb.
    /// On by default — most users want both their installed
    /// fonts AND the embedded fallback set. Turn off for
    /// reproducible builds where the only allowed fonts are the
    /// embedded ones. No effect when `engine = "external"`.
    pub use_system_fonts: bool,
    /// 1.2.5+: when the in-process engine sees `@preview/<pkg>`
    /// (or any non-local package id), use `typst-kit`'s
    /// `PackageStorage` to fetch and unpack it from
    /// packages.typst.org. Cached on disk in the platform's
    /// standard cache directory (`~/Library/Caches/typst/packages`
    /// on macOS, `~/.cache/typst/packages` on Linux,
    /// `%LOCALAPPDATA%\typst\packages` on Windows). Turn off to
    /// fail-fast on package imports — useful for hermetic
    /// builds. No effect when `engine = "external"`.
    pub packages_enabled: bool,
    /// 1.2.6+: when the typst compile splash (Ctrl+B B / Ctrl+B O)
    /// finishes, hold the splash on screen with a
    /// "Press any key to continue…" prompt instead of jumping
    /// straight back to the editor. Lets the user read the
    /// "Build OK / failed" line before the splash disappears.
    /// Cancelled compiles (Esc) skip the wait. Default true.
    #[serde(default = "default_wait_for_key_after_compile")]
    pub wait_for_key_after_compile: bool,
}

fn default_wait_for_key_after_compile() -> bool {
    true
}

impl Default for TypstCompileConfig {
    fn default() -> Self {
        Self {
            error_system_prompt: String::new(),
            engine: "external".to_owned(),
            diagnostics: true,
            diagnostics_idle_seconds: 2,
            semantic_diagnostics: false,
            bundle_fonts: true,
            use_system_fonts: true,
            packages_enabled: true,
            wait_for_key_after_compile: default_wait_for_key_after_compile(),
        }
    }
}

impl TypstCompileConfig {
    pub fn resolved_error_system_prompt(&self) -> String {
        if self.error_system_prompt.trim().is_empty() {
            default_typst_error_system_prompt().into()
        } else {
            self.error_system_prompt.clone()
        }
    }

    /// True when the user has asked for the in-process engine. The
    /// in-process compiler stack (typst + typst-pdf + typst-kit
    /// fonts) is always linked in 1.2.5+; the user opts in by
    /// setting `typst_compile.engine = "inprocess"` in
    /// `inkhaven.hjson`. Anything else falls back to the external
    /// `typst` binary on PATH.
    pub fn use_inprocess_engine(&self) -> bool {
        self.engine.eq_ignore_ascii_case("inprocess")
    }
}

/// Settings for Image nodes (book art / chapter art / inline figures).
/// `preview_enabled` toggles the ratatui-image preview that pops on
/// Enter — flip it off on slow ssh sessions or terminals where the
/// half-block fallback is too noisy. The two size knobs guard against
/// accidental imports of huge files.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ImagesConfig {
    pub preview_enabled: bool,
    pub allowed_extensions: Vec<String>,
    pub max_size_bytes: u64,
}

impl Default for ImagesConfig {
    fn default() -> Self {
        Self {
            preview_enabled: true,
            allowed_extensions: vec![
                "png".into(),
                "jpg".into(),
                "jpeg".into(),
                "gif".into(),
                "webp".into(),
                "svg".into(),
            ],
            // 32 MiB cap — generous for literary cover art, small
            // enough that a misclicked drag of a 200-MB raw scan
            // gets rejected with a clear status message.
            max_size_bytes: 32 * 1024 * 1024,
        }
    }
}

/// Page geometry — fed into `#set page(...)` in the synthesised
/// `settings.typ`. Empty / zero / `"default"` values fall through to
/// typst's own defaults so a user who doesn't touch HJSON still gets
/// a working compile.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TypstPageConfig {
    /// `"us-letter"`, `"a4"`, `"a5"`, etc. — anything typst's `paper:`
    /// argument accepts. Empty = typst default.
    pub paper: String,
    pub margin_top: String,
    pub margin_bottom: String,
    /// Inside / outside replace left / right when typesetting two-
    /// sided books. Typst handles the binding-edge swap automatically
    /// when `inside` / `outside` are used.
    pub margin_inside: String,
    pub margin_outside: String,
    /// Page-number format — `"1"`, `"i"`, `"1 of 1"`. Empty = no
    /// page numbers (typst default).
    pub page_numbering: String,
    /// Single-column documents: 1. Multi-column: 2+. 0 / 1 both fall
    /// through to typst's single-column default.
    pub columns: u32,
}

impl Default for TypstPageConfig {
    fn default() -> Self {
        Self {
            paper: "us-letter".into(),
            margin_top: "2.5cm".into(),
            margin_bottom: "2.5cm".into(),
            margin_inside: "3cm".into(),
            margin_outside: "2cm".into(),
            page_numbering: "1".into(),
            columns: 1,
        }
    }
}

/// `#set text(...)` and language. Empty body / monospace strings let
/// typst pick its bundled defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TypstFontsConfig {
    pub body: String,
    pub body_size: String,
    pub monospace: String,
    /// Two-letter language tag fed to `#set text(lang: ...)`. Drives
    /// typst's hyphenation / smart-quote behaviour.
    pub language: String,
}

impl Default for TypstFontsConfig {
    fn default() -> Self {
        // 1.2.6: defaults are typst's own bundled fonts so the
        // shipped HJSON compiles cleanly on a vanilla host with
        // no extra font installs. Override in HJSON to taste —
        // see `synthesised_settings_typ_header` which always
        // emits a fallback list ending in the bundled font, so
        // a custom name that isn't installed still compiles.
        Self {
            body: "Linux Libertine".into(),
            body_size: "11pt".into(),
            monospace: "DejaVu Sans Mono".into(),
            language: "en".into(),
        }
    }
}

/// Names that ship with typst's own embedded font set — used as
/// the trailing fallback in `#set text(font: ...)` /
/// `#set raw(font: ...)`. Listed bare so the unit tests can match
/// them; consider these the "sure-way" fonts that are present
/// even when the host has no system fonts at all.
const BUNDLED_BODY_FONT: &str = "Linux Libertine";
const BUNDLED_MONO_FONT: &str = "DejaVu Sans Mono";

/// Build the Typst literal for a `font:` argument. When `primary`
/// already matches the bundled fallback, emit the plain string
/// form `"X"`; otherwise emit the array form `("X", "Y")` so a
/// missing primary font falls back to the bundled one instead of
/// erroring.
fn font_literal(primary: &str, fallback: &str) -> String {
    let primary = primary.trim();
    if primary.eq_ignore_ascii_case(fallback) {
        format!("\"{}\"", typst_escape(primary))
    } else {
        format!(
            "(\"{}\", \"{}\")",
            typst_escape(primary),
            typst_escape(fallback)
        )
    }
}

/// Paragraph + heading layout. Empty strings = typst default.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TypstLayoutConfig {
    pub justify: bool,
    pub leading: String,
    /// First-line indent for paragraphs. Empty = no indent.
    pub paragraph_indent: String,
    /// `#set heading(numbering: ...)` argument. `"1."` / `"1.1"` /
    /// `"I."`. Empty = unnumbered (typst default).
    pub heading_numbering: String,
}

impl Default for TypstLayoutConfig {
    fn default() -> Self {
        Self {
            justify: true,
            leading: "0.7em".into(),
            paragraph_indent: String::new(),
            heading_numbering: String::new(),
        }
    }
}

impl Config {
    /// Render the auto-generated header that `Book assembly` prepends
    /// to the synthesised `settings.typ`. Reflects the live values of
    /// `typst_page` / `typst_fonts` / `typst_layout`; the user's
    /// `Typst → <book> → settings.typ` paragraph content is appended
    /// below this header so free-form additions survive every
    /// regeneration.
    pub fn synthesised_settings_typ_header(&self) -> String {
        let mut out = String::new();
        out.push_str(
            "// ── inkhaven auto-generated · do not edit ────────────────\n\
             // Source: typst_page / typst_fonts / typst_layout in\n\
             // inkhaven.hjson. Change values there and re-run Ctrl+B A.\n\
             // Anything below the `User overrides` line below is your\n\
             // free-form paragraph content; preserved across rebuilds.\n\n",
        );

        // #set page(...)
        let p = &self.typst_page;
        if !p.paper.trim().is_empty() {
            let mut args: Vec<String> = Vec::new();
            args.push(format!("paper: \"{}\"", typst_escape(&p.paper)));
            let any_margin = !(p.margin_top.is_empty()
                && p.margin_bottom.is_empty()
                && p.margin_inside.is_empty()
                && p.margin_outside.is_empty());
            if any_margin {
                args.push(format!(
                    "margin: (top: {}, bottom: {}, inside: {}, outside: {})",
                    pad_or(&p.margin_top, "2.5cm"),
                    pad_or(&p.margin_bottom, "2.5cm"),
                    pad_or(&p.margin_inside, "3cm"),
                    pad_or(&p.margin_outside, "2cm"),
                ));
            }
            if !p.page_numbering.trim().is_empty() {
                args.push(format!(
                    "numbering: \"{}\"",
                    typst_escape(&p.page_numbering)
                ));
            }
            if p.columns > 1 {
                args.push(format!("columns: {}", p.columns));
            }
            out.push_str(&format!("#set page({})\n\n", args.join(", ")));
        }

        // #set text(...)
        // Body + monospace font args are emitted as a fallback list
        // (user pick, bundled font) so a missing primary survives.
        let f = &self.typst_fonts;
        let mut text_args: Vec<String> = Vec::new();
        if !f.body.trim().is_empty() {
            text_args.push(format!(
                "font: {}",
                font_literal(&f.body, BUNDLED_BODY_FONT)
            ));
        }
        if !f.body_size.trim().is_empty() {
            text_args.push(format!("size: {}", f.body_size));
        }
        if !f.language.trim().is_empty() {
            text_args.push(format!("lang: \"{}\"", typst_escape(&f.language)));
        }
        if !text_args.is_empty() {
            out.push_str(&format!("#set text({})\n\n", text_args.join(", ")));
        }
        // Raw / code typeface. Typst 0.11+ removed `font:` from the
        // `raw` element, so the only correct way to retarget the
        // monospace face is a `show raw: set text(font: …)` rule.
        // We also style inline raw spans so backticks pick up the
        // same font — `set text` inside a show-rule applies to both
        // block and inline raw.
        if !f.monospace.trim().is_empty() {
            out.push_str(&format!(
                "#show raw: set text(font: {})\n\n",
                font_literal(&f.monospace, BUNDLED_MONO_FONT)
            ));
        }

        // #set par(...) — justify, leading, first-line-indent
        let l = &self.typst_layout;
        let mut par_args: Vec<String> = Vec::new();
        par_args.push(format!("justify: {}", l.justify));
        if !l.leading.trim().is_empty() {
            par_args.push(format!("leading: {}", l.leading));
        }
        if !l.paragraph_indent.trim().is_empty() {
            par_args.push(format!("first-line-indent: {}", l.paragraph_indent));
        }
        out.push_str(&format!("#set par({})\n\n", par_args.join(", ")));

        // #set heading(numbering: ...)
        if !l.heading_numbering.trim().is_empty() {
            out.push_str(&format!(
                "#set heading(numbering: \"{}\")\n\n",
                typst_escape(&l.heading_numbering)
            ));
        }

        out.push_str(
            "// ── User overrides (your settings.typ paragraph below) ─────\n",
        );
        out
    }
}

/// Backslash-escape `\` and `"` so a user-supplied value can be
/// inlined into a Typst string literal without breaking the parser.
/// Strips newlines defensively — HJSON should never produce them in
/// these fields but the user might paste one in.
fn typst_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' | '\r' => out.push(' '),
            other => out.push(other),
        }
    }
    out
}

fn pad_or<'a>(v: &'a str, fallback: &'a str) -> &'a str {
    if v.trim().is_empty() { fallback } else { v }
}

pub fn default_typst_error_system_prompt() -> &'static str {
    "You are an expert Typst typesetter helping debug `typst compile` failures \
     for books assembled by inkhaven. Inkhaven generates a tree of `.typ` files:\n\
     - `<slug>.typ` — root, imports globals.typ + settings.typ, calls wrap_book(include \"book/index.typ\").\n\
     - `globals.typ` — defines wrap_book / wrap_chapter / wrap_subchapter / wrap_paragraph functions.\n\
     - `settings.typ` — document-wide #set / #show rules.\n\
     - `book/index.typ` — sequence of `#include` for chapters at markup scope.\n\
     - `book/<NN-chapter>/index.typ` — calls `#wrap_chapter(\"title\", { include … })` in code mode.\n\
     - `book/<NN-chapter>/<NN-paragraph>.typ` — the user's prose (leading `= title` stripped).\n\n\
     When you receive an error, walk through:\n\
     1. What the error means in plain language.\n\
     2. Which of the file categories above most likely caused it.\n\
     3. The smallest concrete fix the user can apply — either in their inkhaven \
        paragraph (via the editor) or in HJSON config (`typst_templates.wrap_*`).\n\n\
     Be concise. The user wants to ship a PDF, not a tutorial."
}

/// Visual theme for the TUI. Every field is a hex colour string (`#RRGGBB`),
/// or the empty string for "fall back to terminal default" (only meaningful
/// for background fields). Defaults form a Catppuccin Mocha-style dark theme;
/// see `assets/default_project.hjson` for a complete annotated example.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    // Pane backgrounds and foregrounds.
    pub pane_bg: String,
    pub pane_fg: String,
    pub line_number_fg: String,
    pub current_line_bg: String,

    // Pane borders (focused / unfocused / saved / dirty / read-only).
    pub border_focused: String,
    pub border_unfocused: String,
    pub border_dirty: String,
    pub border_saved: String,
    pub border_readonly: String,

    // Modal / floating windows.
    pub modal_bg: String,
    pub modal_border: String,
    pub modal_fg: String,

    // Lexicon highlights overlay.
    pub places_fg: String,
    pub characters_fg: String,
    pub artefacts_fg: String,
    pub notes_underline_fg: String,

    // Search-match overlay in the editor.
    pub search_match_bg: String,
    pub search_current_bg: String,

    // Tree pane chrome.
    pub tree_open_marker: String,
    // Per-kind row colour in the Tree pane. The row title (book /
    // chapter / etc.) renders in the matching colour; the open-paragraph
    // marker and cursor REVERSED still take precedence on the active row.
    pub tree_book_fg: String,
    pub tree_chapter_fg: String,
    pub tree_subchapter_fg: String,
    pub tree_paragraph_fg: String,
    pub tree_image_fg: String,
    pub tree_script_fg: String,

    // Editor pane header — the trailing `L{row} C{col}` cursor read-out
    // gets this colour so it's distinguishable from the title.
    pub editor_position_fg: String,

    // AI pane header — the `scope=…` and `infer=…` chips light up in
    // these colours so the active modes are visible at a glance.
    pub ai_scope_fg: String,
    pub ai_infer_fg: String,

    // Foreground colour applied to characters that differ from the
    // pre-grammar-check baseline after `T` overwrites the buffer with the
    // model's corrected paragraph. Stays visible until the user saves
    // (the user implicitly accepts the changes) or switches paragraphs.
    pub grammar_change_fg: String,

    // Typst syntax colours.
    pub syntax_heading: String,
    pub syntax_bold: String,
    pub syntax_italic: String,
    pub syntax_string: String,
    pub syntax_number: String,
    pub syntax_comment: String,
    pub syntax_keyword: String,
    pub syntax_function: String,
    pub syntax_operator: String,
    pub syntax_list_marker: String,
    pub syntax_raw: String,
    pub syntax_tag: String,
    pub syntax_quote: String,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        // Catppuccin Mocha — chosen for low eye-strain on a dark background
        // and broad community familiarity. All values are RGB hex strings so
        // they re-serialise cleanly into HJSON.
        Self {
            pane_bg: "#1e1e2e".into(),
            pane_fg: "#cdd6f4".into(),
            line_number_fg: "#6c7086".into(),
            current_line_bg: "#313244".into(),

            border_focused: "#cba6f7".into(),
            border_unfocused: "#45475a".into(),
            border_dirty: "#f9e2af".into(),
            border_saved: "#a6e3a1".into(),
            border_readonly: "#94e2d5".into(),

            modal_bg: "#181825".into(),
            modal_border: "#cba6f7".into(),
            modal_fg: "#cdd6f4".into(),

            places_fg: "#89dceb".into(),
            characters_fg: "#f9e2af".into(),
            artefacts_fg: "#fab387".into(),
            notes_underline_fg: "#cdd6f4".into(),

            search_match_bg: "#f38ba8".into(),
            search_current_bg: "#f5c2e7".into(),

            tree_open_marker: "#a6e3a1".into(),
            tree_book_fg: "#f5c2e7".into(),       // pink — books pop at the top
            tree_chapter_fg: "#89b4fa".into(),    // blue — chapter rhythm
            tree_subchapter_fg: "#94e2d5".into(), // teal — subchapter
            tree_paragraph_fg: "#cdd6f4".into(),  // base text — keep prose calm
            tree_image_fg: "#fab387".into(),       // peach — media accent
            tree_script_fg: "#cba6f7".into(),      // mauve — code accent

            editor_position_fg: "#89dceb".into(), // sky — cursor read-out
            ai_scope_fg: "#fab387".into(),        // peach — F9 scope chip
            ai_infer_fg: "#94e2d5".into(),        // teal — F10 inference chip

            grammar_change_fg: "#f38ba8".into(),

            syntax_heading: "#cba6f7".into(),
            syntax_bold: "#f9e2af".into(),
            syntax_italic: "#94e2d5".into(),
            syntax_string: "#a6e3a1".into(),
            syntax_number: "#fab387".into(),
            syntax_comment: "#6c7086".into(),
            syntax_keyword: "#cba6f7".into(),
            syntax_function: "#89dceb".into(),
            syntax_operator: "#94e2d5".into(),
            syntax_list_marker: "#cba6f7".into(),
            syntax_raw: "#fab387".into(),
            syntax_tag: "#89b4fa".into(),
            syntax_quote: "#9399b2".into(),
        }
    }
}

/// Parse a colour spec into a ratatui `Color`. Accepts `#RRGGBB` /
/// `#RGB` / `RRGGBB`. Empty string returns `None` (caller decides what to
/// use as a fallback — typically `Color::Reset`). On parse failure returns
/// `None` and the caller falls back; we never panic on a malformed theme.
pub fn parse_color(s: &str) -> Option<ratatui::style::Color> {
    use ratatui::style::Color;
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    let hex = t.strip_prefix('#').unwrap_or(t);
    let parse_byte = |h: &str| u8::from_str_radix(h, 16).ok();
    match hex.len() {
        3 => {
            let r = parse_byte(&hex[0..1])? * 17;
            let g = parse_byte(&hex[1..2])? * 17;
            let b = parse_byte(&hex[2..3])? * 17;
            Some(Color::Rgb(r, g, b))
        }
        6 => {
            let r = parse_byte(&hex[0..2])?;
            let g = parse_byte(&hex[2..4])?;
            let b = parse_byte(&hex[4..6])?;
            Some(Color::Rgb(r, g, b))
        }
        _ => None,
    }
}

/// Convenience: parse the field, fall back to `default` when empty/invalid.
/// Used everywhere a theme colour gets applied so the renderer never panics
/// because the user typed `pane_fg: ""`.
pub fn color_or(s: &str, default: ratatui::style::Color) -> ratatui::style::Color {
    parse_color(s).unwrap_or(default)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EmbeddingsConfig {
    /// fastembed model name; default is multilingual with strong Russian support
    pub model: String,
    pub chunk_size: usize,
    pub chunk_overlap: f32,
}

impl Default for EmbeddingsConfig {
    fn default() -> Self {
        Self {
            model: "MultilingualE5Small".into(),
            chunk_size: 800,
            chunk_overlap: 0.15,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LlmConfig {
    pub default: String,
    pub providers: std::collections::BTreeMap<String, LlmProvider>,
}

impl Default for LlmConfig {
    fn default() -> Self {
        let mut providers = std::collections::BTreeMap::new();
        // Gemini — Google.
        providers.insert(
            "gemini".into(),
            LlmProvider {
                model: "gemini-2.5-pro".into(),
                api_key_env: Some("GEMINI_API_KEY".into()),
            },
        );
        // Claude — Anthropic. genai routes any `claude-*` model to
        // the Anthropic adapter.
        providers.insert(
            "claude".into(),
            LlmProvider {
                model: "claude-sonnet-4-5".into(),
                api_key_env: Some("ANTHROPIC_API_KEY".into()),
            },
        );
        // OpenAI — `gpt-4o` is the multi-modal workhorse. The user
        // can switch to `gpt-4o-mini` for cheaper / faster runs or
        // `gpt-5-pro` once available; genai picks the right adapter
        // (Responses vs Chat Completions) automatically.
        providers.insert(
            "openai".into(),
            LlmProvider {
                model: "gpt-4o".into(),
                api_key_env: Some("OPENAI_API_KEY".into()),
            },
        );
        // DeepSeek.
        providers.insert(
            "deepseek".into(),
            LlmProvider {
                model: "deepseek-chat".into(),
                api_key_env: Some("DEEPSEEK_API_KEY".into()),
            },
        );
        // Grok — xAI. genai dispatches `grok-*` model names to its
        // Xai adapter, which talks to https://api.x.ai/v1 with the
        // OpenAI-compatible protocol.
        providers.insert(
            "grok".into(),
            LlmProvider {
                model: "grok-2-latest".into(),
                api_key_env: Some("XAI_API_KEY".into()),
            },
        );
        Self {
            default: "gemini".into(),
            providers,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmProvider {
    pub model: String,
    /// Environment variable that holds the provider's API key. Omit for
    /// local providers like Ollama that don't need authentication — when
    /// absent, the auth check is skipped.
    #[serde(default)]
    pub api_key_env: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EditorConfig {
    pub theme: String,
    pub tab_width: usize,
    pub wrap: bool,
    /// Number of seconds of editor inactivity after which the current
    /// paragraph is automatically saved. 0 disables idle autosave (the
    /// quit-time and paragraph-switch autosaves still fire).
    pub autosave_seconds: u64,
    /// Insert the matching close-bracket / quote when the user types
    /// `(`, `[`, `{`, `"` or `'`. Enter inside a bracket pair expands
    /// to a 3-line indented block. Backspace at the inside of a freshly
    /// typed pair removes both halves. Disabled = nothing inserts.
    pub auto_close_pairs: bool,
    /// Snowball stemmer languages used to expand the Places/Characters
    /// highlight overlay so morphological variants light up too — e.g.
    /// "Москва" also matches "Москве", "Москвою". Each entry is one of the
    /// names accepted by `rust-stemmers::Algorithm` (lowercased), see
    /// `parse_stemmer_language` for the supported set.
    pub stemming: StemmingConfig,
    /// Show the project-pulse splash on startup (1.2.4+).
    /// 7-second timed overlay with today/streak/active +
    /// status-ladder counts. Any key press dismisses early.
    /// Set false to skip directly into the editor.
    #[serde(default = "default_startup_splash")]
    pub startup_splash: bool,
}

fn default_startup_splash() -> bool {
    true
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            theme: "default".into(),
            tab_width: 2,
            wrap: true,
            autosave_seconds: 5,
            auto_close_pairs: true,
            stemming: StemmingConfig::default(),
            startup_splash: default_startup_splash(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StemmingConfig {
    /// Languages whose Snowball stemmer is used for the highlight overlay.
    /// The default covers Vladimir's writing languages (English + Russian).
    /// Empty disables stemming and falls back to exact-phrase matching.
    pub languages: Vec<String>,
}

impl Default for StemmingConfig {
    fn default() -> Self {
        Self {
            languages: vec!["english".into(), "russian".into()],
        }
    }
}

/// Map an HJSON-friendly language name onto a `rust_stemmers::Algorithm`.
/// Unknown names return `None`; callers surface a config error to the user.
pub fn parse_stemmer_language(name: &str) -> Option<rust_stemmers::Algorithm> {
    use rust_stemmers::Algorithm;
    let lower = name.trim().to_ascii_lowercase();
    Some(match lower.as_str() {
        "arabic" => Algorithm::Arabic,
        "danish" => Algorithm::Danish,
        "dutch" => Algorithm::Dutch,
        "english" | "en" => Algorithm::English,
        "finnish" => Algorithm::Finnish,
        "french" => Algorithm::French,
        "german" => Algorithm::German,
        "greek" => Algorithm::Greek,
        "hungarian" => Algorithm::Hungarian,
        "italian" => Algorithm::Italian,
        "norwegian" => Algorithm::Norwegian,
        "portuguese" => Algorithm::Portuguese,
        "romanian" => Algorithm::Romanian,
        "russian" | "ru" => Algorithm::Russian,
        "spanish" => Algorithm::Spanish,
        "swedish" => Algorithm::Swedish,
        "tamil" => Algorithm::Tamil,
        "turkish" => Algorithm::Turkish,
        _ => return None,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyBindings {
    pub save: String,
    pub search: String,
    pub ai_prompt: String,
    pub next_pane: String,
    pub prev_pane: String,
    pub page_up: String,
    pub page_down: String,
    /// Meta-prefix chord. When pressed, the next keystroke is interpreted as
    /// an action selector (B add book, C chapter, S subchapter, P paragraph,
    /// D delete, ↑/↓ reorder, Esc cancel). Replaces the old `Ctrl+Shift+*`
    /// chords which many terminals and multiplexers re-encode unhelpfully.
    pub meta_prefix: String,
    /// Bund meta-prefix chord. Parallel to `meta_prefix` but for
    /// scripting actions (R run buffer, E eval, N new script).
    /// Defaults to Ctrl+Z since tui-textarea's undo is bound to
    /// Ctrl+U in this codebase. Set to an empty string to disable
    /// the Bund chord entirely.
    pub bund_prefix: String,
    /// View meta-prefix chord (1.2.4+). Parallel to meta_prefix +
    /// bund_prefix but for markdown export / similar mode /
    /// progress / paragraph target. Defaults to Ctrl+V. Empty
    /// string disables the layer (a terminal that wants Ctrl+V
    /// for "verbatim next" can opt out).
    #[serde(default = "default_view_prefix")]
    pub view_prefix: String,
    /// User overlay for chord-action bindings under the meta- and
    /// bund-prefixes. Each entry is `{ chord, action, scope? }`.
    /// The `chord` string uses shorthand `"<prefix> <suffix>"`
    /// (e.g. `"Ctrl+b y"` rebinds Ctrl+B Y). `action` is the
    /// dotted form (`"tree.morph_type"`, `"bund.run_buffer"`,
    /// `"none"` to disable). `scope` is one of
    /// `"any"` / `"editor"` / `"tree"` / `"ai"` and defaults to
    /// `"any"`. Hard-blocked chords (Ctrl+Q, meta_prefix,
    /// bund_prefix) are rejected with a clear error.
    #[serde(default)]
    pub bindings: Vec<BindingOverride>,
}

/// Single entry inside `keys.bindings`. Parsed at startup into a
/// `keybind::BindingEntry` and applied on top of
/// `KeyBindings::defaults()`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BindingOverride {
    pub chord: String,
    pub action: String,
    #[serde(default)]
    pub scope: Option<String>,
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            save: "Ctrl+s".into(),
            search: "Ctrl+/".into(),
            ai_prompt: "Ctrl+i".into(),
            next_pane: "Tab".into(),
            prev_pane: "Shift+Tab".into(),
            page_up: "PageUp".into(),
            page_down: "PageDown".into(),
            meta_prefix: "Ctrl+b".into(),
            bund_prefix: "Ctrl+z".into(),
            view_prefix: default_view_prefix(),
            bindings: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HierarchyConfig {
    /// If false, only Book → Chapter → Subchapter → Paragraph is allowed.
    /// If true, Subchapter may nest arbitrarily before terminating in Paragraph.
    pub unbounded_subchapters: bool,
}

impl Default for HierarchyConfig {
    fn default() -> Self {
        Self {
            unbounded_subchapters: false,
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> crate::error::Result<Self> {
        let raw = std::fs::read_to_string(path).map_err(crate::error::Error::Io)?;
        serde_hjson::from_str(&raw).map_err(|e| crate::error::Error::Config(e.to_string()))
    }

    #[allow(dead_code)]
    pub fn save(&self, path: &Path) -> crate::error::Result<()> {
        let s = serde_hjson::to_string(self)
            .map_err(|e| crate::error::Error::Config(e.to_string()))?;
        std::fs::write(path, s).map_err(crate::error::Error::Io)
    }
}

/// Writing-progress goals — fuels the status-bar widget +
/// Ctrl+V G modal.
///
/// All numeric fields are inclusive; absent / zero means
/// "no target set" rather than "must be zero". Per-book entries
/// live under `goals.books.<book-slug>` so the slug is the
/// natural lookup key (case-insensitive in the
/// hierarchy → snapshot mapping).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GoalsConfig {
    /// Project-wide daily word-count target. Status-bar shows
    /// `today X/daily_words`. `0` (default) hides the slash.
    pub daily_words: i64,
    /// Project-wide daily active-time target, in minutes (1.2.4+).
    /// Status-bar shows `Nm/Mm` against this when set; the
    /// `hook.on_active_goal_hit` fires the first time today's
    /// active-time crosses the line. `0` (default) disables.
    pub active_minutes_daily: i64,
    /// Missed days forgiven per rolling 7-day window before the
    /// streak breaks. `0` = strict; `1` = one rest day per week.
    pub streak_grace_per_week: i64,
    /// Per-book targets. Key is the book slug (matches
    /// `Node.slug` case-insensitively).
    pub books: std::collections::HashMap<String, BookGoal>,
    /// Trailing-week status-promotion targets. Key is the
    /// status string ("ready", "final", "third", …) lowercased.
    pub status_ladder: std::collections::HashMap<String, i64>,
    /// Auto-promote a paragraph's status to the next ladder rung
    /// (Napkin → First → Second → Third → Final → Ready) on the
    /// first save where `word_count` crosses the paragraph's
    /// `target_words`. Idempotent per `(paragraph, status)` —
    /// won't re-fire until the user manually cycles status.
    /// Default `true`; set `false` to keep promotions manual.
    #[serde(default = "default_auto_promote_on_target")]
    pub auto_promote_on_target: bool,
}

fn default_auto_promote_on_target() -> bool {
    true
}

impl Default for GoalsConfig {
    fn default() -> Self {
        Self {
            daily_words: 0,
            active_minutes_daily: 0,
            streak_grace_per_week: 0,
            books: std::collections::HashMap::new(),
            status_ladder: std::collections::HashMap::new(),
            auto_promote_on_target: default_auto_promote_on_target(),
        }
    }
}

/// Per-book writing target.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct BookGoal {
    /// Total words the book should reach. `0` hides the
    /// per-book pace line.
    pub target_words: i64,
    /// ISO date (`YYYY-MM-DD`) by which `target_words` should
    /// be hit. Empty string disables deadline pacing.
    pub deadline: String,
}

/// Multi-format export hookup for Ctrl+B O.
///
/// When the user "takes" the book, inkhaven first builds the
/// PDF (the existing flow). If `extra_formats` is non-empty, the
/// same combined `.typ` source feeds the in-process converters
/// in `src/export/` and the resulting files land next to the
/// PDF with matching stem. Each entry is a case-insensitive
/// format name — supported today: `markdown`, `tex`, `epub`.
/// Unknown entries log a WARN and are skipped. Per-format
/// errors are reported in the status bar but never abort the
/// take.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OutputConfig {
    pub extra_formats: Vec<String>,
    /// 1.2.6+ — milliseconds the Ctrl+B O extras splash holds
    /// each format on screen so the user can actually see the
    /// transitions (markdown → tex → epub …). Each value is the
    /// sleep applied right after the format is drawn as the
    /// in-flight `▶` step, plus the same delay after the final
    /// `✓` frame. Set to `0` to disable the artificial pause.
    /// Default `400` (≈ 1.2s for a 3-format build).
    pub extras_step_pause_ms: u64,
    /// 1.2.6+ — when true, the final all-✓ frame of the extras
    /// splash holds until the user presses any key (same shape
    /// as `typst_compile.wait_for_key_after_compile`). Useful
    /// for screenshots / demos; off in normal use so a batch
    /// `Ctrl+B O` doesn't trap the user behind a key prompt.
    /// Default `false`.
    pub extras_wait_for_key: bool,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            extra_formats: Vec::new(),
            extras_step_pause_ms: 400,
            extras_wait_for_key: false,
        }
    }
}

/// 1.2.6+ — story timeline feature config. `enabled: false`
/// (the default) hides every timeline chord, CLI subcommand,
/// and Bund word. Once enabled, events become a first-class
/// metadata layer over the existing paragraph tree (see
/// `crate::timeline`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TimelineConfig {
    pub enabled: bool,
    pub default_track: String,
    pub calendar: crate::timeline::calendar::CalendarConfig,
    pub display: TimelineDisplayConfig,
}

impl Default for TimelineConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            default_track: "main".into(),
            calendar: crate::timeline::calendar::CalendarConfig::default(),
            display: TimelineDisplayConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TimelineDisplayConfig {
    pub show_orphans: bool,
    pub swim_lane_max_rows: u32,
    pub default_zoom: f32,
}

impl Default for TimelineDisplayConfig {
    fn default() -> Self {
        Self {
            show_orphans: true,
            swim_lane_max_rows: 12,
            default_zoom: 1.0,
        }
    }
}

/// 1.2.6+ — AI-pane behaviour. Currently per-paragraph memory
/// + the `.example` prompt-seeding switch; future knobs (e.g.
/// ai-pane default scope, max chat history depth) will land
/// here.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AiConfig {
    /// When true, AI prompts sent with scope=Paragraph stamp
    /// both turns onto the open paragraph's `ai_memory`
    /// metadata, and subsequent paragraph-scoped prompts
    /// pre-pend that memory to the chat-history payload. The
    /// project-wide visible chat history is untouched.
    pub per_paragraph_memory: bool,
    /// Maximum total turns (user + assistant) kept per
    /// paragraph. Oldest turns evict first. `0` is treated as
    /// "disabled" regardless of `per_paragraph_memory`.
    pub per_paragraph_memory_max_turns: usize,
    /// 1.2.6+ — auto-populate the `Prompts` system book with
    /// `<name>.example` paragraphs carrying inkhaven's
    /// embedded default prompts (F7 grammar-check, F11
    /// explain-diagnostic, F12 critique-edit + critique-
    /// changes). Runs both at `inkhaven init` and on every
    /// TUI open. Idempotent — existing paragraphs with the
    /// same title are never touched, so only gaps get filled.
    /// Set `false` to disable the seeding entirely (you'll
    /// keep the F-keys but the Prompts book stays as you left
    /// it).
    pub reseed_prompt_examples: bool,
    /// 1.2.6+ — when true, applying an AI rewrite that
    /// replaces the buffer (`r` and `g` chords in the AI
    /// pane) first opens a side-by-side diff modal so the
    /// user can accept / reject / accept-and-edit before any
    /// bytes are written. Additive applies (`i` insert, `t`
    /// prepend, `b` append) skip the review — they don't
    /// destroy existing text. Default true.
    pub diff_review_on_apply: bool,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            per_paragraph_memory: false,
            per_paragraph_memory_max_turns: 10,
            reseed_prompt_examples: true,
            diff_review_on_apply: true,
        }
    }
}

#[cfg(test)]
mod settings_synth_tests {
    use super::*;

    #[test]
    fn synthesised_header_with_defaults_compiles_typst_shape() {
        let cfg = Config::default();
        let s = cfg.synthesised_settings_typ_header();
        // Mandatory headers and the user-override marker.
        assert!(s.contains("auto-generated"));
        assert!(s.contains("User overrides"));
        // Default page / text / par.
        assert!(s.contains("#set page("));
        assert!(s.contains("paper: \"us-letter\""));
        assert!(s.contains("margin: (top: 2.5cm"));
        assert!(s.contains("#set text("));
        assert!(s.contains("lang: \"en\""));
        assert!(s.contains("#set par(justify: true"));
        // No heading numbering by default.
        assert!(!s.contains("#set heading(numbering"));
    }

    #[test]
    fn synthesised_header_emits_numbering_when_set() {
        let mut cfg = Config::default();
        cfg.typst_layout.heading_numbering = "1.1".into();
        let s = cfg.synthesised_settings_typ_header();
        assert!(s.contains("#set heading(numbering: \"1.1\")"));
    }

    #[test]
    fn synthesised_header_omits_text_set_when_all_empty() {
        let mut cfg = Config::default();
        cfg.typst_fonts.body = String::new();
        cfg.typst_fonts.body_size = String::new();
        cfg.typst_fonts.language = String::new();
        let s = cfg.synthesised_settings_typ_header();
        // No #set text(...) but the monospace show-rule is
        // independent — typst 0.11+ uses `show raw: set text(...)`.
        assert!(!s.contains("#set text("));
        assert!(s.contains("#show raw: set text(font:"));
    }

    #[test]
    fn synthesised_header_escapes_double_quotes_in_values() {
        let mut cfg = Config::default();
        cfg.typst_fonts.body = "Bad\"Font".into();
        let s = cfg.synthesised_settings_typ_header();
        // 1.2.6: fonts are emitted as a fallback array, so the
        // user-supplied value sits inside `font: ("…", "Linux
        // Libertine")`. We only assert the escape itself landed.
        assert!(s.contains("\"Bad\\\"Font\""), "got:\n{s}");
    }

    #[test]
    fn synthesised_header_uses_font_fallback_array_for_custom_body() {
        let mut cfg = Config::default();
        cfg.typst_fonts.body = "EB Garamond".into();
        let s = cfg.synthesised_settings_typ_header();
        // Custom body font is paired with the bundled fallback so a
        // missing host font won't fail the compile.
        assert!(
            s.contains("font: (\"EB Garamond\", \"Linux Libertine\")"),
            "got:\n{s}"
        );
    }

    #[test]
    fn synthesised_header_uses_font_fallback_array_for_custom_mono() {
        let mut cfg = Config::default();
        cfg.typst_fonts.monospace = "JetBrains Mono".into();
        let s = cfg.synthesised_settings_typ_header();
        assert!(
            s.contains(
                "#show raw: set text(font: (\"JetBrains Mono\", \"DejaVu Sans Mono\"))"
            ),
            "got:\n{s}"
        );
    }

    #[test]
    fn synthesised_header_never_emits_invalid_set_raw_font() {
        // Typst 0.11+ removed the `font:` parameter from `raw`.
        // Guard against accidentally regressing to `#set raw(font: …)`.
        let cfg = Config::default();
        let s = cfg.synthesised_settings_typ_header();
        assert!(!s.contains("#set raw(font:"), "got:\n{s}");
    }

    #[test]
    fn synthesised_header_dedupes_when_body_matches_bundled() {
        let cfg = Config::default();
        let s = cfg.synthesised_settings_typ_header();
        // Default body IS the bundled fallback → bare string form,
        // no duplicate entry.
        assert!(s.contains("font: \"Linux Libertine\""), "got:\n{s}");
        assert!(
            !s.contains("(\"Linux Libertine\", \"Linux Libertine\")"),
            "got:\n{s}"
        );
    }

    #[test]
    fn synthesised_header_multi_column_emits_columns_arg() {
        let mut cfg = Config::default();
        cfg.typst_page.columns = 2;
        let s = cfg.synthesised_settings_typ_header();
        assert!(s.contains("columns: 2"));
    }
}
