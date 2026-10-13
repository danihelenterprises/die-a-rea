//! Integration tests for the bundled Node adapters.
//!
//! The host strictly parses an adapter's entire stdout as JSON
//! (`SubprocessAdapter`), so a wrapped tool that writes to stdout must be
//! *captured* rather than inherited — otherwise its output corrupts the JSON
//! response and hides the real error (B-057, K-053). These tests spawn the
//! adapters directly and are skipped when `node` is not on `PATH`.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde_json::Value;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Run one bundled adapter with its command env var set, returning the parsed
/// JSON response. Panics if the adapter's stdout is not valid JSON.
fn run_adapter(script: &str, env_key: &str, command: &str) -> Value {
    let request = serde_json::json!({
        "protocol": direa_core::adapter::proto_version(),
        "op": "apply",
        "target": "/tmp/direa-adapter-test-target",
        "kind": "javascript",
        "policy": {},
        "options": { "out": "/tmp/direa-adapter-test-out" },
    });
    let mut child = Command::new("node")
        .arg(repo_root().join(script))
        .env(env_key, command)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("spawning node {script}: {e}"));
    child
        .stdin
        .as_mut()
        .expect("adapter stdin")
        .write_all(request.to_string().as_bytes())
        .expect("writing adapter request");
    let out = child.wait_with_output().expect("waiting for adapter");
    assert!(
        out.status.success(),
        "adapter exited non-zero: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "adapter stdout is not valid JSON ({e}): {:?}",
            String::from_utf8_lossy(&out.stdout)
        )
    })
}

fn node_available() -> bool {
    direa_core::which("node").is_some()
}

/// A noisy but successful wrapped tool must not corrupt the response.
fn assert_capture_ok(script: &str, env_key: &str) {
    let resp = run_adapter(script, env_key, "printf 'noisy tool stdout\\n'");
    assert_eq!(resp["status"], "ok", "{script}: {resp}");
}

/// A failing wrapped tool must surface its output in a finding.
fn assert_failure_surfaced(script: &str, env_key: &str) {
    let resp = run_adapter(script, env_key, "echo boom >&2; exit 3");
    assert_eq!(resp["status"], "error", "{script}: {resp}");
    let msg = resp["findings"][0]["message"].as_str().unwrap_or_default();
    assert!(
        msg.contains("boom"),
        "{script}: captured output missing: {resp}"
    );
}

#[test]
fn clr_adapter_captures_wrapped_tool_output() {
    if !node_available() {
        return;
    }
    assert_capture_ok("adapters/clr/direa-clr", "DIREA_CLR_COMMAND");
    assert_failure_surfaced("adapters/clr/direa-clr", "DIREA_CLR_COMMAND");
}

#[test]
fn jvm_adapter_captures_wrapped_tool_output() {
    if !node_available() {
        return;
    }
    assert_capture_ok("adapters/jvm/direa-jvm", "DIREA_JVM_COMMAND");
    assert_failure_surfaced("adapters/jvm/direa-jvm", "DIREA_JVM_COMMAND");
}

#[test]
fn llvm_adapter_captures_wrapped_tool_output() {
    if !node_available() {
        return;
    }
    assert_capture_ok("adapters/llvm/direa-llvm", "DIREA_LLVM_BUILD");
    assert_failure_surfaced("adapters/llvm/direa-llvm", "DIREA_LLVM_BUILD");
}
