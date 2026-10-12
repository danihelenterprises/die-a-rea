//! Adapter SDK.
//!
//! Adapters are the pluggable, language-specific halves of die-a-rea. Each
//! adapter is a subprocess that speaks a tiny JSON protocol over stdio:
//!
//! ```text
//! die-a-rea (host)  --JSON request-->  adapter process (stdin)
//! die-a-rea (host)  <--JSON response-- adapter process (stdout)
//! ```
//!
//! This keeps the core language-agnostic and license-clean: an adapter may
//! shell out to copyleft or proprietary toolchains without linking them.

use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::error::{DireaError, Result};
use crate::planner::TargetKind;
use crate::policy::ResolvedPolicy;

/// Operation requested from an adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AdapterOp {
    /// Report what the adapter can do for a target.
    Capabilities,
    /// Return the steps this adapter would take (no side effects).
    Plan,
    /// Apply the transformation. Must be idempotent-safe when possible.
    Apply,
    /// Inspect an artifact and return measurements for the resistance score.
    Verify,
}

/// Severity of a finding produced by an adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warn,
    Error,
}

/// A single, human-readable action an adapter plans to take.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub adapter: String,
    pub action: String,
    #[serde(default)]
    pub detail: String,
    /// True when the step shells out to an external binary.
    #[serde(default)]
    pub external: bool,
    /// True when the step modifies or replaces the artifact.
    #[serde(default)]
    pub destructive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub severity: Severity,
    pub code: String,
    pub message: String,
}

impl Finding {
    pub fn info(code: impl Into<String>, message: impl Into<String>) -> Self {
        Finding {
            severity: Severity::Info,
            code: code.into(),
            message: message.into(),
        }
    }
    pub fn warn(code: impl Into<String>, message: impl Into<String>) -> Self {
        Finding {
            severity: Severity::Warn,
            code: code.into(),
            message: message.into(),
        }
    }
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Finding {
            severity: Severity::Error,
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ok,
    Skipped,
    Unavailable,
    Error,
}

/// Request sent to an adapter subprocess.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterRequest {
    /// Protocol version so adapters can negotiate forward-compatibly.
    #[serde(default = "proto_version")]
    pub protocol: u32,
    pub op: AdapterOp,
    pub target: PathBuf,
    pub kind: TargetKind,
    pub policy: ResolvedPolicy,
    #[serde(default)]
    pub options: serde_json::Value,
}

pub fn proto_version() -> u32 {
    1
}

/// Response returned by an adapter subprocess.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterResponse {
    pub adapter: String,
    pub status: Status,
    #[serde(default)]
    pub steps: Vec<PlanStep>,
    #[serde(default)]
    pub findings: Vec<Finding>,
    /// Adapter-specific measurements, consumed by the resistance scorer.
    #[serde(default)]
    pub metrics: serde_json::Value,
    #[serde(default)]
    pub message: String,
}

/// A registry entry describing how to invoke an adapter.
#[derive(Debug, Clone)]
pub struct AdapterEntry {
    pub id: String,
    pub command: String,
    pub args: Vec<String>,
    pub kinds: Vec<TargetKind>,
    /// Built-in steps are implemented by the core `native-post` pipeline and
    /// have no external process; they are always "available".
    pub builtin: bool,
}

impl AdapterEntry {
    /// Whether the adapter's command is resolvable on `PATH`.
    pub fn available(&self) -> bool {
        if self.builtin {
            return true;
        }
        which(&self.command).is_some()
    }
}

/// Environment override for an adapter's executable, e.g. `js` -> `DIREA_JS_ADAPTER`.
pub fn env_key(id: &str) -> String {
    format!(
        "DIREA_{}_ADAPTER",
        id.to_ascii_uppercase().replace('-', "_")
    )
}

/// Resolve an executable on `PATH` without pulling in an extra dependency.
pub fn which(cmd: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(cmd);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        for ext in ["exe", "cmd", "bat"] {
            let candidate = dir.join(format!("{cmd}.{ext}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Runs an adapter as a subprocess using the JSON-over-stdio protocol.
pub struct SubprocessAdapter {
    pub entry: AdapterEntry,
}

impl SubprocessAdapter {
    pub fn new(entry: AdapterEntry) -> Self {
        SubprocessAdapter { entry }
    }

    pub fn run(&self, request: &AdapterRequest) -> Result<AdapterResponse> {
        let mut cmd = Command::new(&self.entry.command);
        cmd.args(&self.entry.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| DireaError::AdapterFailed {
            adapter: self.entry.id.clone(),
            message: e.to_string(),
        })?;

        {
            use std::io::Write;
            let stdin = child
                .stdin
                .as_mut()
                .ok_or_else(|| DireaError::AdapterFailed {
                    adapter: self.entry.id.clone(),
                    message: "adapter stdin unavailable".into(),
                })?;
            let payload = serde_json::to_vec(request)?;
            stdin.write_all(&payload)?;
            stdin.write_all(b"\n")?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| DireaError::AdapterFailed {
                adapter: self.entry.id.clone(),
                message: e.to_string(),
            })?;

        if !output.status.success() {
            return Err(DireaError::AdapterFailed {
                adapter: self.entry.id.clone(),
                message: format!(
                    "exited with {}: {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            });
        }

        let response: AdapterResponse =
            serde_json::from_slice(&output.stdout).map_err(|e| DireaError::AdapterFailed {
                adapter: self.entry.id.clone(),
                message: format!("invalid JSON response: {e}"),
            })?;
        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_entries_are_always_available() {
        let entry = AdapterEntry {
            id: "native-post".into(),
            command: "native-post".into(),
            args: vec![],
            kinds: vec![TargetKind::NativeElf],
            builtin: true,
        };
        assert!(entry.available());
    }

    #[test]
    fn env_keys_are_derived() {
        assert_eq!(env_key("js"), "DIREA_JS_ADAPTER");
        assert_eq!(env_key("jvm-android"), "DIREA_JVM_ANDROID_ADAPTER");
        assert_eq!(env_key("llvm"), "DIREA_LLVM_ADAPTER");
    }

    #[test]
    fn request_round_trips() {
        let req = AdapterRequest {
            protocol: proto_version(),
            op: AdapterOp::Plan,
            target: PathBuf::from("dist/app"),
            kind: TargetKind::JavaScript,
            policy: ResolvedPolicy::default(),
            options: serde_json::json!({}),
        };
        let s = serde_json::to_string(&req).unwrap();
        let back: AdapterRequest = serde_json::from_str(&s).unwrap();
        assert_eq!(back.op, AdapterOp::Plan);
    }
}
