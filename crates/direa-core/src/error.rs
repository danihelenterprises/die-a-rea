use std::path::PathBuf;

use thiserror::Error;

/// Errors raised by die-a-rea core.
#[derive(Debug, Error)]
pub enum DireaError {
    #[error("failed to read manifest at {path}: {source}")]
    ManifestRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse manifest: {0}")]
    ManifestParse(#[from] toml::de::Error),

    #[error("failed to serialize value: {0}")]
    Serialize(#[from] serde_json::Error),

    #[error("invalid configuration: {0}")]
    Invalid(String),

    #[error("target not found: {0}")]
    TargetMissing(PathBuf),

    #[error("no adapter registered for target kind `{0}`")]
    NoAdapter(String),

    #[error("adapter `{adapter}` failed: {message}")]
    AdapterFailed { adapter: String, message: String },

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, DireaError>;
