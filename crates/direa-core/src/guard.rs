//! Runtime instrumentation detection.
//!
//! This is the reusable core of the die-a-rea guard: it answers "is this process
//! being debugged or instrumented right now?" using cheap, portable signals. It
//! is used by `direa-verify` (behind an explicit opt-in) to populate the
//! `runtime-instrumentation-exposure` axis, and can be embedded in a protected
//! program to gate behaviour.
//!
//! It never attaches to or modifies another process; it only inspects this
//! process's own environment (consistent with ADR-0001).

use serde::{Deserialize, Serialize};

/// Result of a runtime instrumentation check.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GuardReport {
    pub instrumented: bool,
    pub findings: Vec<String>,
}

impl GuardReport {
    /// True when no instrumentation indicators were found.
    pub fn clean(&self) -> bool {
        !self.instrumented
    }
}

/// Signatures looked for in process maps and injected-library environment
/// variables. Kept in sync with `[runtime] instrumentation_signatures`.
pub const DEFAULT_SIGNATURES: &[&str] = &[
    "frida",
    "frida-agent",
    "frida-gadget",
    "gum-js-loop",
    "gum-js",
    "libfrida",
    "gadget",
    "cycript",
    "substrate",
    "xposed",
    "magisk",
    "lldb",
    "gdb",
];

/// Environment variables whose values are scanned for injected instrumentation.
const INTERESTING_ENV: &[&str] = &[
    "LD_PRELOAD",
    "DYLD_INSERT_LIBRARIES",
    "FRIDA",
    "NODE_OPTIONS",
];

/// Inspect the current process for instrumentation.
pub fn detect() -> GuardReport {
    let status = std::fs::read_to_string("/proc/self/status").ok();
    let maps = std::fs::read_to_string("/proc/self/maps").ok();
    let env: Vec<(String, String)> = std::env::vars().collect();
    classify(status.as_deref(), maps.as_deref(), &env, DEFAULT_SIGNATURES)
}

/// Pure classifier over already-collected signals (unit-testable).
pub fn classify(
    status: Option<&str>,
    maps: Option<&str>,
    env: &[(String, String)],
    signatures: &[&str],
) -> GuardReport {
    let mut findings = Vec::new();

    // Linux: a non-zero TracerPid means something is ptrace-attached.
    if let Some(status) = status {
        for line in status.lines() {
            if let Some(rest) = line.strip_prefix("TracerPid:") {
                let pid = rest.trim();
                if !pid.is_empty() && pid != "0" {
                    findings.push(format!("TracerPid={pid}"));
                }
            }
        }
    }

    // Loaded modules (Frida agent/gadget, debuggers, hooking frameworks).
    if let Some(maps) = maps {
        let lower = maps.to_ascii_lowercase();
        for sig in signatures {
            let needle = sig.to_ascii_lowercase();
            if lower.contains(&needle) {
                findings.push(format!("map:{sig}"));
            }
        }
    }

    // Injected-library / inspector environment variables.
    for (key, value) in env {
        if !INTERESTING_ENV.contains(&key.as_str()) {
            continue;
        }
        let lower = value.to_ascii_lowercase();
        if *key == "NODE_OPTIONS" && lower.contains("--inspect") {
            findings.push("NODE_OPTIONS:--inspect".to_string());
            continue;
        }
        if *key == "FRIDA"
            || signatures
                .iter()
                .any(|s| lower.contains(&s.to_ascii_lowercase()))
        {
            findings.push(format!("env:{key}"));
        }
    }

    findings.sort();
    findings.dedup();
    GuardReport {
        instrumented: !findings.is_empty(),
        findings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_tracer() {
        let status = "Name:\tapp\nTracerPid:\t4242\nUid:\t1000\n";
        let r = classify(Some(status), None, &[], DEFAULT_SIGNATURES);
        assert!(r.instrumented);
        assert!(r.findings.iter().any(|f| f.contains("TracerPid")));
    }

    #[test]
    fn detects_frida_in_maps() {
        let maps = "7f0000-7f1000 r-xp 00000000 00:00 0 /usr/lib/libfrida-agent.so\n";
        let r = classify(None, Some(maps), &[], DEFAULT_SIGNATURES);
        assert!(r.instrumented);
    }

    #[test]
    fn detects_injected_library_env() {
        let env = vec![(
            "LD_PRELOAD".to_string(),
            "/opt/frida/frida-gadget.so".to_string(),
        )];
        let r = classify(None, None, &env, DEFAULT_SIGNATURES);
        assert!(r.instrumented);
    }

    #[test]
    fn detects_node_inspector() {
        let env = vec![("NODE_OPTIONS".to_string(), "--inspect=9229".to_string())];
        let r = classify(None, None, &env, DEFAULT_SIGNATURES);
        assert!(r.instrumented);
    }

    #[test]
    fn clean_environment_is_reported_clean() {
        let status = "Name:\tapp\nTracerPid:\t0\n";
        let maps = "7f0000-7f1000 r-xp 00000000 00:00 0 /usr/lib/libc.so.6\n";
        let env = vec![("PATH".to_string(), "/usr/bin".to_string())];
        let r = classify(Some(status), Some(maps), &env, DEFAULT_SIGNATURES);
        assert!(!r.instrumented);
        assert!(r.clean());
        assert!(r.findings.is_empty());
    }

    #[test]
    fn unrelated_env_is_not_flagged() {
        let env = vec![("PATH".to_string(), "/usr/bin:/home/frida".to_string())];
        let r = classify(None, None, &env, DEFAULT_SIGNATURES);
        assert!(!r.instrumented, "PATH must not be scanned");
    }
}
