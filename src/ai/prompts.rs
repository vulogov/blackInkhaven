use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prompt {
    pub name: String,
    pub description: String,
    pub template: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PromptLibrary {
    #[serde(default)]
    pub prompts: Vec<Prompt>,
}

impl PromptLibrary {
    pub fn load(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path).map_err(Error::Io)?;
        serde_hjson::from_str(&raw).map_err(|e| Error::Config(e.to_string()))
    }

    pub fn find(&self, name: &str) -> Option<&Prompt> {
        self.prompts.iter().find(|p| p.name == name)
    }
}
