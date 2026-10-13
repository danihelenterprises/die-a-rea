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

/// Native debug metadata markers (DWARF / PDB / Mach-O `__DWARF`).
const NATIVE_DEBUG_MARKERS: &[&[u8]] = &[b".debug_info", b".debug_line", b"__DWARF", b".pdb"];

/// JVM debug-only attribute names (constant-pool `Utf8` entries). Cleared by the
/// built-in `java-post` (B-047).
const JVM_DEBUG_MARKERS: &[&[u8]] = &[
    b"SourceFile",
    b"SourceDebugExtension",
    b"LineNumberTable",
    b"LocalVariableTable",
    b"LocalVariableTypeTable",
];

/// Debug metadata recoverable from one blob, if any. Format-aware so JVM/WASM
/// metadata is measured without false positives on unrelated binaries (B-047).
fn debug_metadata(bytes: &[u8]) -> Option<&'static str> {
    if NATIVE_DEBUG_MARKERS
        .iter()
        .any(|m| count_occurrences(bytes, m) > 0)
    {
        return Some("native debug metadata");
    }
    if bytes.starts_with(&[0xCA, 0xFE, 0xBA, 0xBE])
        && JVM_DEBUG_MARKERS
            .iter()
            .any(|m| count_occurrences(bytes, m) > 0)
    {
        return Some("JVM debug attributes");
    }
    if bytes.starts_with(b"PK\x03\x04")
        && direa_core::jar::class_entries(bytes).iter().any(|c| {
            c.starts_with(&[0xCA, 0xFE, 0xBA, 0xBE])
                && JVM_DEBUG_MARKERS
                    .iter()
                    .any(|m| count_occurrences(c, m) > 0)
        })
    {
        return Some("JVM debug attributes (jar)");
    }
    if direa_core::dex::measure_debug_info(bytes).is_some_and(|r| r.has_debug_info()) {
        return Some("DEX debug info");
    }
    if bytes.starts_with(b"PK\x03\x04")
        && direa_core::jar::entries_with_ext(bytes, "dex")
            .iter()
            .any(|d| direa_core::dex::measure_debug_info(d).is_some_and(|r| r.has_debug_info()))
    {
        return Some("DEX debug info (apk)");
    }
    if bytes.starts_with(b"\0asm")
        && direa_core::wasm::custom_section_names(bytes)
            .iter()
            .any(|n| !n.starts_with(direa_core::wasm::SECTION_PREFIX))
    {
        // die-a-rea's own injected sections (`direa.*`, the agent notice/canaries)
        // are not debug metadata, so they are ignored here (B-046).
        return Some("WASM custom section");
    }
    None
}

