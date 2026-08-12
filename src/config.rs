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
    pub facts: FactsConfig,
    #[serde(default)]
    pub drift: DriftConfig,
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
    #[serde(default)]
    pub cost: CostConfig,
    #[serde(default)]
    pub book_rag: BookRagConfig,
    /// GRAPHMIND (2.x) — the knowledge-graph AI surfaces (Graph scope, `graph ask`).
    #[serde(default)]
    pub graph: GraphConfig,
    /// INNER_EDITOR-1 (1.4.2+) — the Inner Editor companion.
    #[serde(default)]
    pub inner_editor: InnerEditorConfig,
    /// SOURCES-1 (1.4.5+) — the bibliography & citation engine.
    #[serde(default)]
    pub sources: SourcesConfig,
    /// STRUCT-1 — Jinja template paragraph rendering.
    #[serde(default)]
    pub jinja: JinjaConfig,
    /// NARR-1 — narrative-voice (`prose`) profiling.
    #[serde(default)]
    pub prose: ProseConfig,
    /// CHORUS-1 (2.1) — voice & style at book scale.
    #[serde(default)]
    pub chorus: ChorusConfig,
    /// INNER-STYLIST-1 (2.1) — the voice-at-scale coach (Inner-family reader).
    #[serde(default)]
    pub stylist: StylistConfig,
    /// SENTINEL-1 (2.2) — the unified continuity ledger.
    #[serde(default)]
    pub continuity: ContinuityConfig,
    /// LECTOR-1 (2.3) — the read-through.
    #[serde(default)]
    pub lector: LectorConfig,
    #[serde(default)]
    pub dialogue: DialogueConfig,
    #[serde(default)]
    pub utopia: UtopiaConfig,
    /// CHAR-1 — character-arc tracking.
    #[serde(default)]
    pub char: CharConfig,
    /// INNER-THEOLOGIAN-1 — moral/theological reader.
    #[serde(default)]
    pub theologian: TheologianConfig,
    /// RIGOR — the deterministic reasoning-rigor reader (`rigor:` block).
    #[serde(default)]
    pub rigor: RigorConfig,
    /// ORACLE — the conlang well-formedness Oracle on save (`oracle:` block).
    #[serde(default)]
    pub oracle: OracleConfig,
    /// TDOC-1 — technical-documentation tooling (verified code blocks). Off by
    /// default; nothing runs until the author opts in and names runners.
    #[serde(default)]
    pub docs: DocsConfig,
    /// PAPER (1.6.15+) — journal-article front matter (title block: authors,
    /// affiliations, abstract, keywords, funding). Empty by default → renders
    /// nothing, so existing books are unaffected.
    #[serde(default)]
    pub frontmatter: FrontmatterConfig,
    /// PAPER (1.6.15+) — LaTeX export document class + preamble. Defaults
    /// reproduce the historical book preamble; override to target a journal
    /// class (article / IEEEtran / elsarticle / two-column).
    #[serde(default)]
    pub tex_export: TexExportConfig,
    /// TYPST-UNIVERSE (1.6.15+) — the `Ctrl+V #` package import picker source.
    #[serde(default)]
    pub typst_universe: TypstUniverseConfig,
    /// MYTH-1 — mythological & symbolic pattern library.
    #[serde(default)]
    pub myth: MythConfig,
    /// WORLD-12 — the AI world-critique pass (`realworld critique`).
    #[serde(default)]
    pub world: WorldConfig,
    /// RESRCH-1 — the Research Assistant (`inkhaven research`).
    #[serde(default)]
    pub research: ResearchConfig,
    /// The project's declared genre (e.g. `literary_realism`, `fantasy`).
    /// Project-wide; consumed by Inner Editor's genre-aware prompting and open
    /// to other features later. `None` = genre-blind.
    #[serde(default)]
    pub genre: Option<String>,
    /// AUDIENCE-1 (1.4.6+) — the project's **default** Inner Socrates persona id
    /// (e.g. `skeptical-practitioner` for a technical book). Used only when no
    /// persona has been explicitly set for the project; an explicit
    /// `inner-socrates persona set` always wins. `None` = the bundled
    /// `inner-socrates` default.
    #[serde(default)]
    pub inner_socrates_default_persona: Option<String>,
    #[serde(default)]
    pub project_lock: ProjectLockConfig,
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
    /// 1.2.8+ — Scrivener-importer behaviour. Currently
    /// scopes the CustomMeta date-field detection — which
    /// field names in a Scrivener project's
    /// `<CustomMetaDataSettings>` map to events on import.
    #[serde(default)]
    pub scrivener: ScrivenerConfig,
    /// 1.2.8+ — embedded nushell pane (`Ctrl+Z o`). Enabled
    /// by default; disable via `shell.enabled: false` to
    /// strip the chord entirely (the modal action becomes
    /// a no-op with a status hint).
    #[serde(default)]
    pub shell: ShellConfig,
    /// Bund scripting sandbox policy. Defaults deny destructive
    /// categories (fs_write, net, shell, code_eval); writers opt
    /// in by listing the categories or words they want to allow.
    /// See `src/scripting/policy.rs`.
    #[serde(default)]
    pub scripting: crate::scripting::policy::Policy,
    /// 1.3.0 PDF-1 — imposition profiles for `inkhaven pdf impose`
    /// (binding style, sheet size, creep, marks).  Named profiles merge
    /// through the config cascade like everything else.
    #[serde(default)]
    pub imposition: crate::pdf::impose::config::ImpositionConfig,
    /// 1.3.0 PDF-1 P2 — cover/spine defaults for `inkhaven pdf cover`
    /// (trim size, bleed, paper stocks for the computed spine).
    #[serde(default)]
    pub cover: crate::pdf::cover::CoverConfig,
    /// 1.3.0 PDF-1 P2 — preflight DPI targets for `inkhaven pdf preflight`.
    #[serde(default)]
    pub preflight: crate::pdf::preflight::PreflightConfig,
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
    /// 1.2.14+ Phase Q.4 — project-level word-
    /// count goal + pacing settings.  Feeds the
    /// `Ctrl+V Shift+G` projection modal.  Empty
    /// defaults disable the modal contents but
    /// still let the chord open the modal with
    /// a "no goal set" message.
    #[serde(default)]
    pub project: ProjectConfig,
    /// 1.2.15+ Phase H.1 — background health
    /// monitor.  See `crate::health` and
    /// `Documentation/PROPOSALS/1.2.15_PLAN.md`
    /// §3.
    #[serde(default)]
    pub health: HealthConfig,
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
            facts: FactsConfig::default(),
            drift: DriftConfig::default(),
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
            cost: CostConfig::default(),
            book_rag: BookRagConfig::default(),
            graph: GraphConfig::default(),
            inner_editor: InnerEditorConfig::default(),
            sources: SourcesConfig::default(),
            jinja: JinjaConfig::default(),
            prose: ProseConfig::default(),
            chorus: ChorusConfig::default(),
            stylist: StylistConfig::default(),
            continuity: ContinuityConfig::default(),
            lector: LectorConfig::default(),
            dialogue: DialogueConfig::default(),
            utopia: UtopiaConfig::default(),
            char: CharConfig::default(),
            theologian: TheologianConfig::default(),
            rigor: RigorConfig::default(),
            oracle: OracleConfig::default(),
            docs: DocsConfig::default(),
            frontmatter: FrontmatterConfig::default(),
            tex_export: TexExportConfig::default(),
            typst_universe: TypstUniverseConfig::default(),
            myth: MythConfig::default(),
            world: WorldConfig::default(),
            research: ResearchConfig::default(),
            genre: None,
            inner_socrates_default_persona: None,
            project_lock: ProjectLockConfig::default(),
            ai: AiConfig::default(),
            timeline: TimelineConfig::default(),
            scrivener: ScrivenerConfig::default(),
            shell: ShellConfig::default(),
            scripting: crate::scripting::policy::Policy::default(),
            imposition: crate::pdf::impose::config::ImpositionConfig::default(),
            cover: crate::pdf::cover::CoverConfig::default(),
            preflight: crate::pdf::preflight::PreflightConfig::default(),
            language: default_language(),
            project: ProjectConfig::default(),
            health: HealthConfig::default(),
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

    /// 1.2.16+ Phase P.1 — amber chip threshold
    /// for the backup-freshness health check.
    /// Fraction of `max_age` at which the status-
    /// bar chip flips from `✓` clean to `ℹ` amber
    /// ("backup is getting old, plan a refresh
    /// soon").  Above `max_age` the chip flips to
    /// the existing `⚠` yellow warning.  Default
    /// 0.5 — gives the user a midpoint heads-up
    /// before the hard warning fires.  Set 0.0
    /// to disable (chip never amber; only the
    /// hard warning surfaces).
    #[serde(default = "default_amber_threshold")]
    pub amber_threshold: f32,
    /// 1.3.37 — explicit toggle for the exit-hook auto-backup. `false`
    /// disables it without the non-obvious side-effects of clearing
    /// `out_dir` / setting `max_age = 0s`. Default `true`.
    #[serde(default = "default_auto_backup_on_exit")]
    pub auto_backup_on_exit: bool,
    /// 1.3.37 — how many backup `.zip` snapshots to retain in `out_dir`;
    /// after each backup the oldest beyond this count are deleted. `0`
    /// (default) keeps all (prior behaviour — backups accumulate).
    #[serde(default = "default_backup_keep_last")]
    pub keep_last: usize,
}

fn default_backup_wait_for_key() -> bool {
    true
}

fn default_auto_backup_on_exit() -> bool {
    true
}

fn default_backup_keep_last() -> usize {
    0
}

fn default_amber_threshold() -> f32 {
    0.5
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
            amber_threshold: default_amber_threshold(),
            auto_backup_on_exit: default_auto_backup_on_exit(),
            keep_last: default_backup_keep_last(),
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

/// 1.2.8+ — Scrivener-importer behaviour.
///
/// `date_fields`: which Scrivener CustomMeta field names (case-
/// insensitive) should be interpreted as event dates during
/// `inkhaven import-scrivener`. When a matching field's value
/// parses against the project's HJSON calendar, the imported
/// paragraph gets `EventData` attached automatically (anchored
/// at the parsed start tick, no end, the project's
/// `timeline.default_track`). When `timeline.enabled = false`
/// the whole pass is a no-op.
/// 1.2.14+ Phase Q.4 — `project: { … }` HJSON
/// stanza.  Word-count goal + target date drive
/// the `Ctrl+V Shift+G` projection modal.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectConfig {
    /// Total manuscript word-count goal.  `0`
    /// disables the goal display in the modal
    /// (counts still show).
    #[serde(default)]
    pub word_count_goal: u64,
    /// Target completion date in ISO 8601
    /// (`YYYY-MM-DD`).  Empty disables the days-
    /// remaining + projection-date display.
    #[serde(default)]
    pub target_date: String,
    /// Which user books contribute to the project
    /// total.  Empty = every user book.  Useful
    /// when a project has a primary manuscript
    /// book + reference / notes books that
    /// shouldn't count toward the goal.  Match
    /// is against book TITLE, case-insensitive.
    #[serde(default)]
    pub counted_books: Vec<String>,
}

/// 1.2.15+ Phase H.1 + H.2 + H.3 — background
/// health-monitor configuration.  Disabled by
/// default so existing projects don't inherit a
/// new background task without opting in.
///
/// Per-check cadences live in `crate::health`
/// (90 s project, 300 s backup, 3600 s rescue
/// orphans) — they're tuned to the cost of each
/// check, not exposed as HJSON yet.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HealthConfig {
    /// Master switch.  False = no monitor task,
    /// status-bar chip stays hidden.
    pub enabled: bool,
    /// 1.2.15+ Phase H.3 — per-class opt-in for
    /// the auto-repair flow.  All defaults are
    /// false: a user who turns on the monitor
    /// doesn't automatically grant it permission
    /// to mutate project state; each individual
    /// fix has to be enabled explicitly.
    pub auto_repair: AutoRepairConfig,
}

/// HJSON shape for [`crate::health::RepairPolicy`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AutoRepairConfig {
    /// Delete `*.inkhaven-rescue` orphan files
    /// older than `RESCUE_REPAIR_DAYS` (30 d) from
    /// the project tree.  Default false.
    pub rescue_orphans: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScrivenerConfig {
    pub date_fields: Vec<String>,
}

impl Default for ScrivenerConfig {
    fn default() -> Self {
        Self {
            // Common English-language Scrivener templates: "Date"
            // (default text field on the Novel template), "Story Date"
            // (Novel-with-Parts), "Event Date" (custom but widely
            // recommended in the Scrivener forum threads on timeline
            // workflows). Users with non-English templates extend or
            // replace this list in HJSON.
            date_fields: vec![
                "Date".into(),
                "Story Date".into(),
                "Event Date".into(),
            ],
        }
    }
}

/// 1.2.8+ — embedded nushell pane.
///
/// `enabled`: ship the `Ctrl+Z o` chord at all. `false`
/// makes the action a status-hint no-op, useful for users
/// who prefer to keep their writing app shell-free.
///
/// `max_buffered_turns`: how many command/output pairs the
/// pane retains. Older turns roll off the bottom. Picked to
/// fit the working-memory needs of a writing session
/// without growing unbounded across long-lived sessions.
///
/// `insert_template`: the typst markup `Ctrl+Z h` → `i`
/// wraps a selected output in when inserting into the
/// editor. The placeholder `{output}` is replaced with the
/// raw command output verbatim. Default uses a typst `raw`
/// block with `lang: "shell"` for monospace, no markdown
/// reinterpretation. Customise for a framed or themed
/// presentation.
///
/// `max_output_lines`: per-turn cap on stdout (and stderr).
/// A single command (`git log`, `cat very_big_file`, …) can
/// emit thousands of lines and bloat the in-memory turn
/// buffer + slow ratatui rendering.  When a turn's stdout
/// exceeds this many lines, the head is kept and the tail
/// is replaced with `… (N more lines truncated)`.  Same
/// rule applies to stderr.  Independent of
/// `max_buffered_turns` (which caps the number of *turns*).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ShellConfig {
    pub enabled: bool,
    pub max_buffered_turns: usize,
    pub max_output_lines: usize,
    pub insert_template: String,
    /// 1.2.8+ — basenames of external programs that are
    /// **refused before spawn**.  Full-screen TUI apps
    /// (vim, less, top, tmux, …) cannot run inside the
    /// embedded pane: they open `/dev/tty` directly and
    /// write escape sequences past our piped stdio,
    /// corrupting ratatui's alt-screen surface.  Match is
    /// case-insensitive against the program basename, so
    /// `^/usr/bin/vim` and `^vim` both hit.  Override per
    /// project to add internal tools.
    pub blocked_externals: Vec<String>,
    /// 1.2.8+ — wall-clock budget for a single command's
    /// evaluation.  After this many seconds the engine
    /// triggers its interrupt signal, waits a short grace
    /// period, and (if the command is still wedged) spins
    /// up a fresh engine and abandons the worker — losing
    /// any env-var / def state the user accumulated but
    /// keeping the TUI responsive.  Catches TUI apps that
    /// slip past `blocked_externals`.  Set high (e.g.
    /// 600) if you legitimately run long-baked pipelines.
    pub external_timeout_secs: u64,
}

impl Default for ShellConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_buffered_turns: 50,
            max_output_lines: 1000,
            insert_template:
                "#raw(block: true, lang: \"shell\", `{output}`)".into(),
            blocked_externals: default_blocked_externals(),
            external_timeout_secs: 30,
        }
    }
}

/// 1.2.8+ — default list of basenames refused before
/// spawn.  See `ShellConfig::blocked_externals` for the
/// rationale.  Grouped by category for editability:
///
///   editors        — vim/nvim/vi/view/emacs/nano/pico/joe
///   file managers  — mc/mcedit/ranger/nnn/lf/yazi
///   pagers         — less/more/most/pg
///   monitors       — top/htop/btop/atop/iotop/iftop
///   multiplexers   — tmux/screen/byobu/dtach
///   remote shells  — ssh/telnet/mosh
///   debuggers      — gdb/lldb
///   fuzzy finders  — fzf/peco/sk
///   REPLs (TTY)    — ipython/irb/pry
///   db clients     — psql/mysql/sqlite3
///   privileged     — sudo/su/passwd
pub fn default_blocked_externals() -> Vec<String> {
    [
        "vim", "nvim", "vi", "view", "ex",
        "emacs", "emacsclient",
        "nano", "pico", "joe", "jed",
        "mc", "mcedit", "ranger", "nnn", "lf", "yazi",
        "less", "more", "most", "pg",
        "top", "htop", "btop", "atop", "iotop", "iftop", "nethogs", "glances",
        "tmux", "screen", "byobu", "dtach", "abduco",
        "ssh", "telnet", "mosh", "rlogin",
        "gdb", "lldb",
        "fzf", "peco", "sk", "skim",
        "ipython", "irb", "pry",
        "psql", "mysql", "sqlite3", "redis-cli",
        "sudo", "su", "passwd",
    ]
    .into_iter()
    .map(String::from)
    .collect()
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
    /// 1.2.9+ — colour for inline filter-word warnings.
    #[serde(default)]
    pub style_warning_filter_word_fg: String,
    /// 1.2.9+ — colour for repeated-phrase warnings.
    #[serde(default)]
    pub style_warning_repeated_phrase_fg: String,
    /// 1.2.9+ — colour for show-don't-tell warnings.
    #[serde(default)]
    pub style_warning_show_dont_tell_fg: String,
    /// 1.3.9+ — colour for live anachronism warnings (a term that postdates
    /// the configured setting `year`).  Empty falls back to an amber that
    /// reads as "wrong era" caution, distinct from the show-don't-tell teal.
    #[serde(default)]
    pub style_warning_anachronism_fg: String,
    /// 1.4.8+ TERMS-1 — colour for banned-synonym warnings (the Glossary
    /// overlay). Default red `#e05a5a`.
    #[serde(default)]
    pub style_warning_banned_synonym_fg: String,
    /// 1.2.20+ — colour for the live echo overlay
    /// (`Ctrl+B Shift+K`).  Distinct from the
    /// repeated-phrase magenta so a within-paragraph
    /// repeat and a cross-paragraph echo read as
    /// different findings.  Empty falls back to a
    /// muted purple at runtime.
    #[serde(default)]
    pub style_warning_echo_fg: String,
    /// 1.2.13+ — colour for invented-language
    /// dictionary-entry overlays.  Empty falls back to
    /// a soft mauve-teal mix distinct from the four
    /// existing entity-overlay colours (places /
    /// characters / artefacts / notes).  Phase D
    /// extends with per-Language-sub-book overrides.
    #[serde(default)]
    pub language_word_fg: String,
    /// MYTH-1 (1.4.19+) — declared symbol vocabulary highlight (lavender default).
    #[serde(default)]
    pub myth_symbol_fg: String,
    /// 1.2.12+ — per-detector style modifier for the
    /// three style-warning overlays.  Accepts
    /// `"underline"` (default), `"bold"`, `"dim"`,
    /// `"reversed"`, `"italic"`, `"none"`, or
    /// `+`-combined like `"underline+bold"`.  The
    /// previous hard-coded `UNDERLINED` worked great
    /// for most terminals but read faint on some
    /// palettes — these knobs let users dial it up
    /// (or off, with `"none"`) without touching the
    /// detector colours.
    #[serde(default)]
    pub style_warning_filter_word_modifier: String,
    #[serde(default)]
    pub style_warning_repeated_phrase_modifier: String,
    #[serde(default)]
    pub style_warning_show_dont_tell_modifier: String,
    /// 1.2.20+ — modifier for the live echo overlay.
    /// Same grammar as the other style-warning
    /// modifiers; empty maps to `underline`.
    #[serde(default)]
    pub style_warning_echo_modifier: String,
    /// 1.2.14+ Phase C.1 — modifier applied to the
    /// character span of every inline comment.
    /// Empty string keeps the baked-in default
    /// `underline+italic`.  Accepts `+`-combined
    /// tokens like the existing style-warning
    /// fields: `bold`, `dim`, `italic`, `underline`,
    /// `reversed`, `none`.
    #[serde(default)]
    pub comment_span_modifier: String,
    /// 1.2.10+ — POV / character chip background +
    /// foreground.  Explicit RGB so the chip stays
    /// readable across terminal palettes (the named
    /// `Color::Magenta` rendered as a pale pink on
    /// Catppuccin and killed contrast against white).
    #[serde(default)]
    pub pov_chip_bg: String,
    #[serde(default)]
    pub pov_chip_fg: String,

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
            style_warning_filter_word_fg: "#f9c44e".into(),
            style_warning_repeated_phrase_fg: "#eb6f92".into(),
            style_warning_show_dont_tell_fg: "#94e2d5".into(),
            // 1.3.9+ — warm amber-orange "wrong era" caution,
            // distinct from the filter-word gold and the
            // show-don't-tell teal.
            style_warning_anachronism_fg: "#eba672".into(),
            // 1.4.8+ TERMS-1 — clear red "wrong term" caution.
            style_warning_banned_synonym_fg: "#e05a5a".into(),
            // 1.2.20+ — muted purple, distinct from the
            // repeated-phrase magenta so the two
            // repetition overlays don't read as one.
            style_warning_echo_fg: "#b48ead".into(),
            // 1.2.13+ — invented-language overlay; empty
            // falls back to a soft mauve-teal at runtime.
            language_word_fg: String::new(),
            myth_symbol_fg: String::new(),
            // 1.2.12+ — empty defaults map to UNDERLINED
            // (the historical hardcoded modifier).  Users
            // override to "bold", "dim", "reversed",
            // "italic", "none", or "+"-combined chords.
            style_warning_filter_word_modifier: String::new(),
            style_warning_repeated_phrase_modifier: String::new(),
            style_warning_show_dont_tell_modifier: String::new(),
            style_warning_echo_modifier: String::new(),
            comment_span_modifier: String::new(),
            pov_chip_bg: "#8b1d88".into(),
            pov_chip_fg: "#ffffff".into(),

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
    // Guard the byte-slicing below: a non-ASCII char makes `hex.len()`
    // (bytes) disagree with char positions, so `hex[0..1]` could split a
    // multibyte char and panic (e.g. `"#aé"` → `len()==3`).  Non-ASCII
    // can't be hex anyway, so reject it up front — keeping the module's
    // "never panic on a malformed theme" guarantee.
    if !hex.is_ascii() {
        return None;
    }
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
    /// r2d2 connection-pool size for each backing DuckDB file (metadata +
    /// content). Default 4. Clamped to a minimum of 2 at open time so a
    /// background job (e.g. the 1.3.12 deep AI refresh) can always check out a
    /// connection while the TUI holds another — a pool of 1 would deadlock.
    pub pool_size: usize,
}

