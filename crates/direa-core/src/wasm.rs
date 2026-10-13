//! Toolchain-free post-processing for WebAssembly artifacts.
//!
//! Strips **custom sections** (including the `name` section, producers, and
//! source-map URLs) by emptying the section name and zeroing its payload while
//! keeping the section framing valid. Custom sections are non-semantic, so
//! behaviour is unchanged (ADR-0008 spirit).

use crate::native_post::{NativeFormat, StripReport};

/// Zero the contents of every WASM custom section.
pub fn strip_custom_sections(data: &mut [u8]) -> StripReport {
    let mut report = StripReport {
        format: NativeFormat::Wasm,
        ..StripReport::default()
    };
    if data.len() < 8 || &data[0..4] != b"\0asm" {
        return report;
    }
    let mut pos = 8usize;
    while pos < data.len() {
        let id = data[pos];
        pos += 1;
        let Some((size, n)) = read_leb(data, pos) else {
            break;
        };
        pos += n;
        let end = pos.saturating_add(size as usize);
        if end > data.len() {
            break;
        }
        if id == 0 && pos < end {
            // Custom section content = name (vec) + payload.
            data[pos] = 0; // empty name
            for byte in &mut data[pos + 1..end] {
                *byte = 0;
            }
            report.note("wasm custom section", (end - pos) as u64);
        }
        pos = end;
    }
    report
}

/// Names of the custom sections (id 0) present in a WASM module. Empty names
/// (e.g. a section already stripped) are omitted.
///
/// Used by the `debug-info` probe so `wasm-post` wins are measurable for `.wasm`
/// targets instead of silently skipped (B-047, K-004).
pub fn custom_section_names(data: &[u8]) -> Vec<String> {
    let mut names = Vec::new();
    if data.len() < 8 || &data[0..4] != b"\0asm" {
        return names;
    }
    let mut pos = 8usize;
    while pos < data.len() {
        let id = data[pos];
        pos += 1;
        let Some((size, n)) = read_leb(data, pos) else {
            break;
        };
        pos += n;
        let end = pos.saturating_add(size as usize);
        if end > data.len() {
            break;
        }
        if id == 0 && pos < end {
            if let Some((nlen, nbytes)) = read_leb(data, pos) {
                let start = pos + nbytes;
                let stop = start.saturating_add(nlen as usize);
                if stop <= end {
                    if let Ok(name) = std::str::from_utf8(&data[start..stop]) {
                        if !name.is_empty() {
                            names.push(name.to_string());
                        }
                    }
                }
            }
        }
        pos = end;
    }
    names
}

fn read_leb(data: &[u8], mut pos: usize) -> Option<(u64, usize)> {
    let start = pos;
    let mut result: u64 = 0;
    let mut shift: u32 = 0;
    loop {
        let byte = *data.get(pos)?;
        pos += 1;
        result |= ((byte & 0x7f) as u64) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift >= 64 {
            return None;
        }
    }
    Some((result, pos - start))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wasm_fixture() -> Vec<u8> {
        let mut d = b"\0asm\x01\0\0\0".to_vec();
        // Custom section "name" with payload "abc".
        d.push(0x00); // id
        d.push(0x08); // size
        d.push(0x04); // name length
        d.extend_from_slice(b"name");
        d.extend_from_slice(b"abc");
        d
    }

    #[test]
    fn strips_custom_sections() {
        let mut d = wasm_fixture();
        assert!(d.windows(4).any(|w| w == b"name"));
        let r = strip_custom_sections(&mut d);
        assert_eq!(r.format, NativeFormat::Wasm);
        assert!(r.changed);
        assert!(
            !d.windows(4).any(|w| w == b"name"),
            "name section must be cleared"
        );
        assert!(!d.windows(3).any(|w| w == b"abc"));
    }

    #[test]
    fn custom_section_names_reads_and_clears() {
        let mut d = wasm_fixture();
        assert_eq!(custom_section_names(&d), vec!["name".to_string()]);
        strip_custom_sections(&mut d);
        assert!(
            custom_section_names(&d).is_empty(),
            "after stripping, no named custom section should remain"
        );
    }

    #[test]
    fn non_wasm_is_untouched() {
        let mut d = b"not wasm at all".to_vec();
        let before = d.clone();
        let r = strip_custom_sections(&mut d);
        assert!(!r.changed);
        assert_eq!(d, before);
    }
}
