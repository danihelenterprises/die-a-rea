//! Build metadata sidecar.
//!
//! `die-a-rea build` writes a `die-a-rea.build.json` next to the hardened
//! artifacts recording exactly what randomization and tarpit content was
//! injected. `direa-verify` reads it so that decoy content is not mistaken for
//! recovered real content, and so that analysis-noise can be scored.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{DireaError, Result};

pub const BUILD_METADATA_FILE: &str = "die-a-rea.build.json";

/// What was injected into a single hardened target.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InjectedStats {
    pub decoy_functions: u64,
    pub decoy_strings: u64,
    pub decoy_files: u64,
    /// Printable sequences (>= 6 chars) contributed by injected content, so the
    /// string probe can discount decoys instead of counting them as recovered.
    pub injected_string_runs: u64,
    /// Bytes of depth-bomb / nested decoy structure (approximate source bytes).
    pub depth_bytes: u64,
    /// Bytes of pure tarpit padding (comments, token bombs).
    pub padding_bytes: u64,
    /// Signed total of all injected bytes, for the analysis-noise score.
    pub injected_bytes: u64,
}

/// Per-target build record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetMetadata {
    pub target: String,
    pub output: String,
    pub total_bytes: u64,
    #[serde(default)]
    pub injected: InjectedStats,
}

impl TargetMetadata {
    /// Fraction of the artifact that is injected noise, in `[0, 1]`.
    pub fn noise_fraction(&self) -> f64 {
        if self.total_bytes == 0 {
            0.0
        } else {
            (self.injected.injected_bytes as f64 / self.total_bytes as f64).clamp(0.0, 1.0)
        }
    }
}

/// Whole-build metadata for every target.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BuildMetadata {
    pub seed: u64,
    #[serde(default)]
    pub targets: Vec<TargetMetadata>,
}

impl BuildMetadata {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path).map_err(|source| DireaError::ManifestRead {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(serde_json::from_str(&text)?)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(path, text)?;
        Ok(())
    }

    /// Find the record for a hardened artifact path.
    pub fn find(&self, output: &Path) -> Option<&TargetMetadata> {
        if let Some(exact) = self.targets.iter().find(|t| Path::new(&t.output) == output) {
            return Some(exact);
        }
        let name = output.file_name()?;
        self.targets
            .iter()
            .find(|t| Path::new(&t.output).file_name() == Some(name))
    }

    /// Conventional metadata path for a hardened artifact.
    pub fn sidecar_for(hardened: &Path) -> PathBuf {
        hardened
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(|p| p.join(BUILD_METADATA_FILE))
            .unwrap_or_else(|| PathBuf::from(BUILD_METADATA_FILE))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_fraction_is_clamped() {
        let m = TargetMetadata {
            target: "a".into(),
            output: "a".into(),
            total_bytes: 100,
            injected: InjectedStats {
                injected_bytes: 40,
                ..Default::default()
            },
        };
        assert!((m.noise_fraction() - 0.4).abs() < 1e-9);
    }

    #[test]
    fn find_by_name() {
        let meta = BuildMetadata {
            seed: 1,
            targets: vec![TargetMetadata {
                target: "src".into(),
                output: "dist/app.js".into(),
                total_bytes: 10,
                injected: InjectedStats::default(),
            }],
        };
        assert!(meta.find(Path::new("dist/app.js")).is_some());
        assert!(meta.find(Path::new("/somewhere/app.js")).is_some());
    }
}