impl Default for EmbeddingsConfig {
    fn default() -> Self {
        Self {
            model: "MultilingualE5Small".into(),
            chunk_size: 800,
            chunk_overlap: 0.15,
            pool_size: 4,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LlmConfig {
    pub default: String,
    pub providers: std::collections::BTreeMap<String, LlmProvider>,
    /// When the `default` provider's API key is unset, fall back to any other
    /// configured provider whose key IS available (or a keyless local provider).
    /// `true` (default) → "use whatever works"; `false` → fail with a clear error
    /// instead, for users who want the configured provider or nothing.
    pub auto_fallback: bool,
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
            auto_fallback: true,
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
    /// 1.3.37 — seconds between crash-rescue mirrors of the dirty
    /// buffer(s). Lower = a panic loses fewer keystrokes (more disk
    /// churn); `0` mirrors every tick. Default 2.
    #[serde(default = "default_crash_mirror_seconds")]
    pub crash_mirror_seconds: u64,
    /// 1.3.37 — how many deleted paragraphs the kill-ring keeps for
    /// undelete (`Ctrl+V Shift+U`) before the oldest rolls off.
    /// Default 10.
    #[serde(default = "default_deleted_paragraph_history")]
    pub deleted_paragraph_history: usize,
    /// 1.3.37 — when the open file changes on disk and the buffer is
    /// CLEAN: `true` (default) silently reloads it; `false` warns
    /// instead, leaving your view/cursor untouched (useful when an
    /// external `git pull` / script rewrites files).
    #[serde(default = "default_external_change_auto_reload")]
    pub external_change_auto_reload: bool,
    /// 1.3.37 — idle seconds after editing before the background
    /// fact-check fires. Default 5 (mirrors typst diagnostics idle).
    #[serde(default = "default_fact_check_idle_seconds")]
    pub fact_check_idle_seconds: u64,
    /// HAIKU-1 — emit a hand-curated haiku (in the book's language) to the
    /// Output pane at startup, when a new manuscript paragraph is created, and
    /// on `Ctrl+Z p`. No AI, no network. Default: true.
    #[serde(default = "default_startup_haiku")]
    pub startup_haiku: bool,
    /// HAIKU-2 — prefer *semantic* poem selection (nearest the writing context)
    /// when the fastembed engine is already warm; falls back to the HAIKU-1
    /// rotation when it is cold (always at startup). No effect when
    /// `startup_haiku` is false. No AI API, no network. Default: true.
    #[serde(default = "default_startup_haiku")]
    pub haiku_semantic: bool,
    /// HAIKU-3 — what the semantic haiku is chosen to reflect: `"paragraph"`
    /// (the default — the writing context under the cursor) or `"book"` (a
    /// centroid over a spread sample of the whole manuscript, so the poem reflects
    /// the book as a whole rather than the current passage). No effect when
    /// `haiku_semantic` is false or the engine is cold. No AI API, no network.
    #[serde(default = "default_haiku_scope")]
    pub haiku_scope: String,
    /// 1.3.37 — cap on the browser-style visited-paragraph history
    /// (persisted in `.session.json`). `0` (default) = unbounded,
    /// preserving prior behaviour; set e.g. 200 to bound session growth.
    #[serde(default = "default_visited_history_cap")]
    pub visited_history_cap: usize,
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
    /// 1.2.8+ — initial mouse-capture state on launch.
    /// `true` (the default) hands every mouse event to the
    /// TUI: click-to-focus, scroll-wheel scrolling per pane,
    /// in-TUI drag-select. `false` releases capture at
    /// startup so the terminal's native drag-select +
    /// system-clipboard copy (Cmd/Ctrl+Shift+C) work without
    /// pressing `Ctrl+Shift+M` first. The toggle still
    /// flips state at runtime regardless of this knob.
    #[serde(default = "default_mouse_captured")]
    pub mouse_captured: bool,
    /// 1.2.8+ — pop a confirmation modal on Ctrl+Q before
    /// quitting.  Default `false` — Ctrl+Q quits
    /// immediately (auto-saving any dirty buffer first, as
    /// always).  Set `true` to require a Y / Enter
    /// confirmation; N / Esc cancels and returns to the
    /// editor.  Useful for users who hit Ctrl+Q by accident
    /// (terminals with Ctrl+Q as a software-flow-control
    /// chord especially).
    #[serde(default = "default_confirm_quit")]
    pub confirm_quit: bool,
    /// 1.2.9+ — text-to-speech read-aloud (`Ctrl+B S`).
    /// See `TtsConfig` below for per-knob detail.
    #[serde(default)]
    pub tts: TtsConfig,
    /// 1.2.9+ — inline style-warning overlays.  See
    /// `StyleWarningsConfig` for per-detector knobs.
    #[serde(default)]
    pub style_warnings: StyleWarningsConfig,
    /// 1.2.9+ — status-bar POV / character chip.
    /// When enabled, the status bar gains a small chip
    /// showing the most-mentioned character in the
    /// currently-open paragraph (the heuristic POV
    /// character) plus up to three additional named
    /// characters present.  Driven by the project's
    /// existing `characters` lexicon — no separate
    /// tagging required.  Toggle at runtime with
    /// `Ctrl+B Shift+P`.
    #[serde(default = "default_pov_chip_enabled")]
    pub pov_chip_enabled: bool,
    /// 1.2.12+ — prompt-language resolution mode.
    /// `"book_defined"` (default) uses the top-level
    /// `language` field — every AI call resolves prompts
    /// against the project's language.
    /// `"paragraph_detected"` runs whatlang on the live
    /// paragraph body and falls back to `book_defined`
    /// when the paragraph is shorter than
    /// `prompt_language_detection_min_chars` of non-
    /// whitespace text (whatlang is unreliable below ~50
    /// chars).  Session-local override via the runtime
    /// chord (Phase C); HJSON is the persistent default.
    /// See `Documentation/PROPOSALS/MULTILINGUAL_PROMPTS.md`.
    #[serde(default = "default_prompt_language_mode")]
    pub prompt_language_mode: String,
    /// 1.2.12+ — minimum non-whitespace character count
    /// the live paragraph must have before
    /// `prompt_language_mode = "paragraph_detected"`
    /// will even attempt whatlang detection.  Below this,
    /// the resolver silently uses the book language.
    #[serde(default = "default_prompt_language_detection_min_chars")]
    pub prompt_language_detection_min_chars: usize,
    /// 1.2.14+ Phase C.1 — author name stamped onto
    /// every inline comment created via `Ctrl+V c`.
    /// When unset (the default), the comment author
    /// resolver falls through to `$USER` →
    /// `$LOGNAME` → `$HOSTNAME` → `"anonymous"`.
    /// Set this when the inferred author is wrong
    /// (shared workstation, system account) or
    /// when the project shares a manuscript across
    /// authors and per-author attribution matters.
    #[serde(default)]
    pub comment_author: Option<String>,
    /// 1.2.14+ Phase Q.2 — HJSON-driven snippet
    /// expansion table.  When `enabled`, the editor
    /// watches for non-word characters typed after a
    /// trigger string and replaces the trigger
    /// inline with the resolved expansion body.
    /// Empty `triggers` map → no expansion fires.
    /// See `Documentation/PROPOSALS/1.2.14_PLAN.md`
    /// §6.
    #[serde(default)]
    pub snippets: SnippetsConfig,
    /// 1.2.14+ Phase Q.3 — number of previous
    /// paragraphs (in canonical hierarchy order)
    /// sent as voice anchors in the AI continuation
    /// drafting prompt envelope (`Ctrl+V d`).
    /// Default 3.  Larger values give the model
    /// more voice context at the cost of prompt
    /// envelope size.
    #[serde(default = "default_continuation_anchor_count")]
    pub continuation_anchor_count: usize,
    /// 1.2.14+ Phase Q.3 — output style for
    /// `Ctrl+V f` inline footnote insertion.
    /// `"typst"` (the default) inserts
    /// `#footnote[<body>]` at the cursor;
    /// `"markdown"` inserts `[^id]` at the cursor
    /// plus a `[^id]: <body>` trailing reference.
    #[serde(default = "default_footnote_style")]
    pub footnote_style: String,

    /// 1.2.16+ Phase A.5 — worldbuilding glossary
    /// chip in the status bar.  When true (the
    /// default), shows `<N>C·<N>P·<N>A` —
    /// cumulative Characters / Places / Artefacts
    /// entry counts.  Auto-hides when all three
    /// are zero (fresh project).  Set false to
    /// reclaim the screen real estate.
    #[serde(default = "default_show_glossary_chip")]
    pub show_glossary_chip: bool,
    /// 1.2.21+ FF.6 — Facts chip in the status bar.
    /// When true, shows `⚑<N>` — the number of entries
    /// in the Facts book — so the world's invariants are
    /// visible at a glance.  Auto-hides when the Facts
    /// book is empty.  Off by default (opt-in).
    #[serde(default)]
    pub show_facts_chip: bool,

    /// 1.2.18+ R.3 — show a status-bar reading-time
    /// chip for the current book: total audiobook /
    /// read-aloud length + the time remaining from the
    /// open paragraph to the book's end, at
    /// `reading_wpm`.  Default off (the status bar is
    /// already busy; opt in when the metric is useful —
    /// e.g. when targeting an audiobook length).
    #[serde(default)]
    pub reading_time_chip: bool,

    /// 1.2.18+ R.3 — words-per-minute used by the
    /// reading-time chip (and the R.4 reader-pace
    /// preview).  200 wpm is the common silent-reading
    /// average; ~150 is a typical narration pace for
    /// audiobooks.
    #[serde(default = "default_reading_wpm")]
    pub reading_wpm: u32,

    /// 1.2.19+ C.1 — window (in consecutive paragraphs)
    /// for the `echo-repetition` doctor scan.  A
    /// distinctive word reused `echo_min_repeats` times
    /// within this many paragraphs is flagged as an echo.
    #[serde(default = "default_echo_window")]
    pub echo_window: usize,

    /// 1.2.19+ C.1 — occurrences within `echo_window`
    /// required to flag an echo.  Lower = more sensitive
    /// (more findings).
    #[serde(default = "default_echo_min_repeats")]
    pub echo_min_repeats: usize,

    /// 1.2.19+ C.1 — distinctiveness ceiling for the echo
    /// scan: words used more than this many times across a
    /// chapter are treated as common vocabulary (which an
    /// author legitimately reuses) and skipped, even when
    /// clustered.  Tune up for longer works, down for
    /// short stories.
    #[serde(default = "default_echo_max_global")]
    pub echo_max_global: usize,

    /// 1.2.20+ R.3.b — read-time threshold (seconds) for
    /// the `paragraph-too-long` doctor scan.  A paragraph
    /// whose estimated read time at `reading_wpm` exceeds
    /// this is flagged (Info, author-judgment).  Default
    /// 180s (~600 words at 200 wpm) — a genuine wall of
    /// text, not a merely dense paragraph.
    #[serde(default = "default_paragraph_long_secs")]
    pub paragraph_long_secs: u32,

    /// 1.2.20+ Phase G — low-disk pre-flight threshold in
    /// MiB.  When the volume holding the project has less
    /// than this much free space, the editor shows a
    /// one-time warning at startup (atomic writes still
    /// fail safely, but this gives a heads-up before a
    /// long export).  `0` disables the check.  Default
    /// 100 MiB.
    #[serde(default = "default_disk_warn_mb")]
    pub disk_warn_mb: u64,

    /// 1.2.20+ Phase G — when quitting, if the project is
    /// a git repo with uncommitted changes (modified,
    /// staged, or untracked), confirm before exiting.
    /// Best-effort: silently skipped when the project
    /// isn't a git repo or `git` isn't installed.  Default
    /// `true`.
    #[serde(default = "default_warn_uncommitted_on_exit")]
    pub warn_uncommitted_on_exit: bool,

    /// 1.2.20+ C.1.b — default state of the live echo
    /// overlay (Ctrl+B Shift+K): underline, in the open
    /// paragraph, words echoing across nearby paragraphs.
    /// The session toggle overrides this.  Default `false`
    /// (opt in per session, or set `true` to always start
    /// on).  Uses the `echo_window` / `echo_min_repeats` /
    /// `echo_max_global` tunables shared with the
    /// `echo-repetition` doctor scan.
    #[serde(default)]
    pub echo_overlay: bool,
}

fn default_warn_uncommitted_on_exit() -> bool {
    true
}

fn default_paragraph_long_secs() -> u32 {
    180
}

fn default_disk_warn_mb() -> u64 {
    100
}

fn default_crash_mirror_seconds() -> u64 {
    2
}

fn default_deleted_paragraph_history() -> usize {
    10
}

fn default_external_change_auto_reload() -> bool {
    true
}

fn default_startup_haiku() -> bool {
    true
}

fn default_haiku_scope() -> String {
    "paragraph".to_string()
}

fn default_fact_check_idle_seconds() -> u64 {
    5
}

fn default_visited_history_cap() -> usize {
    0
}

fn default_echo_window() -> usize {
    5
}

fn default_echo_min_repeats() -> usize {
    3
}

fn default_echo_max_global() -> usize {
    40
}

fn default_continuation_anchor_count() -> usize {
    3
}

fn default_footnote_style() -> String {
    "typst".into()
}

fn default_show_glossary_chip() -> bool {
    true
}

fn default_reading_wpm() -> u32 {
    200
}

/// 1.2.14+ Phase Q.2 — `editor.snippets` HJSON
/// stanza.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnippetsConfig {
    /// Master switch.  When false, no snippet
    /// expansion fires regardless of the `triggers`
    /// map.  Defaults to true so the templates
    /// inkhaven ships with (project-level HJSON
    /// gets `\dt` / `\time` / `\sig` etc.) work
    /// without a flag flip.
    #[serde(default = "default_snippets_enabled")]
    pub enabled: bool,
    /// Map of trigger string → expansion body.
    /// Triggers are matched as substrings at the
    /// END of the buffer up to the cursor — when
    /// the user types a non-word char (space,
    /// punctuation, newline) immediately after a
    /// trigger string, the trigger gets replaced
    /// by the expansion body and the non-word
    /// char stays.  Placeholder syntax inside the
    /// body: `{today}`, `{today:%Y-%m-%d}`,
    /// `{now}`, `{paragraph_title}`,
    /// `{paragraph_slug}`, `{selection}`,
    /// `{author}`.  Unknown placeholders pass
    /// through verbatim so the author can spot
    /// typos.
    #[serde(default)]
    pub triggers: std::collections::HashMap<String, String>,
}

impl Default for SnippetsConfig {
    fn default() -> Self {
        Self {
            enabled: default_snippets_enabled(),
            triggers: std::collections::HashMap::new(),
        }
    }
}

fn default_snippets_enabled() -> bool {
    true
}

fn default_pov_chip_enabled() -> bool {
    true
}

fn default_prompt_language_mode() -> String {
    "book_defined".into()
}

fn default_prompt_language_detection_min_chars() -> usize {
    50
}

/// 1.2.9+ — `editor.style_warnings.*` HJSON stanza.
/// Enables inline highlighting of stylistically weak
/// prose constructs: filter words first (this release),
/// repeated phrases / show-don't-tell / sentence-rhythm
/// next.  All detectors are off by default and toggled
/// individually so a user who only wants filter-word
/// flagging doesn't get adverb noise.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StyleWarningsConfig {
    /// Master enable for the in-editor style warning
    /// overlays.  `false` disables every detector
    /// regardless of the per-detector flags.  Runtime
    /// `Ctrl+V w` toggle flips a session-only override
    /// without rewriting HJSON.
    pub enabled: bool,
    /// Filter-word detector: flag intensifier crutches
    /// + hedges (`just`, `really`, `very`, `просто`,
    /// `очень`, …).  Built-in lists ship for English,
    /// Russian, French, German, Spanish; the active list
    /// is keyed by the project's top-level `language`
    /// field.  `extra_words` is a user union added on
    /// top of the language default — empty by default.
    pub filter_words: FilterWordsConfig,
    /// 1.2.9+ — repeated-phrase detector.  Slides an
    /// `n`-word window across the open paragraph,
    /// stems each window with the project's Snowball
    /// algorithm, and flags every occurrence of any
    /// n-gram that repeats `threshold` or more times.
    /// `lifted her shoulders` matches `lifting her
    /// shoulders` (stems align), so a writer's
    /// favourite gesture surfaces immediately.
    /// Multilingual via the same Snowball setup as
    /// filter-words — no language-specific tuning
    /// needed beyond setting the top-level `language`.
    pub repeated_phrases: RepeatedPhrasesConfig,
    /// 1.2.9+ — show-don't-tell detector.  Flags
    /// "telling" prose patterns: copula + emotion-
    /// adjective (`she was angry`, `Il était triste`),
    /// manner-of-emotion adverbs (`angrily`,
    /// `sadly`), and direct cognition verbs that label
    /// internal state for the reader (`realised`,
    /// `understood`, `knew`).  Inline overlay shares
    /// the master toggle.  See `ShowDontTellConfig`
    /// for per-language knobs.
    pub show_dont_tell: ShowDontTellConfig,
    /// 1.3.8 — anachronism detector. Set `anachronism.year` to the
    /// manuscript's setting; terms in the built-in lexicon (plus your
    /// `terms` additions) whose earliest plausible year is *after* the
    /// setting are flagged ("wristwatch" in an 1840 novel). Off until a
    /// year is set.
    #[serde(default)]
    pub anachronism: AnachronismConfig,
}

/// 1.3.8 `facts:` block — series-shared canon. `shared_path` points at a
/// directory of plain-text fact files (one fact per file: the file stem is
/// the title, its contents the body), shared by every book of a series so
/// the canon lives in one place. Layered into `facts check` (local wins on
/// a title clash); copied in with `inkhaven facts import`.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct FactsConfig {
    pub shared_path: Option<String>,
}

/// 1.3.10 WORLD-2 — `drift` semantic-drift retrieval tuning. `top_k` vector
/// hits are pulled per entity, then name-filtered + capped at `max_snippets`
/// (which bounds the AI judge prompt).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct DriftConfig {
    /// Vector hits pulled per entity before name-filtering + capping.
    pub top_k: usize,
    /// Max description snippets kept per entity (bounds the judge prompt).
    pub max_snippets: usize,
    /// 1.3.13 — per-language coref pronoun sets for ANY language (keyed by
    /// lowercased name). Written by `inkhaven lang bootstrap`; takes precedence
    /// over the built-in five.
    pub pronouns: std::collections::BTreeMap<String, PronounLangSet>,
}

/// 1.3.13 — the coref pronoun sets for one (bootstrapped) language, by entity
/// kind (lowercased pronouns; matched as whole words).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct PronounLangSet {
    pub character: Vec<String>,
    pub place: Vec<String>,
    pub artefact: Vec<String>,
}

impl Default for DriftConfig {
    fn default() -> Self {
        Self {
            top_k: 24,
            max_snippets: 8,
            pronouns: std::collections::BTreeMap::new(),
        }
    }
}

/// `editor.style_warnings.anachronism.*` — the setting year + any
/// project-specific period-bound terms (each with its earliest plausible
/// year). Empty / no year → the detector is off.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AnachronismConfig {
    /// The manuscript's setting year (e.g. `1840`). `None` disables the
    /// detector.
    pub year: Option<i32>,
    /// Project additions / overrides to the built-in lexicon.
    pub terms: Vec<AnachronismTerm>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnachronismTerm {
    pub term: String,
    /// The earliest year the term/concept plausibly appears.
    pub earliest: i32,
}

/// 1.2.9+ — `editor.style_warnings.show_dont_tell.*`
/// HJSON stanza.  Three lists per language:
///   * `*_linking_verbs` — `be`, `seem`, `feel`,
///     `look`, `appear`, `become`.  Used as
///     pattern-anchor in the 2-gram `(verb)(adj)`
///     detector.  Snowball-stemmed at init time so
///     `was` / `is` / `were` all key on `be`.
///   * `*_emotion_adjectives` — `angry`, `sad`,
///     `happy`, `afraid`, …  Triggered as the
///     second token of the 2-gram pattern.
///   * `*_manner_adverbs` — `angrily`, `sadly`,
///     `nervously`, …  Flagged on their own — these
///     adverbs almost always label emotion outright.
///   * `*_cognition_verbs` — `realised`, `knew`,
///     `understood`, `wondered`, `decided`, …
///     Flagged on their own.
///
/// Empty configured list = use built-in default for
/// that language; non-empty = REPLACE the default.
/// Same rule as `filter_words`.  English ships with
/// curated lists; the other languages start empty so
/// users can fill them in for their corpus.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ShowDontTellConfig {
    pub enabled: bool,
    /// Apply Snowball stemming before matching so
    /// inflections collapse (e.g. `was` / `is` /
    /// `were` all match a single `be` entry).
    /// Disable for exact-form matching.
    pub use_stemming: bool,
    // English defaults populated via `built_in_*` —
    // configured lists override.
    pub english_linking_verbs: Vec<String>,
    pub english_emotion_adjectives: Vec<String>,
    pub english_manner_adverbs: Vec<String>,
    pub english_cognition_verbs: Vec<String>,
    pub russian_linking_verbs: Vec<String>,
    pub russian_emotion_adjectives: Vec<String>,
    pub russian_manner_adverbs: Vec<String>,
    pub russian_cognition_verbs: Vec<String>,
    pub french_linking_verbs: Vec<String>,
    pub french_emotion_adjectives: Vec<String>,
    pub french_manner_adverbs: Vec<String>,
    pub french_cognition_verbs: Vec<String>,
    pub german_linking_verbs: Vec<String>,
    pub german_emotion_adjectives: Vec<String>,
    pub german_manner_adverbs: Vec<String>,
    pub german_cognition_verbs: Vec<String>,
    pub spanish_linking_verbs: Vec<String>,
    pub spanish_emotion_adjectives: Vec<String>,
    pub spanish_manner_adverbs: Vec<String>,
    pub spanish_cognition_verbs: Vec<String>,
    /// 1.3.13 — per-language show-don't-tell lists for ANY language (keyed by
    /// lowercased name). Written by `inkhaven lang bootstrap`; takes precedence.
    #[serde(default)]
    pub languages: std::collections::BTreeMap<String, SdtLangLists>,
}

