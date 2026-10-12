//! Built-in, dependency-free probes used to measure reverse-engineering
//! exposure before and after hardening.
//!
//! These probes are *proxies*: they approximate what a decompiler, `strings`,
//! `nm`, or a debugger would recover, using only the raw artifact bytes. They
//! are intentionally transparent so results can be reproduced and extended.

use std::path::{Path, PathBuf};

use direa_core::scan::count_tokens;
use direa_core::Axis;

/// Whether a probe's raw value is a count (normalised as a ratio vs. baseline)
/// or already a boolean/normalised value in `[0, 1]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeKind {
    Count,
    Boolean,
}

/// One raw probe reading.
#[derive(Debug, Clone)]
pub struct ProbeReading {
    pub axis: Axis,
    pub raw: f64,
    pub detail: String,
}

/// A measurement probe.
pub trait Probe {
    fn name(&self) -> &'static str;
    fn axis(&self) -> Axis;
    fn kind(&self) -> ProbeKind;
    fn measure(&self, target: &Path) -> Result<ProbeReading, String>;
}

/// Cap on per-file bytes read, to keep verification bounded on huge artifacts.
const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_FILES: usize = 20_000;

/// Collect bytes from a target: a single file, or every regular file under a
/// directory (bounded). Used by the byte-oriented probes.
pub fn collect_bytes(target: &Path) -> Result<Vec<Vec<u8>>, String> {
    if target.is_file() {
        Ok(vec![read_file(target)?])
    } else if target.is_dir() {
        let mut out = Vec::new();
        walk(target, &mut out, &mut 0)?;
        Ok(out)
    } else {
        Err(format!("target does not exist: {}", target.display()))
    }
}

fn read_file(path: &Path) -> Result<Vec<u8>, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if meta.len() > MAX_FILE_BYTES {
        return Err(format!(
            "{}: file too large for bounded read",
            path.display()
        ));
    }
    std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn walk(dir: &Path, out: &mut Vec<Vec<u8>>, files: &mut usize) -> Result<(), String> {
    if *files >= MAX_FILES {
        return Ok(());
    }
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out, files)?;
        } else if path.is_file() {
            *files += 1;
            if let Ok(bytes) = read_file(&path) {
                out.push(bytes);
            }
        }
    }
    Ok(())
}

/// A token scanner for recoverable identifier/literal names (a `strings`-style
/// proxy that does not collapse on minified single-line files).
pub struct StringProbe;

impl Probe for StringProbe {
    fn name(&self) -> &'static str {
        "strings"
    }
    fn axis(&self) -> Axis {
        Axis::StringExposure
    }
    fn kind(&self) -> ProbeKind {
        ProbeKind::Count
    }
    fn measure(&self, target: &Path) -> Result<ProbeReading, String> {
        let blobs = collect_bytes(target)?;
        let mut count = 0u64;
        for bytes in &blobs {
            count += count_tokens(bytes, direa_core::scan::DEFAULT_MIN_TOKEN);
        }
        Ok(ProbeReading {
            axis: self.axis(),
            raw: count as f64,
            detail: format!("{count} identifier/literal tokens (>=6 chars)"),
        })
    }
}

/// Counts symbol/relocation section markers as a proxy for symbol recovery.
pub struct SymbolProbe;

const SYMBOL_MARKERS: &[&[u8]] = &[
    b".symtab",
    b".dynsym",
    b".strtab",
    b".symtab_shndx",
    b".rela.text",
    b".rela.dyn",
    b"__LINKEDIT",
    b".edata",
    b".pdata",
    b"LC_SYMTAB",
    b"__symbol_stub",
];

impl Probe for SymbolProbe {
    fn name(&self) -> &'static str {
        "symbols"
    }
    fn axis(&self) -> Axis {
        Axis::SymbolExposure
    }
    fn kind(&self) -> ProbeKind {
        ProbeKind::Count
    }
    fn measure(&self, target: &Path) -> Result<ProbeReading, String> {
        let blobs = collect_bytes(target)?;
        let mut count = 0usize;
        for bytes in &blobs {
            for marker in SYMBOL_MARKERS {
                count += count_occurrences(bytes, marker);
            }
        }
        Ok(ProbeReading {
            axis: self.axis(),
            raw: count as f64,
            detail: format!("{count} symbol/relocation section markers"),
        })
    }
}

/// Detects debug metadata (DWARF / PDB) presence.
pub struct DebugInfoProbe;

impl Probe for DebugInfoProbe {
    fn name(&self) -> &'static str {
        "debug-info"
    }
    fn axis(&self) -> Axis {
        Axis::DebugMetadataExposure
    }
    fn kind(&self) -> ProbeKind {
        ProbeKind::Boolean
    }
    fn measure(&self, target: &Path) -> Result<ProbeReading, String> {
        let blobs = collect_bytes(target)?;
        let mut present = false;
        for bytes in &blobs {
            if count_occurrences(bytes, b".debug_info") > 0
                || count_occurrences(bytes, b".debug_line") > 0
                || count_occurrences(bytes, b"__DWARF") > 0
                || count_occurrences(bytes, b".pdb") > 0
            {
                present = true;
                break;
            }
        }
        Ok(ProbeReading {
            axis: self.axis(),
            raw: if present { 1.0 } else { 0.0 },
            detail: if present {
                "debug metadata present".into()
            } else {
                "no debug metadata markers".into()
            },
        })
    }
}

