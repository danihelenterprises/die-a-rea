//! Toolchain-free post-processing for WebAssembly artifacts.
//!
//! Strips **custom sections** (including the `name` section, producers, and
//! source-map URLs) by emptying the section name and zeroing its payload while
//! keeping the section framing valid. Custom sections are non-semantic, so
//! behaviour is unchanged (ADR-0008 spirit).

use crate::native_post::{NativeFormat, StripReport};

/// Namespace prefix for custom sections die-a-rea injects. Such sections carry
/// agent-defense content, not debug metadata, so the `debug-info` probe ignores
/// them (see `direa_verify::probes`).
pub const SECTION_PREFIX: &str = "direa.";

/// Custom section carrying the honest authorization notice + canary tokens.
pub const NOTICE_SECTION: &str = "direa.notice";

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

/// Append a custom section (id 0) to a WASM module.
///
/// Custom sections are non-semantic and may appear after the last section, so
/// appending is behaviour-safe. Returns `false` if `data` is not a WASM module.
pub fn inject_custom_section(data: &mut Vec<u8>, name: &str, payload: &[u8]) -> bool {
    if data.len() < 8 || &data[0..4] != b"\0asm" {
        return false;
    }
    let mut content = Vec::with_capacity(name.len() + payload.len() + 8);
    write_leb(&mut content, name.len() as u64);
    content.extend_from_slice(name.as_bytes());
    content.extend_from_slice(payload);
    data.push(0); // custom-section id
    write_leb(data, content.len() as u64);
    data.extend_from_slice(&content);
    true
}

fn write_leb(out: &mut Vec<u8>, mut value: u64) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            return;
        }
    }
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

    #[test]
    fn injects_a_custom_section() {
        let mut d = b"\0asm\x01\0\0\0".to_vec();
        assert!(inject_custom_section(
            &mut d,
            NOTICE_SECTION,
            b"hello notice"
        ));
        assert_eq!(custom_section_names(&d), vec![NOTICE_SECTION.to_string()]);
    }

    #[test]
    fn injection_preserves_existing_sections() {
        let mut d = wasm_fixture(); // already has a "name" section
        assert!(inject_custom_section(&mut d, NOTICE_SECTION, b"x"));
        assert_eq!(
            custom_section_names(&d),
            vec!["name".to_string(), NOTICE_SECTION.to_string()]
        );
    }

    #[test]
    fn inject_on_non_wasm_is_false() {
        let mut d = b"nope".to_vec();
        assert!(!inject_custom_section(&mut d, NOTICE_SECTION, b"x"));
        assert_eq!(d, b"nope");
    }
}