/// 1.3.13 — the four show-don't-tell lists for one (bootstrapped) language.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SdtLangLists {
    pub linking_verbs: Vec<String>,
    pub emotion_adjectives: Vec<String>,
    pub manner_adverbs: Vec<String>,
    pub cognition_verbs: Vec<String>,
}

impl Default for ShowDontTellConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            use_stemming: true,
            english_linking_verbs: Vec::new(),
            english_emotion_adjectives: Vec::new(),
            english_manner_adverbs: Vec::new(),
            english_cognition_verbs: Vec::new(),
            russian_linking_verbs: Vec::new(),
            russian_emotion_adjectives: Vec::new(),
            russian_manner_adverbs: Vec::new(),
            russian_cognition_verbs: Vec::new(),
            french_linking_verbs: Vec::new(),
            french_emotion_adjectives: Vec::new(),
            french_manner_adverbs: Vec::new(),
            french_cognition_verbs: Vec::new(),
            german_linking_verbs: Vec::new(),
            german_emotion_adjectives: Vec::new(),
            german_manner_adverbs: Vec::new(),
            german_cognition_verbs: Vec::new(),
            spanish_linking_verbs: Vec::new(),
            spanish_emotion_adjectives: Vec::new(),
            spanish_manner_adverbs: Vec::new(),
            spanish_cognition_verbs: Vec::new(),
            languages: std::collections::BTreeMap::new(),
        }
    }
}

/// 1.2.9+ — built-in show-don't-tell lists per
/// language.  English ships with curated lists drawn
/// from common writing-craft references; other
/// languages return empty slices so the user can fill
/// them in for their corpus (Russian / French /
/// German / Spanish emotion vocabulary varies enough
/// per genre that defaults would mislead more than
/// help).
pub fn built_in_linking_verbs(language: &str) -> &'static [&'static str] {
    // 1.2.11+ — built-ins now ship for all five
    // supported languages.  Conservative, dictionary-
    // shape lemmas; per-genre tuning belongs in
    // `inkhaven show-dont-tell bootstrap <lang>`,
    // which emits a richer HJSON snippet a user can
    // paste over these defaults.  Snowball stemming is
    // applied at match time so a handful of common
    // inflections cover the rest.
    match language.to_lowercase().as_str() {
        "english" | "" => &[
            "be", "is", "am", "are", "was", "were", "been", "being",
            "seem", "seems", "seemed", "seeming",
            "feel", "feels", "felt", "feeling",
            "appear", "appears", "appeared", "appearing",
            "look", "looks", "looked", "looking",
            "become", "becomes", "became", "becoming",
            "remain", "remains", "remained", "remaining",
            "grow", "grows", "grew", "growing",
            "sound", "sounds", "sounded",
        ],
        "russian" => &[
            // быть — copula in past + present-zero +
            // future forms.  Russian drops the present-
            // tense copula in prose, so the detector
            // mostly fires on past + future.
            "быть", "был", "была", "было", "были",
            "буду", "будешь", "будет", "будем", "будете", "будут",
            "есть",
            // казаться — "to seem"
            "казаться", "кажется", "казался", "казалась", "казалось", "казались",
            // выглядеть — "to look (like)"
            "выглядеть", "выглядит", "выглядел", "выглядела", "выглядело",
            // становиться / стать — "to become"
            "становиться", "становится", "становился", "становилась",
            "стать", "стал", "стала", "стало", "стали",
            // оставаться — "to remain"
            "оставаться", "остаётся", "оставался", "оставалась",
            // чувствовать (себя) — "to feel"
            "чувствовать", "чувствует", "чувствовал", "чувствовала",
            // оказаться — "to turn out / appear to be"
            "оказаться", "оказался", "оказалась", "оказалось",
        ],
        "french" => &[
            // être
            "être", "est", "sont", "étais", "était", "étions", "étiez", "étaient",
            "fus", "fut", "fûmes", "furent",
            "sera", "seront", "serait", "seraient",
            // paraître / sembler
            "paraître", "paraît", "paraissait", "paraissent",
            "sembler", "semble", "semblait", "semblent",
            // devenir / rester / demeurer
            "devenir", "devient", "devenait", "deviennent",
            "rester", "reste", "restait", "restent",
            "demeurer", "demeure", "demeurait",
            // se sentir / avoir l'air
            "sentir", "sent", "sentait",
            "avoir", "a", "avait", "ont",
        ],
        "german" => &[
            // sein
            "sein", "ist", "sind", "war", "waren", "bin", "bist", "seid",
            "gewesen",
            // scheinen / wirken
            "scheinen", "scheint", "schien", "schienen",
            "wirken", "wirkt", "wirkte", "wirkten",
            // werden / bleiben / aussehen
            "werden", "wird", "wurde", "wurden", "geworden",
            "bleiben", "bleibt", "blieb", "blieben",
            "aussehen", "sieht", "sah",
            // fühlen (sich)
            "fühlen", "fühlt", "fühlte", "fühlten",
        ],
        "spanish" => &[
            // ser
            "ser", "es", "son", "era", "eran", "fue", "fueron",
            "será", "serán", "sería", "serían",
            // estar
            "estar", "está", "están", "estaba", "estaban", "estuvo", "estuvieron",
            // parecer / sentirse / quedar(se)
            "parecer", "parece", "parecía", "parecen",
            "sentir", "sentirse", "siente", "sentía",
            "quedar", "quedarse", "queda", "quedaba",
            // volverse / ponerse / hallarse / encontrarse
            "volverse", "vuelve", "volvía",
            "ponerse", "pone", "puso", "ponía",
            "encontrarse", "encuentra", "encontraba",
        ],
        _ => &[],
    }
}

pub fn built_in_emotion_adjectives(language: &str) -> &'static [&'static str] {
    // 1.2.11+ — defaults for RU/FR/DE/ES.  Cover the
    // major emotion families (anger / sadness / fear /
    // joy / fatigue / surprise / shame); per-genre
    // additions belong in
    // `inkhaven show-dont-tell bootstrap`.
    match language.to_lowercase().as_str() {
        "english" | "" => &[
            // Anger family
            "angry", "mad", "furious", "livid", "irate", "enraged",
            "annoyed", "irritated", "agitated",
            // Sadness family
            "sad", "depressed", "melancholy", "gloomy", "miserable",
            "unhappy", "dejected", "downcast", "forlorn",
            // Fear family
            "afraid", "scared", "frightened", "terrified", "anxious",
            "nervous", "worried", "uneasy", "panicked", "apprehensive",
            // Joy family
            "happy", "joyful", "glad", "content", "pleased", "delighted",
            "thrilled", "elated", "ecstatic", "cheerful",
            // Fatigue family
            "tired", "exhausted", "weary", "drained", "spent",
            // Confusion family
            "confused", "puzzled", "perplexed", "baffled",
            // Surprise family
            "surprised", "shocked", "stunned", "astonished", "amazed",
            // Shame family
            "embarrassed", "ashamed", "humiliated", "mortified",
            // Pride / envy / loneliness
            "proud", "smug",
            "jealous", "envious",
            "lonely", "isolated",
            // Boredom
            "bored", "listless", "restless",
            // Excitement (low intensity)
            "excited", "eager", "enthusiastic",
            // Determination / despair
            "determined", "resolute",
            "hopeless", "helpless", "defeated",
        ],
        "russian" => &[
            // Anger
            "сердитый", "злой", "разгневанный", "раздражённый",
            // Sadness
            "грустный", "печальный", "несчастный", "унылый", "тоскливый",
            // Fear
            "испуганный", "напуганный", "тревожный", "встревоженный", "испугавшийся",
            // Joy
            "счастливый", "радостный", "довольный", "весёлый", "восторженный",
            // Fatigue
            "усталый", "измождённый", "утомлённый", "обессиленный",
            // Surprise
            "удивлённый", "поражённый", "ошеломлённый", "изумлённый",
            // Confusion
            "растерянный", "смущённый", "озадаченный",
            // Shame
            "пристыженный", "сконфуженный",
            // Pride / envy / loneliness / boredom
            "гордый", "ревнивый", "завистливый", "одинокий", "скучающий",
            // Excitement / determination / despair
            "взволнованный", "возбуждённый",
            "решительный",
            "безнадёжный", "беспомощный",
        ],
        "french" => &[
            // Anger
            "furieux", "furieuse", "en colère", "fâché", "fâchée",
            "irrité", "irritée", "agacé", "agacée",
            // Sadness
            "triste", "malheureux", "malheureuse", "mélancolique", "abattu", "abattue",
            // Fear
            "effrayé", "effrayée", "apeuré", "apeurée",
            "anxieux", "anxieuse", "inquiet", "inquiète", "nerveux", "nerveuse",
            // Joy
            "heureux", "heureuse", "joyeux", "joyeuse",
            "ravi", "ravie", "content", "contente",
            // Fatigue
            "fatigué", "fatiguée", "épuisé", "épuisée", "las", "lasse",
            // Surprise
            "surpris", "surprise", "étonné", "étonnée", "stupéfait", "stupéfaite",
            // Confusion / shame
            "confus", "confuse", "perplexe",
            "honteux", "honteuse", "gêné", "gênée",
            // Pride / envy / loneliness / boredom / excitement
            "fier", "fière", "jaloux", "jalouse", "envieux", "envieuse",
            "seul", "seule", "ennuyé", "ennuyée",
            "excité", "excitée", "enthousiaste",
            // Despair
            "désespéré", "désespérée", "impuissant", "impuissante",
        ],
        "german" => &[
            // Anger
            "wütend", "zornig", "verärgert", "gereizt",
            // Sadness
            "traurig", "betrübt", "niedergeschlagen", "trübselig", "unglücklich",
            // Fear
            "ängstlich", "verängstigt", "besorgt", "nervös", "panisch",
            // Joy
            "glücklich", "fröhlich", "erfreut", "zufrieden", "begeistert",
            // Fatigue
            "müde", "erschöpft", "ermattet",
            // Surprise
            "überrascht", "erstaunt", "verblüfft", "schockiert",
            // Confusion / shame
            "verwirrt", "verlegen", "beschämt",
            // Pride / envy / loneliness / boredom / excitement / despair
            "stolz", "eifersüchtig", "neidisch",
            "einsam", "gelangweilt",
            "aufgeregt", "entschlossen",
            "hoffnungslos", "hilflos",
        ],
        "spanish" => &[
            // Anger
            "enfadado", "enfadada", "enojado", "enojada", "furioso", "furiosa",
            "irritado", "irritada",
            // Sadness
            "triste", "afligido", "afligida", "deprimido", "deprimida",
            "melancólico", "melancólica", "desdichado", "desdichada",
            // Fear
            "asustado", "asustada", "aterrado", "aterrada",
            "ansioso", "ansiosa", "nervioso", "nerviosa", "preocupado", "preocupada",
            // Joy
            "feliz", "alegre", "contento", "contenta", "encantado", "encantada",
            // Fatigue
            "cansado", "cansada", "agotado", "agotada", "exhausto", "exhausta",
            // Surprise
            "sorprendido", "sorprendida", "asombrado", "asombrada", "atónito", "atónita",
            // Confusion / shame
            "confundido", "confundida", "perplejo", "perpleja",
            "avergonzado", "avergonzada",
            // Pride / envy / loneliness / boredom / excitement / despair
            "orgulloso", "orgullosa", "celoso", "celosa", "envidioso", "envidiosa",
            "solo", "sola", "aburrido", "aburrida",
            "emocionado", "emocionada", "decidido", "decidida",
            "desesperado", "desesperada", "impotente",
        ],
        _ => &[],
    }
}

pub fn built_in_manner_adverbs(language: &str) -> &'static [&'static str] {
    // 1.2.11+ — defaults for RU/FR/DE/ES.  Emotion-
    // labelling adverbs (the `-ly` family in English,
    // `-о/-е` in Russian, `-ment` in French, `-mente`
    // in Spanish, plain adjective-form in German for
    // adverbial use).
    match language.to_lowercase().as_str() {
        "english" | "" => &[
            "angrily", "sadly", "happily", "fearfully", "nervously",
            "anxiously", "calmly", "frantically", "wearily", "tiredly",
            "excitedly", "gleefully", "miserably", "joyfully",
            "furiously", "irritably", "annoyedly", "bitterly",
            "proudly", "smugly", "jealously", "enviously",
            "lovingly", "tenderly", "coldly", "warmly",
            "desperately", "hopelessly", "helplessly",
            "embarrassedly", "shamefully", "guiltily",
            "bored", "boredly", "listlessly",
            "confusedly",
        ],
        "russian" => &[
            "сердито", "злобно", "раздражённо",
            "грустно", "печально", "уныло", "тоскливо",
            "испуганно", "тревожно", "нервно",
            "счастливо", "радостно", "весело",
            "устало", "измождённо",
            "удивлённо", "поражённо",
            "растерянно", "смущённо",
            "гордо", "ревниво", "одиноко",
            "взволнованно", "решительно",
            "безнадёжно", "беспомощно",
            "холодно", "тепло",
            "горько", "нежно",
        ],
        "french" => &[
            "furieusement", "rageusement", "tristement", "mélancoliquement",
            "peureusement", "nerveusement", "anxieusement",
            "joyeusement", "heureusement", "gaiement",
            "fatiguement",
            "tendrement", "amoureusement", "froidement", "chaleureusement",
            "fièrement", "jalousement", "envieusement",
            "désespérément", "honteusement", "calmement",
            "amèrement", "douloureusement",
        ],
        "german" => &[
            "wütend", "zornig", "ärgerlich",
            "traurig", "betrübt", "unglücklich",
            "ängstlich", "nervös", "besorgt",
            "fröhlich", "glücklich", "freudig",
            "müde", "erschöpft",
            "überrascht", "verwirrt",
            "stolz", "eifersüchtig",
            "einsam", "gelangweilt",
            "aufgeregt", "verzweifelt", "hilflos",
            "kalt", "warm", "liebevoll", "bitter",
        ],
        "spanish" => &[
            "furiosamente", "rabiosamente", "enojadamente",
            "tristemente", "melancólicamente",
            "miedosamente", "nerviosamente", "ansiosamente",
            "felizmente", "alegremente", "gozosamente",
            "cansadamente",
            "sorprendidamente",
            "orgullosamente", "celosamente", "envidiosamente",
            "solamente", "aburridamente",
            "desesperadamente", "vergonzosamente",
            "fríamente", "cálidamente", "amorosamente", "amargamente",
        ],
        _ => &[],
    }
}

pub fn built_in_cognition_verbs(language: &str) -> &'static [&'static str] {
    // 1.2.11+ — defaults for RU/FR/DE/ES.  Verbs that
    // narrate thought instead of showing it.  Past-
    // tense forms dominate because that's where the
    // "she realised" / "elle comprit" telling pattern
    // typically lands in fiction.
    match language.to_lowercase().as_str() {
        "english" | "" => &[
            "realised", "realized",
            "understood", "knew", "thought",
            "wondered", "wished", "hoped",
            "believed", "supposed", "decided",
            "concluded", "discovered", "recognised", "recognized",
            "remembered", "considered",
            "assumed", "expected",
        ],
        "russian" => &[
            "понял", "поняла", "понять", "понимал", "понимала",
            "знал", "знала", "знать",
            "подумал", "подумала", "думать",
            "осознал", "осознала", "осознать",
            "решил", "решила", "решить",
            "вспомнил", "вспомнила", "вспомнить",
            "заметил", "заметила",
            "почувствовал", "почувствовала",
            "поверил", "поверила", "верить",
            "догадался", "догадалась",
        ],
        "french" => &[
            "réalisa", "réalisé", "réaliser",
            "comprit", "compris", "comprendre",
            "sut", "su", "savoir",
            "pensa", "pensé", "penser",
            "songea", "songer",
            "décida", "décidé", "décider",
            "se souvint", "se rappela",
            "crut", "cru", "croire",
            "supposa", "supposer",
            "remarqua", "aperçut",
        ],
        "german" => &[
            "erkannte", "erkannt", "erkennen",
            "verstand", "verstanden", "verstehen",
            "wusste", "gewusst", "wissen",
            "dachte", "gedacht", "denken",
            "überlegte", "überlegt",
            "beschloss", "entschied", "entschieden",
            "erinnerte", "erinnert",
            "bemerkte", "bemerkt",
            "glaubte", "geglaubt",
            "vermutete",
        ],
        "spanish" => &[
            "se dio cuenta", "comprendió", "comprender",
            "entendió", "entender",
            "supo", "sabía", "saber",
            "pensó", "pensar",
            "creyó", "creer", "creía",
            "decidió", "decidir",
            "recordó", "recordar",
            "notó", "advirtió",
            "supuso", "esperaba",
            "concluyó",
        ],
        _ => &[],
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RepeatedPhrasesConfig {
    pub enabled: bool,
    /// Number of consecutive words to compare.  4 is
    /// the sweet spot — 3 catches too many incidental
    /// "she said the X" patterns, 5+ misses most
    /// writer-crutches.
    pub n: u8,
    /// Flag when an n-gram appears at least this many
    /// times in the paragraph.  3 is the default — a
    /// phrase has to occur 3 times before it's worth
    /// flagging; twice is often a deliberate echo.
    pub threshold: u8,
    /// Apply Snowball stemming to align inflections
    /// before n-gram comparison.  Default `true`.
    pub use_stemming: bool,
    /// 1.2.9+ — stop-word list per language: words
    /// excluded from n-gram comparison so common
    /// connectives (`the`, `and`, `и`, `в`) don't
    /// inflate the count.  Empty list = use built-in
    /// default for the active language.  Same lookup
    /// rule as filter-words.  Built-in lists are
    /// conservative (closed-class words only); users
    /// extend via this field.
    pub english_stop_words: Vec<String>,
    pub russian_stop_words: Vec<String>,
    pub french_stop_words: Vec<String>,
    pub german_stop_words: Vec<String>,
    pub spanish_stop_words: Vec<String>,
    /// 1.3.13 — per-language stop-words for ANY language (keyed by lowercased
    /// name). Written by `inkhaven lang bootstrap`; takes precedence.
    #[serde(default)]
    pub languages: std::collections::BTreeMap<String, Vec<String>>,
}

impl Default for RepeatedPhrasesConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            n: 4,
            threshold: 3,
            use_stemming: true,
            english_stop_words: Vec::new(),
            russian_stop_words: Vec::new(),
            french_stop_words: Vec::new(),
            german_stop_words: Vec::new(),
            spanish_stop_words: Vec::new(),
            languages: std::collections::BTreeMap::new(),
        }
    }
}

/// 1.2.9+ — built-in stop-word list per language.
/// Conservative: only function words that almost never
/// carry meaning.  Users extend via the per-language
/// `*_stop_words` fields when an n-gram with a domain
/// word feels noisy in their writing.
pub fn built_in_stop_words(language: &str) -> &'static [&'static str] {
    match language.to_lowercase().as_str() {
        "russian" => &[
            "и", "в", "на", "не", "с", "что", "это", "как",
            "а", "по", "из", "у", "от", "к", "за", "о",
            "но", "же", "так", "то", "бы", "ли", "вот",
            "только", "ещё", "также", "был", "была",
            "было", "были", "есть",
        ],
        "french" => &[
            "le", "la", "les", "un", "une", "des", "de",
            "du", "et", "à", "au", "aux", "en", "dans",
            "pour", "par", "sur", "avec", "sans", "que",
            "qui", "ce", "se", "il", "elle", "ils",
            "elles", "ne", "pas",
        ],
        "german" => &[
            "der", "die", "das", "den", "dem", "des",
            "ein", "eine", "und", "in", "zu", "von", "mit",
            "auf", "ist", "war", "sind", "waren", "es",
            "er", "sie", "wir", "du", "ich", "nicht",
        ],
        "spanish" => &[
            "el", "la", "los", "las", "un", "una", "y",
            "de", "del", "en", "a", "con", "por", "para",
            "que", "no", "es", "son", "se", "su", "lo",
        ],
        "english" | "en" | "" => &[
            "the", "a", "an", "and", "or", "but", "of",
            "to", "in", "on", "at", "by", "for", "with",
            "as", "is", "was", "were", "are", "be",
            "been", "being", "have", "has", "had", "do",
            "does", "did", "it", "he", "she", "they",
            "we", "you", "his", "her", "their", "its",
            "this", "that", "these", "those", "not", "no",
        ],
        // 1.3.13 — non-curated language → no built-in stop-words (the
        // repeated-phrase detector still runs, just without stop-word
        // filtering), never English stop-words on foreign prose.
        _ => &[],
    }
}