/// Detects debug metadata: native DWARF/PDB, JVM debug attributes, WASM custom
/// sections (name/producers). Boolean probe (B-047).
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
        let found = blobs.iter().find_map(|b| debug_metadata(b));
        Ok(ProbeReading {
            axis: self.axis(),
            raw: if found.is_some() { 1.0 } else { 0.0 },
            detail: found.unwrap_or("no debug metadata markers").to_string(),
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

/// Locate a target for the optional external REA adversary.
///
/// The adversary is bring-your-own: its provider is chosen by the configured
/// command, so any existing artifact (JS/Electron, native, WASM, Java, ...) is
/// handed over and the adversary reports what it could do. Only a missing path
/// yields `None` (B-048).
pub fn rea_target(target: &Path) -> Option<PathBuf> {
    (target.is_file() || target.is_dir()).then(|| target.to_path_buf())
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
    fn jvm_debug_attributes_are_measured() {
        // magic + version + constant_pool_count=2 + Utf8 "SourceFile".
        let mut data = vec![0xCA, 0xFE, 0xBA, 0xBE, 0x00, 0x00, 0x00, 0x34];
        data.extend_from_slice(&2u16.to_be_bytes());
        data.push(0x01);
        data.extend_from_slice(&10u16.to_be_bytes());
        data.extend_from_slice(b"SourceFile");
        let p = temp_file("dbg.class", &data);
        assert_eq!(DebugInfoProbe.measure(&p).unwrap().raw, 1.0);

        let mut stripped = data.clone();
        direa_core::java::strip_debug_attributes(&mut stripped);
        let p2 = temp_file("dbg-stripped.class", &stripped);
        assert_eq!(
            DebugInfoProbe.measure(&p2).unwrap().raw,
            0.0,
            "java-post must make the debug axis measurable and clear"
        );
        let _ = std::fs::remove_file(p);
        let _ = std::fs::remove_file(p2);
    }

    #[test]
    fn jar_debug_attributes_are_measured() {
        // A stored-entry jar whose `.class` carries a debug attribute name.
        let mut class = vec![0xCA, 0xFE, 0xBA, 0xBE, 0x00, 0x00, 0x00, 0x34];
        class.extend_from_slice(&2u16.to_be_bytes());
        class.push(0x01);
        class.extend_from_slice(&10u16.to_be_bytes());
        class.extend_from_slice(b"SourceFile");
        let jar = stored_zip(b"App.class", &class);

        let p = temp_file("dbg.jar", &jar);
        assert_eq!(DebugInfoProbe.measure(&p).unwrap().raw, 1.0);

        let (stripped, _) = direa_core::jar::harden_archive(&jar, &|bytes| {
            let mut v = bytes.to_vec();
            direa_core::java::strip_debug_attributes(&mut v);
            v
        });
        let p2 = temp_file("dbg-stripped.jar", &stripped);
        assert_eq!(
            DebugInfoProbe.measure(&p2).unwrap().raw,
            0.0,
            "jar rewriting must make the debug axis measurable and clear"
        );
        let _ = std::fs::remove_file(p);
        let _ = std::fs::remove_file(p2);
    }

    /// A minimal ZIP with one *stored* (uncompressed) entry. The probe never
    /// validates the CRC, so it is written as zero.
    fn stored_zip(name: &[u8], data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let local_off = 0u32;
        out.extend_from_slice(b"PK\x03\x04");
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        out.extend_from_slice(&0u16.to_le_bytes()); // flags
        out.extend_from_slice(&0u16.to_le_bytes()); // method: store
        out.extend_from_slice(&0u16.to_le_bytes()); // time
        out.extend_from_slice(&0u16.to_le_bytes()); // date
        out.extend_from_slice(&0u32.to_le_bytes()); // crc (unchecked)
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name);
        out.extend_from_slice(data);

        let cd_off = out.len() as u32;
        let mut cd = Vec::new();
        cd.extend_from_slice(b"PK\x01\x02");
        cd.extend_from_slice(&20u16.to_le_bytes());
        cd.extend_from_slice(&20u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u32.to_le_bytes());
        cd.extend_from_slice(&(data.len() as u32).to_le_bytes());
        cd.extend_from_slice(&(data.len() as u32).to_le_bytes());
        cd.extend_from_slice(&(name.len() as u16).to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes()); // extra len
        cd.extend_from_slice(&0u16.to_le_bytes()); // comment len
        cd.extend_from_slice(&0u16.to_le_bytes()); // disk start
        cd.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
        cd.extend_from_slice(&0u32.to_le_bytes()); // external attrs
        cd.extend_from_slice(&local_off.to_le_bytes());
        cd.extend_from_slice(name);
        let cd_size = cd.len() as u32;
        out.extend_from_slice(&cd);

        out.extend_from_slice(b"PK\x05\x06");
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&cd_size.to_le_bytes());
        out.extend_from_slice(&cd_off.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out
    }

    /// A minimal DEX with one class and one method; `debug` sets the method's
    /// `debug_info_off` (source_file_idx is NO_INDEX).
    fn dex_bytes(debug: bool) -> Vec<u8> {
        let mut d = vec![0u8; 192];
        d[0..8].copy_from_slice(b"dex\n035\0");
        let put =
            |d: &mut [u8], off: usize, v: u32| d[off..off + 4].copy_from_slice(&v.to_le_bytes());
        let len = d.len() as u32;
        put(&mut d, 32, len);
        put(&mut d, 36, 112);
        put(&mut d, 96, 1); // class_defs_size
        put(&mut d, 100, 112); // class_defs_off
        put(&mut d, 112 + 16, 0xFFFF_FFFF); // source_file_idx = NO_INDEX
        put(&mut d, 112 + 24, 144); // class_data_off
        d[146] = 1; // direct_methods_size
        d[148] = 0; // method_idx_diff
        d[149] = 1; // access_flags
        d[150] = 0xB0; // code_off = 176 (uleb)
        d[151] = 0x01;
        put(&mut d, 176 + 8, if debug { 1 } else { 0 }); // debug_info_off
        d
    }

    #[test]
    fn dex_debug_info_is_measured() {
        let p = temp_file("dbg.dex", &dex_bytes(true));
        assert_eq!(DebugInfoProbe.measure(&p).unwrap().raw, 1.0);
        let p2 = temp_file("dbg-nodbg.dex", &dex_bytes(false));
        assert_eq!(DebugInfoProbe.measure(&p2).unwrap().raw, 0.0);
        let _ = std::fs::remove_file(p);
        let _ = std::fs::remove_file(p2);
    }

    #[test]
    fn apk_dex_debug_info_is_measured() {
        let apk = stored_zip(b"classes.dex", &dex_bytes(true));
        let p = temp_file("dbg.apk", &apk);
        assert_eq!(DebugInfoProbe.measure(&p).unwrap().raw, 1.0);
        let apk_clean = stored_zip(b"classes.dex", &dex_bytes(false));
        let p2 = temp_file("dbg-clean.apk", &apk_clean);
        assert_eq!(DebugInfoProbe.measure(&p2).unwrap().raw, 0.0);
        let _ = std::fs::remove_file(p);
        let _ = std::fs::remove_file(p2);
    }

    #[test]
    fn wasm_custom_section_is_measured() {
        let mut data = b"\0asm\x01\0\0\0".to_vec();
        data.extend_from_slice(b"\x00\x08\x04nameabc");
        let p = temp_file("dbg.wasm", &data);
        assert_eq!(DebugInfoProbe.measure(&p).unwrap().raw, 1.0);

        let mut stripped = data.clone();
        direa_core::wasm::strip_custom_sections(&mut stripped);
        let p2 = temp_file("dbg-stripped.wasm", &stripped);
        assert_eq!(
            DebugInfoProbe.measure(&p2).unwrap().raw,
            0.0,
            "wasm-post must make the debug axis measurable and clear"
        );
        let _ = std::fs::remove_file(p);
        let _ = std::fs::remove_file(p2);
    }

    #[test]
    fn wasm_agent_notice_is_not_measured_as_debug() {
        // A real debug custom section is measured...
        let mut data = b"\0asm\x01\0\0\0".to_vec();
        data.extend_from_slice(b"\x00\x08\x04nameabc");
        let p = temp_file("dbg-notice.wasm", &data);
        assert_eq!(DebugInfoProbe.measure(&p).unwrap().raw, 1.0);
        // ...but after wasm-post strips it and the notice is injected, the axis
        // is clear (die-a-rea's own `direa.*` section is not debug metadata).
        direa_core::wasm::strip_custom_sections(&mut data);
        direa_core::wasm::inject_custom_section(
            &mut data,
            direa_core::wasm::NOTICE_SECTION,
            b"protected",
        );
        let p2 = temp_file("dbg-notice2.wasm", &data);
        assert_eq!(DebugInfoProbe.measure(&p2).unwrap().raw, 0.0);
        let _ = std::fs::remove_file(p);
        let _ = std::fs::remove_file(p2);
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
