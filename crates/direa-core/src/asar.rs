//! Toolchain-free rewriting of Electron `.asar` archives.
//!
//! An asar archive is `[u32 4][u32 headerSize][header pickle][file data]`, where
//! the header pickle is `[u32 payloadLen][u32 jsonLen][json][padding]` and the
//! JSON maps a virtual tree to `{ size, offset }` entries into the data region.
//! [`harden_archive`] rewrites the embedded JS files in place (via a caller
//! transform) and rebuilds the framing with fresh offsets.
//!
//! Per-file `integrity` hashes are dropped on rewrite, since the content changes;
//! Electron only verifies them when asar integrity is explicitly fused on.

use serde_json::{Map, Value};

/// Outcome of hardening one archive.
#[derive(Debug, Default, Clone)]
pub struct AsarReport {
    pub changed: bool,
    /// JS files that were rewritten.
    pub files_hardened: usize,
    /// Files (of any type) that carry content in the archive.
    pub files_total: usize,
}

fn is_js(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".js") || lower.ends_with(".mjs") || lower.ends_with(".cjs")
}

/// Parse the framing: returns the header JSON bytes and the data-region offset.
fn read_header(data: &[u8]) -> Option<(&[u8], usize)> {
    if data.len() < 16 {
        return None;
    }
    if u32::from_le_bytes(data[0..4].try_into().ok()?) != 4 {
        return None;
    }
    let header_size = u32::from_le_bytes(data[4..8].try_into().ok()?) as usize;
    if 8 + header_size > data.len() {
        return None;
    }
    let header = &data[8..8 + header_size];
    if header.len() < 8 {
        return None;
    }
    let payload_len = u32::from_le_bytes(header[0..4].try_into().ok()?) as usize;
    let json_len = u32::from_le_bytes(header[4..8].try_into().ok()?) as usize;
    if payload_len < 4 + json_len || 8 + json_len > header.len() {
        return None;
    }
    Some((&header[8..8 + json_len], 8 + header_size))
}

/// Rebuild the framing around `json` and `data`.
fn frame(json: &[u8], data: &[u8]) -> Vec<u8> {
    let json_len = json.len();
    let pad = (4 - (json_len % 4)) % 4;
    let payload_len = 4 + json_len + pad;
    let header_buf_len = 4 + payload_len;
    let mut out = Vec::with_capacity(8 + header_buf_len + data.len());
    out.extend_from_slice(&4u32.to_le_bytes());
    out.extend_from_slice(&(header_buf_len as u32).to_le_bytes());
    out.extend_from_slice(&(payload_len as u32).to_le_bytes());
    out.extend_from_slice(&(json_len as u32).to_le_bytes());
    out.extend_from_slice(json);
    out.resize(out.len() + pad, 0);
    out.extend_from_slice(data);
    out
}

fn rewrite_dir(
    files: &mut Map<String, Value>,
    data_region: &[u8],
    transform: &dyn Fn(&str) -> String,
    out: &mut Vec<u8>,
    report: &mut AsarReport,
) -> bool {
    let mut changed = false;
    for (name, entry) in files.iter_mut() {
        let Some(obj) = entry.as_object_mut() else {
            continue;
        };
        if let Some(sub) = obj.get_mut("files").and_then(|v| v.as_object_mut()) {
            changed |= rewrite_dir(sub, data_region, transform, out, report);
            continue;
        }
        // File entry: needs an offset into the data region (unpacked files don't).
        let Some(size) = obj.get("size").and_then(|v| v.as_u64()) else {
            continue;
        };
        let Some(offset) = obj
            .get("offset")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<usize>().ok())
        else {
            continue;
        };
        let end = offset.saturating_add(size as usize);
        if end > data_region.len() {
            continue;
        }
        report.files_total += 1;
        let content = &data_region[offset..end];
        let mut new_content = content.to_vec();
        if is_js(name) {
            if let Ok(text) = std::str::from_utf8(content) {
                new_content = transform(text).into_bytes();
                report.files_hardened += 1;
                changed = true;
            }
        }
        let new_offset = out.len();
        out.extend_from_slice(&new_content);
        obj.insert("size".into(), Value::from(new_content.len() as u64));
        obj.insert("offset".into(), Value::from(new_offset.to_string()));
        obj.remove("integrity");
    }
    changed
}

