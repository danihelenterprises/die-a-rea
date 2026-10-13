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

/// A behavior-safe runtime guard for JavaScript/Electron artifacts.
///
/// Appended by `build` when `policy.anti_instrumentation` is set. It never
/// throws, blocks, or changes application logic: it records a result on
/// `globalThis.__DIREA_GUARD__` and calls `globalThis.__DIREA_ON_INSTRUMENTED__`
/// if the operator defined it.
pub const JS_GUARD_SOURCE: &str = r#"/* die-a-rea runtime guard */
(function () {
  try {
    var findings = [];
    var env = (typeof process !== 'undefined' && process.env) || {};
    if (env.NODE_OPTIONS && String(env.NODE_OPTIONS).indexOf('--inspect') >= 0) {
      findings.push('NODE_OPTIONS:--inspect');
    }
    if (env.FRIDA) {
      findings.push('env:FRIDA');
    }
    if (typeof globalThis !== 'undefined' && globalThis.__REACT_DEVTOOLS_GLOBAL_HOOK__) {
      findings.push('devtools');
    }
    globalThis.__DIREA_GUARD__ = { instrumented: findings.length > 0, findings: findings };
    if (findings.length > 0 && typeof globalThis.__DIREA_ON_INSTRUMENTED__ === 'function') {
      globalThis.__DIREA_ON_INSTRUMENTED__(findings);
    }
  } catch (_) {}
})();
"#;

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

/// Inspect the current process for instrumentation using the default signatures.
pub fn detect() -> GuardReport {
    detect_with(&[])
}

/// Inspect the current process using caller-supplied signatures (e.g. from a
/// manifest's `[runtime] instrumentation_signatures`). When the list is empty,
/// the built-in defaults are used.
pub fn detect_with(signatures: &[String]) -> GuardReport {
    let owned: Vec<String>;
    let refs: Vec<&str> = if signatures.is_empty() {
        DEFAULT_SIGNATURES.to_vec()
    } else {
        owned = signatures.to_vec();
        owned.iter().map(|s| s.as_str()).collect()
    };
    let mut findings = Vec::new();
    platform_findings(&mut findings, &refs);
    env_findings(&mut findings, &refs);
    findings.sort();
    findings.dedup();
    GuardReport {
        instrumented: !findings.is_empty(),
        findings,
    }
}

/// Platform-specific debugger/tracer detection.
fn platform_findings(findings: &mut Vec<String>, signatures: &[&str]) {
    #[cfg(target_os = "linux")]
    {
        if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
            for line in status.lines() {
                if let Some(rest) = line.strip_prefix("TracerPid:") {
                    let pid = rest.trim();
                    if !pid.is_empty() && pid != "0" {
                        findings.push(format!("TracerPid={pid}"));
                    }
                }
            }
        }
        if let Ok(maps) = std::fs::read_to_string("/proc/self/maps") {
            let lower = maps.to_ascii_lowercase();
            for sig in signatures {
                if lower.contains(&sig.to_ascii_lowercase()) {
                    findings.push(format!("map:{sig}"));
                }
            }
        }
    }

    #[cfg(windows)]
    {
        use core::ffi::c_void;
        extern "system" {
            fn IsDebuggerPresent() -> i32;
            fn GetCurrentProcess() -> *mut c_void;
            fn CheckRemoteDebuggerPresent(handle: *mut c_void, present: *mut i32) -> i32;
        }
        unsafe {
            if IsDebuggerPresent() != 0 {
                findings.push("IsDebuggerPresent".to_string());
            }
            let mut remote = 0i32;
            if CheckRemoteDebuggerPresent(GetCurrentProcess(), &mut remote) != 0 && remote != 0 {
                findings.push("remote-debugger".to_string());
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        // sysctl(KERN_PROC_PID) -> kinfo_proc.kp_proc.p_flag & P_TRACED.
        extern "C" {
            fn sysctl(
                name: *mut i32,
                namelen: u32,
                oldp: *mut u8,
                oldlenp: *mut usize,
                newp: *mut u8,
                newlen: usize,
            ) -> i32;
        }
        let mut mib = [1i32, 14, 1, std::process::id() as i32];
        let mut buf = [0u8; 648];
        let mut len = buf.len();
        let rc = unsafe {
            sysctl(
                mib.as_mut_ptr(),
                4,
                buf.as_mut_ptr(),
                &mut len,
                std::ptr::null_mut(),
                0,
            )
        };
        if rc == 0 && len >= 36 {
            let flag = u32::from_ne_bytes([buf[32], buf[33], buf[34], buf[35]]);
            if flag & 0x0000_0800 != 0 {
                findings.push("P_TRACED".to_string());
            }
        }
    }

    #[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
    {
        let _ = (findings, signatures);
    }
}

/// Injected-library / inspector environment variables.
fn env_findings(findings: &mut Vec<String>, signatures: &[&str]) {
    for (key, value) in std::env::vars() {
        if !INTERESTING_ENV.contains(&key.as_str()) {
            continue;
        }
        let lower = value.to_ascii_lowercase();
        if key == "NODE_OPTIONS" && lower.contains("--inspect") {
            findings.push("NODE_OPTIONS:--inspect".to_string());
            continue;
        }
        if key == "FRIDA"
            || signatures
                .iter()
                .any(|s| lower.contains(&s.to_ascii_lowercase()))
        {
            findings.push(format!("env:{key}"));
        }
    }
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
    fn js_guard_source_is_well_formed() {
        assert!(JS_GUARD_SOURCE.contains("__DIREA_GUARD__"));
        assert!(JS_GUARD_SOURCE.contains("__DIREA_ON_INSTRUMENTED__"));
        assert!(JS_GUARD_SOURCE.contains("--inspect"));
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
    fn custom_signature_is_honored() {
        let maps = "7f0000-7f1000 r-xp 00000000 00:00 0 /opt/my-hook.so\n";
        let r = classify(None, Some(maps), &[], &["my-hook"]);
        assert!(r.instrumented);
        // A signature not in the list is ignored.
        let r2 = classify(None, Some(maps), &[], &["frida"]);
        assert!(!r2.instrumented);
    }

    #[test]
    fn native_guard_compiles_and_runs() {
        if crate::adapter::which("cc").is_none() {
            return; // no C compiler available; skip
        }
        let manifest = env!("CARGO_MANIFEST_DIR");
        let src = format!("{manifest}/../../runtime/guard/direa_guard.c");
        let inc = format!("{manifest}/../../runtime/guard");
        let dir = std::env::temp_dir().join(format!("direa-cguard-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let main_c = dir.join("main.c");
        std::fs::write(
            &main_c,
            "#include <stdio.h>\n#include \"direa_guard.h\"\n\
             int main(void){struct direa_guard_result r; \
             int v=direa_guard_check(&r); \
             printf(\"instrumented=%d findings=%d\\n\", v, r.finding_count); return 0;}\n",
        )
        .unwrap();
        let bin = dir.join("guardtest");
        let status = std::process::Command::new("cc")
            .arg("-I")
            .arg(&inc)
            .arg(&src)
            .arg(&main_c)
            .arg("-o")
            .arg(&bin)
            .status()
            .unwrap();
        assert!(status.success(), "cc failed to compile the native guard");
        let output = std::process::Command::new(&bin).output().unwrap();
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(text.contains("instrumented="), "guard output: {text}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unrelated_env_is_not_flagged() {
        let env = vec![("PATH".to_string(), "/usr/bin:/home/frida".to_string())];
        let r = classify(None, None, &env, DEFAULT_SIGNATURES);
        assert!(!r.instrumented, "PATH must not be scanned");
    }
}
