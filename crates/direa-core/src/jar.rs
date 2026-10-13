//! Toolchain-free rewriting of Java `.jar` archives (ZIP containers).
//!
//! A `.jar` is a ZIP file. [`harden_archive`] walks the central directory,
//! decompresses the embedded `.class` entries through a caller transform (the
//! built-in `java-post`), and repacks the archive with fresh local headers,
//! CRCs and central-directory offsets. Non-`.class` entries keep their original
//! compressed bytes byte-for-byte; only the framing around them is rebuilt.
//!
//! The reader/writer is deliberately conservative: archives that use ZIP64,
//! encryption, data descriptors or a compression method other than *store* (0)
//! or *deflate* (8) are left untouched (`changed == false`), so a caller falls
//! back to copying. `flate2` (pure-Rust `miniz_oxide`, permissively licensed)
//! provides raw DEFLATE; nothing here executes or network-fetches.

use std::io::{Read, Write};

const EOCD_SIG: &[u8; 4] = b"PK\x05\x06";
const CDFH_SIG: &[u8; 4] = b"PK\x01\x02";
const LFH_SIG: &[u8; 4] = b"PK\x03\x04";

const FLAG_ENCRYPTED: u16 = 0x0001;
const FLAG_DATA_DESCRIPTOR: u16 = 0x0008;

/// Outcome of hardening one archive.
#[derive(Debug, Default, Clone)]
pub struct JarReport {
    pub changed: bool,
    /// Matching entries whose bytes the transform altered.
    pub entries_hardened: usize,
    /// Entries the transform inspected.
    pub entries_total: usize,
    /// Files (of any type) that carry content in the archive.
    pub files_total: usize,
}

