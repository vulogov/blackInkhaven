use std::path::Path;

use tracing::info;

use crate::config::{Config, DEFAULT_PROJECT_CONFIG, DEFAULT_PROMPTS};
use crate::error::{Error, Result};
use crate::project::{PROMPTS_FILE_DEFAULT, ProjectLayout};
use crate::store::Store;

pub fn run(path: &Path, force: bool) -> Result<()> {
    let layout = ProjectLayout::new(path);

    if layout.is_initialized() && !force {
        return Err(Error::ProjectExists(layout.root.clone()));
    }

    layout.create_layout()?;

    let config_path = layout.config_path();
    if !config_path.exists() || force {
        std::fs::write(&config_path, DEFAULT_PROJECT_CONFIG)?;
        info!(path = %config_path.display(), "wrote project config");
    }

    let prompts_path = layout.root.join(PROMPTS_FILE_DEFAULT);
    if !prompts_path.exists() || force {
        std::fs::write(&prompts_path, DEFAULT_PROMPTS)?;
        info!(path = %prompts_path.display(), "wrote prompt library");
    }

    // Round-trip parse the config to validate it.
    let cfg = Config::load(&config_path)?;

    // Open the document store. This creates `metadata.db` + `vecstore/` if
    // they don't yet exist. We immediately drop it; the embedding model
    // download (if first run) happens here.
    let _store = Store::open(layout.clone(), &cfg)?;

    eprintln!("Initialized inkhaven project at {}", layout.root.display());
    eprintln!("  config:    {}", layout.config_path().display());
    eprintln!("  prompts:   {}", layout.root.join(PROMPTS_FILE_DEFAULT).display());
    eprintln!("  store db:  {}", layout.metadata_db_path().display());
    eprintln!("  vecstore:  {}", layout.vecstore_path().display());
    eprintln!("  books:     {}", layout.books_path().display());
    Ok(())
}
