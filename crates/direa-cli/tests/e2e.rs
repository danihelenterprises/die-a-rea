//! End-to-end: build an artifact, verify it, and assert resistance improved.

use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_die-a-rea")
}

#[test]
fn build_then_verify_improves_resistance() {
    let dir = std::env::temp_dir().join(format!("direa-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("app.js"),
        "// internal\n//# sourceMappingURL=app.js.map\n\
         function verifyLicense(key) {\n  \
         const magic = \"SUPER_SECRET_LICENSE_MAGIC_STRING\";\n  \
         const fallback = \"another_long_hardcoded_string_value\";\n  \
         return key === magic && fallback.length > 0;\n}\n\
         module.exports = { verifyLicense };\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("die-a-rea.toml"),
        "[project]\nname = \"e2e\"\n[policy]\nprofile = \"high\"\n\
         [randomization]\nenabled = false\n[tarpit]\nenabled = false\n\
         [[targets]]\npath = \"app.js\"\n",
    )
    .unwrap();

    let build = Command::new(bin())
        .current_dir(&dir)
        .args(["build", "--out", "dist"])
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let verify = Command::new(bin())
        .current_dir(&dir)
        .args([
            "verify",
            "--original",
            "app.js",
            "--hardened",
            "dist/app.js",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        verify.status.success(),
        "verify failed: {}",
        String::from_utf8_lossy(&verify.stderr)
    );

    let report: serde_json::Value = serde_json::from_slice(&verify.stdout).unwrap();
    let rrs = report["rrs"].as_u64().unwrap();
    assert!(rrs >= 25, "expected RRS >= 25, got {rrs}");

    // The source-map axis must improve (the map is removed).
    let measurements = report["measurements"].as_array().unwrap();
    if let Some(m) = measurements
        .iter()
        .find(|m| m["axis"] == "source-map-exposure")
    {
        assert!(
            m["hardened"].as_f64().unwrap() < m["baseline"].as_f64().unwrap(),
            "source-map exposure should drop"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn doctor_reports_wasm_and_decompiler_tools() {
    // `doctor` must surface the WASM toolchain and common decompilers, not just
    // the adapter tools (B-051).
    let dir = std::env::temp_dir().join(format!("direa-doctor-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let out = Command::new(bin())
        .current_dir(&dir)
        .args(["doctor", "--json"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "doctor failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let names: Vec<&str> = report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|c| c["name"].as_str())
        .collect();
    for want in [
        "tool:wasm-opt",
        "tool:wasm-mutate",
        "tool:wasm2wat",
        "tool:wat2wasm",
        "tool:ghidra",
        "tool:radare2",
        "tool:rizin",
    ] {
        assert!(
            names.contains(&want),
            "doctor must report {want}; got {names:?}"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}
