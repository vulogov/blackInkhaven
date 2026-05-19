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
            prompts_file: default_prompts_path(),
            sync_interval_seconds: default_sync_interval(),
        }
    }
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
                api_key_env: "GEMINI_API_KEY".into(),
            },
        );
        providers.insert(
            "deepseek".into(),
            LlmProvider {
                model: "deepseek-chat".into(),
                api_key_env: "DEEPSEEK_API_KEY".into(),
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
    pub api_key_env: String,
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
