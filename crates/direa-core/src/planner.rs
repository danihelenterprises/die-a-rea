use std::fs::File;
use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::adapter::{AdapterEntry, PlanStep};
use crate::manifest::Manifest;

/// The class of a target artifact, used to select adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TargetKind {
    NativeElf,
    NativeMachO,
    NativePe,
    Go,
    DotNet,
    JavaScript,
    ElectronAsar,
    Wasm,
    JavaClass,
    AndroidApk,
    Unknown,
}

impl TargetKind {
    pub fn as_str(self) -> &'static str {
        match self {
            TargetKind::NativeElf => "native-elf",
            TargetKind::NativeMachO => "native-macho",
            TargetKind::NativePe => "native-pe",
            TargetKind::Go => "go",
            TargetKind::DotNet => "dotnet",
            TargetKind::JavaScript => "javascript",
            TargetKind::ElectronAsar => "electron-asar",
            TargetKind::Wasm => "wasm",
            TargetKind::JavaClass => "java-class",
            TargetKind::AndroidApk => "android-apk",
            TargetKind::Unknown => "unknown",
        }
    }
}

/// Detect the target kind from the filesystem path and leading bytes.
pub fn detect_kind(path: &Path, bytes: &[u8]) -> TargetKind {
    if path.is_dir() {
        // Directory-shaped targets.
        if path.join("AndroidManifest.xml").exists() || path.join("classes.dex").exists() {
            return TargetKind::AndroidApk;
        }
        if has_asar(path) {
            return TargetKind::ElectronAsar;
        }
        if path.join("package.json").exists() {
            return TargetKind::JavaScript;
        }
        return TargetKind::Unknown;
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();

    // Directory-shaped .asar archives.
    if ext == "asar" {
        return TargetKind::ElectronAsar;
    }
    if ext == "apk" || ext == "aab" {
        return TargetKind::AndroidApk;
    }
    if ext == "wasm" {
        return TargetKind::Wasm;
    }
    if ext == "class" {
        return TargetKind::JavaClass;
    }

    if bytes.starts_with(b"\x7fELF") {
        return if looks_like_go(bytes) || file_contains_any(path, GO_MARKERS) {
            TargetKind::Go
        } else {
            TargetKind::NativeElf
        };
    }
    if bytes.starts_with(b"\0asm") {
        return TargetKind::Wasm;
    }
    if is_macho(bytes) {
        return TargetKind::NativeMachO;
    }
    if bytes.starts_with(b"MZ") {
        if looks_like_dotnet(bytes) || file_contains_any(path, DOTNET_MARKERS) {
            return TargetKind::DotNet;
        }
        return TargetKind::NativePe;
    }

    match ext.as_str() {
        "exe" | "dll" | "so" | "dylib" => TargetKind::NativePe,
        "js" | "mjs" | "cjs" | "ts" => TargetKind::JavaScript,
        _ => TargetKind::Unknown,
    }
}

fn is_macho(bytes: &[u8]) -> bool {
    matches!(
        bytes.get(0..4),
        Some([0xfe, 0xed, 0xfa, 0xce])
            | Some([0xfe, 0xed, 0xfa, 0xcf])
            | Some([0xce, 0xfa, 0xed, 0xfe])
            | Some([0xcf, 0xfa, 0xed, 0xfe])
            | Some([0xca, 0xfe, 0xba, 0xbe])
    )
}

/// Markers that identify a Go binary (build ID / build info / runtime symbol).
const GO_MARKERS: &[&[u8]] = &[
    b"Go build ID",
    b"go:buildid",
    b"runtime.main",
    b"\xff Go buildinf:",
];

/// Markers that identify a .NET assembly (CLI metadata root / CLR import).
const DOTNET_MARKERS: &[&[u8]] = &[b"mscoree.dll", b"BSJB", b".NETCoreApp"];

/// Upper bound on the streaming scan used when a marker is not in the sniff
/// prefix. Bounds the cost on huge artifacts.
const DETECT_SCAN_LIMIT: u64 = 128 * 1024 * 1024;

fn looks_like_go(bytes: &[u8]) -> bool {
    GO_MARKERS.iter().any(|m| contains(bytes, m))
}

fn looks_like_dotnet(bytes: &[u8]) -> bool {
    DOTNET_MARKERS.iter().any(|m| contains(bytes, m))
}

/// Search a file for any of `needles`, streaming in bounded chunks with overlap
/// so a marker straddling a chunk boundary is still found.
///
/// Many format markers sit far from the start of the file (a Go `runtime.main`
/// can be megabytes in; a .NET `BSJB` metadata root is addressed by a PE data
/// directory), so detection cannot rely on a fixed sniff prefix (K-047).
fn file_contains_any(path: &Path, needles: &[&[u8]]) -> bool {
    let max = needles.iter().map(|n| n.len()).max().unwrap_or(1);
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut buf = vec![0u8; 64 * 1024];
    let mut carry: Vec<u8> = Vec::new();
    let mut total: u64 = 0;
    loop {
        let n = match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => return false,
        };
        total += n as u64;
        let mut window = Vec::with_capacity(carry.len() + n);
        window.extend_from_slice(&carry);
        window.extend_from_slice(&buf[..n]);
        if needles.iter().any(|nd| contains(&window, nd)) {
            return true;
        }
        let keep = max.saturating_sub(1).min(window.len());
        carry = window[window.len() - keep..].to_vec();
        if total >= DETECT_SCAN_LIMIT {
            break;
        }
    }
    false
}

