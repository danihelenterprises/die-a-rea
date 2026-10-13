//! Toolchain-free post-processing for Java `.class` files.
//!
//! Zeroes the constant-pool `Utf8` entries that name **debug-only** attributes
//! (`SourceFile`, `LineNumberTable`, `LocalVariableTable`, ...). The JVM skips
//! attributes whose name is unknown, so the attributes become inert while the
//! class stays loadable. Reflection-sensitive attributes (`Signature`,
//! `MethodParameters`) are deliberately left intact.

use crate::native_post::{NativeFormat, StripReport};

/// Debug-only attribute names whose constant-pool strings are cleared.
const DEBUG_ATTRS: &[&[u8]] = &[
    b"SourceFile",
    b"SourceDebugExtension",
    b"LineNumberTable",
    b"LocalVariableTable",
    b"LocalVariableTypeTable",
];

/// Clear the constant-pool names of debug-only attributes.
pub fn strip_debug_attributes(data: &mut [u8]) -> StripReport {
    let mut report = StripReport {
        format: NativeFormat::JavaClass,
        ..StripReport::default()
    };
    if data.len() < 10 || data[0..4] != [0xCA, 0xFE, 0xBA, 0xBE] {
        return report;
    }
    let cp_count = u16::from_be_bytes([data[8], data[9]]) as usize;
    let mut pos = 10usize;
    let mut utf8: Vec<(usize, usize)> = Vec::new();
    let mut i = 1usize;
    while i < cp_count {
        let Some(&tag) = data.get(pos) else {
            return report;
        };
        pos += 1;
        let step: usize = match tag {
            1 => {
                let len = match data.get(pos..pos + 2) {
                    Some(b) => u16::from_be_bytes([b[0], b[1]]) as usize,
                    None => return report,
                };
                pos += 2;
                utf8.push((pos, len));
                len
            }
            3 | 4 => 4,
            5 | 6 => {
                i += 1;
                8
            }
            7 | 8 | 16 | 19 | 20 => 2,
            9 | 10 | 11 | 12 | 17 | 18 => 4,
            15 => 3,
            _ => return report,
        };
        pos += step;
        if pos > data.len() {
            return report;
        }
        i += 1;
    }

    for (off, len) in utf8 {
        if off + len > data.len() {
            continue;
        }
        if DEBUG_ATTRS.iter().any(|a| data[off..off + len] == **a) {
            for byte in &mut data[off..off + len] {
                *byte = 0;
            }
            report.note("java debug attribute", len as u64);
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    fn class_fixture() -> Vec<u8> {
        let mut d = vec![0xCA, 0xFE, 0xBA, 0xBE, 0x00, 0x00, 0x00, 0x34];
        d.extend_from_slice(&2u16.to_be_bytes()); // constant_pool_count = 2
        d.push(0x01); // Utf8
        d.extend_from_slice(&10u16.to_be_bytes());
        d.extend_from_slice(b"SourceFile");
        d
    }

    #[test]
    fn strips_debug_attribute_names() {
        let mut d = class_fixture();
        assert!(d.windows(10).any(|w| w == b"SourceFile"));
        let r = strip_debug_attributes(&mut d);
        assert_eq!(r.format, NativeFormat::JavaClass);
        assert!(r.changed);
        assert!(!d.windows(10).any(|w| w == b"SourceFile"));
    }

    #[test]
    fn non_class_is_untouched() {
        let mut d = b"not a class".to_vec();
        let before = d.clone();
        assert!(!strip_debug_attributes(&mut d).changed);
        assert_eq!(d, before);
    }
}
