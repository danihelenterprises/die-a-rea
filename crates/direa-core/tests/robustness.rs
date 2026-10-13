//! Robustness: the binary parsers must never panic and must fall back
//! conservatively on malformed input (K-011). The corpus is deterministic:
//! truncations, single-byte corruptions and random bytes, all fed through every
//! parser in `direa_core`.

use direa_core::Rng;

/// Feed one input to every parser. A panic fails the test; a parser that
/// reports "not changed" must leave the buffer untouched.
fn run_all(data: &[u8]) {
    // DEX
    let _ = direa_core::dex::measure_debug_info(data);
    let mut v = data.to_vec();
    if !direa_core::dex::strip_debug_info(&mut v) {
        assert_eq!(v, data, "dex::strip_debug_info mutated without reporting");
    }

    // WASM
    let _ = direa_core::wasm::custom_section_names(data);
    let mut v = data.to_vec();
    if !direa_core::wasm::strip_custom_sections(&mut v).changed {
        assert_eq!(v, data, "wasm::strip_custom_sections mutated silently");
    }
    let mut v = data.to_vec();
    let _ = direa_core::wasm::inject_custom_section(&mut v, "direa.robustness", b"x");

    // Java `.class`
    let mut v = data.to_vec();
    if !direa_core::java::strip_debug_attributes(&mut v).changed {
        assert_eq!(v, data, "java::strip_debug_attributes mutated silently");
    }

    // Native
    let mut v = data.to_vec();
    if !direa_core::native_post::strip_symbols(&mut v).changed {
        assert_eq!(v, data, "native_post::strip_symbols mutated silently");
    }

    // asar
    let (out, rep) = direa_core::asar::harden_archive(data, &|t| t.to_string());
    if !rep.changed {
        assert_eq!(out, data, "asar::harden_archive changed without reporting");
    }

    // ZIP containers
    let (out, rep) = direa_core::jar::harden_archive(data, &|b| b.to_vec());
    if !rep.changed {
        assert_eq!(out, data, "jar::harden_archive changed without reporting");
    }
    let _ = direa_core::jar::class_entries(data);
    let _ = direa_core::jar::entries_with_ext(data, "dex");
    let _ = direa_core::jar::add_entry(data, "direa-robustness.txt", b"x");
}

fn wasm_fixture() -> Vec<u8> {
    let mut d = b"\0asm\x01\0\0\0".to_vec();
    d.extend_from_slice(b"\x00\x08\x04nameabc");
    d
}

fn dex_fixture() -> Vec<u8> {
    let mut d = vec![0u8; 192];
    d[0..8].copy_from_slice(b"dex\n035\0");
    let put = |d: &mut [u8], off: usize, v: u32| d[off..off + 4].copy_from_slice(&v.to_le_bytes());
    let len = d.len() as u32;
    put(&mut d, 32, len);
    put(&mut d, 36, 112);
    put(&mut d, 96, 1);
    put(&mut d, 100, 112);
    put(&mut d, 112 + 16, 0xFFFF_FFFF);
    put(&mut d, 112 + 24, 144);
    d[146] = 1;
    d[150] = 0xB0;
    d[151] = 0x01;
    put(&mut d, 176 + 8, 1);
    d
}

/// A minimal stored ZIP with one entry.
fn zip_fixture(name: &[u8], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let local_off = 0u32;
    out.extend_from_slice(b"PK\x03\x04");
    out.extend_from_slice(&20u16.to_le_bytes()); // version
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
    cd.extend_from_slice(&20u16.to_le_bytes()); // made by
    cd.extend_from_slice(&20u16.to_le_bytes()); // needed
    cd.extend_from_slice(&0u16.to_le_bytes()); // flags
    cd.extend_from_slice(&0u16.to_le_bytes()); // method
    cd.extend_from_slice(&0u16.to_le_bytes()); // time
    cd.extend_from_slice(&0u16.to_le_bytes()); // date
    cd.extend_from_slice(&0u32.to_le_bytes()); // crc
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

fn asar_fixture() -> Vec<u8> {
    let js = b"var x=1;\n";
    let json = format!(
        "{{\"files\":{{\"index.js\":{{\"size\":{},\"offset\":\"0\"}}}}}}",
        js.len()
    );
    let json = json.into_bytes();
    let pad = (4 - json.len() % 4) % 4;
    let payload_len = 4 + json.len() + pad;
    let header_buf_len = 4 + payload_len;
    let mut out = Vec::new();
    out.extend_from_slice(&4u32.to_le_bytes());
    out.extend_from_slice(&(header_buf_len as u32).to_le_bytes());
    out.extend_from_slice(&(payload_len as u32).to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(&json);
    out.resize(out.len() + pad, 0);
    out.extend_from_slice(js);
    out
}

fn fixtures() -> Vec<Vec<u8>> {
    vec![
        wasm_fixture(),
        dex_fixture(),
        zip_fixture(b"App.class", &[0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 0x34]),
        asar_fixture(),
        b"\x7fELF\x02\x01\x01\x00.debug_info\x00.symtab".to_vec(),
        b"MZ\x90\x00with .pdb visible".to_vec(),
    ]
}

#[test]
fn parsers_never_panic_on_truncations() {
    for f in fixtures() {
        run_all(&f); // the intact fixture
        for cut in 0..f.len() {
            run_all(&f[..cut]);
        }
    }
}

#[test]
fn parsers_never_panic_on_corruption() {
    for f in fixtures() {
        for i in 0..f.len() {
            let mut v = f.clone();
            v[i] ^= 0xFF;
            run_all(&v);
        }
    }
}

#[test]
fn parsers_never_panic_on_random_bytes() {
    let mut rng = Rng::new(0xC0FFEE);
    for _ in 0..4000 {
        let len = rng.below(512) as usize;
        let mut v = Vec::with_capacity(len);
        for _ in 0..len {
            v.push((rng.next_u64() & 0xff) as u8);
        }
        // Occasionally prepend a real magic so the parser goes deep.
        if rng.bool(0.5) {
            let mut prefixed = b"\x7fELF".to_vec();
            prefixed.extend_from_slice(&v);
            run_all(&prefixed);
        } else {
            run_all(&v);
        }
    }
}
