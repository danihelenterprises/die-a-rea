//! Heuristic discovery of a project's shipped artifacts.
//!
//! Used by `die-a-rea init` to pre-fill targets. It reads common manifests
//! (`Cargo.toml`, `package.json`, `pom.xml`/Gradle, `go.mod`, a .NET project,
//! an Android project, CMake/Make) and looks in conventional output
//! directories. It never fails: it returns whatever exists.

use std::collections::HashSet;
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

fn push_if_kind(out: &mut Vec<Discovered>, path: PathBuf, kinds: &[TargetKind], reason: &str) {
    if path.is_file() {
        let kind = kind_of(&path);
        if kinds.contains(&kind) {
            out.push(Discovered {
                path,
                kind,
                reason: reason.to_string(),
            });
        }
    }
}

/// First file under `dir` (recursive, breadth-first, budget-bounded) whose
/// detected kind is in `kinds`, skipping extensions in `skip`.
fn first_kind_in(
    dir: &Path,
    kinds: &[TargetKind],
    skip: &[&str],
    budget: &mut usize,
) -> Option<PathBuf> {
    if *budget == 0 {
        return None;
    }
    *budget -= 1;
    let mut subdirs = Vec::new();
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            subdirs.push(path);
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .unwrap_or_default();
        if skip.contains(&ext.as_str()) {
            continue;
        }
        if kinds.contains(&kind_of(&path)) {
            return Some(path);
        }
    }
    for d in subdirs {
        if let Some(f) = first_kind_in(&d, kinds, skip, budget) {
            return Some(f);
        }
    }
    None
}

/// First file with `ext` under `dir` (recursive, budget-bounded).
fn first_with_ext_in(dir: &Path, ext: &str, budget: &mut usize) -> Option<PathBuf> {
    if *budget == 0 {
        return None;
    }
    *budget -= 1;
    let mut subdirs = Vec::new();
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            subdirs.push(path);
            continue;
        }
        if path
            .extension()
            .map(|e| e.eq_ignore_ascii_case(ext))
            .unwrap_or(false)
        {
            return Some(path);
        }
    }
    for d in subdirs {
        if let Some(f) = first_with_ext_in(&d, ext, budget) {
            return Some(f);
        }
    }
    None
}

fn file_contains(path: &Path, needle: &str) -> bool {
    std::fs::read_to_string(path)
        .map(|t| t.contains(needle))
        .unwrap_or(false)
}

/// The last path segment of a `go.mod` `module` line (the conventional output
/// binary name).
fn go_module_name(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join("go.mod")).ok()?;
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("module") {
            let rest = rest.trim();
            if !rest.is_empty() {
                return rest.rsplit('/').next().map(|s| s.to_string());
            }
        }
    }
    None
}

