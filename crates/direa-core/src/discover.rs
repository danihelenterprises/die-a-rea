//! Heuristic discovery of a project's shipped artifacts.
//!
//! Used by `die-a-rea init` to pre-fill targets. It reads common manifests
//! (`Cargo.toml`, `package.json`, `pom.xml`, Gradle) and looks in conventional
//! output directories. It never fails: it returns whatever exists.

use std::path::{Path, PathBuf};

use crate::planner::{detect_kind, TargetKind};

/// A discovered candidate artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discovered {
    pub path: PathBuf,
    pub kind: TargetKind,
    pub reason: String,
}

fn kind_of(path: &Path) -> TargetKind {
    let bytes = std::fs::read(path)
        .ok()
        .and_then(|b| b.get(..b.len().min(4096)).map(|s| s.to_vec()))
        .unwrap_or_default();
    detect_kind(path, &bytes)
}

fn cargo_package_name(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join("Cargo.toml")).ok()?;
    let mut in_package = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if in_package {
            if let Some(rest) = line.strip_prefix("name") {
                let rest = rest.trim_start().trim_start_matches('=').trim();
                return Some(rest.trim_matches('"').to_string());
            }
        }
    }
    None
}

fn push_if_file(out: &mut Vec<Discovered>, path: PathBuf, reason: &str) {
    if path.is_file() {
        let kind = kind_of(&path);
        out.push(Discovered {
            path,
            kind,
            reason: reason.to_string(),
        });
    }
}

fn push_if_dir(out: &mut Vec<Discovered>, path: PathBuf, reason: &str) {
    if path.is_dir() {
        let kind = kind_of(&path);
        out.push(Discovered {
            path,
            kind,
            reason: reason.to_string(),
        });
    }
}

fn first_with_ext(dir: &Path, ext: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .extension()
            .map(|e| e.eq_ignore_ascii_case(ext))
            .unwrap_or(false)
        {
            return Some(path);
        }
    }
    None
}

/// Discover candidate artifacts under `root`.
pub fn discover(root: &Path) -> Vec<Discovered> {
    let mut out = Vec::new();

    // Rust: target/release/<name>
    if let Some(name) = cargo_package_name(root) {
        push_if_file(
            &mut out,
            root.join("target/release").join(&name),
            "Cargo.toml target/release",
        );
        push_if_file(
            &mut out,
            root.join("target/debug").join(&name),
            "Cargo.toml target/debug",
        );
    }

    // Node / Electron: package.json main/bin, or dist/
    if let Ok(text) = std::fs::read_to_string(root.join("package.json")) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(main) = json.get("main").and_then(|v| v.as_str()) {
                push_if_file(&mut out, root.join(main), "package.json main");
            }
            if let Some(bin) = json.get("bin").and_then(|v| v.as_str()) {
                push_if_file(&mut out, root.join(bin), "package.json bin");
            }
        }
        push_if_dir(&mut out, root.join("dist"), "package.json dist/");
        push_if_dir(&mut out, root.join("build"), "package.json build/");
        if let Some(asar) = first_with_ext(&root.join("dist"), "asar") {
            push_if_file(&mut out, asar, "dist/*.asar");
        }
    }

    // JVM: target/*.jar or build/libs/*.jar
    if root.join("pom.xml").is_file() || root.join("build.gradle").is_file() {
        if let Some(jar) = first_with_ext(&root.join("target"), "jar") {
            push_if_file(&mut out, jar, "JVM target/*.jar");
        }
        if let Some(jar) = first_with_ext(&root.join("build/libs"), "jar") {
            push_if_file(&mut out, jar, "JVM build/libs/*.jar");
        }
    }

    // Generic: a dist/ directory if nothing else matched.
    if out.is_empty() {
        push_if_dir(&mut out, root.join("dist"), "dist/");
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_rust_binary() {
        let dir = std::env::temp_dir().join(format!("direa-discover-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("target/release")).unwrap();
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"myapp\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("target/release/myapp"),
            b"\x7fELF\x02\x01\x01\x00rest",
        )
        .unwrap();
        let found = discover(&dir);
        assert!(found
            .iter()
            .any(|d| d.path.ends_with("target/release/myapp") && d.kind == TargetKind::NativeElf));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn finds_node_main() {
        let dir = std::env::temp_dir().join(format!("direa-discover-node-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("package.json"), "{\"main\":\"app.js\"}").unwrap();
        std::fs::write(dir.join("app.js"), "var x=1;\n").unwrap();
        let found = discover(&dir);
        assert!(found
            .iter()
            .any(|d| d.path.ends_with("app.js") && d.kind == TargetKind::JavaScript));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