impl Default for StyleWarningsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            filter_words: FilterWordsConfig::default(),
            repeated_phrases: RepeatedPhrasesConfig::default(),
            show_dont_tell: ShowDontTellConfig::default(),
            anachronism: AnachronismConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FilterWordsConfig {
    pub enabled: bool,
    /// 1.2.9+ — match via Snowball stemming so `seemed`
    /// catches `seems` / `seeming`, `казалось` catches
    /// `казался` / `казалась` / `казались`, and the
    /// per-language list stays compact (one entry per
    /// lemma, not per inflection).  Default `true`.
    /// Disable to fall back to exact-lowercased match
    /// (faster, but you'd need to list every form
    /// manually).
    pub use_stemming: bool,
    /// User-supplied words added on top of the language
    /// default.  Case-insensitive; one entry per word.
    /// Stems with the language stemmer when
    /// `use_stemming` is on, so `["lift"]` flags
    /// `lifted` / `lifts` / `lifting`.
    pub extra_words: Vec<String>,
    /// Per-language curated lists.  Empty list means
    /// "use the built-in default for this language";
    /// any non-empty list **replaces** the default
    /// (use `extra_words` for additive overrides).  The
    /// active list is keyed by the project's top-level
    /// `language` field; unknown languages fall back
    /// to `english`.  Default values shipped by
    /// `built_in_filter_words()` — run
    /// `inkhaven doctor --filter-words-snippet` to get
    /// a copy-paste-ready HJSON dump.
    pub english: Vec<String>,
    pub russian: Vec<String>,
    pub french: Vec<String>,
    pub german: Vec<String>,
    pub spanish: Vec<String>,
    /// 1.3.13 — per-language lists for ANY language, keyed by lowercased name
    /// (e.g. `italian`). Written by `inkhaven lang bootstrap`; takes precedence
    /// over the fixed fields + built-ins. Lets a project enable a language
    /// beyond the curated five.
    #[serde(default)]
    pub languages: std::collections::BTreeMap<String, Vec<String>>,
}

impl Default for FilterWordsConfig {
    fn default() -> Self {
        // Defaults left empty so an HJSON dumped from a
        // bare Config doesn't carry 100+ lines of
        // language-specific lists.  Empty list at the
        // detector means "use the built-in default" —
        // see `built_in_filter_words()`.  Users who
        // want the defaults visible in their HJSON can
        // populate the arrays from
        // `inkhaven doctor --filter-words-snippet`.
        Self {
            enabled: true,
            use_stemming: true,
            extra_words: Vec::new(),
            english: Vec::new(),
            russian: Vec::new(),
            french: Vec::new(),
            german: Vec::new(),
            spanish: Vec::new(),
            languages: std::collections::BTreeMap::new(),
        }
    }
}

/// 1.2.9+ — accessor for the user's per-language list.
/// Returns the configured list when non-empty;
/// otherwise the built-in default.  Caller passes
/// `language` from `cfg.language`.  Currently only
/// referenced from tests + future detectors that don't
/// want to duplicate the lookup logic; kept under
/// `#[allow(dead_code)]` so it survives the unused-
/// helper lint while remaining a documented surface.
#[allow(dead_code)]
pub fn effective_filter_words<'a>(
    cfg: &'a FilterWordsConfig,
    language: &str,
) -> &'a [String] {
    let configured: &Vec<String> = match language.to_lowercase().as_str() {
        "russian" => &cfg.russian,
        "french" => &cfg.french,
        "german" => &cfg.german,
        "spanish" => &cfg.spanish,
        _ => &cfg.english,
    };
    if !configured.is_empty() {
        return configured.as_slice();
    }
    // Fall back to the built-in default for that
    // language.  `built_in_filter_words` returns a
    // `&'static [&'static str]` which we can't return
    // as `&[String]` directly without allocating; the
    // detector calls `built_in_filter_words` separately
    // when this returns an empty slice.
    &[]
}

/// 1.2.9+ — built-in per-language filter-word lists.
/// Public so `inkhaven doctor --filter-words-snippet`
/// can emit them for paste-into-HJSON.
pub fn built_in_filter_words(language: &str) -> &'static [&'static str] {
    match language.to_lowercase().as_str() {
        "russian" => BUILT_IN_RUSSIAN,
        "french" => BUILT_IN_FRENCH,
        "german" => BUILT_IN_GERMAN,
        "spanish" => BUILT_IN_SPANISH,
        "english" | "en" | "" => BUILT_IN_ENGLISH,
        // 1.3.13 — a non-curated language gets an EMPTY list (the detector is
        // off), never English words flagged in foreign prose. Bootstrap or
        // configure a list to enable it.
        _ => &[],
    }
}

const BUILT_IN_ENGLISH: &[&str] = &[
    // Hedges / intensifier crutches.  Use stems where
    // it matters — `seem` covers `seemed` / `seems` /
    // `seeming` via Snowball.
    "just", "really", "very", "pretty", "quite",
    "rather", "fairly", "somewhat", "slightly",
    "that", "actually", "basically", "literally",
    "essentially", "simply", "definitely", "certainly",
    "absolutely", "totally", "completely",
    // Sensory / hedging verbs — listed as base form;
    // stemmer catches the inflections.
    "seem", "feel", "look", "appear", "sound", "notice",
    "begin", "start",
    "suddenly", "perhaps", "maybe",
];

const BUILT_IN_RUSSIAN: &[&str] = &[
    // Intensifier crutches + hedges
    "очень", "просто", "именно", "довольно", "слишком",
    "весьма", "крайне", "вполне", "достаточно",
    // Generic placeholders
    "собственно", "буквально", "практически",
    "фактически", "действительно", "реально",
    "конечно", "разумеется", "безусловно",
    // Sensory / hedging verbs as lemmas — Snowball
    // stems both list entry and editor text, so
    // `казаться` catches `казался / казалась /
    // казалось / казались`.
    "казаться", "почувствовать", "выглядеть",
    "заметить",
    "вдруг", "внезапно", "наверное", "возможно",
];

const BUILT_IN_FRENCH: &[&str] = &[
    "vraiment", "très", "assez", "plutôt",
    "juste", "simplement", "actuellement", "littéralement",
    "essentiellement", "absolument", "totalement", "complètement",
    "sembler", "paraître", "sentir",
    "soudainement", "peut-être",
];

const BUILT_IN_GERMAN: &[&str] = &[
    "sehr", "wirklich", "ziemlich", "eher", "etwas",
    "einfach", "tatsächlich", "buchstäblich",
    "absolut", "völlig", "komplett",
    "scheinen", "fühlen", "sehen",
    "plötzlich", "vielleicht",
];

const BUILT_IN_SPANISH: &[&str] = &[
    "muy", "realmente", "bastante", "algo",
    "solo", "simplemente", "actualmente", "literalmente",
    "esencialmente", "absolutamente", "totalmente", "completamente",
    "parecer", "sentir", "ver",
    "repentinamente", "quizás",
];

/// 1.2.9+ — `editor.tts.*` HJSON stanza.  `Ctrl+B S` in
/// the editor pane reads the open paragraph aloud via
/// the host OS's TTS engine.  Default voice is `Milena`
/// (Russian female, ships free with macOS + Windows
/// after a one-time language download).  The match is a
/// case-insensitive substring search against installed
/// voice names — "Milena" picks the standard or the
/// "Milena (Enhanced)" / "Milena (Premium)" variant
/// when available.  `speed` is a multiplier over the
/// engine's "normal" rate (0.8 = 80%, 1.2 = 120%).
/// Clamped to the engine's `[min_rate, max_rate]` at
/// playback time.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TtsConfig {
    pub enabled: bool,
    pub voice: String,
    pub speed: f32,
    /// 1.2.9+ — text spoken at TUI startup, just after the
    /// daily-progress splash finishes.  Empty string (the
    /// default) skips the greeting entirely.  Use this for
    /// a personal welcome — "Welcome back, captain", "Доброе
    /// утро, Владимир", etc.  Honoured only when
    /// `enabled = true`.  Non-blocking: speech starts and
    /// the editor lands on the cursor while audio plays
    /// in parallel.
    pub greeting: String,
    /// 1.2.9+ — text spoken at TUI shutdown, just before
    /// the terminal tears down.  Empty string (the default)
    /// skips it.  Blocking: inkhaven waits up to 5 seconds
    /// for the speech to complete before returning, so the
    /// shell doesn't truncate the audio mid-word.  Keep it
    /// short (a few words).
    pub goodbye: String,

    // ── 1.2.17+ — Piper transition fields ─────────────────

    /// 1.2.17+ — selects the TTS backend.  Three values:
    ///   * `"auto"` (default) — prefer Piper if the binary
    ///     resolves; fall back to the 1.2.9 `system`
    ///     backend (macOS `/usr/bin/say`).  The status-bar
    ///     diagnostic chip reports which is active.
    ///   * `"piper"` — force Piper; error if unresolvable
    ///     instead of falling back.
    ///   * `"system"` — force the 1.2.9 macOS backend;
    ///     errors on non-macOS hosts.
    ///
    /// T.1 (1.2.17): the Piper backend is a stub that
    /// always errors on construction, so `"auto"` falls
    /// through to `"system"` on every host.  Real
    /// dispatch lands in T.2+.
    pub engine: String,

    /// 1.2.17+ — directory for Piper voice models +
    /// catalog cache.  Resolved via
    /// `crate::path_safety::resolve_within(project_root,
    /// voices_dir)` so a malicious project can't escape
    /// into `~/.ssh/`.  Relative paths join the project
    /// root (default `.inkhaven/voices` lives there);
    /// absolute paths are rejected.
    pub voices_dir: String,

    /// 1.2.17+ — when `true`, missing voice models are
    /// streamed from the Hugging Face catalog on first
    /// use.  When `false`, missing voices produce a clear
    /// "voice X is not downloaded; run `inkhaven tts
    /// voice download X`" error.
    pub auto_download: bool,

    /// 1.2.17+ — Piper voice catalog URL.  Defaults to
    /// the upstream Hugging Face manifest.  Override only
    /// if you maintain a private / mirrored catalog with
    /// the same JSON shape.
    pub catalog_url: String,

    /// 1.2.17+ — how long the local catalog cache is
    /// fresh, in hours.  After expiry the next voice
    /// operation re-fetches.  Network failures during
    /// refresh fall back to the stale cache + log a
    /// warning rather than blocking synthesis.
    pub catalog_ttl_hours: u32,

    /// 1.2.17+ — explicit path to a `piper` binary.
    /// When `None` (default), inkhaven autoresolves:
    /// PATH first, then `~/.cache/inkhaven/piper-<plat>/`.
    /// When the resolver finds nothing and
    /// `auto_download_binary` is true, the platform's
    /// piper release is downloaded into the user cache
    /// (NOT the project tree — the binary is identical
    /// across projects).
    pub binary_path: Option<String>,

    /// 1.2.17+ — when `true`, a missing Piper binary
    /// triggers a one-time download from GitHub
    /// Releases.  When `false`, the resolver reports
    /// "Piper not found" and falls back to System under
    /// `engine: "auto"` or errors under `engine:
    /// "piper"`.
    pub auto_download_binary: bool,

    /// 1.2.17+ — LRU cache cap on the project's voices
    /// directory.  When the count exceeds this number,
    /// the least-recently-used voice is evicted (its
    /// `.onnx` + `.onnx.json` removed).  Voice models
    /// are 25–100 MB each; the default `5` caps the
    /// directory at ~125–500 MB per project.
    pub cache_max_voices: usize,

    /// 1.2.17+ — override the platform default playback
    /// command.  `None` (default) uses `afplay` on macOS,
    /// `paplay` / `aplay` on Linux, PowerShell
    /// `Media.SoundPlayer` on Windows.  Set to a string
    /// containing `{path}` (replaced with the synthesised
    /// WAV path) to use a custom player (`mpv`, `ffplay`,
    /// `sox play`, etc.).
    pub play_command: Option<String>,

    /// 1.2.17+ — sample rate for Piper synthesis output
    /// in Hz.  Piper's native rate is 22050 Hz; changing
    /// this triggers a resample inside the playback
    /// pipeline.  Most users should leave the default.
    pub sample_rate_hz: u32,

    /// 1.2.17+ — when `true`, the first auto-downloaded
    /// voice appends `.inkhaven/voices/` to the project's
    /// `.gitignore` (creating the file if absent).
    /// Voices are large opaque blobs; committing them
    /// pollutes git history and the working tree.  Set
    /// `false` if you manage `.gitignore` strictly by
    /// hand.  One-time, idempotent, atomic via
    /// `crate::io_atomic`.
    pub auto_gitignore: bool,
}

impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            voice: "Milena".into(),
            speed: 1.0,
            greeting: String::new(),
            goodbye: String::new(),
            engine: "auto".into(),
            voices_dir: ".inkhaven/voices".into(),
            auto_download: true,
            catalog_url:
                "https://huggingface.co/rhasspy/piper-voices/raw/main/voices.json"
                    .into(),
            catalog_ttl_hours: 24,
            binary_path: None,
            auto_download_binary: true,
            cache_max_voices: 5,
            play_command: None,
            sample_rate_hz: 22_050,
            auto_gitignore: true,
        }
    }
}

fn default_startup_splash() -> bool {
    true
}

fn default_mouse_captured() -> bool {
    true
}

fn default_confirm_quit() -> bool {
    false
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            theme: "default".into(),
            tab_width: 2,
            wrap: true,
            autosave_seconds: 5,
            auto_close_pairs: true,
            crash_mirror_seconds: default_crash_mirror_seconds(),
            deleted_paragraph_history: default_deleted_paragraph_history(),
            external_change_auto_reload: default_external_change_auto_reload(),
            fact_check_idle_seconds: default_fact_check_idle_seconds(),
            startup_haiku: default_startup_haiku(),
            haiku_semantic: default_startup_haiku(),
            haiku_scope: default_haiku_scope(),
            visited_history_cap: default_visited_history_cap(),
            stemming: StemmingConfig::default(),
            startup_splash: default_startup_splash(),
            mouse_captured: default_mouse_captured(),
            confirm_quit: default_confirm_quit(),
            tts: TtsConfig::default(),
            style_warnings: StyleWarningsConfig::default(),
            pov_chip_enabled: default_pov_chip_enabled(),
            prompt_language_mode: default_prompt_language_mode(),
            prompt_language_detection_min_chars:
                default_prompt_language_detection_min_chars(),
            comment_author: None,
            snippets: SnippetsConfig::default(),
            continuation_anchor_count: default_continuation_anchor_count(),
            footnote_style: default_footnote_style(),
            show_glossary_chip: default_show_glossary_chip(),
            show_facts_chip: false,
            reading_time_chip: false,
            reading_wpm: default_reading_wpm(),
            echo_window: default_echo_window(),
            echo_min_repeats: default_echo_min_repeats(),
            echo_max_global: default_echo_max_global(),
            paragraph_long_secs: default_paragraph_long_secs(),
            disk_warn_mb: default_disk_warn_mb(),
            warn_uncommitted_on_exit: default_warn_uncommitted_on_exit(),
            echo_overlay: false,
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
        // Guard against deeply-nested HJSON that would stack-overflow the
        // recursive parser into an uncatchable SIGABRT on startup.
        crate::hjson_guard::check_hjson_depth(&raw).map_err(crate::error::Error::Config)?;
        let mut cfg: Self = serde_hjson::from_str(&raw)
            .map_err(|e| crate::error::Error::Config(e.to_string()))?;
        cfg.harden_security_floor();
        Ok(cfg)
    }

    /// M5 — re-assert the shipped security floor after config load /
    /// merge. Setting `shell.blocked_externals` in a project or global
    /// overlay REPLACES the list wholesale (deep-merge merges objects,
    /// not arrays), so without this an overlay that means "also block
    /// X" silently drops every shipped block (vim/ssh/sudo/…) that
    /// guards the embedded shell against alt-screen corruption and
    /// privilege escalation. Union the defaults back in: the user can
    /// still ADD blocks, but can't accidentally wipe the floor. This is
    /// the security carve-out to the otherwise-permissive config.
    fn harden_security_floor(&mut self) {
        let have: std::collections::HashSet<&str> = self
            .shell
            .blocked_externals
            .iter()
            .map(|s| s.as_str())
            .collect();
        let missing: Vec<String> = default_blocked_externals()
            .into_iter()
            .filter(|d| !have.contains(d.as_str()))
            .collect();
        self.shell.blocked_externals.extend(missing);
    }

    /// 1.2.20+ — load the project config, then layer any
    /// user-global override files on top.  Precedence, low
    /// → high: built-in defaults → project `inkhaven.hjson`
    /// → `~/.config/inkhaven/config.hjson` →
    /// `~/.config/inkhaven/conf/*.hjson` (sorted lexically).
    ///
    /// The global files are **partial** — only the keys
    /// they contain override; everything else falls through
    /// to the project.  This lets a user keep one personal
    /// theme / keybind set that applies to every project
    /// without editing each project's HJSON.
    ///
    /// Global overrides win over the project **deliberately**:
    /// `inkhaven init` writes a *full* config, so a
    /// project-wins cascade would mask the user's global
    /// preferences entirely.  A malformed *global* file is
    /// skipped with a WARN (a typo there must never brick
    /// every project + command); a malformed *project* file
    /// stays fatal, exactly like [`Config::load`].
    pub fn load_layered(project_path: &Path) -> crate::error::Result<Self> {
        Self::load_layered_from(project_path, global_config_dir().as_deref())
    }

    /// [`Config::load_layered`] with the global config
    /// directory injected — the seam the tests drive without
    /// touching the process-wide `$XDG_CONFIG_HOME`.
    fn load_layered_from(
        project_path: &Path,
        global_dir: Option<&Path>,
    ) -> crate::error::Result<Self> {
        // Base = the built-in defaults as a JSON value, so
        // the final typed `from_value` always sees a
        // complete object no matter which keys the layers
        // carry.
        let mut merged = serde_json::to_value(Config::default())
            .map_err(|e| crate::error::Error::Config(e.to_string()))?;

        // Project layer — required + fatal on parse error,
        // matching `load`.
        let project = read_hjson_value(project_path)?;
        merge_value(&mut merged, project);

        // Global override layer(s) — best-effort: a broken
        // file degrades to "skipped", never to a hard error.
        if let Some(dir) = global_dir {
            for path in global_config_files_in(dir) {
                match read_hjson_value(&path) {
                    Ok(v) => merge_value(&mut merged, v),
                    Err(e) => tracing::warn!(
                        target: "inkhaven::config",
                        "skipping malformed global config `{}`: {e}",
                        path.display(),
                    ),
                }
            }
        }

        let mut cfg: Self = serde_json::from_value(merged)
            .map_err(|e| crate::error::Error::Config(e.to_string()))?;
        cfg.harden_security_floor();
        Ok(cfg)
    }

    #[allow(dead_code)]
    pub fn save(&self, path: &Path) -> crate::error::Result<()> {
        let s = serde_hjson::to_string(self)
            .map_err(|e| crate::error::Error::Config(e.to_string()))?;
        std::fs::write(path, s).map_err(crate::error::Error::Io)
    }
}

// ── config layering (1.2.20+) ────────────────────────────

/// Parse an HJSON file into a generic JSON value for
/// layering.  Going through `serde_json::Value` (rather
/// than straight to `Config`) is what makes *partial*
/// override files work: keys absent from the file simply
/// aren't in the value, so the deep-merge leaves the lower
/// layer's value in place.
fn read_hjson_value(path: &Path) -> crate::error::Result<serde_json::Value> {
    let raw = std::fs::read_to_string(path).map_err(crate::error::Error::Io)?;
    crate::hjson_guard::check_hjson_depth(&raw).map_err(crate::error::Error::Config)?;
    serde_hjson::from_str::<serde_json::Value>(&raw)
        .map_err(|e| crate::error::Error::Config(e.to_string()))
}

/// Deep-merge `overlay` onto `base`: two objects merge
/// key-by-key (recursively); every other shape (scalar,
/// array, or a type change) replaces wholesale.  The
/// overlay always wins on a conflict.
fn merge_value(base: &mut serde_json::Value, overlay: serde_json::Value) {
    match (base, overlay) {
        (serde_json::Value::Object(b), serde_json::Value::Object(o)) => {
            for (k, v) in o {
                merge_value(b.entry(k).or_insert(serde_json::Value::Null), v);
            }
        }
        (b, o) => *b = o,
    }
}

/// The user-global inkhaven config directory:
/// `$XDG_CONFIG_HOME/inkhaven` when that env var is set +
/// non-empty, else `$HOME/.config/inkhaven`.  `None` when
/// neither is set (layering is then simply skipped).  This
/// matches the `~/.config/inkhaven/inkhaven.hjson`
/// convention already documented for provider API keys.
fn global_config_dir() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return Some(PathBuf::from(xdg).join("inkhaven"));
        }
    }
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config").join("inkhaven"))
}

/// The override files under the global dir, in ascending
/// precedence: `config.hjson` first, then every
/// `conf/*.hjson` in sorted (lexical) order — so a user can
/// split overrides into `conf/10-theme.hjson`,
/// `conf/20-keys.hjson`, … and reason about who wins.  A
/// missing dir / `conf` subdir yields an empty list.
fn global_config_files_in(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let top = dir.join("config.hjson");
    if top.is_file() {
        files.push(top);
    }
    if let Ok(entries) = std::fs::read_dir(dir.join("conf")) {
        let mut confs: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("hjson")
            })
            .collect();
        confs.sort();
        files.extend(confs);
    }
    files
}

/// Writing-progress goals — fuels the status-bar widget +
/// Ctrl+V G modal.
///
/// All numeric fields are inclusive; absent / zero means
/// "no target set" rather than "must be zero". Per-book entries
/// live under `goals.books.<book-slug>` so the slug is the
/// natural lookup key (case-insensitive in the
/// hierarchy → snapshot mapping).
/// AI-cost knobs surfaced in `inkhaven cost`. The daily caps are
/// **informative, not gates** (the slow tracks warn and continue past
/// them, per Inkhaven's permissive principle) — they drive the usage
/// bars and the warning thresholds. Defaults match the values that
/// shipped hardcoded through 1.3.36.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CostConfig {
    /// Daily ceiling on world fact-check slow-track LLM calls.
    pub world_daily_call_cap: i64,
    /// Daily ceiling on Inner Socrates slow-track LLM calls.
    pub inner_socrates_daily_call_cap: i64,
    /// Trailing days of per-category AI-call tallies kept in
    /// `.inkhaven/ai_usage.json` before the oldest are pruned.
    pub usage_retention_days: usize,
    /// R2-E — per-model USD price table (input/output per **million** tokens),
    /// keyed by a substring of the model name (longest match wins). Drives the
    /// research session-cost display; the `default` fallback prices any model not
    /// listed. Informative only — cost never blocks.
    pub pricing: std::collections::BTreeMap<String, ModelPrice>,
    /// Fallback price (USD per million tokens) for models absent from `pricing`.
    pub default_input_per_1m: f64,
    pub default_output_per_1m: f64,
}

