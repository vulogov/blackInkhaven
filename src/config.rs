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
    /// Seconds between background calls to `Store::sync()` (flushes HNSW
    /// index + DuckDB checkpoint). 0 disables the background sync; explicit
    /// sync-on-save still fires.
    #[serde(default = "default_sync_interval")]
    pub sync_interval_seconds: u64,
}

fn default_sync_interval() -> u64 {
    60
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
}

impl Default for TypstTemplatesConfig {
    fn default() -> Self {
        Self {
            wrap_book: default_wrap_book().into(),
            wrap_chapter: default_wrap_chapter().into(),
            wrap_subchapter: default_wrap_subchapter().into(),
            wrap_paragraph: default_wrap_paragraph().into(),
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

    /// Concatenated body for the per-book `globals.typ` paragraph: the
    /// editor-chrome heading line, a brief comment header explaining
    /// the four functions, then each wrap function in order.
    pub fn globals_typ_body(&self) -> String {
        let mut out = String::new();
        out.push_str("= globals.typ\n\n");
        out.push_str(
            "// Wrap functions used by inkhaven's `Book assembly` (Ctrl+B A).\n\
             // Each level of the manuscript tree is fed through the matching\n\
             // wrap_* call when the assembler synthesises index.typ files.\n\
             // Customise to taste — page breaks, headings, fonts, etc.\n\n",
        );
        out.push_str(&self.resolved_wrap_book());
        out.push('\n');
        out.push_str(&self.resolved_wrap_chapter());
        out.push('\n');
        out.push_str(&self.resolved_wrap_subchapter());
        out.push('\n');
        out.push_str(&self.resolved_wrap_paragraph());
        out
    }
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

            search_match_bg: "#f38ba8".into(),
            search_current_bg: "#f5c2e7".into(),

            tree_open_marker: "#a6e3a1".into(),
            tree_book_fg: "#f5c2e7".into(),       // pink — books pop at the top
            tree_chapter_fg: "#89b4fa".into(),    // blue — chapter rhythm
            tree_subchapter_fg: "#94e2d5".into(), // teal — subchapter
            tree_paragraph_fg: "#cdd6f4".into(),  // base text — keep prose calm

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
        providers.insert(
            "gemini".into(),
            LlmProvider {
                model: "gemini-2.5-pro".into(),
                api_key_env: Some("GEMINI_API_KEY".into()),
            },
        );
        providers.insert(
            "deepseek".into(),
            LlmProvider {
                model: "deepseek-chat".into(),
                api_key_env: Some("DEEPSEEK_API_KEY".into()),
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
    /// Snowball stemmer languages used to expand the Places/Characters
    /// highlight overlay so morphological variants light up too — e.g.
    /// "Москва" also matches "Москве", "Москвою". Each entry is one of the
    /// names accepted by `rust-stemmers::Algorithm` (lowercased), see
    /// `parse_stemmer_language` for the supported set.
    pub stemming: StemmingConfig,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            theme: "default".into(),
            tab_width: 2,
            wrap: true,
            autosave_seconds: 5,
            stemming: StemmingConfig::default(),
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
