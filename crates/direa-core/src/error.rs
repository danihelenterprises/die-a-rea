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

    /// A refusal with a stable, machine-readable code (e.g. build safety guards).
    #[error("{message}")]
    Refused { code: &'static str, message: String },

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl DireaError {
    /// Stable, machine-readable error code.
    pub fn code(&self) -> &'static str {
        match self {
            DireaError::ManifestRead { .. } => "manifest_read",
            DireaError::ManifestParse(_) => "manifest_parse",
            DireaError::Serialize(_) => "serialize",
            DireaError::Invalid(_) => "invalid_config",
            DireaError::TargetMissing(_) => "target_missing",
            DireaError::NoAdapter(_) => "no_adapter",
            DireaError::AdapterFailed { .. } => "adapter_failed",
            DireaError::Refused { code, .. } => code,
            DireaError::Io(_) => "io",
        }
    }
}

pub type Result<T> = std::result::Result<T, DireaError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_stable() {
        assert_eq!(DireaError::Invalid("x".into()).code(), "invalid_config");
        assert_eq!(
            DireaError::Refused {
                code: "output_over_source",
                message: "x".into()
            }
            .code(),
            "output_over_source"
        );
    }
}
