use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("project root not found at {0}; run `inkhaven init <path>` first")]
    ProjectNotFound(PathBuf),

    #[error("project already exists at {0}")]
    ProjectExists(PathBuf),

    #[error("config error: {0}")]
    Config(String),

    #[error("store error: {0}")]
    Store(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