/// R2-E — one model's input/output price in **USD per million tokens**.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelPrice {
    pub input_per_1m: f64,
    pub output_per_1m: f64,
}

impl Default for ModelPrice {
    fn default() -> Self {
        // Mirrors the legacy flat ~$0.003/1K = $3/1M estimate when a row omits a side.
        Self { input_per_1m: 3.0, output_per_1m: 3.0 }
    }
}

impl CostConfig {
    /// The price row for `model` — the longest key that is a substring of the
    /// model name, else the `default_*` fallback. (Keys like `gpt-4o`,
    /// `claude-sonnet`, `gemini-2.5-pro` match their family.)
    pub fn price_for(&self, model: &str) -> ModelPrice {
        let m = model.to_ascii_lowercase();
        let best = self
            .pricing
            .iter()
            .filter(|(k, _)| m.contains(k.to_ascii_lowercase().as_str()))
            .max_by_key(|(k, _)| k.len());
        match best {
            Some((_, p)) => *p,
            None => ModelPrice {
                input_per_1m: self.default_input_per_1m,
                output_per_1m: self.default_output_per_1m,
            },
        }
    }
}

impl Default for CostConfig {
    fn default() -> Self {
        // Published list prices (USD / 1M tokens) as of the 1.5.3 cut — adjust in
        // config as prices move. Keys match on a model-name substring.
        let pricing = [
            ("gemini-2.5-pro", 1.25, 10.0),
            ("gemini-2.5-flash", 0.30, 2.50),
            ("gemini", 1.25, 10.0),
            ("claude-opus", 15.0, 75.0),
            ("claude-sonnet", 3.0, 15.0),
            ("claude-haiku", 0.80, 4.0),
            ("gpt-4o-mini", 0.15, 0.60),
            ("gpt-4o", 2.50, 10.0),
            ("deepseek", 0.27, 1.10),
            ("grok", 2.0, 10.0),
        ]
        .iter()
        .map(|(k, i, o)| (k.to_string(), ModelPrice { input_per_1m: *i, output_per_1m: *o }))
        .collect();
        Self {
            world_daily_call_cap: 200,
            inner_socrates_daily_call_cap: 150,
            usage_retention_days: 30,
            pricing,
            default_input_per_1m: 3.0,
            default_output_per_1m: 3.0,
        }
    }
}

/// BOOK_RAG-1 (1.4.x) — the AI pane's **Book scope is retrieval-augmented**:
/// a prompt in Book scope retrieves the semantically relevant paragraphs
/// (current book + the included author-content system books) and grounds
/// the answer in them with markdown citations, instead of sending the whole
/// book. There is no on/off — Book scope *is* RAG; this only tunes it.
/// System books are listed by their lowercase `system_tag`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BookRagConfig {
    /// Top-K paragraphs retrieved by semantic similarity.
    pub top_k: usize,
    /// ± this many surrounding paragraphs included around each hit.
    pub context_expansion: usize,
    /// Hard cap on the composed context (estimated tokens ≈ chars/4).
    pub max_context_tokens: usize,
    /// Author-content system books also searched, by `system_tag`.
    pub include_system_books: Vec<String>,
    /// Meta system books never searched, by `system_tag`.
    pub exclude_system_books: Vec<String>,
}

impl Default for BookRagConfig {
    fn default() -> Self {
        Self {
            top_k: 5,
            context_expansion: 1,
            max_context_tokens: 8000,
            include_system_books: [
                "notes", "research", "places", "characters",
                "artefacts", "world", "language",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            exclude_system_books: ["scripts", "prompts", "typst", "help", "intent", "sources", "glossary", "snippets"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }
}

/// GRAPHMIND (2.x) — the knowledge-graph AI surfaces: the Graph AI scope
/// (GM-P4, chat with your graph) and `graph ask` (GM-P5, the traversal
/// tool-loop). These knobs bound the *cost* of a graph question; per Inkhaven's
/// permissive principle they inform and cap, they never block. Retrieval width
/// for the Graph scope is shared with Book scope ([`BookRagConfig`]).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GraphConfig {
    /// `graph ask` — the maximum number of LLM turns the traversal loop may take
    /// before it is forced to answer from what it has already observed. Each
    /// turn is one graph query; a higher cap explores deeper at higher cost.
    pub ask_max_steps: usize,
    /// `graph ask` — how many seed nodes each `search` action returns (and thus
    /// how many handles the model can branch from per search).
    pub ask_search_width: usize,
}

impl Default for GraphConfig {
    fn default() -> Self {
        Self { ask_max_steps: 8, ask_search_width: 6 }
    }
}

/// INNER_EDITOR-1 (1.4.2+) — the Inner Editor literary/stylistic companion.
/// All knobs default to behaviour-preserving values; the feature is enabled by
/// default but only engages when an LLM provider is configured. Cost caps are
/// **informative, never blocking** (Inkhaven's permissive principle).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct InnerEditorConfig {
    /// Master switch. `false` fully disables; the manual chord then shows an
    /// informational message. Requires an LLM provider regardless.
    pub enabled: bool,
    pub engagement: InnerEditorEngagement,
    pub context: InnerEditorContext,
    pub persona: InnerEditorPersona,
    pub output: InnerEditorOutput,
    pub llm: InnerEditorLlm,
}

impl Default for InnerEditorConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            engagement: InnerEditorEngagement::default(),
            context: InnerEditorContext::default(),
            persona: InnerEditorPersona::default(),
            output: InnerEditorOutput::default(),
            llm: InnerEditorLlm::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct InnerEditorEngagement {
    /// Paragraph-pause idle threshold before auto-engaging (seconds).
    pub idle_threshold_seconds: u64,
    /// Same-paragraph cooldown from the last engagement (seconds). Edits during
    /// the window reset the timer.
    pub cooldown_seconds: u64,
    /// Cap on findings surfaced per paragraph per engagement.
    pub max_findings_per_paragraph: usize,
}

impl Default for InnerEditorEngagement {
    fn default() -> Self {
        Self { idle_threshold_seconds: 60, cooldown_seconds: 120, max_findings_per_paragraph: 3 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct InnerEditorContext {
    /// Preceding paragraphs included as interpretation context.
    pub preceding_paragraphs: usize,
    /// Following paragraphs (default 0 — the Editor reads as you write).
    pub following_paragraphs: usize,
}

impl Default for InnerEditorContext {
    fn default() -> Self {
        Self { preceding_paragraphs: 3, following_paragraphs: 0 }
    }
}

/// The single Editor persona's tuning. Strings are parsed tolerantly into the
/// `inner_editor::types` enums (a bad value falls back to the default, never a
/// config-parse failure).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct InnerEditorPersona {
    pub tone: String,             // critical | balanced | encouraging
    pub verbosity: String,        // concise | standard | detailed
    pub praise_frequency: String, // rare | moderate | frequent
    pub genre_aware: bool,
    pub belief_stance_enabled: bool,
    pub categories: InnerEditorCategories,
}

impl Default for InnerEditorPersona {
    fn default() -> Self {
        Self {
            tone: "balanced".into(),
            verbosity: "concise".into(),
            praise_frequency: "moderate".into(),
            genre_aware: true,
            belief_stance_enabled: true,
            categories: InnerEditorCategories::default(),
        }
    }
}

/// Per-category enable/disable. All eight on by default.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct InnerEditorCategories {
    pub literary_richness: bool,
    pub tautology: bool,
    pub style_observation: bool,
    pub style_instability: bool,
    pub dictionary_richness: bool,
    pub belief_stance: bool,
    pub craft_praise: bool,
    pub editorial_suggestions: bool,
}

impl Default for InnerEditorCategories {
    fn default() -> Self {
        Self {
            literary_richness: true,
            tautology: true,
            style_observation: true,
            style_instability: true,
            dictionary_richness: true,
            belief_stance: true,
            craft_praise: true,
            editorial_suggestions: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct InnerEditorOutput {
    /// Minimum severity shown by default. `note` hides Praise (filter to reveal).
    pub severity_threshold: String, // praise | note | concern
    pub group_by_paragraph: bool,
    pub always_show_persona_label: bool,
}

impl Default for InnerEditorOutput {
    fn default() -> Self {
        Self {
            severity_threshold: "note".into(),
            group_by_paragraph: true,
            always_show_persona_label: true,
        }
    }
}

/// LLM cost knobs. The caps inform the preflight; they never gate a request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct InnerEditorLlm {
    pub editor_engagement: InnerEditorCap,
    pub conversation: InnerEditorCap,
    pub backoff_max_retries: usize,
    pub backoff_initial_seconds: u64,
}

impl Default for InnerEditorLlm {
    fn default() -> Self {
        Self {
            editor_engagement: InnerEditorCap {
                max_calls_per_session: 80,
                confirm_above_calls: 40,
                max_calls_per_day: 200,
                max_calls_per_month: 4000,
            },
            conversation: InnerEditorCap {
                max_calls_per_session: 30,
                confirm_above_calls: 1,
                max_calls_per_day: 80,
                max_calls_per_month: 1500,
            },
            backoff_max_retries: 3,
            backoff_initial_seconds: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct InnerEditorCap {
    pub max_calls_per_session: i64,
    pub confirm_above_calls: i64,
    pub max_calls_per_day: i64,
    pub max_calls_per_month: i64,
}

impl Default for InnerEditorCap {
    fn default() -> Self {
        Self {
            max_calls_per_session: 80,
            confirm_above_calls: 40,
            max_calls_per_day: 200,
            max_calls_per_month: 4000,
        }
    }
}

/// SOURCES-1 (1.4.5+) — the bibliography & citation engine. HJSON entries in the
/// `Sources` book compile to `sources.bib` at assembly; Typst renders the
/// bibliography. Optional block; defaults give a shared citation pool with the
/// IEEE style, auto-appended.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SourcesConfig {
    /// `true` (default): every HJSON entry in the Sources book feeds a shared
    /// `.bib`. `false`: only the `Sources/<BookTitle>` chapter for the assembled
    /// book (per-book scoping).
    pub all: bool,
    /// Typst bibliography style name (`ieee`, `apa`, `chicago-author-date`,
    /// `mla`, …).
    pub bibliography_style: String,
    /// `true` (default): the assembler appends `#bibliography(...)` after
    /// `#wrap_book(...)`. `false`: place it manually (e.g. in `globals.typ`).
    pub auto_bibliography: bool,
    /// LOCI — `true`: the assembler emits an **Index Locorum** (every `@key[locus]`
    /// cited across the book, grouped by source) after the bibliography. Off by
    /// default — a specialized apparatus for scripture / classics / law.
    pub index_locorum: bool,
    /// LEXICON — `true`: the assembler emits an **Index Verborum** (every scholarly
    /// lexicon term used in the book — its original-language form, senses, and the
    /// chapters that use it) after the Index Locorum. Off by default.
    pub index_verborum: bool,
    /// LEXICON — `true`: the assembler emits a **Glossary** chapter (every defined
    /// term, alphabetical — a lexicon term shows its original-language form and its
    /// distinct senses; an ordinary term shows its definition). Off by default.
    pub glossary: bool,
    /// LOCI — named **reference schemes** for validating `@key[locus]` citations.
    /// A source declares which it uses via a `scheme:` line in its Sources entry
    /// (the value is a key here); the three scripture keys — `bible`, `quran`,
    /// `book-of-mormon` — carry built-in schemes and need no entry. A locus that
    /// does not match its source's scheme is reported by `inkhaven index-locorum`
    /// (and `inkhaven build`), so `@bible[John 3:sixteen]` is caught before it
    /// ships. Example: `kant-ab: { pattern: "^A\\d+(/B\\d+)?$", format: "A{n}/B{n}" }`.
    #[serde(default)]
    pub ref_schemes: std::collections::BTreeMap<String, RefScheme>,
}

/// LOCI — one reference scheme: a regex a locus must fully match to be valid, and
/// a human `format` hint shown when it doesn't.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RefScheme {
    /// A regular expression the locus must fully match (anchored automatically).
    pub pattern: String,
    /// A human hint shown for a malformed locus, e.g. `{book} {ch}:{v}`.
    pub format: String,
}

impl Default for SourcesConfig {
    fn default() -> Self {
        Self {
            all: true,
            bibliography_style: "ieee".into(),
            auto_bibliography: true,
            index_locorum: false,
            index_verborum: false,
            glossary: false,
            ref_schemes: std::collections::BTreeMap::new(),
        }
    }
}

/// PAPER (1.6.15+) — one author of a scientific paper. All fields optional; an
/// author with an empty `name` is skipped when rendering the title block.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Author {
    pub name: String,
    /// Institutional affiliation. Authors sharing an affiliation share one
    /// superscript number in the rendered block.
    pub affiliation: String,
    /// ORCID iD (bare `0000-0000-0000-0000` or a full URL — rendered verbatim).
    pub orcid: String,
    pub email: String,
    /// The corresponding author gets a `*` mark and an email note.
    pub corresponding: bool,
}

/// PAPER (1.6.15+) — journal-article front matter. Rendered into a Typst title
/// block (title, authors + affiliations, abstract, keywords, funding) that is
/// prepended to the assembled document for the `typst`/`pdf`/`tex` exports and
/// the TUI book assembly. Empty (no authors, abstract, or keywords) → renders
/// nothing, so books that don't opt in are byte-for-byte unchanged. Labels key
/// off the project language ([`Config::language`]).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct FrontmatterConfig {
    /// The paper abstract (a single paragraph of plain prose).
    #[serde(rename = "abstract")]
    pub abstract_text: String,
    pub keywords: Vec<String>,
    pub authors: Vec<Author>,
    /// Funding / acknowledgements statement. Identifying — dropped under `--blind`.
    pub funding: String,
    /// Data-availability statement (where the study's data lives). Kept under
    /// `--blind` (the author anonymises repository links themselves).
    pub data_availability: String,
    /// Code-availability statement (where the analysis code lives). Kept under
    /// `--blind`.
    pub code_availability: String,
}

/// Localized front-matter labels, in the five first-class languages (English
/// fallback). Mirrors the `Labels::for_language` pattern used elsewhere.
struct FrontmatterLabels {
    abstract_: &'static str,
    keywords: &'static str,
    corresponding: &'static str,
    funding: &'static str,
    data_availability: &'static str,
    code_availability: &'static str,
}

impl FrontmatterLabels {
    fn for_language(language: &str) -> FrontmatterLabels {
        match language.trim().to_lowercase().as_str() {
            "ru" | "russian" | "русский" => FrontmatterLabels {
                abstract_: "Аннотация",
                keywords: "Ключевые слова",
                corresponding: "Автор для корреспонденции",
                funding: "Финансирование",
                data_availability: "Доступность данных",
                code_availability: "Доступность кода",
            },
            "fr" | "french" | "français" | "francais" => FrontmatterLabels {
                abstract_: "Résumé",
                keywords: "Mots-clés",
                corresponding: "Auteur correspondant",
                funding: "Financement",
                data_availability: "Disponibilité des données",
                code_availability: "Disponibilité du code",
            },
            "de" | "german" | "deutsch" => FrontmatterLabels {
                abstract_: "Zusammenfassung",
                keywords: "Schlüsselwörter",
                corresponding: "Korrespondierender Autor",
                funding: "Förderung",
                data_availability: "Datenverfügbarkeit",
                code_availability: "Codeverfügbarkeit",
            },
            "es" | "spanish" | "español" | "espanol" => FrontmatterLabels {
                abstract_: "Resumen",
                keywords: "Palabras clave",
                corresponding: "Autor para correspondencia",
                funding: "Financiación",
                data_availability: "Disponibilidad de datos",
                code_availability: "Disponibilidad del código",
            },
            _ => FrontmatterLabels {
                abstract_: "Abstract",
                keywords: "Keywords",
                corresponding: "Corresponding author",
                funding: "Funding",
                data_availability: "Data availability",
                code_availability: "Code availability",
            },
        }
    }
}

/// Escape Typst markup-special characters so author-supplied plain text sits
/// safely in content (`[...]`) mode. Mirrors `conlang::output::typst_text`.
fn frontmatter_escape_content(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(
            c,
            '#' | '*' | '_' | '`' | '$' | '\\' | '<' | '>' | '@' | '[' | ']'
        ) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

impl FrontmatterConfig {
    /// `true` when any renderable front matter is configured.
    fn has_content(&self) -> bool {
        self.authors.iter().any(|a| !a.name.trim().is_empty())
            || !self.abstract_text.trim().is_empty()
            || self.keywords.iter().any(|k| !k.trim().is_empty())
            || !self.data_availability.trim().is_empty()
            || !self.code_availability.trim().is_empty()
    }

    /// Render the front matter as a Typst title block. Returns an empty string
    /// when nothing is configured (so existing exports are unchanged). `title`
    /// is the book/paper title; `language` selects the localized labels. When
    /// `blind` is set (double-blind submission), the identifying parts — authors,
    /// affiliations, ORCID, corresponding author, funding — are omitted, keeping
    /// the title, abstract, keywords, and availability statements.
    pub fn to_typst_block(&self, language: &str, title: &str, blind: bool) -> String {
        if !self.has_content() {
            return String::new();
        }
        let lx = FrontmatterLabels::for_language(language);
        let esc = frontmatter_escape_content;
        let show_identity = !blind;
        let mut s = String::new();
        s.push_str("// inkhaven front matter — paper title block\n");

        // Document metadata (PDF outline / accessibility). Authors omitted under
        // blind so the PDF metadata doesn't leak identity either.
        s.push_str(&format!("#set document(title: \"{}\"", typst_escape(title)));
        if show_identity {
            let author_names: Vec<String> = self
                .authors
                .iter()
                .filter(|a| !a.name.trim().is_empty())
                .map(|a| format!("\"{}\"", typst_escape(a.name.trim())))
                .collect();
            if !author_names.is_empty() {
                s.push_str(&format!(", author: ({},)", author_names.join(", ")));
            }
        }
        s.push_str(")\n\n");

        // Distinct affiliations → shared superscript numbers.
        let mut affils: Vec<String> = Vec::new();
        if show_identity {
            for a in &self.authors {
                let aff = a.affiliation.trim();
                if !aff.is_empty() && !affils.iter().any(|x| x == aff) {
                    affils.push(aff.to_string());
                }
            }
        }

        // Centered title + author line.
        s.push_str("#align(center)[\n");
        s.push_str(&format!(
            "  #text(size: 1.6em, weight: \"bold\")[{}]\n\n",
            esc(title)
        ));
        let mut author_bits: Vec<String> = Vec::new();
        if show_identity {
            for a in &self.authors {
                let name = a.name.trim();
                if name.is_empty() {
                    continue;
                }
                let mut bit = esc(name);
                let aff = a.affiliation.trim();
                if !aff.is_empty() {
                    if let Some(idx) = affils.iter().position(|x| x == aff) {
                        bit.push_str(&format!("#super[{}]", idx + 1));
                    }
                }
                if a.corresponding {
                    bit.push_str("#super[\\*]");
                }
                if !a.orcid.trim().is_empty() {
                    bit.push_str(&format!(
                        " #box[#text(size: 0.7em)[ORCID {}]]",
                        esc(a.orcid.trim())
                    ));
                }
                author_bits.push(bit);
            }
        }
        if !author_bits.is_empty() {
            s.push_str(&format!("  #v(0.5em)\n  {}\n", author_bits.join(", ")));
        }
        if !affils.is_empty() {
            s.push_str("\n  #text(size: 0.85em)[\n");
            let mut lines: Vec<String> = affils
                .iter()
                .enumerate()
                .map(|(i, aff)| format!("    #super[{}]{}", i + 1, esc(aff)))
                .collect();
            if let Some(corr) = self
                .authors
                .iter()
                .find(|a| a.corresponding && !a.email.trim().is_empty())
            {
                lines.push(format!(
                    "    #super[\\*]{}: {}",
                    esc(lx.corresponding),
                    esc(corr.email.trim())
                ));
            }
            s.push_str(&lines.join(" \\\n"));
            s.push_str("\n  ]\n");
        }
        s.push_str("]\n\n");

        // Abstract.
        if !self.abstract_text.trim().is_empty() {
            s.push_str(&format!(
                "#block(inset: (x: 2em))[\n  *{}.* {}\n]\n\n",
                esc(lx.abstract_),
                esc(self.abstract_text.trim())
            ));
        }

        // Keywords.
        let kws: Vec<String> = self
            .keywords
            .iter()
            .map(|k| k.trim())
            .filter(|k| !k.is_empty())
            .map(esc)
            .collect();
        if !kws.is_empty() {
            s.push_str(&format!(
                "#block(inset: (x: 2em))[\n  *{}:* {}\n]\n\n",
                esc(lx.keywords),
                kws.join(", ")
            ));
        }

        // Data-availability statement (kept under blind — author anonymises).
        if !self.data_availability.trim().is_empty() {
            s.push_str(&format!(
                "#block(inset: (x: 2em))[\n  *{}:* {}\n]\n\n",
                esc(lx.data_availability),
                esc(self.data_availability.trim())
            ));
        }

        // Code-availability statement (kept under blind).
        if !self.code_availability.trim().is_empty() {
            s.push_str(&format!(
                "#block(inset: (x: 2em))[\n  *{}:* {}\n]\n\n",
                esc(lx.code_availability),
                esc(self.code_availability.trim())
            ));
        }

        // Funding — identifying (grant numbers), dropped under blind.
        if show_identity && !self.funding.trim().is_empty() {
            s.push_str(&format!(
                "#block(inset: (x: 2em))[\n  *{}:* {}\n]\n\n",
                esc(lx.funding),
                esc(self.funding.trim())
            ));
        }

        s
    }
}

/// PAPER (1.6.15+) — LaTeX export document class + preamble. `inkhaven export
/// tex` runs tylax, which already emits a complete `\documentclass{article}`
/// document. When `document_class` is set, inkhaven rewrites that line to the
/// named journal class (`IEEEtran`, `elsarticle`, `article`, …) and injects
/// `extra_packages` + raw `preamble` lines before `\begin{document}`. All
/// fields empty (the default) → tylax's `article` output is left untouched, so
/// existing exports are byte-for-byte unchanged.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TexExportConfig {
    /// Journal document class, e.g. `IEEEtran`, `elsarticle`, `article`. Empty
    /// → keep tylax's `article`.
    pub document_class: String,
    /// Class options, e.g. `conference`, `twocolumn`, `11pt,a4paper`. Empty →
    /// `\documentclass{class}` with no bracket.
    pub class_options: String,
    /// Extra `\usepackage`s — bare names (`amsmath`) or full `\usepackage{...}`
    /// lines (for options like `\usepackage[numbers]{natbib}`).
    pub extra_packages: Vec<String>,
    /// Raw preamble lines inserted verbatim before `\begin{document}`.
    pub preamble: Vec<String>,
}

/// TYPST-UNIVERSE (1.6.15+) — the `Ctrl+V #` import picker's package source.
/// The manifest is fetched once and cached under `.inkhaven/`; `ttl_hours`
/// bounds cache freshness.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TypstUniverseConfig {
    /// Package manifest URL. Defaults to the community "with-stars" list.
    pub url: String,
    /// Cache freshness window, in hours.
    pub ttl_hours: u32,
}

impl Default for TypstUniverseConfig {
    fn default() -> Self {
        Self {
            url: crate::typst_universe::DEFAULT_URL.to_string(),
            ttl_hours: 24,
        }
    }
}

#[cfg(test)]
mod frontmatter_tests {
    use super::*;