fn has_asar(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok()).any(|e| {
                e.path()
                    .extension()
                    .map(|x| x.eq_ignore_ascii_case("asar"))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// A resolved plan for one target: ordered steps plus diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildPlan {
    pub target: String,
    pub kind: TargetKind,
    pub steps: Vec<PlanStep>,
    /// Adapter ids selected for this target.
    pub adapters: Vec<String>,
    /// Adapter ids selected but not available on this host.
    pub missing: Vec<String>,
}

/// Default adapter registry, mapping target kinds to adapter commands.
pub fn default_registry() -> Vec<AdapterEntry> {
    let builtin = |id: &str, kinds: Vec<TargetKind>| AdapterEntry {
        id: id.to_string(),
        command: id.to_string(),
        args: vec![],
        kinds,
        builtin: true,
    };
    let external = |id: &str, command: &str, kinds: Vec<TargetKind>| AdapterEntry {
        id: id.to_string(),
        command: command.to_string(),
        args: vec![],
        kinds,
        builtin: false,
    };

    let native_kinds = vec![
        TargetKind::NativeElf,
        TargetKind::NativeMachO,
        TargetKind::NativePe,
        TargetKind::Go,
    ];

    vec![
        // Built-in, in-process adapters.
        builtin("native-post", native_kinds.clone()),
        builtin("wasm-post", vec![TargetKind::Wasm]),
        builtin("java-post", vec![TargetKind::JavaClass]),
        builtin("agent-canary", TargetKind::ALL.to_vec()),
        // External, subprocess adapters (bring-your-own toolchain; ADR-0002/0009).
        external(
            "js",
            "direa-js",
            vec![TargetKind::JavaScript, TargetKind::ElectronAsar],
        ),
        external("clr", "direa-clr", vec![TargetKind::DotNet]),
        external("wasm", "direa-wasm", vec![TargetKind::Wasm]),
        external("jvm", "direa-jvm", vec![TargetKind::AndroidApk]),
        external("llvm", "direa-llvm", native_kinds.clone()),
        external("go", "direa-go", vec![TargetKind::Go]),
    ]
}

impl TargetKind {
    pub const ALL: [TargetKind; 11] = [
        TargetKind::NativeElf,
        TargetKind::NativeMachO,
        TargetKind::NativePe,
        TargetKind::Go,
        TargetKind::DotNet,
        TargetKind::JavaScript,
        TargetKind::ElectronAsar,
        TargetKind::Wasm,
        TargetKind::JavaClass,
        TargetKind::AndroidApk,
        TargetKind::Unknown,
    ];
}

/// Build a plan for every target in a manifest.
pub fn plan_manifest(manifest: &Manifest, registry: &[AdapterEntry]) -> Vec<BuildPlan> {
    let policy = manifest.resolved_policy();
    manifest
        .targets
        .iter()
        .map(|spec| {
            let path = &spec.path;
            let kind = spec.kind.unwrap_or_else(|| {
                let bytes = std::fs::read(path)
                    .ok()
                    .and_then(|b| b.get(..b.len().min(4096)).map(|s| s.to_vec()))
                    .unwrap_or_default();
                detect_kind(path, &bytes)
            });

            let allow: Option<&[String]> = if spec.adapters.is_empty() {
                None
            } else {
                Some(&spec.adapters)
            };

            let mut steps = Vec::new();
            let mut adapters = Vec::new();
            let mut missing = Vec::new();

            for entry in registry.iter().filter(|e| e.kinds.contains(&kind)) {
                if let Some(allow) = allow {
                    if !allow.iter().any(|a| a == &entry.id) {
                        continue;
                    }
                }
                adapters.push(entry.id.clone());
                if !entry.available() {
                    missing.push(entry.id.clone());
                }
                steps.push(PlanStep {
                    adapter: entry.id.clone(),
                    action: format!("{}::plan", entry.id),
                    detail: format!(
                        "profile={} builtin={} available={}",
                        policy.profile.as_str(),
                        entry.builtin,
                        entry.available()
                    ),
                    external: !entry.builtin,
                    destructive: entry.id != "agent-canary",
                });
            }

            BuildPlan {
                target: path.display().to_string(),
                kind,
                steps,
                adapters,
                missing,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_formats() {
        assert_eq!(
            detect_kind(Path::new("a.bin"), b"\x7fELF..."),
            TargetKind::NativeElf
        );
        assert_eq!(
            detect_kind(Path::new("a.wasm"), b"\0asm\x01\0\0\0"),
            TargetKind::Wasm
        );
        assert_eq!(
            detect_kind(Path::new("a.dll"), b"MZ....mscoree.dll"),
            TargetKind::DotNet
        );
        assert_eq!(
            detect_kind(Path::new("a.dll"), b"MZ....plain native"),
            TargetKind::NativePe
        );
        assert_eq!(
            detect_kind(Path::new("app.js"), b"const x = 1"),
            TargetKind::JavaScript
        );
        assert_eq!(
            detect_kind(Path::new("a.asar"), b"whatever"),
            TargetKind::ElectronAsar
        );
    }

    #[test]
    fn detects_go() {
        let mut elf = b"\x7fELF".to_vec();
        elf.extend_from_slice(b".... Go build ID: abc ....");
        assert_eq!(detect_kind(Path::new("bin"), &elf), TargetKind::Go);
    }

    #[test]
    fn plans_short_elf_file() {
        // Regression: files smaller than the 4096-byte sniff window must still
        // be detected (a ranged get used to return None for short files).
        let mut path = std::env::temp_dir();
        path.push("direa-short-elf.bin");
        std::fs::write(&path, b"\x7fELF\x02\x01\x01\x00binary").unwrap();
        let m = Manifest {
            targets: vec![crate::manifest::TargetSpec {
                path: path.clone(),
                kind: None,
                adapters: vec![],
            }],
            ..Default::default()
        };
        let plans = plan_manifest(&m, &default_registry());
        assert_eq!(plans[0].kind, TargetKind::NativeElf);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn detects_go_marker_beyond_the_sniff_window() {
        // The `runtime.main` marker sits past the 4 KiB sniff prefix (as in real
        // Go binaries, e.g. fzf at ~2 MB); detection must still find it (B-053).
        let path = std::env::temp_dir().join(format!("direa-go-late-{}", std::process::id()));
        let mut data = b"\x7fELF\x02\x01\x01\x00".to_vec();
        data.resize(8192, 0);
        data.extend_from_slice(b".... runtime.main ....");
        std::fs::write(&path, &data).unwrap();
        let prefix = &data[..data.len().min(4096)];
        assert_eq!(detect_kind(&path, prefix), TargetKind::Go);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn detects_dotnet_marker_beyond_the_sniff_window() {
        // The `BSJB` metadata root is beyond the sniff prefix (as in real
        // assemblies, e.g. Newtonsoft.Json at ~212 KB) (B-053).
        let path = std::env::temp_dir().join(format!("direa-dotnet-late-{}", std::process::id()));
        let mut data = b"MZ".to_vec();
        data.resize(8192, 0);
        data.extend_from_slice(b"....BSJB....");
        std::fs::write(&path, &data).unwrap();
        let prefix = &data[..data.len().min(4096)];
        assert_eq!(detect_kind(&path, prefix), TargetKind::DotNet);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn plan_selects_expected_adapters() {
        let m = Manifest {
            targets: vec![crate::manifest::TargetSpec {
                path: std::path::PathBuf::from("app.js"),
                kind: Some(TargetKind::JavaScript),
                adapters: vec![],
            }],
            ..Default::default()
        };
        let plans = plan_manifest(&m, &default_registry());
        assert_eq!(plans.len(), 1);
        assert!(plans[0].adapters.contains(&"js".to_string()));
        assert!(plans[0].adapters.contains(&"agent-canary".to_string()));
        assert!(!plans[0].adapters.contains(&"clr".to_string()));
    }
}
