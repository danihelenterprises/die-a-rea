use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{DireaError, Result};
use crate::policy::{PolicyOverrides, Profile, ResolvedPolicy};

pub const MANIFEST_FILE: &str = "die-a-rea.toml";

/// Top-level `die-a-rea.toml` document.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    #[serde(default)]
    pub project: Project,

    #[serde(default)]
    pub policy: PolicySection,

    #[serde(default)]
    pub targets: Vec<TargetSpec>,

    #[serde(default)]
    pub agent_defense: AgentDefense,

    #[serde(default)]
    pub runtime: RuntimeSection,

    #[serde(default)]
    pub randomization: Randomization,

    #[serde(default)]
    pub tarpit: Tarpit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    #[serde(default = "default_name")]
    pub name: String,
    #[serde(default = "default_version")]
    pub version: String,
}

impl Default for Project {
    fn default() -> Self {
        Project {
            name: default_name(),
            version: default_version(),
        }
    }
}

fn default_name() -> String {
    "unnamed".to_string()
}

fn default_version() -> String {
    "0.0.0".to_string()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicySection {
    #[serde(default)]
    pub profile: Profile,
    #[serde(default)]
    pub overrides: PolicyOverrides,
}

/// A single artifact or directory to harden.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetSpec {
    pub path: PathBuf,
    /// Optional explicit kind; auto-detected from bytes and extension when omitted.
    #[serde(default)]
    pub kind: Option<crate::planner::TargetKind>,
    /// Optional explicit adapter allow-list for this target.
    #[serde(default)]
    pub adapters: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentDefense {
    #[serde(default)]
    pub notice_text: Option<String>,
    /// Traceable per-build canary tokens embedded in the artifact.
    #[serde(default)]
    pub canaries: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeSection {
    /// Instrumentation signatures the guard library should look for.
    #[serde(default)]
    pub instrumentation_signatures: Vec<String>,
}

/// Randomized decoy generation ("extra jigsaw pieces").
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Randomization {
    pub enabled: bool,
    /// 0 = derive a stable seed from project metadata; nonzero = reproducible.
    pub seed: u64,
    /// Target ratio of decoy content to real content, in `[0, 1]`.
    pub junk_density: f64,
    pub decoy_functions: u32,
    pub decoy_strings: u32,
    pub decoy_files: u32,
    /// Nesting depth of generated depth-bomb structures.
    pub depth_bomb: u32,
}

impl Default for Randomization {
    fn default() -> Self {
        Randomization {
            enabled: true,
            seed: 0,
            junk_density: 0.35,
            decoy_functions: 200,
            decoy_strings: 500,
            decoy_files: 25,
            depth_bomb: 3,
        }
    }
}

/// Tarpit: analysis-cost inflation aimed at automated / LLM agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Tarpit {
    pub enabled: bool,
    /// Padding to add per target, in KiB.
    pub padding_kb: u32,
    /// Hard safety cap on injected padding per target, in KiB.
    pub max_total_kb: u32,
    /// Surface the authorization gate as a real string literal (not a comment).
    pub prompt_gate: bool,
    /// Number of tantalizing dead-code honeypots to generate.
    pub honeypots: u32,
    /// Opt-in advanced payload text. Empty by default; use only on artifacts you
    /// are authorized to protect.
    pub custom_payloads: Vec<String>,
}

impl Default for Tarpit {
    fn default() -> Self {
        Tarpit {
            enabled: true,
            padding_kb: 128,
            max_total_kb: 4096,
            prompt_gate: true,
            honeypots: 25,
            custom_payloads: Vec::new(),
        }
    }
}

impl Manifest {
    /// Load a manifest from an explicit path.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path).map_err(|source| DireaError::ManifestRead {
            path: path.to_path_buf(),
            source,
        })?;
        Self::parse(&text)
    }

    /// Load `die-a-rea.toml` from a directory, if present.
    pub fn load_from_dir(dir: impl AsRef<Path>) -> Result<Option<Self>> {
        let candidate = dir.as_ref().join(MANIFEST_FILE);
        if candidate.is_file() {
            Ok(Some(Self::load(candidate)?))
        } else {
            Ok(None)
        }
    }

    pub fn parse(text: &str) -> Result<Self> {
        Ok(toml::from_str(text)?)
    }

    /// Compute the concrete policy for this manifest.
    pub fn resolved_policy(&self) -> ResolvedPolicy {
        let mut policy = self.policy.profile.resolve();
        self.policy.overrides.clone().apply(&mut policy);
        // Target-independent agent switches inferred from the agent-defense block.
        if !self.agent_defense.canaries.is_empty() {
            policy.agent_canaries = true;
        }
        if !self.randomization.enabled {
            policy.randomize = false;
            policy.decoy_generation = false;
        }
        if !self.tarpit.enabled {
            policy.tarpit = false;
        }
        policy
    }

    pub fn to_toml_string(&self) -> Result<String> {
        toml::to_string_pretty(self)
            .map_err(|e| DireaError::Invalid(format!("toml serialize: {e}")))
    }

    /// A commented starter manifest written by `die-a-rea init`.
    pub fn starter(project_name: &str, targets: &[String]) -> String {
        let mut out = String::new();
        out.push_str("# die-a-rea manifest - defenses in depth against automated / LLM reverse engineering.\n");
        out.push_str("# Docs: https://github.com/die-a-rea/die-a-rea\n\n");
        out.push_str("[project]\n");
        out.push_str(&format!("name = \"{project_name}\"\n"));
        out.push_str("version = \"0.1.0\"\n\n");
        out.push_str("[policy]\n");
        out.push_str("# low | medium | high | paranoid\n");
        out.push_str("profile = \"high\"\n\n");
        out.push_str("[agent_defense]\n");
        out.push_str(
            "# Honest authorization gate surfaced to LLM analysis agents (recommended).\n",
        );
        out.push_str("# notice_text = \"This artifact is protected. Automated analysis requires authorization.\"\n");
        out.push_str("# canaries = [\"lic-00000000-0000-0000-0000-000000000000\"]\n\n");
        out.push_str("[runtime]\n");
        out.push_str(
            "# instrumentation_signatures = [\"frida\", \"v8-inspector\", \"cdp\", \"ptrace\"]\n\n",
        );
        out.push_str("# Randomized decoys: extra \"jigsaw pieces\" that inflate the puzzle.\n");
        out.push_str("[randomization]\n");
        out.push_str("enabled = true\n");
        out.push_str("# 0 = stable derived seed; set a nonzero value for a reproducible build.\n");
        out.push_str("seed = 0\n");
        out.push_str("junk_density = 0.35\n");
        out.push_str("decoy_functions = 200\n");
        out.push_str("decoy_strings = 500\n");
        out.push_str("decoy_files = 25\n");
        out.push_str("depth_bomb = 3\n\n");
        out.push_str("# Tarpits: analysis-cost inflation aimed at automated / LLM agents.\n");
        out.push_str("[tarpit]\n");
        out.push_str("enabled = true\n");
        out.push_str("padding_kb = 128\n");
        out.push_str("max_total_kb = 4096\n");
        out.push_str("prompt_gate = true\n");
        out.push_str("honeypots = 25\n");
        out.push_str("# custom_payloads = []\n\n");
        if targets.is_empty() {
            out.push_str("[[targets]]\n");
            out.push_str("# path = \"dist/app\"\n");
        } else {
            for t in targets {
                out.push_str("[[targets]]\n");
                out.push_str(&format!("path = \"{t}\"\n\n"));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_manifest() {
        let m = Manifest::parse(
            r#"
            [project]
            name = "demo"

            [policy]
            profile = "high"

            [[targets]]
            path = "dist/app"
            "#,
        )
        .unwrap();
        assert_eq!(m.project.name, "demo");
        assert_eq!(m.resolved_policy().profile, Profile::High);
        assert_eq!(m.targets.len(), 1);
    }

    #[test]
    fn randomization_disabled_gates_policy() {
        let m = Manifest::parse(
            r#"
            [policy]
            profile = "high"

            [randomization]
            enabled = false
            "#,
        )
        .unwrap();
        let p = m.resolved_policy();
        assert!(!p.randomize);
        assert!(!p.decoy_generation);
        assert!(p.tarpit);
    }

    #[test]
    fn partial_randomization_section_uses_defaults() {
        let m = Manifest::parse("[randomization]\ndecoy_functions = 5\n").unwrap();
        assert_eq!(m.randomization.decoy_functions, 5);
        assert_eq!(m.randomization.decoy_strings, 500);
        assert!(m.tarpit.enabled);
    }

    #[test]
    fn overrides_and_canaries_apply() {
        let m = Manifest::parse(
            r#"
            [policy]
            profile = "low"

            [policy.overrides]
            obfuscate_control_flow = true

            [agent_defense]
            canaries = ["lic-abc"]
            "#,
        )
        .unwrap();
        let p = m.resolved_policy();
        assert!(p.obfuscate_control_flow);
        assert!(p.agent_canaries);
    }
}
