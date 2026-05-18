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
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
pub struct EditorConfig {
    pub theme: String,
    pub tab_width: usize,
    pub wrap: bool,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            theme: "default".into(),
            tab_width: 2,
            wrap: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBindings {
    pub save: String,
    pub search: String,
    pub ai_prompt: String,
    pub add_book: String,
    pub add_chapter: String,
    pub add_subchapter: String,
    pub add_paragraph: String,
    pub delete_node: String,
    pub next_pane: String,
    pub prev_pane: String,
    pub page_up: String,
    pub page_down: String,
    pub move_up: String,
    pub move_down: String,
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            save: "Ctrl+s".into(),
            search: "Ctrl+/".into(),
            ai_prompt: "Ctrl+i".into(),
            add_book: "Ctrl+Shift+b".into(),
            add_chapter: "Ctrl+Shift+c".into(),
            add_subchapter: "Ctrl+Shift+s".into(),
            add_paragraph: "Ctrl+Shift+p".into(),
            delete_node: "Ctrl+Shift+d".into(),
            next_pane: "Tab".into(),
            prev_pane: "Shift+Tab".into(),
            page_up: "PageUp".into(),
            page_down: "PageDown".into(),
            move_up: "Ctrl+Shift+Up".into(),
            move_down: "Ctrl+Shift+Down".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
