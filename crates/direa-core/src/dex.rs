//! Minimal DEX (Dalvik executable) reader for measuring and stripping debug
//! information.
//!
//! There is no stable byte marker for DEX debug info, so the probe must walk
//! the structure: header → `class_def_item`s → `class_data_item` → each
//! method's `code_item`, where `debug_info_off != 0` marks a method with a
//! line-number/`LocalVariableTable` program. The class's `source_file_idx`
//! (the `SourceFile` equivalent) is counted too. [`measure_debug_info`] only
//! reads; [`strip_debug_info`] nulls those pointers in place and recomputes the
//! Adler-32 header checksum. Both bail on anything they do not understand. DEX
//! is little-endian.

/// `NO_INDEX` sentinel for an absent `source_file_idx`.
const NO_INDEX: u32 = 0xFFFF_FFFF;

/// What a DEX file exposes for the debug-metadata axis.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DexReport {
    pub classes: usize,
    /// Methods that carry a `code_item`.
    pub methods_with_code: usize,
    /// Methods whose `code_item.debug_info_off != 0`.
    pub methods_with_debug_info: usize,
    /// Classes whose `source_file_idx` is not `NO_INDEX`.
    pub classes_with_source_file: usize,
}

impl DexReport {
    /// True when any recoverable debug metadata is present.
    pub fn has_debug_info(&self) -> bool {
        self.methods_with_debug_info > 0 || self.classes_with_source_file > 0
    }
}

/// A scan result: the report plus the byte offsets to clear when stripping.
struct Scan {
    report: DexReport,
    /// `(offset, value)` of each 4-byte pointer to overwrite.
    clear: Vec<(usize, u32)>,
}