    #[test]
    fn empty_frontmatter_renders_nothing() {
        let fm = FrontmatterConfig::default();
        assert_eq!(fm.to_typst_block("english", "A Book", false), "");
    }

    #[test]
    fn renders_title_authors_affiliations_and_abstract() {
        let fm = FrontmatterConfig {
            abstract_text: "We show a thing.".into(),
            keywords: vec!["alpha".into(), "beta".into()],
            authors: vec![
                Author {
                    name: "Ada Lovelace".into(),
                    affiliation: "Analytical Engine Co.".into(),
                    orcid: "0000-0000-0000-0001".into(),
                    email: "ada@example.org".into(),
                    corresponding: true,
                },
                Author {
                    name: "Charles Babbage".into(),
                    affiliation: "Analytical Engine Co.".into(),
                    ..Default::default()
                },
            ],
            funding: "Grant 42.".into(),
            ..Default::default()
        };
        let out = fm.to_typst_block("english", "On Engines", false);
        assert!(out.contains("#set document(title: \"On Engines\""));
        assert!(out.contains("Ada Lovelace"));
        assert!(out.contains("Charles Babbage"));
        // Shared affiliation → a single superscript group.
        assert!(out.contains("#super[1]Analytical Engine Co."));
        assert!(!out.contains("#super[2]"));
        assert!(out.contains("*Abstract.*"));
        assert!(out.contains("*Keywords:* alpha, beta"));
        assert!(out.contains("*Funding:*"));
        assert!(out.contains("ORCID 0000-0000-0000-0001"));
        // `@` is escaped in Typst content mode (bare `@` starts a reference).
        assert!(out.contains("Corresponding author"));
        assert!(out.contains("ada\\@example.org"), "{out}");
    }

    #[test]
    fn labels_key_off_project_language() {
        let fm = FrontmatterConfig {
            abstract_text: "Резюме текста.".into(),
            keywords: vec!["ключ".into()],
            ..Default::default()
        };
        let ru = fm.to_typst_block("russian", "Труд", false);
        assert!(ru.contains("*Аннотация.*"), "{ru}");
        assert!(ru.contains("*Ключевые слова:*"), "{ru}");
        let fr = fm.to_typst_block("fr", "Œuvre", false);
        assert!(fr.contains("*Résumé.*"), "{fr}");
    }

    #[test]
    fn blind_omits_identity_but_keeps_content_and_availability() {
        let fm = FrontmatterConfig {
            abstract_text: "A finding.".into(),
            keywords: vec!["k".into()],
            authors: vec![Author {
                name: "Ada Lovelace".into(),
                affiliation: "Engine Co.".into(),
                orcid: "0000-0000-0000-0001".into(),
                email: "ada@example.org".into(),
                corresponding: true,
            }],
            funding: "Grant 42.".into(),
            data_availability: "Data at example.org/data.".into(),
            code_availability: "Code at example.org/code.".into(),
        };
        let blind = fm.to_typst_block("english", "T", true);
        // Identity stripped.
        assert!(!blind.contains("Ada Lovelace"), "{blind}");
        assert!(!blind.contains("Engine Co."), "{blind}");
        assert!(!blind.contains("ORCID"), "{blind}");
        assert!(!blind.contains("*Funding:*"), "{blind}");
        assert!(!blind.contains("author: ("), "{blind}");
        // Content kept.
        assert!(blind.contains("*Abstract.*"), "{blind}");
        assert!(blind.contains("*Keywords:*"), "{blind}");
        assert!(blind.contains("*Data availability:*"), "{blind}");
        assert!(blind.contains("*Code availability:*"), "{blind}");
        // Non-blind keeps identity + funding.
        let open = fm.to_typst_block("english", "T", false);
        assert!(open.contains("Ada Lovelace"));
        assert!(open.contains("*Funding:*"));
    }

    #[test]
    fn markup_specials_in_author_text_are_escaped() {
        let fm = FrontmatterConfig {
            authors: vec![Author {
                name: "A #hash* Name".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let out = fm.to_typst_block("english", "T", false);
        assert!(out.contains("A \\#hash\\* Name"), "{out}");
    }
}

/// STRUCT-1 — Jinja template paragraphs (`content_type: "jinja"`). The assembler
/// renders these to `.typ` before `typst compile`. Optional block; the default
/// aborts assembly on the first render error so a broken template can't silently
/// drop content from the PDF.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct JinjaConfig {
    /// `false` (default): a Jinja render failure aborts the whole book assembly
    /// with the offending paragraph + error in the message — broken templates
    /// stop the build until fixed (CI-safe). `true`: write a visible Typst error
    /// block into the paragraph's output and keep assembling the rest, so the
    /// author can fix templates one at a time.
    pub continue_on_error: bool,
}

impl Default for JinjaConfig {
    fn default() -> Self {
        Self { continue_on_error: false }
    }
}

/// NARR-1 — narrative-voice (`prose`) profiling. Deterministic, zero-AI voice
/// metrics per chapter, stored in `.inkhaven/prose.duckdb`. All optional; the
/// defaults give a shallow (Tier-1 + language-sensitive) pass with thresholds
/// tuned to English fiction.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProseConfig {
    /// Include Tier-2 metrics (sensory balance + active/passive ratio).
    pub deep_metrics: bool,
    /// MATTR sliding-window size in tokens.
    pub mattr_window: usize,
    /// Chapter that drift / violations are measured against.
    pub baseline_chapter: u32,
    /// Prose language override (`en`/`ru`/`de`/`fr`/`es`); `null` → project
    /// language → English with a note.
    pub language: Option<String>,
    /// Drift thresholds — a chapter metric crossing its threshold vs the
    /// baseline emits an informational `prose` finding.
    pub thresholds: ProseThresholds,
    /// Tokens appended to the active language's modal/epistemic list (e.g.
    /// genre-specific subjunctive collocations). Single words are unigrams;
    /// two/three-word entries are bigrams/trigrams.
    pub extra_modal_tokens: Vec<String>,
    /// Phrases appended to the active language's interiority (FID) list.
    pub extra_interiority_phrases: Vec<String>,
    /// TUI ambient auto-check: re-run the background prose check after an
    /// editing pause. Off by default (manual `Ctrl+V V` only).
    pub ambient: bool,
    /// Cooldown floor (seconds) between ambient prose checks — a whole-book
    /// scan, so longer than the per-paragraph companions.
    pub ambient_cooldown_secs: u64,
}

impl Default for ProseConfig {
    fn default() -> Self {
        Self {
            deep_metrics: false,
            mattr_window: 100,
            baseline_chapter: 1,
            language: None,
            thresholds: ProseThresholds::default(),
            extra_modal_tokens: Vec::new(),
            extra_interiority_phrases: Vec::new(),
            ambient: false,
            ambient_cooldown_secs: 90,
        }
    }
}

/// CHORUS-1 (2.1) — `chorus:` block. Voice & style at book scale (character
/// voice fingerprints, the distinctiveness matrix, and the discipline pillars).
/// CH-P2 lands the distinctiveness knobs; later phases extend this block. All
/// optional; per the permissive principle these inform, they never block.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ChorusConfig {
    /// The RMS z-distance below which two characters' voices are flagged
    /// **indistinguishable** (~"less than this many pooled std-devs apart per
    /// metric, on average"). Genre-relative — the baseline is your own cast's
    /// spread. Lower = fewer, only near-identical pairs flagged. Calibration is
    /// project-dependent; the default is deliberately conservative.
    pub distinct_threshold: f32,
    /// Character pairs to never flag as indistinguishable — deliberate twins, a
    /// uniform chorus, aliases of one speaker. Each entry is two names separated
    /// by `|`, order- and case-insensitive: `["Mara|Joren"]`.
    pub distinct_ignore_pairs: Vec<String>,
    /// The absolute change in a register metric (contraction rate, archaism
    /// density, formality, latinate density — all fractions/ratios) versus the
    /// baseline chapter that flags a **register drift** (CH-P6). Higher = only
    /// larger shifts flagged. Advisory.
    pub register_drift_threshold: f32,
}

impl Default for ChorusConfig {
    fn default() -> Self {
        Self {
            distinct_threshold: 0.5,
            distinct_ignore_pairs: Vec::new(),
            register_drift_threshold: 0.08,
        }
    }
}

/// INNER-STYLIST-1 (CH-P7) — `stylist:` block. The seventh Inner-family reader,
/// the voice-at-scale coach. Its measurement is deterministic + free; only the
/// slow-track LLM coaching draws on a provider. Per the permissive principle the
/// budget informs, it never blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StylistConfig {
    /// Master switch for the Inner Stylist.
    pub enabled: bool,
    /// Informative daily budget (USD) for the LLM coaching track — shown in the
    /// cost dashboard, never enforced.
    pub session_budget: f32,
    /// Language override for the coaching prompt (default: the project language).
    pub language: Option<String>,
}

impl Default for StylistConfig {
    fn default() -> Self {
        Self { enabled: true, session_budget: 0.15, language: None }
    }
}

/// SENTINEL-1 (CT-P3) — `continuity:` block. The unified continuity ledger's one
/// config namespace: a master switch, the incremental-watch toggle + cooldown,
/// per-detector toggles, and the referenced-before-introduced tolerance. The
/// existing scattered knobs (`timeline.critique`, `editor.echo_*`) are untouched —
/// the engine reads those where relevant; this block only adds. Deterministic +
/// free at the core, so it's on by default (the permissive principle).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ContinuityConfig {
    /// Master switch for the continuity ledger in the review pass. When off, the
    /// review pass emits no `continuity` findings (the standalone `inkhaven
    /// continuity check` command still runs — it's explicitly invoked).
    pub enabled: bool,
    /// CT-P5 — re-check the edit's scope on save (incremental "watches itself").
    pub ambient: bool,
    /// Minimum seconds between ambient re-checks (CT-P5 throttle).
    pub ambient_cooldown_secs: u64,
    /// Per-detector toggles. All on by default. `timeline` gates the engine's own
    /// timeline critique; note the review pass already surfaces orphan / overlap
    /// on their own line, so it excludes the engine's timeline pass regardless of
    /// this flag — this toggle governs the standalone `continuity check`.
    pub co_location: bool,
    pub timeline: bool,
    pub numeric: bool,
    pub char_facts: bool,
    pub introduce: bool,
    /// Referenced-before-introduced tolerance, in chapters: a first reference at
    /// most this many chapters before the introduction is treated as
    /// foreshadowing, not a break (0 = flag any earlier-chapter reference).
    pub introduce_tolerance: u32,
}

impl Default for ContinuityConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            ambient: false,
            ambient_cooldown_secs: 30,
            co_location: true,
            timeline: true,
            numeric: true,
            char_facts: true,
            introduce: true,
            introduce_tolerance: 0,
        }
    }
}

impl ContinuityConfig {
    /// Whether a detector (by its `continuity_intel::engine::DETECTORS` key) is
    /// enabled in config. Unknown keys default enabled (forward-compatible).
    pub fn detector_enabled(&self, key: &str) -> bool {
        match key {
            "co_location" => self.co_location,
            "timeline" => self.timeline,
            "numeric" => self.numeric,
            "char_facts" => self.char_facts,
            "introduce" => self.introduce,
            _ => true,
        }
    }
}

/// LECTOR-1 (LR-P6) — `lector:` block. The read-through's config. Master switch
/// for the deterministic read-through line in the review pass (the synthetic
/// first-read is always explicit — the ledger's `k` / `readthrough --deep` — so it
/// has no ambient toggle). Deterministic + free, so on by default. LR-P7 extends
/// this with the genre-framework + intensity knobs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LectorConfig {
    /// Master switch for the read-through line in the review pass. When off, the
    /// review pass emits no `lector` findings (the standalone `inkhaven readthrough`
    /// command still runs — it's explicitly invoked).
    pub enabled: bool,
    /// The story-structure framework whose expected-tension curve the read-through
    /// compares the measured shape against (`three_act` | `save_the_cat` |
    /// `story_circle` | `hero_journey` | `seven_point` | `kishotenketsu`). `null`
    /// suggests one from the project `genre`, falling back to Three-Act.
    pub framework: Option<String>,
}

impl Default for LectorConfig {
    fn default() -> Self {
        Self { enabled: true, framework: None }
    }
}

/// DIALOG-1 — `dialogue:` block. Tunes the dialogue detection windows,
/// finding thresholds, and the genre-specific verb extras. All optional;
/// omitting the block uses these defaults (RFC §13).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DialogueConfig {
    /// Token window for the attribution name search around a span boundary.
    pub attribution_window: usize,
    /// Consecutive unattributed turns tolerated in an established two-speaker
    /// exchange before the zero-attribution finding fires.
    pub unattributed_run_threshold: u32,
    /// Consecutive dialogue-only paragraphs before the talking-head finding.
    pub talking_head_threshold: u32,
    /// Minimum word count for a non-speech sentence to count as an action beat
    /// (and clear the talking-head counter).
    pub beat_min_words: u32,
    /// Said-bookism density delta (above the book baseline) that triggers the
    /// finding.
    pub said_bookism_threshold: f32,
    /// Minimum attributed utterances for a character fingerprint to be shown.
    pub fingerprint_min_utterances: u32,
    /// Dialogue language override (`en`/`ru`/`de`/`fr`/`es`); `null` → project
    /// language → English fallback.
    pub language: Option<String>,
    /// Verbs appended to the active language's *neutral* tag list (e.g. SF
    /// `transmitted`, `intoned`) so they are not counted as said-bookisms.
    pub extra_neutral_verbs: Vec<String>,
    /// Verbs appended to the active language's said-bookism list.
    pub extra_said_bookisms: Vec<String>,
}

impl Default for DialogueConfig {
    fn default() -> Self {
        Self {
            attribution_window: 60,
            unattributed_run_threshold: 8,
            talking_head_threshold: 6,
            beat_min_words: 8,
            said_bookism_threshold: 0.15,
            fingerprint_min_utterances: 5,
            language: None,
            extra_neutral_verbs: Vec::new(),
            extra_said_bookisms: Vec::new(),
        }
    }
}

/// WORLD-6 — `utopia:` block. Tunes the coherence checker's caching and the
/// Stage-2 cost warning. All optional; omitting the block uses these defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UtopiaConfig {
    /// Stage 2 cost-warning threshold (USD). Fires before pairing if the
    /// projected cost exceeds this — informs, never blocks.
    pub stage2_cost_warn: f32,
    /// Stage 2 hard safety cap on claim-pairs checked per group per run. Stage 2
    /// pairing is quadratic in consequence claims; beyond this many pairs the run
    /// refuses (before any LLM call) with guidance to split the premise group,
    /// rather than firing thousands of sequential paid calls. Raisable — it
    /// guards a runaway, it does not cap ordinary use.
    pub stage2_max_pairs: usize,
    /// Chapters processed per Stage 3 idle/background pass.
    pub stage3_batch_size: usize,
    /// Minimum chapter word count to include in the Stage 3 entailment scan.
    pub stage3_min_chapter_words: usize,
    /// Consecutive non-claim paragraphs that break a premise group (default 1;
    /// 0 = all tagged paragraphs are one group).
    pub group_gap_threshold: usize,
}

impl Default for UtopiaConfig {
    fn default() -> Self {
        Self {
            stage2_cost_warn: 0.10,
            stage2_max_pairs: 200,
            stage3_batch_size: 5,
            stage3_min_chapter_words: 200,
            group_gap_threshold: 1,
        }
    }
}

/// CHAR-1 — `char:` block. Tunes the character-arc tracker: the agency windows,
/// the stall threshold, the minimum chapters before LLM arc checks run, the
/// cross-system enrichment toggles, and the genre verb extras. All optional;
/// omitting the block uses these defaults (RFC §15).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CharConfig {
    /// Consecutive unchanged chapters (after the baseline) before the stall
    /// finding fires.
    pub stall_threshold: u32,
    /// Tokens before an action verb a character name may sit and still count as
    /// the actor (active presence).
    pub active_window_before: usize,
    /// Tokens after a verb a character name may sit and count as the patient
    /// (passive presence).
    pub active_window_after: usize,
    /// Minimum chapters of extracted state before the LLM arc-completeness
    /// checks run for a character (fewer → stall only).
    pub min_chapters_for_check: usize,
    /// Enrich the state chain with DIALOG-1 utterance/hedge signals.
    pub enrich_from_dialogue: bool,
    /// Enrich the state chain with NARR-1 chapter interiority.
    pub enrich_from_voice: bool,
    /// Arc language override (`en`/`ru`/`de`/`fr`/`es`); `null` → project
    /// language → English fallback.
    pub language: Option<String>,
    /// Verbs appended to the active language's action-verb list (genre verbs the
    /// agency scorer should treat as deliberate action).
    pub extra_action_verbs: Vec<String>,
    /// State-extraction cost-warning threshold (USD). Informs, never blocks.
    pub extraction_cost_warn: f32,
}

impl Default for CharConfig {
    fn default() -> Self {
        Self {
            stall_threshold: 4,
            active_window_before: 5,
            active_window_after: 8,
            min_chapters_for_check: 3,
            enrich_from_dialogue: true,
            enrich_from_voice: true,
            language: None,
            extra_action_verbs: Vec::new(),
            extraction_cost_warn: 0.20,
        }
    }
}

/// INNER-THEOLOGIAN-1 — `theologian:` block. The tradition-neutral moral/
/// TDOC-1 — technical-documentation tooling. Today: verified code blocks.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DocsConfig {
    pub verify: DocsVerifyConfig,
    /// TDOC-3 — single-sourcing variables. `{{key}}` in any paragraph body is
    /// replaced by its value at assembly, across every export. Empty = no
    /// substitution.
    pub variables: std::collections::BTreeMap<String, String>,
    /// TDOC-4 — HTML static-site export (`inkhaven export html -o <dir>`).
    pub html: DocsHtmlConfig,
    /// INDEX-1 — back-of-book index (`inkhaven index`).
    pub index: DocsIndexConfig,
}

/// INDEX-1 — back-of-book index settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DocsIndexConfig {
    /// Include every Glossary canonical term (and its synonyms as `see`-refs).
    pub from_glossary: bool,
    /// Extra index terms (names, topics) beyond the Glossary.
    pub terms: Vec<String>,
}

impl Default for DocsIndexConfig {
    fn default() -> Self {
        Self { from_glossary: true, terms: Vec::new() }
    }
}

/// TDOC-4 — HTML static-site export settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DocsHtmlConfig {
    /// Site title; `null` → the exported user book's title.
    pub site_title: Option<String>,
    /// Bundled theme name (only `default` today).
    pub theme: String,
    /// Optional project override root holding `functional/` and/or `theme/`
    /// template files (default `html/`); a file present there wins over the
    /// bundled default. See `examples/html_templates/`.
    pub template_dir: String,
    /// HJSON file whose parsed contents are exposed to templates as `site`.
    pub variables_file: String,
    /// TDOC-4.2 — build the client-side search index (accepted now, wired later).
    pub search: bool,
    /// TDOC-4.2 — `author-year` | `numeric` (accepted now, wired later).
    pub citation_style: String,
    /// TDOC-4.3+ — which companion books to fold into the site (accepted now,
    /// wired in later phases).
    pub include: DocsHtmlInclude,
}

impl Default for DocsHtmlConfig {
    fn default() -> Self {
        Self {
            site_title: None,
            theme: "default".into(),
            template_dir: "html".into(),
            variables_file: "html.hjson".into(),
            search: true,
            citation_style: "author-year".into(),
            include: DocsHtmlInclude::default(),
        }
    }
}

/// TDOC-4 — companion-book inclusion toggles for the HTML site.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DocsHtmlInclude {
    pub sources: bool,
    pub glossary: bool,
    pub characters: bool,
    pub places: bool,
    pub language: bool,
    pub world: bool,
    pub mythology: bool,
    pub notes: bool,
    /// INDEX-1 — fold a back-of-book index page into the site.
    pub index: bool,
}