/// Counts JavaScript source-map exposure markers.
pub struct SourceMapProbe;

impl Probe for SourceMapProbe {
    fn name(&self) -> &'static str {
        "source-maps"
    }
    fn axis(&self) -> Axis {
        Axis::SourceMapExposure
    }
    fn kind(&self) -> ProbeKind {
        ProbeKind::Count
    }
    fn measure(&self, target: &Path) -> Result<ProbeReading, String> {
        let mut count = 0usize;
        if target.is_dir() {
            count += count_extension(target, "map");
        } else if target
            .extension()
            .map(|e| e.eq_ignore_ascii_case("map"))
            .unwrap_or(false)
        {
            count += 1;
        }
        let blobs = collect_bytes(target)?;
        for bytes in &blobs {
            count += count_occurrences(bytes, b"sourceMappingURL");
            count += count_occurrences(bytes, b"sourcesContent");
        }
        Ok(ProbeReading {
            axis: self.axis(),
            raw: count as f64,
            detail: format!("{count} source-map / sourceMappingURL markers"),
        })
    }
}

fn count_extension(dir: &Path, ext: &str) -> usize {
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                count += count_extension(&path, ext);
            } else if path
                .extension()
                .map(|e| e.eq_ignore_ascii_case(ext))
                .unwrap_or(false)
            {
                count += 1;
            }
        }
    }
    count
}

/// Measures recoverable control flow with an external decompiler/disassembler
/// command (`{artifact}` is substituted). There is no built-in proxy: without a
/// configured tool the control-flow axis is **skipped**, not fabricated
/// (ADR-0017, K-004). The command must print an integer count of recoverable
/// functions (see `tools/controlflow-count`).
pub struct ExternalControlFlowProbe {
    pub command: String,
}

impl Probe for ExternalControlFlowProbe {
    fn name(&self) -> &'static str {
        "control-flow"
    }
    fn axis(&self) -> Axis {
        Axis::ControlFlowExposure
    }
    fn kind(&self) -> ProbeKind {
        ProbeKind::Count
    }
    fn measure(&self, target: &Path) -> Result<ProbeReading, String> {
        let cmd = self
            .command
            .replace("{artifact}", &target.display().to_string());
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .output()
            .map_err(|e| format!("spawning decompiler command failed: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "decompiler command failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let count = text
            .split_whitespace()
            .find_map(|t| t.parse::<i64>().ok())
            .ok_or_else(|| "no integer count in decompiler output".to_string())?;
        Ok(ProbeReading {
            axis: self.axis(),
            raw: count.max(0) as f64,
            detail: format!("{count} recoverable functions (external)"),
        })
    }
}

fn count_occurrences(haystack: &[u8], needle: &[u8]) -> usize {
    if needle.is_empty() || haystack.len() < needle.len() {
        return 0;
    }
    haystack
        .windows(needle.len())
        .filter(|w| *w == needle)
        .count()
}

/// Locate a conventional REA JavaScript target entry for the optional REA probe.
pub fn rea_js_target(target: &Path) -> Option<PathBuf> {
    if target.is_dir() && target.join("package.json").exists() {
        return Some(target.to_path_buf());
    }
    target
        .extension()
        .map(|e| e.eq_ignore_ascii_case("asar"))
        .unwrap_or(false)
        .then(|| target.to_path_buf())
}

/// All default probes, in report order. The control-flow axis is intentionally
/// absent: it requires an external decompiler (ADR-0017).
pub fn default_probes() -> Vec<Box<dyn Probe>> {
    vec![
        Box::new(StringProbe),
        Box::new(SymbolProbe),
        Box::new(DebugInfoProbe),
        Box::new(SourceMapProbe),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_file(name: &str, bytes: &[u8]) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("direa-test-{name}"));
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(bytes).unwrap();
        p
    }

    #[test]
    fn strings_counts_tokens() {
        let p = temp_file(
            "strings.bin",
            b"\x00helloworld\x00hi\x00longenoughstring\x00anothertoken\x00",
        );
        let reading = StringProbe.measure(&p).unwrap();
        assert!(reading.raw >= 3.0);
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn strings_does_not_collapse_on_one_line() {
        let p = temp_file(
            "minified.js",
            b"var abcdef=123456;function ghijkl(){return mnoPQR;}",
        );
        let reading = StringProbe.measure(&p).unwrap();
        assert!(reading.raw >= 3.0, "a minified line must yield many tokens");
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn debug_info_boolean() {
        let p = temp_file("dbg.bin", b"....section .debug_info....");
        assert_eq!(DebugInfoProbe.measure(&p).unwrap().raw, 1.0);
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn sourcemap_counts() {
        let p = temp_file("app.js", b"//# sourceMappingURL=app.js.map\nvar x=1;");
        assert!(SourceMapProbe.measure(&p).unwrap().raw >= 1.0);
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn external_control_flow_reads_a_count() {
        let target = temp_file("cf.bin", b"x");
        let ok = ExternalControlFlowProbe {
            command: "printf 10".into(),
        };
        assert_eq!(ok.measure(&target).unwrap().raw, 10.0);

        let bad = ExternalControlFlowProbe {
            command: "printf notanumber".into(),
        };
        assert!(bad.measure(&target).is_err());

        let fail = ExternalControlFlowProbe {
            command: "false".into(),
        };
        assert!(fail.measure(&target).is_err());

        let _ = std::fs::remove_file(target);
    }
}
