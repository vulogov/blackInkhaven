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
    #[serde(default = "default_prompts_path")]
    pub prompts_file: PathBuf,
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
            prompts_file: default_prompts_path(),
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
            out_dir: "backups".into(),
            // Roughly a week. Vladimir's books move fast enough that a
            // weekly snapshot pairs sensibly with the per-paragraph
            // snapshots the editor already supports.
            max_age: std::time::Duration::from_secs(7 * 24 * 3600),
        }
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
