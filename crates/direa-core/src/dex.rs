//! Minimal DEX (Dalvik executable) reader for measuring debug information.
//!
//! There is no stable byte marker for DEX debug info, so the probe must walk
//! the structure: header → `class_def_item`s → `class_data_item` → each
//! method's `code_item`, where `debug_info_off != 0` marks a method with a
//! line-number/`LocalVariableTable` program. The class's `source_file_idx`
//! (the `SourceFile` equivalent) is counted too. This is a *measurement* (and
//! the foundation a future APK strip in B-042 builds on); it never mutates and
//! bails on anything it does not understand. DEX is little-endian.

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

/// Parse a DEX file and report its debug information.
///
/// Returns `None` when `data` is not a DEX file or is truncated/malformed.
pub fn measure_debug_info(data: &[u8]) -> Option<DexReport> {
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
    for i in 0..class_defs_size {
        let cd = class_defs_off + i * 32;
        report.classes += 1;
        if u32le(data, cd + 16)? != NO_INDEX {
            report.classes_with_source_file += 1;
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
                    }
                }
            }
        }
    }
    Some(report)
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
}