/// Search budget (directory entries visited) for each recursive discovery pass.
const DISCOVER_BUDGET: usize = 5000;

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

    // Tauri: the Rust backend lives in `src-tauri/`, so its binary is under
    // `src-tauri/target/{release,debug}/<name>` (the frontend `dist/` is found
    // below via `package.json`). Discovery-only support (B-059).
    let tauri = root.join("src-tauri");
    if tauri.join("tauri.conf.json").is_file() {
        if let Some(name) = cargo_package_name(&tauri) {
            push_if_file(
                &mut out,
                tauri.join("target/release").join(&name),
                "Tauri src-tauri target/release",
            );
            push_if_file(
                &mut out,
                tauri.join("target/debug").join(&name),
                "Tauri src-tauri target/debug",
            );
        }
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

    // Go: go.mod -> a binary named after the module (root/bin/dist).
    if root.join("go.mod").is_file() {
        if let Some(name) = go_module_name(root) {
            for cand in [root.join(&name), root.join("bin").join(&name)] {
                push_if_kind(&mut out, cand, &[TargetKind::Go], "go.mod build output");
            }
        }
        let mut budget = DISCOVER_BUDGET;
        if let Some(bin) = first_kind_in(&root.join("bin"), &[TargetKind::Go], &[], &mut budget) {
            push_if_kind(&mut out, bin, &[TargetKind::Go], "go.mod bin/ output");
        }
    }

    // .NET: a *.csproj/*.sln project -> an assembly under publish/ or bin/.
    let has_dotnet_project = ["csproj", "fsproj", "vbproj", "sln"]
        .iter()
        .any(|e| first_with_ext(root, e).is_some());
    if has_dotnet_project {
        for sub in ["publish", "bin"] {
            let mut budget = DISCOVER_BUDGET;
            if let Some(dll) =
                first_kind_in(&root.join(sub), &[TargetKind::DotNet], &[], &mut budget)
            {
                push_if_kind(&mut out, dll, &[TargetKind::DotNet], ".NET build output");
            }
        }
    }

    // Android: an Android project -> an APK under build/outputs.
    let is_android = root.join("AndroidManifest.xml").is_file()
        || root.join("app").join("AndroidManifest.xml").is_file()
        || file_contains(&root.join("build.gradle"), "com.android")
        || file_contains(&root.join("build.gradle.kts"), "com.android");
    if is_android {
        for base in ["app/build/outputs/apk", "build/outputs/apk"] {
            let mut budget = DISCOVER_BUDGET;
            if let Some(apk) = first_with_ext_in(&root.join(base), "apk", &mut budget) {
                push_if_kind(&mut out, apk, &[TargetKind::AndroidApk], "Android APK");
            }
        }
    }

    // C/C++: CMake/Make/meson -> a linked binary under build/ (objects skipped).
    if root.join("CMakeLists.txt").is_file()
        || root.join("Makefile").is_file()
        || root.join("meson.build").is_file()
    {
        const NATIVE: &[TargetKind] = &[
            TargetKind::NativeElf,
            TargetKind::NativePe,
            TargetKind::NativeMachO,
        ];
        const OBJECT_EXTS: &[&str] = &[
            "o", "a", "so", "dylib", "dll", "lib", "obj", "bc", "ll", "rlib", "rmeta",
        ];
        let mut budget = DISCOVER_BUDGET;
        if let Some(bin) = first_kind_in(&root.join("build"), NATIVE, OBJECT_EXTS, &mut budget) {
            push_if_kind(&mut out, bin, NATIVE, "C/C++ build output");
        }
    }

    // Generic: a dist/ directory if nothing else matched.
    if out.is_empty() {
        push_if_dir(&mut out, root.join("dist"), "dist/");
    }

    // Different heuristics can name the same artifact; keep the first sighting.
    let mut seen = HashSet::new();
    out.retain(|d| seen.insert(d.path.clone()));
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

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("direa-discover-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn finds_go_binary() {
        let dir = temp_dir("go");
        std::fs::write(dir.join("go.mod"), "module example.com/myapp\n\ngo 1.22\n").unwrap();
        std::fs::write(
            dir.join("myapp"),
            b"\x7fELF\x02\x01\x01\x00padding runtime.main",
        )
        .unwrap();
        let found = discover(&dir);
        assert!(
            found
                .iter()
                .any(|d| d.path.ends_with("myapp") && d.kind == TargetKind::Go),
            "go.mod project must discover the module binary: {found:?}"
        );
        let unique: HashSet<_> = found.iter().map(|d| &d.path).collect();
        assert_eq!(unique.len(), found.len(), "no duplicate paths: {found:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn finds_dotnet_assembly() {
        let dir = temp_dir("dotnet");
        std::fs::write(dir.join("App.csproj"), "<Project/>").unwrap();
        let out = dir.join("bin/Release/net8.0");
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(out.join("App.dll"), b"MZ\x90\x00padding BSJB metadata").unwrap();
        let found = discover(&dir);
        assert!(
            found
                .iter()
                .any(|d| d.path.ends_with("App.dll") && d.kind == TargetKind::DotNet),
            ".NET project must discover the built assembly: {found:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn finds_android_apk() {
        let dir = temp_dir("android");
        std::fs::write(dir.join("AndroidManifest.xml"), "<manifest/>").unwrap();
        let apk = dir.join("build/outputs/apk/debug");
        std::fs::create_dir_all(&apk).unwrap();
        std::fs::write(apk.join("app-debug.apk"), b"PK\x03\x04apk").unwrap();
        let found = discover(&dir);
        assert!(
            found
                .iter()
                .any(|d| d.path.ends_with("app-debug.apk") && d.kind == TargetKind::AndroidApk),
            "Android project must discover the APK: {found:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn finds_cpp_binary_and_skips_objects() {
        let dir = temp_dir("cpp");
        std::fs::write(dir.join("CMakeLists.txt"), "project(app)\n").unwrap();
        let build = dir.join("build");
        std::fs::create_dir_all(&build).unwrap();
        let elf = b"\x7fELF\x02\x01\x01\x00no go markers here";
        std::fs::write(build.join("app.o"), elf).unwrap();
        std::fs::write(build.join("app"), elf).unwrap();
        let found = discover(&dir);
        assert!(
            found
                .iter()
                .any(|d| d.path.ends_with("build/app") && d.kind == TargetKind::NativeElf),
            "C/C++ project must discover the linked binary: {found:?}"
        );
        assert!(
            !found.iter().any(|d| d.path.ends_with("app.o")),
            "object files must be skipped: {found:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn finds_tauri_backend_binary() {
        let dir = temp_dir("tauri");
        let tauri = dir.join("src-tauri");
        std::fs::create_dir_all(tauri.join("target/release")).unwrap();
        std::fs::write(
            tauri.join("Cargo.toml"),
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        std::fs::write(tauri.join("tauri.conf.json"), "{}").unwrap();
        std::fs::write(
            tauri.join("target/release/app"),
            b"\x7fELF\x02\x01\x01\x00no go markers",
        )
        .unwrap();
        let found = discover(&dir);
        assert!(
            found
                .iter()
                .any(|d| d.path.ends_with("src-tauri/target/release/app")
                    && d.kind == TargetKind::NativeElf),
            "Tauri project must discover the backend binary: {found:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