fn u16le(b: &[u8], off: usize) -> Option<u16> {
    b.get(off..off + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
}

fn u32le(b: &[u8], off: usize) -> Option<u32> {
    b.get(off..off + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn is_class(name: &[u8]) -> bool {
    name.len() >= 6 && name[name.len() - 6..].eq_ignore_ascii_case(b".class")
}

/// Locate the End Of Central Directory record (the signature is searched
/// backwards over the trailing comment region, ≤ 64 KiB + 22 bytes).
fn find_eocd(data: &[u8]) -> Option<usize> {
    if data.len() < 22 {
        return None;
    }
    let lower = data.len().saturating_sub(22 + 0xffff);
    let mut i = data.len() - 22;
    loop {
        if data[i..i + 4] == *EOCD_SIG {
            return Some(i);
        }
        if i <= lower {
            return None;
        }
        i -= 1;
    }
}

struct CentralEntry {
    name: Vec<u8>,
    method: u16,
    flags: u16,
    mod_time: u16,
    mod_date: u16,
    crc: u32,
    comp_size: u32,
    uncomp_size: u32,
    local_offset: u32,
    version_made: u16,
    version_needed: u16,
    internal_attrs: u16,
    external_attrs: u32,
    central_extra: Vec<u8>,
    comment: Vec<u8>,
}

/// Parse the central directory. Returns `None` for any layout this module
/// deliberately does not rewrite (ZIP64, encryption, multi-disk).
fn read_central(data: &[u8]) -> Option<Vec<CentralEntry>> {
    let eocd = find_eocd(data)?;
    if u16le(data, eocd + 4)? != 0 || u16le(data, eocd + 6)? != 0 {
        return None; // multi-disk archive
    }
    let total = u16le(data, eocd + 10)? as usize;
    let cd_size = u32le(data, eocd + 12)? as usize;
    let cd_off = u32le(data, eocd + 16)? as usize;
    if total == 0xffff || cd_size == 0xffff_ffff || cd_off == 0xffff_ffff {
        return None; // ZIP64
    }
    if cd_off.checked_add(cd_size)? > data.len() {
        return None;
    }

    let mut entries = Vec::with_capacity(total);
    let mut pos = cd_off;
    for _ in 0..total {
        if data.get(pos..pos + 4)? != &CDFH_SIG[..] {
            return None;
        }
        let flags = u16le(data, pos + 8)?;
        if flags & FLAG_ENCRYPTED != 0 {
            return None;
        }
        let comp_size = u32le(data, pos + 20)?;
        let uncomp_size = u32le(data, pos + 24)?;
        if comp_size == 0xffff_ffff || uncomp_size == 0xffff_ffff {
            return None; // ZIP64 entry
        }
        let name_len = u16le(data, pos + 28)? as usize;
        let extra_len = u16le(data, pos + 30)? as usize;
        let comment_len = u16le(data, pos + 32)? as usize;
        let name = data.get(pos + 46..pos + 46 + name_len)?.to_vec();
        let extra = data
            .get(pos + 46 + name_len..pos + 46 + name_len + extra_len)?
            .to_vec();
        let comment = data
            .get(pos + 46 + name_len + extra_len..pos + 46 + name_len + extra_len + comment_len)?
            .to_vec();
        entries.push(CentralEntry {
            name,
            method: u16le(data, pos + 10)?,
            flags,
            mod_time: u16le(data, pos + 12)?,
            mod_date: u16le(data, pos + 14)?,
            crc: u32le(data, pos + 16)?,
            comp_size,
            uncomp_size,
            local_offset: u32le(data, pos + 42)?,
            version_made: u16le(data, pos + 4)?,
            version_needed: u16le(data, pos + 6)?,
            internal_attrs: u16le(data, pos + 36)?,
            external_attrs: u32le(data, pos + 38)?,
            central_extra: extra,
            comment,
        });
        pos += 46 + name_len + extra_len + comment_len;
    }
    Some(entries)
}

/// The compressed bytes of one entry, located via its local file header.
fn compressed<'a>(data: &'a [u8], e: &CentralEntry) -> Option<&'a [u8]> {
    let lo = e.local_offset as usize;
    if data.get(lo..lo + 4)? != &LFH_SIG[..] {
        return None;
    }
    let name_len = u16le(data, lo + 26)? as usize;
    let extra_len = u16le(data, lo + 28)? as usize;
    let start = lo + 30 + name_len + extra_len;
    data.get(start..start + e.comp_size as usize)
}

/// Decompress one entry (`store` or raw `deflate`).
fn decompress(e: &CentralEntry, raw: &[u8]) -> Option<Vec<u8>> {
    match e.method {
        0 => Some(raw.to_vec()),
        8 => {
            let mut out = Vec::with_capacity(e.uncomp_size as usize);
            flate2::read::DeflateDecoder::new(raw)
                .read_to_end(&mut out)
                .ok()?;
            Some(out)
        }
        _ => None,
    }
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = flate2::Crc::new();
    crc.update(bytes);
    crc.sum()
}

struct OutEntry {
    name: Vec<u8>,
    method: u16,
    flags: u16,
    mod_time: u16,
    mod_date: u16,
    crc: u32,
    comp_size: u32,
    uncomp_size: u32,
    version_made: u16,
    version_needed: u16,
    internal_attrs: u16,
    external_attrs: u32,
    central_extra: Vec<u8>,
    comment: Vec<u8>,
    data: Vec<u8>,
}

/// Re-emit an entry's payload. `store` keeps the original compression method
/// (uncompressed entries stay uncompressed); otherwise raw DEFLATE is used.
fn encode(data: &[u8], store: bool) -> Option<(Vec<u8>, u16, u32, u32)> {
    let crc = crc32(data);
    if store {
        return Some((data.to_vec(), 0, crc, data.len() as u32));
    }
    let mut enc = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(data).ok()?;
    let payload = enc.finish().ok()?;
    let comp_size = payload.len() as u32;
    Some((payload, 8, crc, comp_size))
}

fn write_zip(entries: &[OutEntry]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut offsets = Vec::with_capacity(entries.len());
    for e in entries {
        if out.len() > u32::MAX as usize {
            return None;
        }
        offsets.push(out.len() as u32);
        out.extend_from_slice(LFH_SIG);
        out.extend_from_slice(&e.version_needed.to_le_bytes());
        out.extend_from_slice(&(e.flags & !FLAG_DATA_DESCRIPTOR).to_le_bytes());
        out.extend_from_slice(&e.method.to_le_bytes());
        out.extend_from_slice(&e.mod_time.to_le_bytes());
        out.extend_from_slice(&e.mod_date.to_le_bytes());
        out.extend_from_slice(&e.crc.to_le_bytes());
        out.extend_from_slice(&e.comp_size.to_le_bytes());
        out.extend_from_slice(&e.uncomp_size.to_le_bytes());
        out.extend_from_slice(&(e.name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // local extra dropped
        out.extend_from_slice(&e.name);
        out.extend_from_slice(&e.data);
    }

    if out.len() > u32::MAX as usize {
        return None;
    }
    let cd_start = out.len() as u32;
    for (e, off) in entries.iter().zip(&offsets) {
        out.extend_from_slice(CDFH_SIG);
        out.extend_from_slice(&e.version_made.to_le_bytes());
        out.extend_from_slice(&e.version_needed.to_le_bytes());
        out.extend_from_slice(&(e.flags & !FLAG_DATA_DESCRIPTOR).to_le_bytes());
        out.extend_from_slice(&e.method.to_le_bytes());
        out.extend_from_slice(&e.mod_time.to_le_bytes());
        out.extend_from_slice(&e.mod_date.to_le_bytes());
        out.extend_from_slice(&e.crc.to_le_bytes());
        out.extend_from_slice(&e.comp_size.to_le_bytes());
        out.extend_from_slice(&e.uncomp_size.to_le_bytes());
        out.extend_from_slice(&(e.name.len() as u16).to_le_bytes());
        out.extend_from_slice(&(e.central_extra.len() as u16).to_le_bytes());
        out.extend_from_slice(&(e.comment.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // disk start
        out.extend_from_slice(&e.internal_attrs.to_le_bytes());
        out.extend_from_slice(&e.external_attrs.to_le_bytes());
        out.extend_from_slice(&off.to_le_bytes());
        out.extend_from_slice(&e.name);
        out.extend_from_slice(&e.central_extra);
        out.extend_from_slice(&e.comment);
    }
    let cd_size = out.len() as u32 - cd_start;
    let count = entries.len() as u16;
    out.extend_from_slice(EOCD_SIG);
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_start.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    Some(out)
}

fn central_to_out(e: &CentralEntry, data: Vec<u8>) -> OutEntry {
    OutEntry {
        name: e.name.clone(),
        method: e.method,
        flags: e.flags,
        mod_time: e.mod_time,
        mod_date: e.mod_date,
        crc: e.crc,
        comp_size: e.comp_size,
        uncomp_size: e.uncomp_size,
        version_made: e.version_made,
        version_needed: e.version_needed,
        internal_attrs: e.internal_attrs,
        external_attrs: e.external_attrs,
        central_extra: e.central_extra.clone(),
        comment: e.comment.clone(),
        data,
    }
}

/// Rewrite a jar, applying `transform` to each embedded `.class` entry.
///
/// On a malformed or unsupported archive the input is returned unchanged
/// (`changed == false`) so a caller can fall back to copying.
pub fn harden_archive(data: &[u8], transform: &dyn Fn(&[u8]) -> Vec<u8>) -> (Vec<u8>, JarReport) {
    harden_entries(data, "class", transform)
}

/// Rewrite any ZIP container, applying `transform` to each entry whose name
/// ends with `.{ext}` (case-insensitive), then repacking. Works for `.jar`
/// (`class`) and `.apk` (`dex`). Same conservative fallbacks as
/// [`harden_archive`].
pub fn harden_entries(
    data: &[u8],
    ext: &str,
    transform: &dyn Fn(&[u8]) -> Vec<u8>,
) -> (Vec<u8>, JarReport) {
    let mut report = JarReport::default();
    let Some(entries) = read_central(data) else {
        return (data.to_vec(), report);
    };
    let suffix = format!(".{}", ext.to_ascii_lowercase());
    let mut out_entries = Vec::with_capacity(entries.len());
    for e in &entries {
        report.files_total += 1;
        let matches = String::from_utf8_lossy(&e.name)
            .to_ascii_lowercase()
            .ends_with(&suffix);
        if matches {
            report.entries_total += 1;
            let Some(raw) = compressed(data, e) else {
                return (data.to_vec(), JarReport::default());
            };
            let Some(plain) = decompress(e, raw) else {
                return (data.to_vec(), JarReport::default());
            };
            let hardened = transform(&plain);
            if hardened != plain {
                report.changed = true;
                report.entries_hardened += 1;
            }
            let Some((payload, method, crc, comp_size)) = encode(&hardened, e.method == 0) else {
                return (data.to_vec(), JarReport::default());
            };
            let mut out = central_to_out(e, payload);
            out.method = method;
            out.crc = crc;
            out.comp_size = comp_size;
            out.uncomp_size = hardened.len() as u32;
            out_entries.push(out);
            continue;
        }
        let Some(raw) = compressed(data, e) else {
            return (data.to_vec(), JarReport::default());
        };
        out_entries.push(central_to_out(e, raw.to_vec()));
    }

    if !report.changed {
        return (data.to_vec(), report);
    }
    match write_zip(&out_entries) {
        Some(bytes) => (bytes, report),
        None => (data.to_vec(), JarReport::default()),
    }
}

/// Decompressed bytes of every `.class` entry (for measurement). Bounded and
/// best-effort: an unsupported archive yields an empty vector.
pub fn class_entries(data: &[u8]) -> Vec<Vec<u8>> {
    let Some(entries) = read_central(data) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in &entries {
        if is_class(&e.name) {
            if let Some(raw) = compressed(data, e) {
                if let Some(bytes) = decompress(e, raw) {
                    out.push(bytes);
                }
            }
        }
    }
    out
}

/// Decompressed bytes of every entry whose name ends with `.{ext}` (case-
/// insensitive). Best-effort: an unsupported archive yields an empty vector.
/// Used to reach members of other containers (e.g. `classes*.dex` in an APK).
pub fn entries_with_ext(data: &[u8], ext: &str) -> Vec<Vec<u8>> {
    let Some(entries) = read_central(data) else {
        return Vec::new();
    };
    let suffix = format!(".{}", ext.to_ascii_lowercase());
    let mut out = Vec::new();
    for e in &entries {
        if String::from_utf8_lossy(&e.name)
            .to_ascii_lowercase()
            .ends_with(&suffix)
        {
            if let Some(raw) = compressed(data, e) {
                if let Some(bytes) = decompress(e, raw) {
                    out.push(bytes);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a ZIP from `(name, content)` pairs, optionally deflating.
    fn build_zip(pairs: &[(&str, &[u8])], deflate: bool) -> Vec<u8> {
        let mut out = Vec::new();
        let mut central = Vec::new();
        let mut count = 0u16;
        for (name, content) in pairs {
            let (payload, method) = if deflate {
                let mut enc =
                    flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
                enc.write_all(content).unwrap();
                (enc.finish().unwrap(), 8u16)
            } else {
                (content.to_vec(), 0u16)
            };
            let crc = crc32(content);
            let comp = payload.len() as u32;
            let local_off = out.len() as u32;
            out.extend_from_slice(LFH_SIG);
            out.extend_from_slice(&20u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&method.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&crc.to_le_bytes());
            out.extend_from_slice(&comp.to_le_bytes());
            out.extend_from_slice(&(content.len() as u32).to_le_bytes());
            out.extend_from_slice(&(name.len() as u16).to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(&payload);

            central.extend_from_slice(CDFH_SIG);
            central.extend_from_slice(&20u16.to_le_bytes());
            central.extend_from_slice(&20u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&method.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&crc.to_le_bytes());
            central.extend_from_slice(&comp.to_le_bytes());
            central.extend_from_slice(&(content.len() as u32).to_le_bytes());
            central.extend_from_slice(&(name.len() as u16).to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u32.to_le_bytes());
            central.extend_from_slice(&local_off.to_le_bytes());
            central.extend_from_slice(name.as_bytes());
            count += 1;
        }
        let cd_off = out.len() as u32;
        let cd_size = central.len() as u32;
        out.extend_from_slice(&central);
        out.extend_from_slice(EOCD_SIG);
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&cd_size.to_le_bytes());
        out.extend_from_slice(&cd_off.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out
    }

    fn class_fixture() -> Vec<u8> {
        let mut d = vec![0xCA, 0xFE, 0xBA, 0xBE, 0x00, 0x00, 0x00, 0x34];
        d.extend_from_slice(&2u16.to_be_bytes());
        d.push(0x01);
        d.extend_from_slice(&10u16.to_be_bytes());
        d.extend_from_slice(b"SourceFile");
        d
    }

    fn read_named(data: &[u8], name: &str) -> Vec<u8> {
        for e in read_central(data).unwrap() {
            if e.name == name.as_bytes() {
                return decompress(&e, compressed(data, &e).unwrap()).unwrap();
            }
        }
        panic!("entry {name} not found");
    }

    #[test]
    fn hardens_deflated_classes_and_repacks() {
        let class = class_fixture();
        let manifest = b"Manifest-Version: 1.0\n";
        let archive = build_zip(
            &[("META-INF/MANIFEST.MF", manifest), ("App.class", &class)],
            true,
        );
        assert!(!class_entries(&archive).is_empty());
        assert!(class_entries(&archive)[0]
            .windows(10)
            .any(|w| w == b"SourceFile"));

        let (new, rep) = harden_archive(&archive, &|bytes| {
            let mut v = bytes.to_vec();
            crate::java::strip_debug_attributes(&mut v);
            v
        });
        assert!(rep.changed);
        assert_eq!(rep.entries_total, 1);
        assert_eq!(rep.entries_hardened, 1);
        assert_eq!(rep.files_total, 2);

        // The class is rewritten; the manifest and other bytes are preserved.
        let hard = read_named(&new, "App.class");
        assert!(!hard.windows(10).any(|w| w == b"SourceFile"));
        assert_eq!(read_named(&new, "META-INF/MANIFEST.MF"), manifest);
        // Framing is well-formed and re-parseable.
        assert!(read_central(&new).is_some());
    }

    #[test]
    fn stored_classes_are_handled() {
        let class = class_fixture();
        let archive = build_zip(&[("App.class", &class)], false);
        let (new, rep) = harden_archive(&archive, &|bytes| {
            let mut v = bytes.to_vec();
            crate::java::strip_debug_attributes(&mut v);
            v
        });
        assert!(rep.changed);
        let hard = read_named(&new, "App.class");
        assert!(!hard.windows(10).any(|w| w == b"SourceFile"));
    }

    #[test]
    fn no_class_leaves_archive_unchanged() {
        let archive = build_zip(&[("readme.txt", b"hello world")], true);
        let (new, rep) = harden_archive(&archive, &|b| b.to_vec());
        assert!(!rep.changed);
        assert_eq!(new, archive);
    }

    #[test]
    fn malformed_archive_is_untouched() {
        let data = b"PK\x03\x04 not really a zip";
        let (out, rep) = harden_archive(data, &|b| b.to_vec());
        assert!(!rep.changed);
        assert_eq!(out, data);
        assert!(class_entries(data).is_empty());
    }

    #[test]
    fn extracts_entries_by_extension() {
        let archive = build_zip(
            &[
                ("classes.dex", b"dex\n035\0payload"),
                ("META-INF/MANIFEST.MF", b"Manifest-Version: 1.0\n"),
            ],
            true,
        );
        let dexes = entries_with_ext(&archive, "dex");
        assert_eq!(dexes.len(), 1);
        assert_eq!(dexes[0], b"dex\n035\0payload");
        assert!(entries_with_ext(&archive, "class").is_empty());
    }

    #[test]
    fn harden_entries_matches_extension() {
        let archive = build_zip(
            &[("classes.dex", b"dex\n035\0AAAA"), ("other.txt", b"x")],
            true,
        );
        let (new, rep) = harden_entries(&archive, "dex", &|b| {
            let mut v = b.to_vec();
            v.push(b'!');
            v
        });
        assert!(rep.changed);
        assert_eq!(rep.entries_total, 1);
        assert_eq!(rep.entries_hardened, 1);
        assert_eq!(read_named(&new, "classes.dex"), b"dex\n035\0AAAA!");
        assert_eq!(read_named(&new, "other.txt"), b"x");
    }
}