fn u32le(data: &[u8], off: usize) -> Option<u32> {
    data.get(off..off + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// Read an unsigned LEB128 value, bounded by the buffer.
fn read_uleb(data: &[u8], pos: &mut usize) -> Option<u32> {
    let mut result: u32 = 0;
    let mut shift: u32 = 0;
    loop {
        let byte = *data.get(*pos)?;
        *pos += 1;
        result |= ((byte & 0x7f) as u32) << shift;
        if byte & 0x80 == 0 {
            return Some(result);
        }
        shift += 7;
        if shift > 28 {
            return None; // malformed / overlong
        }
    }
}

/// Walk a DEX file, recording debug pointers and offsets to clear.
///
/// Returns `None` when `data` is not a DEX file or is truncated/malformed.
fn scan(data: &[u8]) -> Option<Scan> {
    if data.len() < 112 || &data[0..4] != b"dex\n" {
        return None;
    }
    let class_defs_size = u32le(data, 96)? as usize;
    let class_defs_off = u32le(data, 100)? as usize;
    let end = class_defs_off.checked_add(class_defs_size.checked_mul(32)?)?;
    if end > data.len() {
        return None;
    }

    let mut report = DexReport::default();
    let mut clear = Vec::new();
    for i in 0..class_defs_size {
        let cd = class_defs_off + i * 32;
        report.classes += 1;
        if u32le(data, cd + 16)? != NO_INDEX {
            report.classes_with_source_file += 1;
            clear.push((cd + 16, NO_INDEX));
        }
        let class_data_off = u32le(data, cd + 24)? as usize;
        if class_data_off == 0 {
            continue;
        }
        let mut pos = class_data_off;
        let static_fields = read_uleb(data, &mut pos)?;
        let instance_fields = read_uleb(data, &mut pos)?;
        let direct_methods = read_uleb(data, &mut pos)?;
        let virtual_methods = read_uleb(data, &mut pos)?;
        for _ in 0..static_fields.saturating_add(instance_fields) {
            read_uleb(data, &mut pos)?; // field_idx_diff
            read_uleb(data, &mut pos)?; // access_flags
        }
        for count in [direct_methods, virtual_methods] {
            for _ in 0..count {
                read_uleb(data, &mut pos)?; // method_idx_diff
                read_uleb(data, &mut pos)?; // access_flags
                let code_off = read_uleb(data, &mut pos)? as usize;
                if code_off != 0 {
                    report.methods_with_code += 1;
                    if u32le(data, code_off + 8)? != 0 {
                        report.methods_with_debug_info += 1;
                        clear.push((code_off + 8, 0));
                    }
                }
            }
        }
    }
    Some(Scan { report, clear })
}

/// Parse a DEX file and report its debug information.
///
/// Returns `None` when `data` is not a DEX file or is truncated/malformed.
pub fn measure_debug_info(data: &[u8]) -> Option<DexReport> {
    scan(data).map(|s| s.report)
}

/// Strip DEX debug metadata in place: null every method's `debug_info_off` and
/// reset every class's `source_file_idx` to `NO_INDEX`, then recompute the DEX
/// Adler-32 checksum. Returns `true` if anything changed.
///
/// Behavior-safe: ART treats absent debug info as optional. The DEX `signature`
/// (SHA-1) is left as-is — ART does not verify it at runtime, and any APK edit
/// already invalidates the APK signature, so the APK must be re-signed anyway.
pub fn strip_debug_info(data: &mut [u8]) -> bool {
    let Some(scan) = scan(data) else {
        return false;
    };
    if scan.clear.is_empty() {
        return false;
    }
    for (off, value) in &scan.clear {
        if let Some(slot) = data.get_mut(*off..*off + 4) {
            slot.copy_from_slice(&value.to_le_bytes());
        }
    }
    let checksum = adler32(&data[12..]);
    if let Some(slot) = data.get_mut(8..12) {
        slot.copy_from_slice(&checksum.to_le_bytes());
    }
    true
}

/// Adler-32, as used by the DEX header checksum.
fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in data {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put_u32(d: &mut [u8], off: usize, v: u32) {
        d[off..off + 4].copy_from_slice(&v.to_le_bytes());
    }

    fn uleb(mut v: u32) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let mut byte = (v & 0x7f) as u8;
            v >>= 7;
            if v != 0 {
                byte |= 0x80;
            }
            out.push(byte);
            if v == 0 {
                return out;
            }
        }
    }

    /// A minimal DEX: header, one class_def, one direct method, one code_item.
    /// `source_file_idx` and the method's `debug_info_off` are configurable.
    fn dex_fixture(debug_info_off: u32, source_file_idx: u32) -> Vec<u8> {
        let mut d = vec![0u8; 192];
        d[0..8].copy_from_slice(b"dex\n035\0");
        let len = d.len() as u32;
        put_u32(&mut d, 32, len); // file_size
        put_u32(&mut d, 36, 112); // header_size
        put_u32(&mut d, 40, 0x1234_5678); // endian_tag
        put_u32(&mut d, 96, 1); // class_defs_size
        put_u32(&mut d, 100, 112); // class_defs_off

        let cd = 112;
        put_u32(&mut d, cd + 4, 1); // access_flags
        put_u32(&mut d, cd + 8, NO_INDEX); // superclass_idx
        put_u32(&mut d, cd + 16, source_file_idx);
        put_u32(&mut d, cd + 24, 144); // class_data_off

        d[144] = 0; // static_fields_size
        d[145] = 0; // instance_fields_size
        d[146] = 1; // direct_methods_size
        d[147] = 0; // virtual_methods_size
        d[148] = 0; // method_idx_diff
        d[149] = 1; // access_flags
        let code = uleb(176);
        d[150..150 + code.len()].copy_from_slice(&code); // code_off

        put_u32(&mut d, 176 + 8, debug_info_off); // debug_info_off
        put_u32(&mut d, 176 + 12, 0); // insns_size
        d
    }

    #[test]
    fn measures_method_debug_info() {
        let r = measure_debug_info(&dex_fixture(1, NO_INDEX)).unwrap();
        assert_eq!(r.classes, 1);
        assert_eq!(r.methods_with_code, 1);
        assert_eq!(r.methods_with_debug_info, 1);
        assert!(r.has_debug_info());
    }

    #[test]
    fn no_debug_info_when_offsets_are_empty() {
        let r = measure_debug_info(&dex_fixture(0, NO_INDEX)).unwrap();
        assert_eq!(r.methods_with_debug_info, 0);
        assert_eq!(r.classes_with_source_file, 0);
        assert!(!r.has_debug_info());
    }

    #[test]
    fn counts_source_file_index() {
        let r = measure_debug_info(&dex_fixture(0, 0)).unwrap();
        assert_eq!(r.classes_with_source_file, 1);
        assert!(r.has_debug_info());
    }

    #[test]
    fn non_dex_is_none() {
        assert!(measure_debug_info(b"not a dex file at all").is_none());
        assert!(measure_debug_info(&[]).is_none());
    }

    #[test]
    fn truncated_dex_is_none() {
        let mut d = dex_fixture(1, NO_INDEX);
        d.truncate(120); // header claims a class that is no longer there
        assert!(measure_debug_info(&d).is_none());
    }

    #[test]
    fn strip_clears_pointers_and_recomputes_checksum() {
        let mut d = dex_fixture(1, 0); // method debug + source file
        assert!(measure_debug_info(&d).unwrap().has_debug_info());
        assert!(strip_debug_info(&mut d));
        let r = measure_debug_info(&d).unwrap();
        assert!(!r.has_debug_info());
        assert_eq!(r.methods_with_debug_info, 0);
        assert_eq!(r.classes_with_source_file, 0);
        // The header checksum now matches the body.
        let expect = adler32(&d[12..]);
        assert_eq!(u32le(&d, 8).unwrap(), expect);
    }

    #[test]
    fn strip_is_a_noop_without_debug_info() {
        let mut d = dex_fixture(0, NO_INDEX);
        assert!(!strip_debug_info(&mut d));
        assert!(!strip_debug_info(&mut b"not a dex".to_vec()));
    }

    #[test]
    fn adler32_known_vector() {
        assert_eq!(adler32(b"abc"), 0x024D_0127);
    }
}