impl Default for DocsHtmlInclude {
    fn default() -> Self {
        Self {
            sources: true,
            glossary: true,
            characters: false,
            places: false,
            language: false,
            world: false,
            mythology: false,
            notes: false,
            index: false,
        }
    }
}

/// TDOC-1 — verified code blocks (`inkhaven docs verify`). Off by default; a
/// language runs only if `runners` names a command for it, and only for code
/// blocks whose fence carries the `verify` flag.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DocsVerifyConfig {
    /// Master switch — nothing runs unless `true`.
    pub enabled: bool,
    /// Per-block wall-clock cap in seconds.
    pub timeout_seconds: u64,
    /// language → shell command. `{file}` is replaced by a temp file holding the
    /// block's code; `{dir}` by its parent directory. Run via `sh -c`.
    pub runners: std::collections::BTreeMap<String, String>,
    /// language → temp-file extension (rust→`rs`, …); seeded with common
    /// languages, overridable. Unknown languages fall back to `.txt`.
    pub extensions: std::collections::BTreeMap<String, String>,
}

impl Default for DocsVerifyConfig {
    fn default() -> Self {
        let extensions = [
            ("rust", "rs"), ("python", "py"), ("bash", "sh"), ("sh", "sh"),
            ("go", "go"), ("javascript", "js"), ("typescript", "ts"), ("c", "c"),
            ("cpp", "cpp"), ("c++", "cpp"), ("java", "java"), ("ruby", "rb"),
            ("toml", "toml"), ("json", "json"), ("yaml", "yaml"), ("html", "html"),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        Self { enabled: false, timeout_seconds: 30, runners: std::collections::BTreeMap::new(), extensions }
    }
}

/// theological reader. All optional; omitting the block uses these defaults
/// (RFC §14). Fully opt-out via `enabled: false`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TheologianConfig {
    /// Master switch. `false` gates everything, including fast-track.
    pub enabled: bool,
    /// Fire a Category-1 question on paragraph idle (auto-fire). `false` → only
    /// `Ctrl+B J→T` and the Output-pane fast-track signals.
    pub on_paragraph_idle: bool,
    /// Idle seconds before the auto-fire question; `null` → Inner Socrates' Slow
    /// idle threshold.
    pub idle_threshold_seconds: Option<u64>,
    /// Slow-track LLM sub-budget (USD per session). Caps inform, never block.
    pub session_budget: f32,
    /// Run fast-track detection in the review pass / background deep-refresh.
    pub fast_track: bool,
    /// Paragraphs after a harm event checked for acknowledgment (Signal 1).
    pub moral_invisibility_window: usize,
    /// Paragraphs after lethal violence checked for consequence (Signal 2).
    pub consequence_gap_window: usize,
    /// Emit the sacred-vocabulary-in-levity signal (Signal 3).
    pub sacred_levity_signal: bool,
    /// Tradition lens codes to EXCLUDE from slow-track lens hints (default none —
    /// all eleven available). E.g. `["gnostic"]`.
    pub disabled_lenses: Vec<String>,
    /// Question/marker language override (`en`/`ru`/`de`/`fr`/`es`); `null` →
    /// project language → English.
    pub language: Option<String>,
}

impl Default for TheologianConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            on_paragraph_idle: true,
            idle_threshold_seconds: None,
            session_budget: 0.15,
            fast_track: true,
            moral_invisibility_window: 3,
            consequence_gap_window: 5,
            sacred_levity_signal: true,
            disabled_lenses: Vec::new(),
            language: None,
        }
    }
}

/// RIGOR (1.6.20+) — `rigor:` block. The deterministic, zero-AI reasoning-rigor
/// reader (`⊬`) that flags argument-rigor signals — false dichotomy,
/// question-begging, straw man, overgeneralization, non-sequitur — via
/// language-keyed cue markers. Advisory, never a verdict; the argument-side
/// complement to the Inner Theologian. Per-category toggles let a project mute a
/// signal that its genre uses legitimately.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RigorConfig {
    /// Master switch. `false` gates everything.
    pub enabled: bool,
    /// Run the reader in the review pass / deep-refresh (the ambient surface).
    pub fast_track: bool,
    /// Marker language override (`en`/`ru`/`de`/`fr`/`es`); `null` → project
    /// language → English.
    pub language: Option<String>,
    /// Flag forced-binary framings ("either … or", "the only alternative").
    pub false_dichotomy: bool,
    /// Flag unargued assertions ("obviously", "of course").
    pub question_begging: bool,
    /// Flag dismissive characterizations ("so-called", "would have us believe").
    pub straw_man: bool,
    /// Flag strong absolutes ("always", "never", "without exception").
    pub overgeneralization: bool,
    /// Flag a conclusion connective with no warrant marker in the paragraph.
    pub non_sequitur: bool,
    /// LEXICON — flag an equivocation-watched, multi-sense term (declared in the
    /// Glossary with `watch_equivocation`) used repeatedly without pinning a sense.
    pub equivocation: bool,
}

impl Default for RigorConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            fast_track: true,
            language: None,
            false_dichotomy: true,
            question_begging: true,
            straw_man: true,
            overgeneralization: true,
            non_sequitur: true,
            equivocation: true,
        }
    }
}

/// ORACLE (1.7.9+) — `oracle:` block. The conlang well-formedness Oracle watching
/// your prose. On save, the phonotactic guardian checks the conlang words in the
/// saved paragraph — words that segment fully into a language's inventory but are
/// not listed — and flags any that break that language's phonotactics, as an
/// advisory Output finding. Zero-AI, deterministic, never edits prose.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OracleConfig {
    /// Master switch. `false` gates the on-save scan entirely.
    pub enabled: bool,
    /// Run the phonotactic guardian when a paragraph is saved.
    pub on_save: bool,
}

impl Default for OracleConfig {
    fn default() -> Self {
        Self { enabled: true, on_save: true }
    }
}

/// MYTH-1 — `myth:` block. The mythological & symbolic pattern library over the
/// declared Mythology book. All optional; omitting the block uses these defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MythConfig {
    /// Master switch. `false` gates the deterministic review-pass findings and
    /// the heatmap chord.
    pub enabled: bool,
    /// Number of chapter buckets the heatmap collapses the book into.
    pub heatmap_buckets: usize,
    /// A symbol must appear in at least this many chapters before the LLM
    /// consistency check runs on it.
    pub consistency_min_chapters: u32,
    /// A motif must have at least this many occurrences before the LLM
    /// completeness check runs on it.
    pub motif_min_occurrences: u32,
    /// The final act = the last this-percent of chapters (motif-absent check).
    pub final_act_pct: u32,
    /// Warn (inform, never block) when an `inkhaven myth check` LLM run is
    /// estimated to exceed this many USD.
    pub check_cost_warn: f32,
}

impl Default for MythConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            heatmap_buckets: 8,
            consistency_min_chapters: 5,
            motif_min_occurrences: 3,
            final_act_pct: 25,
            check_cost_warn: 0.08,
        }
    }
}

/// WORLD-12 — `world:` block. Tunes the AI world-critique pass (`inkhaven
/// realworld critique`). All optional; omitting the block uses these defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WorldConfig {
    /// Master switch for the AI critique pass. `false` makes `realworld critique`
    /// run the deterministic lints only, skipping the LLM call.
    pub critique_enabled: bool,
    /// Per-call soft cap (estimated tokens) for the critique LLM call. Informs
    /// via the preflight; `--force` / `--max-cost` override it. `0` disables.
    pub critique_max_tokens: usize,
    /// Warn (inform, never block) when a critique run is estimated to exceed this
    /// many USD.
    pub critique_cost_warn: f32,
}

impl Default for WorldConfig {
    fn default() -> Self {
        Self { critique_enabled: true, critique_max_tokens: 24_000, critique_cost_warn: 0.10 }
    }
}

/// RESRCH-1 — `research:` block tuning the Research Assistant (`inkhaven
/// research`). All optional; omitting the block uses these defaults (RFC §23).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ResearchConfig {
    /// Default thread to open (null = picker / `default`).
    pub default_thread: Option<String>,
    /// Max Facts paragraphs prepended per query as RAG context.
    pub rag_top_n: usize,
    /// Per-session cost-cap warning (USD). Informs, never blocks.
    pub session_budget_warn: f64,
    /// Max pinned nodes in the Facts tree.
    pub max_pinned_nodes: usize,
    /// Show the keybind hints bar by default.
    pub show_keybind_hints: bool,
    /// Minimum terminal width; below it, a resize message shows.
    pub min_width: u16,
    /// Facts tree / chat split: tree columns out of 10 (4 = 40% tree).
    pub split_ratio: u32,
    /// `/diff`: number of similar facts to show.
    pub diff_top_n: usize,
    /// `/verify`: minimum sentence word count for claim extraction.
    pub verify_min_sentence_words: usize,
    /// RESRCH-2.1 — similarity score (0..1) at/above which a `/fact` insert warns
    /// of a near-duplicate before committing (informs, never blocks).
    pub dedup_warn_score: f64,
    /// RESRCH-3 (R3-E) — when true, a `/fact` from a `model` / `web` / `document`
    /// source is **triangulated** across the structured sources before it commits
    /// (cross-source agreement replaces the single-source self-check). Off by
    /// default — it is network-heavy. Informs; a weak verdict just asks to
    /// confirm again.
    pub triangulate_gate: bool,
    /// RESRCH-6-lite (R6-A) — when true, a `/fact` from a `model` / `document`
    /// source that is *not* otherwise gated gets one **adversarial refutation**
    /// pass before it commits: the model actively tries to refute the claim, and
    /// a `REFUTED` verdict asks the author to confirm again (advisory — never a
    /// hard block). Off by default.
    pub refute_gate: bool,
    /// RESRCH-2 (R2-B) — max characters per embedded chunk when importing a
    /// document (`/import`).
    pub import_chunk_chars: usize,
    /// RESRCH-2 (R2-C) — web search & fetch settings.
    #[serde(default)]
    pub web: WebConfig,
    /// RESRCH-3 (R3-A) — `research.wikidata` block for `/wikidata`.
    pub wikidata: WikidataConfig,
    /// RESRCH-3 (R3-B) — `research.scholarly` block for `/openalex` + `/arxiv`.
    pub scholarly: ScholarlyConfig,
    /// RESRCH-6-lite — `research.geonames` block for `/geonames`.
    pub geonames: GeonamesConfig,
    /// RESRCH-GUTENBERG — `research.gutenberg` block for `/gutenberg`.
    pub gutenberg: GutenbergConfig,
    /// RESRCH-ARCHIVE — `research.archive` block for `/archive`.
    pub archive: ArchiveConfig,
    /// RESRCH-WIKISOURCE — `research.wikisource` block for `/wikisource`.
    pub wikisource: WikisourceConfig,
    /// RESRCH-SCRIPTURE — `research.scripture` block for `/bible` + `/quran` +
    /// `/bookofmormon`.
    pub scripture: ScriptureConfig,
    /// RESRCH-6 — `research.agentic` block gating the autonomous deep-research
    /// loop (decompose → gather → emit Facts → critique). On by default; set
    /// `research.agentic.enabled: false` to disable it entirely.
    pub agentic: AgenticConfig,
}

/// RESRCH-6 — `research.agentic` block. The autonomous, gap-driven research loop
/// that decomposes a topic into sub-questions, gathers evidence, and **emits its
/// findings as Facts paragraphs into the Facts system book** (each with
/// provenance, at an untrusted tier for the author to review/promote) — never a
/// standalone article. On by default; the author can turn it off.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AgenticConfig {
    /// Master switch. `true` (default) allows agentic runs; `false` disables the
    /// loop entirely (the command refuses with a hint to re-enable).
    pub enabled: bool,
    /// The **total** sub-question budget for a run, across every round — a hard
    /// ceiling on how many Facts one `--agentic` run can emit (and thus the LLM
    /// cost). The loop stops when this is spent.
    pub max_subquestions: usize,
    /// RESRCH-6 (R6-P3) — the maximum number of gap-driven iterate rounds. After
    /// the initial plan+gather, a critic proposes follow-up sub-questions for the
    /// gaps; the loop runs at most this many further rounds (or until it converges
    /// / the budget is spent). `1` disables iteration (single pass).
    pub max_rounds: usize,
}

impl Default for AgenticConfig {
    fn default() -> Self {
        // On by default (user-decided 2026-07-26); bounded so a run is affordable.
        Self { enabled: true, max_subquestions: 6, max_rounds: 3 }
    }
}

/// RESRCH-GUTENBERG — `research.gutenberg` block. `/gutenberg` searches the
/// keyless **Gutendex** catalogue and ingests a public-domain book's plain text
/// as a research source. `max_chars` bounds the embedded portion of a book.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GutenbergConfig {
    /// Master switch for `/gutenberg` (keyless — on by default).
    pub enabled: bool,
    /// Base URL of the Gutendex API host (override for a mirror).
    pub endpoint: String,
    /// Max characters of a book's text to ingest (bounds embedding cost).
    pub max_chars: usize,
    /// Auto-create a SOURCES-1 `BibEntry` for an ingested book (PG-P3).
    pub auto_cite: bool,
}

impl Default for GutenbergConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            endpoint: "https://gutendex.com".to_string(),
            max_chars: 300_000,
            auto_cite: true,
        }
    }
}

/// RESRCH-ARCHIVE (1.6.16+) — `research.archive` block. `/archive` searches the
/// keyless **Internet Archive** (`advancedsearch.php`, scoped to `mediatype:texts`)
/// and ingests a public-domain text's OCR plain text as a research source.
/// `max_chars` bounds the embedded portion.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ArchiveConfig {
    /// Master switch for `/archive` (keyless — on by default).
    pub enabled: bool,
    /// Base URL of the Internet Archive host (override for a mirror).
    pub endpoint: String,
    /// Max characters of a text to ingest (bounds embedding cost).
    pub max_chars: usize,
    /// Auto-create a SOURCES-1 `BibEntry` for an ingested text.
    pub auto_cite: bool,
}

impl Default for ArchiveConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            endpoint: "https://archive.org".to_string(),
            max_chars: 300_000,
            auto_cite: true,
        }
    }
}

/// RESRCH-WIKISOURCE (1.6.16+) — `research.wikisource` block. `/wikisource`
/// searches `{lang}.wikisource.org` via the keyless MediaWiki API and ingests a
/// public-domain page's plain-text extract as a research source. The subdomain
/// is the book's language code (falling back to `default_lang`), so a native
/// author gets native public-domain texts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WikisourceConfig {
    /// Master switch for `/wikisource` (keyless — on by default).
    pub enabled: bool,
    /// Language subdomain used when the book language can't be resolved.
    pub default_lang: String,
    /// Max characters of a page to ingest (bounds embedding cost).
    pub max_chars: usize,
    /// Auto-create a SOURCES-1 `BibEntry` for an ingested page.
    pub auto_cite: bool,
}

impl Default for WikisourceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_lang: "en".to_string(),
            max_chars: 300_000,
            auto_cite: true,
        }
    }
}

/// RESRCH-SCRIPTURE (1.6.18+) — `research.scripture` block for `/bible`, `/quran`,
/// `/bookofmormon`. All keyless and public-domain: bolls.life (Bible; en=WEB,
/// ru=SYNOD, fr=FRLSG, de=LUT, es=RV1960 by project language), api.alquran.cloud
/// (Qur'an; en=en.sahih, ru=ru.kuliev, … + Arabic `quran-uthmani`), and the
/// bcbooks public-domain 1830 Book of Mormon corpus. Each ingest auto-cites a
/// *stable* key (`bible` / `quran` / `book-of-mormon`) so loci — `@bible[John
/// 3:16]` — group in the Index Locorum.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptureConfig {
    /// Master switch for `/bible` + `/quran` + `/bookofmormon` (keyless — on).
    pub enabled: bool,
    /// bolls.life base URL (override for a mirror).
    pub bible_endpoint: String,
    /// api.alquran.cloud v1 base URL.
    pub quran_endpoint: String,
    /// The public-domain Book of Mormon JSON corpus (1830 English).
    pub bom_url: String,
    /// Force a bolls translation code regardless of project language (null = pick
    /// by language: en=WEB, ru=SYNOD, fr=FRLSG, de=LUT, es=RV1960).
    pub bible_translation: Option<String>,
    /// Force an alquran.cloud edition regardless of language (null = pick by
    /// language; set `quran-uthmani` for the Arabic original).
    pub quran_translation: Option<String>,
    /// Max characters of a passage to ingest (bounds embedding cost).
    pub max_chars: usize,
    /// Auto-create the stable SOURCES-1 `BibEntry` for an ingested passage.
    pub auto_cite: bool,
}

impl Default for ScriptureConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bible_endpoint: "https://bolls.life".to_string(),
            quran_endpoint: "https://api.alquran.cloud/v1".to_string(),
            bom_url: "https://raw.githubusercontent.com/bcbooks/scriptures-json/master/book-of-mormon.json"
                .to_string(),
            bible_translation: None,
            quran_translation: None,
            max_chars: 200_000,
            auto_cite: true,
        }
    }
}

/// RESRCH-3 (R3-B) — `research.scholarly` block. `/openalex` and `/arxiv` are
/// keyless; `mailto` joins OpenAlex's "polite pool" (recommended, avoids the
/// anonymous rate limit). Scholarly tier — a `/fact` from a paper auto-creates a
/// SOURCES-1 `BibEntry`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScholarlyConfig {
    /// Master switch for `/openalex` + `/arxiv`.
    pub enabled: bool,
    /// Contact email for OpenAlex's polite pool (optional but recommended).
    pub mailto: String,
    /// Auto-create a SOURCES-1 bibliography entry when a `/fact` is taken from a
    /// paper.
    pub auto_cite: bool,
}

impl Default for ScholarlyConfig {
    fn default() -> Self {
        Self { enabled: true, mailto: String::new(), auto_cite: true }
    }
}

/// RESRCH-3 (R3-A) — `research.wikidata` block. `/wikidata` queries Wikidata's
/// **structured** entity claims (Q-ID-cited, keyless) — the top of the trust
/// ladder, so a `/fact` from it skips the fact-check gate. Wikipedia's prose is
/// deliberately excluded; we ground on the triples, not the narrative.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WikidataConfig {
    /// Master switch for `/wikidata` (keyless — on by default).
    pub enabled: bool,
    /// Base URL of the Wikibase API host.
    pub endpoint: String,
    /// Max property statements rendered per entity.
    pub max_statements: usize,
}

impl Default for WikidataConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            endpoint: "https://www.wikidata.org".to_string(),
            max_statements: 24,
        }
    }
}

/// RESRCH-6-lite — `research.geonames` block for `/geonames` (real-world places
/// via the GeoNames gazetteer). GeoNames needs a free **username** (registration,
/// not a key), so it stays unavailable until `username` is set.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GeonamesConfig {
    /// Master switch for `/geonames`.
    pub enabled: bool,
    /// Base URL of the GeoNames API host.
    pub endpoint: String,
    /// Free GeoNames username (register at geonames.org). Empty → unavailable.
    pub username: String,
}

impl Default for GeonamesConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            endpoint: "http://api.geonames.org".to_string(),
            username: String::new(),
        }
    }
}

/// RESRCH-2 (R2-C) — `research.web` block. `/web` is unavailable until a
/// provider is configured; everything degrades cleanly when absent / offline.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WebConfig {
    /// Master switch for `/web`.
    pub enabled: bool,
    /// `tavily` | `searxng` | `none`.
    pub provider: String,
    /// API key (Tavily).
    pub api_key: String,
    /// Base URL of a SearXNG instance (e.g. `https://searx.example.org`).
    pub endpoint: String,
    /// Max results to retrieve.
    pub max_results: usize,
    /// Fetch each result's full page text (SearXNG; Tavily returns content
    /// inline). When false, only titles + snippets are used.
    pub fetch: bool,
    /// Default pipeline for a bare `/web`: `chat` (LLM + factcheck-before-commit)
    /// or `ingest` (embed pages as cited research sources).
    pub pipeline: String,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: "none".to_string(),
            api_key: String::new(),
            endpoint: String::new(),
            max_results: 5,
            fetch: true,
            pipeline: "chat".to_string(),
        }
    }
}

impl Default for ResearchConfig {
    fn default() -> Self {
        Self {
            default_thread: None,
            rag_top_n: 5,
            session_budget_warn: 0.50,
            max_pinned_nodes: 3,
            show_keybind_hints: true,
            min_width: 80,
            split_ratio: 4,
            diff_top_n: 3,
            verify_min_sentence_words: 8,
            dedup_warn_score: 0.92,
            triangulate_gate: false,
            refute_gate: false,
            import_chunk_chars: 1500,
            web: WebConfig::default(),
            wikidata: WikidataConfig::default(),
            scholarly: ScholarlyConfig::default(),
            geonames: GeonamesConfig::default(),
            gutenberg: GutenbergConfig::default(),
            archive: ArchiveConfig::default(),
            wikisource: WikisourceConfig::default(),
            scripture: ScriptureConfig::default(),
            agentic: AgenticConfig::default(),
        }
    }
}

/// Per-metric drift thresholds for the `prose` finding category.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProseThresholds {
    pub sent_len_cv: f32,
    pub burstiness_b: f32,
    pub mattr: f32,
    pub modal_density: f32,
    pub interiority_ratio: f32,
    pub de_erlebte_rede_particle_density: f32,
    pub sensory_channel_max: f32,
    pub active_passive_ratio: f32,
}

impl Default for ProseThresholds {
    fn default() -> Self {
        Self {
            sent_len_cv: 0.15,
            burstiness_b: 0.15,
            mattr: 0.05,
            modal_density: 0.020,
            interiority_ratio: 0.10,
            de_erlebte_rede_particle_density: 0.05,
            sensory_channel_max: 0.15,
            active_passive_ratio: 1.5,
        }
    }
}