/// Rewrite an asar archive, applying `transform` to each embedded JS file.
///
/// On a malformed archive the input is returned unchanged (`changed == false`),
/// so a caller can fall back to copying.
pub fn harden_archive(data: &[u8], transform: &dyn Fn(&str) -> String) -> (Vec<u8>, AsarReport) {
    let mut report = AsarReport::default();
    let Some((json, data_start)) = read_header(data) else {
        return (data.to_vec(), report);
    };
    let Ok(mut root) = serde_json::from_slice::<Value>(json) else {
        return (data.to_vec(), report);
    };
    let Some(files) = root.get_mut("files").and_then(|v| v.as_object_mut()) else {
        return (data.to_vec(), report);
    };

    let data_region = &data[data_start..];
    let mut out_region = Vec::new();
    report.changed = rewrite_dir(files, data_region, transform, &mut out_region, &mut report);

    if !report.changed {
        return (data.to_vec(), report);
    }
    let Ok(json_bytes) = serde_json::to_vec(&root) else {
        return (data.to_vec(), AsarReport::default());
    };
    (frame(&json_bytes, &out_region), report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_asar(files: &[(&str, &str)]) -> Vec<u8> {
        let mut data = Vec::new();
        let mut entries = Map::new();
        for (name, content) in files {
            let offset = data.len();
            data.extend_from_slice(content.as_bytes());
            let mut e = Map::new();
            e.insert("size".into(), Value::from(content.len() as u64));
            e.insert("offset".into(), Value::from(offset.to_string()));
            entries.insert((*name).into(), Value::Object(e));
        }
        let mut root = Map::new();
        root.insert("files".into(), Value::Object(entries));
        let json = serde_json::to_vec(&Value::Object(root)).unwrap();
        frame(&json, &data)
    }

    fn read_file(data: &[u8], name: &str) -> Option<String> {
        let (json, start) = read_header(data)?;
        let root: Value = serde_json::from_slice(json).ok()?;
        let entry = root.get("files")?.get(name)?;
        let size = entry.get("size")?.as_u64()? as usize;
        let offset = entry.get("offset")?.as_str()?.parse::<usize>().ok()?;
        let region = &data[start..];
        let bytes = region.get(offset..offset + size)?;
        String::from_utf8(bytes.to_vec()).ok()
    }

    #[test]
    fn hardens_embedded_js_and_repacks() {
        let mut archive = synthetic_asar(&[
            ("index.js", "var a=1; // secret\nconsole.log(a);\n"),
            ("package.json", "{\"main\":\"index.js\"}"),
        ]);
        assert!(read_file(&archive, "index.js").unwrap().contains("secret"));

        let before_len = archive.len();
        let (new, report) = harden_archive(&archive, &|text| {
            text.replace("secret", "REDACTED").replace("\n", "\n/*x*/")
        });
        assert!(report.changed);
        assert_eq!(report.files_hardened, 1);
        assert_eq!(report.files_total, 2);

        let js = read_file(&new, "index.js").unwrap();
        assert!(js.contains("REDACTED") && !js.contains("secret"));
        // package.json is not JS and must be untouched.
        assert_eq!(
            read_file(&new, "package.json").unwrap(),
            "{\"main\":\"index.js\"}"
        );
        // Framing is well-formed and the archive changed size.
        assert!(read_header(&new).is_some());
        assert_ne!(before_len, new.len());

        // Idempotent shape: hardening the result still parses.
        let (again, _) = harden_archive(&new, &|t| t.to_string());
        assert!(read_header(&again).is_some());
        let _ = std::mem::take(&mut archive);
    }

    #[test]
    fn drops_integrity_field() {
        let mut root = Map::new();
        let mut e = Map::new();
        e.insert("size".into(), Value::from(3u64));
        e.insert("offset".into(), Value::from("0"));
        e.insert("integrity".into(), Value::from("stale-hash"));
        let mut files = Map::new();
        files.insert("a.js".into(), Value::Object(e));
        root.insert("files".into(), Value::Object(files));
        let json = serde_json::to_vec(&Value::Object(root)).unwrap();
        let archive = frame(&json, b"abc");

        let (new, report) = harden_archive(&archive, &|t| format!("{t}{t}"));
        assert!(report.changed);
        let (json, _) = read_header(&new).unwrap();
        let text = String::from_utf8(json.to_vec()).unwrap();
        assert!(
            !text.contains("integrity"),
            "stale integrity must be dropped"
        );
    }

    #[test]
    fn malformed_archive_is_untouched() {
        let data = b"not an asar archive at all";
        let (out, report) = harden_archive(data, &|t| t.to_string());
        assert!(!report.changed);
        assert_eq!(out, data);
    }
}