/// Advisory single-instance project lock (1.3.36). The data-safety
/// carve-out to the permissive principle: it *informs*, and by default
/// never hard-blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectLockConfig {
    /// Acquire the lock at all. `false` disables single-instance
    /// guarding entirely (e.g. if you knowingly run two read-mostly
    /// sessions). Default `true`.
    pub enabled: bool,
    /// What to do when another *live* session already holds the lock:
    /// `"prompt"` (default — interactive `y/N`; warn-and-proceed when
    /// non-interactive), `"warn"` (always warn and proceed), or
    /// `"refuse"` (never open a second session).
    pub on_conflict: String,
}

impl Default for ProjectLockConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            on_conflict: "prompt".into(),
        }
    }
}

/// When the writing "day" rolls over for streaks, daily word/active
/// totals, AI-usage tallies, and the slow-track daily caps. `Utc`
/// (default, preserving prior behaviour) resets at 00:00 UTC; `Local`
/// resets at the writer's local midnight — so an evening session in a
/// far-from-UTC timezone isn't attributed to "tomorrow" and the streak
/// doesn't flip mid-evening.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DayBoundary {
    #[default]
    Utc,
    Local,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GoalsConfig {
    /// When the writing day rolls over (`utc` default, or `local`).
    /// Governs the streak, daily word/active totals, AI-usage tallies,
    /// and the slow-track daily caps so they all agree on "today".
    pub day_boundary: DayBoundary,
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
            day_boundary: DayBoundary::default(),
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
/// Two 1.3.0 PDF-1 entries operate on the just-built PDF rather
/// than the `.typ` source: `imposed_pdf` (impose into signatures,
/// see `imposed_pdf_config`) and `cover_pdf` (generate a
/// cover-and-spine from the page count + `cover:` config).
/// `docx` (1.3.1) builds a Shunn-format Word document from the
/// book's chapters (the same model as `inkhaven docx`).
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
    /// 1.3.0 PDF-1 — the imposition profile used when `imposed_pdf` is in
    /// `extra_formats`.  Names a profile from `imposition.profiles`
    /// (default `default`).
    #[serde(default = "default_imposed_pdf_config")]
    pub imposed_pdf_config: String,
}

fn default_imposed_pdf_config() -> String {
    "default".to_string()
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            extra_formats: Vec::new(),
            extras_step_pause_ms: 400,
            extras_wait_for_key: false,
            imposed_pdf_config: default_imposed_pdf_config(),
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
    /// TIMELINE-2-INTEGRATION — the refactored timeline critique (orphan +
    /// fuzzy-precision overlap).
    pub critique: TimelineCritiqueConfig,
}

impl Default for TimelineConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            default_track: "main".into(),
            calendar: crate::timeline::calendar::CalendarConfig::default(),
            display: TimelineDisplayConfig::default(),
            critique: TimelineCritiqueConfig::default(),
        }
    }
}

/// TIMELINE-2-INTEGRATION — knobs for the two retained, timeline-internal critique
/// checks plus optional LLM elaboration. Sensible defaults; users tune per project.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TimelineCritiqueConfig {
    /// Master switch — when false the critique never runs (chords + CLI no-op).
    pub enabled: bool,
    pub orphan: TimelineOrphanConfig,
    pub fuzzy_overlap: TimelineFuzzyOverlapConfig,
    pub elaboration: TimelineElaborationConfig,
    pub legacy_flag_deprecation: TimelineLegacyDeprecationConfig,
}

impl Default for TimelineCritiqueConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            orphan: TimelineOrphanConfig::default(),
            fuzzy_overlap: TimelineFuzzyOverlapConfig::default(),
            elaboration: TimelineElaborationConfig::default(),
            legacy_flag_deprecation: TimelineLegacyDeprecationConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TimelineOrphanConfig {
    pub enabled: bool,
    /// Don't emit orphan findings for events younger than this (days). `0` = emit
    /// immediately.
    pub min_orphan_age_days: i64,
    /// Lowest significance to surface — `"low" | "moderate" | "high"`.
    pub min_significance: String,
}

impl Default for TimelineOrphanConfig {
    fn default() -> Self {
        Self { enabled: true, min_orphan_age_days: 0, min_significance: "low".into() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TimelineFuzzyOverlapConfig {
    pub enabled: bool,
    /// Lowest suspicion to surface — `"low" | "moderate" | "high"`.
    pub min_suspicion: String,
    /// Minimum events for a cluster (vs pairwise) finding.
    pub cluster_min_size: usize,
}

impl Default for TimelineFuzzyOverlapConfig {
    fn default() -> Self {
        Self { enabled: true, min_suspicion: "moderate".into(), cluster_min_size: 3 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TimelineElaborationConfig {
    /// Use LLM elaboration of pattern-detected findings when a provider is
    /// configured. Falls back to pattern-only text otherwise.
    pub enabled: bool,
    /// Hard cap on elaboration LLM calls per critique run.
    pub max_calls_per_run: usize,
    /// Ask for confirmation once a run would exceed this many calls.
    pub confirm_above_calls: usize,
}

impl Default for TimelineElaborationConfig {
    fn default() -> Self {
        Self { enabled: true, max_calls_per_run: 20, confirm_above_calls: 10 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TimelineLegacyDeprecationConfig {
    /// Print a deprecation warning when `event critique --legacy` is used.
    pub warn_on_use: bool,
}

impl Default for TimelineLegacyDeprecationConfig {
    fn default() -> Self {
        Self { warn_on_use: true }
    }
}

impl TimelineCritiqueConfig {
    /// Parse `orphan.min_significance` into the critique enum (defaults to Low on an
    /// unrecognised value).
    pub fn min_significance(&self) -> crate::timeline::critique::Significance {
        use crate::timeline::critique::Significance;
        match self.orphan.min_significance.to_ascii_lowercase().as_str() {
            "high" => Significance::High,
            "moderate" => Significance::Moderate,
            _ => Significance::Low,
        }
    }

    /// Parse `fuzzy_overlap.min_suspicion` into the critique enum (defaults to
    /// Moderate on an unrecognised value).
    pub fn min_suspicion(&self) -> crate::timeline::critique::Suspicion {
        use crate::timeline::critique::Suspicion;
        match self.fuzzy_overlap.min_suspicion.to_ascii_lowercase().as_str() {
            "low" => Suspicion::Low,
            "high" => Suspicion::High,
            _ => Suspicion::Moderate,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TimelineDisplayConfig {
    pub show_orphans: bool,
    pub swim_lane_max_rows: u32,
    pub default_zoom: f32,
    /// 1.2.7+ — paint a faint vertical bar every N days across
    /// the swim-lane view (axis row + each track row, in cells
    /// that aren't already covered by an event marker or by the
    /// time cursor). Set to `0` to disable the grid entirely.
    /// Default `7` — one stripe per week, useful for sols /
    /// gregorian calendars. Custom calendars: assumes
    /// `base_unit = "day"` (the typical case); 1 day == 1 tick.
    pub grid_every_days: u32,
}

impl Default for TimelineDisplayConfig {
    fn default() -> Self {
        Self {
            show_orphans: true,
            swim_lane_max_rows: 12,
            default_zoom: 1.0,
            grid_every_days: 7,
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

// ── config layering (1.2.20+) ────────────────────────────
#[cfg(test)]
mod layering_tests {
    use super::*;
    use std::path::Path;

    fn write(path: &Path, body: &str) {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p).unwrap();
        }
        std::fs::write(path, body).unwrap();
    }

    // ── merge_value ───────────────────────────────────

    #[test]
    fn merge_objects_recursively_overlay_wins() {
        let mut base = serde_json::json!({
            "theme": { "pane_fg": "#aaa", "modal_fg": "#bbb" },
            "editor": { "reading_wpm": 200 }
        });
        let overlay = serde_json::json!({
            "theme": { "pane_fg": "#ccc" }
        });
        merge_value(&mut base, overlay);
        // Overridden key wins…
        assert_eq!(base["theme"]["pane_fg"], "#ccc");
        // …siblings untouched…
        assert_eq!(base["theme"]["modal_fg"], "#bbb");
        // …unrelated subtrees untouched.
        assert_eq!(base["editor"]["reading_wpm"], 200);
    }

    #[test]
    fn security_floor_survives_a_block_list_override() {
        // M5 — a project config that sets `shell.blocked_externals` to
        // add one tool must NOT drop the shipped security blocks.
        let tmp = tempfile::tempdir().unwrap();
        let proj = tmp.path().join("inkhaven.hjson");
        write(&proj, "{ shell: { blocked_externals: [\"mytool\"] } }");
        let cfg = Config::load_layered_from(&proj, None).unwrap();
        let blocked = &cfg.shell.blocked_externals;
        // The user's addition is present…
        assert!(blocked.iter().any(|b| b == "mytool"));
        // …and the high-risk shipped blocks were NOT wiped.
        for must in ["sudo", "ssh", "vim", "less", "passwd"] {
            assert!(blocked.iter().any(|b| b == must), "floor lost `{must}`");
        }
    }

    #[test]
    fn merge_scalar_replaces_object_wholesale() {
        // A type change (object → scalar) replaces, never
        // tries to merge into the scalar.
        let mut base = serde_json::json!({ "x": { "a": 1 } });
        merge_value(&mut base, serde_json::json!({ "x": 7 }));
        assert_eq!(base["x"], 7);
    }

    // ── global_config_files_in ────────────────────────

    #[test]
    fn global_files_config_first_then_sorted_conf() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write(&dir.join("config.hjson"), "{}");
        write(&dir.join("conf/20-b.hjson"), "{}");
        write(&dir.join("conf/10-a.hjson"), "{}");
        write(&dir.join("conf/ignore.txt"), "not hjson");
        let files = global_config_files_in(dir);
        let names: Vec<String> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["config.hjson", "10-a.hjson", "20-b.hjson"]);
    }

    #[test]
    fn global_files_empty_when_dir_absent() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(global_config_files_in(&tmp.path().join("nope")).is_empty());
    }

    // ── load_layered_from ─────────────────────────────

    fn project_with(tmp: &Path, body: &str) -> std::path::PathBuf {
        let p = tmp.join("inkhaven.hjson");
        write(&p, body);
        p
    }

    #[test]
    fn global_overrides_project_value() {
        let tmp = tempfile::tempdir().unwrap();
        let proj = project_with(tmp.path(), r##"{ theme: { pane_fg: "#111111" } }"##);
        let gdir = tmp.path().join("global");
        write(&gdir.join("config.hjson"), r##"{ theme: { pane_fg: "#222222" } }"##);

        let cfg = Config::load_layered_from(&proj, Some(&gdir)).unwrap();
        // The whole point: global wins over the project's
        // own (full) config so one personal override applies
        // everywhere without editing each project.
        assert_eq!(cfg.theme.pane_fg, "#222222");
    }

    #[test]
    fn unset_global_key_falls_through_to_project() {
        let tmp = tempfile::tempdir().unwrap();
        let proj = project_with(
            tmp.path(),
            r##"{ theme: { pane_fg: "#111111", modal_fg: "#999999" } }"##,
        );
        let gdir = tmp.path().join("global");
        // Partial override — only pane_fg.
        write(&gdir.join("config.hjson"), r##"{ theme: { pane_fg: "#222222" } }"##);

        let cfg = Config::load_layered_from(&proj, Some(&gdir)).unwrap();
        assert_eq!(cfg.theme.pane_fg, "#222222"); // overridden
        assert_eq!(cfg.theme.modal_fg, "#999999"); // fell through
    }

    #[test]
    fn conf_dir_later_file_wins_over_config_hjson() {
        let tmp = tempfile::tempdir().unwrap();
        let proj = project_with(tmp.path(), r##"{ theme: { pane_fg: "#000000" } }"##);
        let gdir = tmp.path().join("global");
        write(&gdir.join("config.hjson"), r##"{ theme: { pane_fg: "#111111" } }"##);
        write(&gdir.join("conf/50-late.hjson"), r##"{ theme: { pane_fg: "#222222" } }"##);

        let cfg = Config::load_layered_from(&proj, Some(&gdir)).unwrap();
        // config.hjson < conf/*.hjson in precedence.
        assert_eq!(cfg.theme.pane_fg, "#222222");
    }

    #[test]
    fn no_global_dir_is_plain_project_load() {
        let tmp = tempfile::tempdir().unwrap();
        let proj = project_with(tmp.path(), r##"{ theme: { pane_fg: "#abcabc" } }"##);
        let cfg = Config::load_layered_from(&proj, None).unwrap();
        assert_eq!(cfg.theme.pane_fg, "#abcabc");
    }

    #[test]
    fn malformed_global_is_skipped_not_fatal() {
        let tmp = tempfile::tempdir().unwrap();
        let proj = project_with(tmp.path(), r##"{ theme: { pane_fg: "#314159" } }"##);
        let gdir = tmp.path().join("global");
        // Not valid HJSON — a dangling brace.
        write(&gdir.join("config.hjson"), "{ theme: { pane_fg: ");

        // Must still succeed, keeping the project value.
        let cfg = Config::load_layered_from(&proj, Some(&gdir)).unwrap();
        assert_eq!(cfg.theme.pane_fg, "#314159");
    }

    #[test]
    fn malformed_project_is_fatal() {
        let tmp = tempfile::tempdir().unwrap();
        let proj = project_with(tmp.path(), "{ broken ");
        assert!(Config::load_layered_from(&proj, None).is_err());
    }

    #[test]
    fn partial_project_fills_from_defaults() {
        // A minimal (non-init) project config must still
        // produce a complete Config — the defaults base
        // guarantees it.
        let tmp = tempfile::tempdir().unwrap();
        let proj = project_with(tmp.path(), r#"{ language: "russian" }"#);
        let cfg = Config::load_layered_from(&proj, None).unwrap();
        assert_eq!(cfg.language, "russian");
        // A field absent from the project comes from default.
        assert_eq!(cfg.theme.pane_fg, ThemeConfig::default().pane_fg);
    }

    // The shipped `color_styles/*.hjson` presets must each
    // be valid HJSON, contain only real `theme` colour keys
    #[test]
    fn parse_color_handles_non_ascii_without_panic() {
        // 1.2.23 stability fix: a multibyte char in a 3- or 6-byte hex
        // string used to split a UTF-8 char and panic; now → None.
        assert_eq!(parse_color("#aé"), None); // "aé" = 3 bytes
        assert_eq!(parse_color("aébcd"), None); // 5 chars / 6 bytes
        assert_eq!(parse_color("#ñ"), None);
        assert_eq!(parse_color("日本語"), None);
        // The valid cases still work.
        assert!(parse_color("#fff").is_some());
        assert!(parse_color("#89b4fa").is_some());
        assert!(parse_color("89b4fa").is_some());
        assert_eq!(parse_color(""), None);
        assert_eq!(parse_color("#xyz"), None); // ASCII but not hex
    }

    // that parse, and layer cleanly into a complete Config —
    // guards the presets against bit-rot when fields change.
    #[test]
    fn color_style_presets_all_parse() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("color_styles");
        let mut count = 0;
        for entry in std::fs::read_dir(&dir).expect("color_styles dir exists") {
            let path = entry.unwrap().path();
            if path.extension().and_then(|s| s.to_str()) != Some("hjson") {
                continue;
            }
            let raw = std::fs::read_to_string(&path).unwrap();
            let v: serde_json::Value = serde_hjson::from_str(&raw)
                .unwrap_or_else(|e| panic!("{}: invalid HJSON: {e}", path.display()));
            let theme = v
                .get("theme")
                .and_then(|t| t.as_object())
                .unwrap_or_else(|| panic!("{}: no theme object", path.display()));
            // Every value must be a parseable colour string.
            for (k, val) in theme {
                let hex = val.as_str().unwrap_or_else(|| {
                    panic!("{}: theme.{k} is not a string", path.display())
                });
                assert!(
                    parse_color(hex).is_some(),
                    "{}: theme.{k} = `{hex}` is not a valid colour",
                    path.display(),
                );
            }
            // And the file must layer into a complete Config
            // (the cascade the presets are meant for).
            let mut merged = serde_json::to_value(Config::default()).unwrap();
            merge_value(&mut merged, v);
            let _: Config = serde_json::from_value(merged)
                .unwrap_or_else(|e| panic!("{}: not a valid Config: {e}", path.display()));
            count += 1;
        }
        assert!(count >= 15, "expected >= 15 presets, found {count}");
    }
}

// 1.3.32+ (road to 1.4.0) — config parser property sweep.
#[cfg(test)]
mod research_config_tests {
    use super::{Config, ResearchConfig};

    #[test]
    fn defaults_match_rfc() {
        let r = ResearchConfig::default();
        assert_eq!(r.rag_top_n, 5);
        assert_eq!(r.max_pinned_nodes, 3);
        assert_eq!(r.min_width, 80);
        assert_eq!(r.split_ratio, 4);
        assert_eq!(r.diff_top_n, 3);
        assert_eq!(r.verify_min_sentence_words, 8);
        assert!(r.show_keybind_hints);
        assert!((r.session_budget_warn - 0.50).abs() < 1e-9);
        assert!((r.dedup_warn_score - 0.92).abs() < 1e-9);
        assert!(r.default_thread.is_none());
    }

    #[test]
    fn chorus_block_defaults_and_overrides() {
        // Absent → the CHORUS defaults.
        let cfg: Config = serde_hjson::from_str("{}").unwrap();
        assert_eq!(cfg.chorus.distinct_threshold, 0.5);
        assert!(cfg.chorus.distinct_ignore_pairs.is_empty());
        assert_eq!(cfg.chorus.register_drift_threshold, 0.08);
        // A partial block overrides only the named field.
        let cfg2: Config =
            serde_hjson::from_str("{ chorus: { distinct_ignore_pairs: [\"Mara|Joren\"] } }").unwrap();
        assert_eq!(cfg2.chorus.distinct_ignore_pairs, vec!["Mara|Joren".to_string()]);
        assert_eq!(cfg2.chorus.distinct_threshold, 0.5); // untouched default
        // The Inner Stylist block defaults + overrides.
        assert!(cfg.stylist.enabled);
        assert_eq!(cfg.stylist.session_budget, 0.15);
        let cfg3: Config = serde_hjson::from_str("{ stylist: { enabled: false } }").unwrap();
        assert!(!cfg3.stylist.enabled);
        assert_eq!(cfg3.stylist.session_budget, 0.15); // untouched default
    }

    #[test]
    fn continuity_block_defaults_and_overrides() {
        // Absent → the SENTINEL defaults: on, all detectors on, tolerance 0.
        let cfg: Config = serde_hjson::from_str("{}").unwrap();
        assert!(cfg.continuity.enabled);
        assert!(!cfg.continuity.ambient);
        assert_eq!(cfg.continuity.introduce_tolerance, 0);
        for key in ["co_location", "timeline", "numeric", "char_facts", "introduce"] {
            assert!(cfg.continuity.detector_enabled(key), "{key} on by default");
        }
        // A partial block overrides only the named fields; the rest stay default.
        let cfg2: Config = serde_hjson::from_str(
            "{ continuity: { numeric: false, introduce_tolerance: 2 } }",
        )
        .unwrap();
        assert!(!cfg2.continuity.detector_enabled("numeric"));
        assert!(cfg2.continuity.detector_enabled("co_location")); // untouched default
        assert_eq!(cfg2.continuity.introduce_tolerance, 2);
        assert!(cfg2.continuity.enabled); // untouched default
        // An unknown detector key is forward-compatibly enabled.
        assert!(cfg.continuity.detector_enabled("future_detector"));
    }

    #[test]
    fn lector_block_defaults_and_overrides() {
        let cfg: Config = serde_hjson::from_str("{}").unwrap();
        assert!(cfg.lector.enabled, "read-through on by default");
        let cfg2: Config = serde_hjson::from_str("{ lector: { enabled: false } }").unwrap();
        assert!(!cfg2.lector.enabled);
    }

    #[test]
    fn graph_block_defaults_and_overrides() {
        // Absent → the GRAPHMIND defaults.
        let cfg: Config = serde_hjson::from_str("{}").unwrap();
        assert_eq!(cfg.graph.ask_max_steps, 8);
        assert_eq!(cfg.graph.ask_search_width, 6);
        // A partial block overrides only the named field.
        let cfg2: Config = serde_hjson::from_str("{ graph: { ask_max_steps: 12 } }").unwrap();
        assert_eq!(cfg2.graph.ask_max_steps, 12);
        assert_eq!(cfg2.graph.ask_search_width, 6); // untouched default
    }

    #[test]
    fn missing_block_uses_defaults_and_overrides_apply() {
        // Absent → defaults.
        let cfg: Config = serde_hjson::from_str("{}").unwrap();
        assert_eq!(cfg.research.split_ratio, 4);
        // A partial block overrides only the named fields.
        let cfg2: Config = serde_hjson::from_str("{ research: { split_ratio: 5, diff_top_n: 7 } }").unwrap();
        assert_eq!(cfg2.research.split_ratio, 5);
        assert_eq!(cfg2.research.diff_top_n, 7);
        assert_eq!(cfg2.research.max_pinned_nodes, 3); // untouched default
    }
}

#[cfg(test)]
mod prop_tests {
    use super::Config;
    use proptest::prelude::*;

    proptest! {
        /// HJSON config parsing must return Ok|Err on arbitrary input — never panic.
        /// The layered loader feeds untrusted `inkhaven.hjson` / `~/.config` files
        /// through `serde_hjson::from_str`; a panic here would crash the editor at
        /// open. Bounded length keeps the proptest fast.
        #[test]
        fn config_hjson_parse_never_panics(s in ".{0,256}") {
            let _ = serde_hjson::from_str::<Config>(&s);
        }

        /// Even structurally-plausible HJSON (brace/quote soup) must not panic — only
        /// fail cleanly. Exercises the parser's recovery paths, not just the reject.
        #[test]
        fn config_bracey_hjson_never_panics(
            s in proptest::collection::vec(
                proptest::sample::select(vec!["{", "}", "[", "]", ":", ",", "\"", "a", "1", " ", "\n"]),
                0..64,
            ).prop_map(|v| v.concat())
        ) {
            let _ = serde_hjson::from_str::<Config>(&s);
        }
    }
}
