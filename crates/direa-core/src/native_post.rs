//! Built-in, in-process stripping of native artifacts (ELF, PE, Mach-O).
//!
//! This is the `native-post` adapter's engine. It is deliberately conservative:
//! it removes *symbol and debug metadata* by zeroing the owning structures and
//! scrubbing their names, and never rewrites code, relocations, or dynamic
//! symbol tables. The goal is to shrink the recoverable signal that a
//! decompiler, `nm`, or `strings` would read — not to change behaviour.
//!
//! Every operation is bounds-checked. On an unrecognised or unsupported layout
//! the function returns a report with `changed == false` rather than guessing.
//!
//! Limits (see ADR-0008): Mach-O DWARF sections and segment names are not yet
//! removed; PE resources and Mach-O fat containers are left intact.

use std::fmt;

/// Detected container format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NativeFormat {
    Elf,
    MachO,
    Pe,
    #[default]
    Unknown,
}

impl fmt::Display for NativeFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            NativeFormat::Elf => "elf",
            NativeFormat::MachO => "macho",
            NativeFormat::Pe => "pe",
            NativeFormat::Unknown => "unknown",
        };
        f.write_str(s)
    }
}

/// What a strip pass removed.
#[derive(Debug, Clone, Default)]
pub struct StripReport {
    pub format: NativeFormat,
    /// Names of sections / structures that were cleared.
    pub removed: Vec<String>,
    /// Approximate number of bytes overwritten with zeroes.
    pub bytes_zeroed: u64,
    /// Whether the buffer was modified.
    pub changed: bool,
}

impl StripReport {
    fn note(&mut self, name: impl Into<String>, bytes: u64) {
        self.removed.push(name.into());
        self.bytes_zeroed += bytes;
        self.changed = true;
    }
}

#[derive(Debug, Clone, Copy)]
enum Endian {
    Little,
    Big,
}

impl Endian {
    fn read16(self, d: &[u8], o: usize) -> Option<u16> {
        let b = d.get(o..o + 2)?;
        Some(match self {
            Endian::Little => u16::from_le_bytes([b[0], b[1]]),
            Endian::Big => u16::from_be_bytes([b[0], b[1]]),
        })
    }
    fn read32(self, d: &[u8], o: usize) -> Option<u32> {
        let b = d.get(o..o + 4)?;
        let a = [b[0], b[1], b[2], b[3]];
        Some(match self {
            Endian::Little => u32::from_le_bytes(a),
            Endian::Big => u32::from_be_bytes(a),
        })
    }
    fn read64(self, d: &[u8], o: usize) -> Option<u64> {
        let b = d.get(o..o + 8)?;
        let a = [b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]];
        Some(match self {
            Endian::Little => u64::from_le_bytes(a),
            Endian::Big => u64::from_be_bytes(a),
        })
    }
    fn write32(self, d: &mut [u8], o: usize, v: u32) {
        let bytes = match self {
            Endian::Little => v.to_le_bytes(),
            Endian::Big => v.to_be_bytes(),
        };
        if let Some(b) = d.get_mut(o..o + 4) {
            b.copy_from_slice(&bytes);
        }
    }
    fn write64(self, d: &mut [u8], o: usize, v: u64) {
        let bytes = match self {
            Endian::Little => v.to_le_bytes(),
            Endian::Big => v.to_be_bytes(),
        };
        if let Some(b) = d.get_mut(o..o + 8) {
            b.copy_from_slice(&bytes);
        }
    }
}

/// Zero `len` bytes at `off` if the range is within the buffer. Returns the
/// number of bytes actually zeroed.
fn zero_range(data: &mut [u8], off: u64, len: u64) -> u64 {
    let end = match off.checked_add(len) {
        Some(e) => e,
        None => return 0,
    };
    if len == 0 || end as usize > data.len() {
        return 0;
    }
    let (o, e) = (off as usize, end as usize);
    data[o..e].fill(0);
    len
}

fn detect(data: &[u8]) -> NativeFormat {
    if data.len() >= 4 && &data[0..4] == b"\x7fELF" {
        return NativeFormat::Elf;
    }
    if data.len() >= 2 && &data[0..2] == b"MZ" {
        return NativeFormat::Pe;
    }
    if data.len() >= 4 {
        let m = &data[0..4];
        let macho = matches!(
            m,
            [0xce, 0xfa, 0xed, 0xfe]
                | [0xfe, 0xed, 0xfa, 0xce]
                | [0xcf, 0xfa, 0xed, 0xfe]
                | [0xfe, 0xed, 0xfa, 0xcf]
                // Fat / universal containers.
                | [0xca, 0xfe, 0xba, 0xbe]
                | [0xbe, 0xba, 0xfe, 0xca]
                | [0xca, 0xfe, 0xba, 0xbf]
                | [0xbf, 0xba, 0xfe, 0xca]
        );
        if macho {
            return NativeFormat::MachO;
        }
    }
    NativeFormat::Unknown
}

/// Strip symbol/debug metadata from a native artifact in place.
pub fn strip_symbols(data: &mut [u8]) -> StripReport {
    let mut report = StripReport {
        format: detect(data),
        ..StripReport::default()
    };
    let result = match report.format {
        NativeFormat::Elf => strip_elf(data, &mut report),
        NativeFormat::Pe => strip_pe(data, &mut report),
        NativeFormat::MachO => strip_macho(data, &mut report),
        NativeFormat::Unknown => Ok(()),
    };
    // Go build metadata is format-agnostic (it can live in ELF, PE or Mach-O).
    strip_go_build_info(data, &mut report);
    if result.is_err() {
        // Unsupported layout: leave the buffer untouched and report no change.
        report.changed = false;
        report.removed.clear();
        report.bytes_zeroed = 0;
    }
    report
}

// ── ELF ──────────────────────────────────────────────────────────────────────

fn elf_is_target(name: &str) -> bool {
    matches!(
        name,
        ".symtab"
            | ".strtab"
            | ".symtab_shndx"
            | ".comment"
            | ".note.gnu.build-id"
            | ".gnu_debuglink"
            | ".gdb_index"
            | ".go.buildinfo"
            | ".note.go.buildid"
    ) || name.starts_with(".debug")
        || name.starts_with(".zdebug")
}

fn strip_elf(data: &mut [u8], report: &mut StripReport) -> Result<(), String> {
    if data.len() < 64 || &data[0..4] != b"\x7fELF" {
        return Err("not ELF".into());
    }
    let class64 = data[4] == 2;
    let endian = match data[5] {
        1 => Endian::Little,
        2 => Endian::Big,
        _ => return Err("bad EI_DATA".into()),
    };
    let e = endian;

    let (shoff, shentsize, shnum, shstrndx) = if class64 {
        (
            e.read64(data, 0x28).ok_or("e_shoff")?,
            e.read16(data, 0x3A).ok_or("e_shentsize")? as u64,
            e.read16(data, 0x3C).ok_or("e_shnum")? as u64,
            e.read16(data, 0x3E).ok_or("e_shstrndx")?,
        )
    } else {
        (
            e.read32(data, 0x20).ok_or("e_shoff")? as u64,
            e.read16(data, 0x2E).ok_or("e_shentsize")? as u64,
            e.read16(data, 0x30).ok_or("e_shnum")? as u64,
            e.read16(data, 0x32).ok_or("e_shstrndx")?,
        )
    };

    if shoff == 0 || shnum == 0 || shstrndx == 0 {
        return Ok(()); // nothing to strip (already minimal)
    }
    if shstrndx == 0xffff {
        return Err("extended section numbering unsupported".into());
    }

    // Field offsets within a section header.
    let (o_name, o_type, o_offset, o_size) = (
        0usize,
        4usize,
        if class64 { 24 } else { 16 },
        if class64 { 32 } else { 20 },
    );
    let need = shoff + shnum * shentsize;
    if need as usize > data.len() {
        return Err("section headers out of bounds".into());
    }
    let shdr_off = |idx: u64| (shoff + idx * shentsize) as usize;
    let read_u32_at = |d: &[u8], o: usize| e.read32(d, o);
    let read_u64_at = |d: &[u8], o: usize| e.read64(d, o);

    // Resolve the shstrtab so we can name sections.
    let str_off = shdr_off(shstrndx as u64);
    let (str_file, str_size) = if class64 {
        (
            read_u64_at(data, str_off + o_offset).ok_or("shstrtab offset")?,
            read_u64_at(data, str_off + o_size).ok_or("shstrtab size")?,
        )
    } else {
        (
            read_u32_at(data, str_off + o_offset).ok_or("shstrtab offset")? as u64,
            read_u32_at(data, str_off + o_size).ok_or("shstrtab size")? as u64,
        )
    };
    let shstrtab = data
        .get(str_file as usize..(str_file + str_size) as usize)
        .ok_or("shstrtab out of bounds")?
        .to_vec();

    let cstr = |off: u64| -> Option<String> {
        let start = off as usize;
        let bytes = shstrtab.get(start..)?;
        let end = bytes.iter().position(|&b| b == 0)?;
        String::from_utf8(bytes[..end].to_vec()).ok()
    };

    let mut removed_names: Vec<String> = Vec::new();
    let mut total_zeroed = 0u64;

    for idx in 0..shnum {
        let base = shdr_off(idx);
        let name_off = read_u32_at(data, base + o_name).ok_or("sh_name")? as u64;
        if name_off == 0 {
            continue;
        }
        let Some(name) = cstr(name_off) else { continue };
        if !elf_is_target(&name) {
            continue;
        }
        let (sec_off, sec_size) = if class64 {
            (
                read_u64_at(data, base + o_offset).unwrap_or(0),
                read_u64_at(data, base + o_size).unwrap_or(0),
            )
        } else {
            (
                read_u32_at(data, base + o_offset).unwrap_or(0) as u64,
                read_u32_at(data, base + o_size).unwrap_or(0) as u64,
            )
        };

        total_zeroed += zero_range(data, sec_off, sec_size);

        // Neutralise the section header: SHT_NULL, no name, no data.
        let _ = read_u32_at(data, base + o_type); // bounds guard
        e.write32(data, base + o_type, 0);
        if class64 {
            e.write64(data, base + o_offset, 0);
            e.write64(data, base + o_size, 0);
            e.write64(data, base + 16, 0); // sh_addr
        } else {
            e.write32(data, base + o_offset, 0);
            e.write32(data, base + o_size, 0);
            e.write32(data, base + 12, 0); // sh_addr
        }
        e.write32(data, base + o_name, 0);

        removed_names.push(name);
    }

    // Scrub the removed names from .shstrtab so the literals no longer appear.
    for name in &removed_names {
        let needle = name.as_bytes();
        scrub_all(data, needle, b' ');
    }

    if !removed_names.is_empty() {
        report.changed = true;
        report.bytes_zeroed += total_zeroed;
        for n in removed_names {
            report.removed.push(n);
        }
    }
    Ok(())
}

/// The Go build-info magic. External tools (and REA's Go provider) locate the
/// embedded compiler version, module list, replacements and build settings here.
const GO_BUILDINFO_MAGIC: &[u8] = b"\xff Go buildinf:";

fn find_sub(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Remove Go build metadata: the `.go.buildinfo` magic and the build-ID strings
/// `Go build ID: "..."`. The runtime does not need the magic (it uses
/// `runtime.modinfo`), so this hides the facts without changing behaviour.
fn strip_go_build_info(data: &mut [u8], report: &mut StripReport) {
    // Magic itself.
    if find_sub(data, GO_BUILDINFO_MAGIC).is_some() {
        scrub_all(data, GO_BUILDINFO_MAGIC, 0);
        report.note("go build info magic", GO_BUILDINFO_MAGIC.len() as u64);
    }
    // Build ID strings, both quote styles.
    for quote in [b'"', b'\''] {
        let mut needle = b"Go build ID: ".to_vec();
        needle.push(quote);
        let mut from = 0usize;
        while let Some(rel) = find_sub(&data[from..], &needle) {
            let start = from + rel;
            let id_start = start + needle.len();
            let Some(rel_end) = data[id_start..].iter().position(|&b| b == quote) else {
                break;
            };
            let end = id_start + rel_end + 1;
            for b in &mut data[start..end] {
                if *b != 0 {
                    *b = 0;
                }
            }
            report.note("go build ID", (end - start) as u64);
            from = end;
        }
    }
}

/// Overwrite every occurrence of `needle` in `data` with `fill`, preserving NUL
/// terminators. `fill` must not be a token byte (see `direa_core::scan`) so the
/// scrubbed name does not itself count as a recoverable token.
fn scrub_all(data: &mut [u8], needle: &[u8], fill: u8) {
    if needle.is_empty() || data.len() < needle.len() {
        return;
    }
    let mut i = 0;
    while i + needle.len() <= data.len() {
        if &data[i..i + needle.len()] == needle {
            for b in data[i..i + needle.len()].iter_mut() {
                if *b != 0 {
                    *b = fill;
                }
            }
            i += needle.len();
        } else {
            i += 1;
        }
    }
}

// ── PE ───────────────────────────────────────────────────────────────────────

fn strip_pe(data: &mut [u8], report: &mut StripReport) -> Result<(), String> {
    if data.len() < 0x40 || &data[0..2] != b"MZ" {
        return Err("not PE".into());
    }
    let e = Endian::Little;
    let pe_off = e.read32(data, 0x3C).ok_or("e_lfanew")? as usize;
    if data.get(pe_off..pe_off + 4) != Some(b"PE\0\0") {
        return Err("bad PE signature".into());
    }
    strip_pe_rich_header(data, pe_off, report);
    let coff = pe_off + 4;
    let timestamp_off = coff + 4;
    let ptr_sym_off = coff + 8;
    let num_sym_off = coff + 12;
    let size_opt_off = coff + 16;
    let num_sections = e.read16(data, coff + 2).ok_or("NumberOfSections")? as usize;
    let size_opt = e.read16(data, size_opt_off).ok_or("SizeOfOptionalHeader")? as usize;

    // COFF symbol table + build timestamp.
    let ptr_sym = e.read32(data, ptr_sym_off).ok_or("PointerToSymbolTable")? as u64;
    let num_sym = e.read32(data, num_sym_off).ok_or("NumberOfSymbols")? as u64;
    if ptr_sym != 0 && num_sym != 0 {
        let n = zero_range(data, ptr_sym, num_sym.saturating_mul(18));
        report.note("COFF symbol table", n);
        e.write32(data, ptr_sym_off, 0);
        e.write32(data, num_sym_off, 0);
    }
    if e.read32(data, timestamp_off).unwrap_or(0) != 0 {
        e.write32(data, timestamp_off, 0);
        report.note("COFF TimeDateStamp", 4);
    }

    // Optional header: locate data directories (debug = index 6).
    let opt = pe_off + 24;
    if size_opt < 4 {
        return Ok(());
    }
    let magic = e.read16(data, opt).ok_or("optional magic")?;
    let (num_dirs_off, dirs_off) = match magic {
        0x10b => (opt + 92, opt + 96),   // PE32
        0x20b => (opt + 108, opt + 112), // PE32+
        _ => return Ok(()),
    };
    let num_dirs = e.read16(data, num_dirs_off).unwrap_or(0) as usize;
    if num_dirs <= 6 {
        return Ok(());
    }
    let debug_dir_off = dirs_off + 6 * 8;
    let debug_rva = e.read32(data, debug_dir_off).unwrap_or(0);
    let debug_size = e.read32(data, debug_dir_off + 4).unwrap_or(0);

    if debug_rva != 0 && debug_size != 0 {
        if let Some(file_off) =
            rva_to_offset(data, e, pe_off, opt, size_opt, num_sections, debug_rva)
        {
            // Walk IMAGE_DEBUG_DIRECTORY entries and zero each entry plus its data.
            let count = debug_size as usize / 28;
            for i in 0..count {
                let ent = file_off + i * 28;
                if ent + 28 > data.len() {
                    break;
                }
                let size_of_data = e.read32(data, ent + 16).unwrap_or(0) as u64;
                let ptr_raw = e.read32(data, ent + 24).unwrap_or(0) as u64;
                let z = zero_range(data, ptr_raw, size_of_data);
                if z > 0 {
                    report.bytes_zeroed += z;
                }
                zero_range(data, ent as u64, 28);
                report.changed = true;
            }
            report.removed.push("debug directory".into());
        }
        e.write32(data, debug_dir_off, 0);
        e.write32(data, debug_dir_off + 4, 0);
        report.changed = true;
    }
    Ok(())
}

/// Zero the PE "Rich" header: the build-tool fingerprint that sits between the
/// DOS stub and the PE header. Resources are deliberately left intact — they are
/// needed at runtime (icons, manifests, version info) — so only the fingerprint
/// is removed.
fn strip_pe_rich_header(data: &mut [u8], pe_off: usize, report: &mut StripReport) {
    let limit = pe_off.min(data.len());
    if limit < 4 {
        return;
    }
    if let Some(pos) = find_sub(&data[..limit], b"Rich") {
        let n = zero_range(data, pos as u64, (limit - pos) as u64);
        if n > 0 {
            report.note("PE Rich header", n);
        }
    }
}

/// Map a PE RVA to a file offset using the section table.
fn rva_to_offset(
    data: &[u8],
    e: Endian,
    pe_off: usize,
    opt: usize,
    size_opt: usize,
    num_sections: usize,
    rva: u32,
) -> Option<usize> {
    let sect_off = opt + size_opt;
    for i in 0..num_sections {
        let s = sect_off + i * 40;
        let vsize = e.read32(data, s + 8)?;
        let vaddr = e.read32(data, s + 12)?;
        let raw_size = e.read32(data, s + 16)?;
        let ptr_raw = e.read32(data, s + 20)?;
        let span = vsize.max(raw_size);
        if rva >= vaddr && rva < vaddr.checked_add(span)? {
            let delta = rva - vaddr;
            if delta < raw_size {
                return Some((ptr_raw.checked_add(delta)?) as usize);
            }
        }
    }
    let _ = pe_off;
    None
}

// ── Mach-O ───────────────────────────────────────────────────────────────────

fn strip_macho(data: &mut [u8], report: &mut StripReport) -> Result<(), String> {
    if data.len() < 4 {
        return Err("not Mach-O".into());
    }
    // Fat / universal containers wrap thin Mach-O slices.
    match &data[0..4] {
        [0xca, 0xfe, 0xba, 0xbe] => strip_fat(data, report, Endian::Big, false),
        [0xbe, 0xba, 0xfe, 0xca] => strip_fat(data, report, Endian::Little, false),
        [0xca, 0xfe, 0xba, 0xbf] => strip_fat(data, report, Endian::Big, true),
        [0xbf, 0xba, 0xfe, 0xca] => strip_fat(data, report, Endian::Little, true),
        _ => strip_macho_thin(data, report),
    }
}

/// Strip every slice of a fat (universal) Mach-O container.
fn strip_fat(
    data: &mut [u8],
    report: &mut StripReport,
    e: Endian,
    is64: bool,
) -> Result<(), String> {
    let nfat = e.read32(data, 4).ok_or("fat nfat_arch")? as usize;
    let arch_size = if is64 { 32 } else { 20 };
    let mut any = false;
    for i in 0..nfat {
        let base = 8 + i * arch_size;
        let (off, size) = if is64 {
            (
                e.read64(data, base + 8).ok_or("fat offset")?,
                e.read64(data, base + 16).ok_or("fat size")?,
            )
        } else {
            (
                e.read32(data, base + 8).ok_or("fat offset")? as u64,
                e.read32(data, base + 12).ok_or("fat size")? as u64,
            )
        };
        let end = match off.checked_add(size) {
            Some(v) => v,
            None => continue,
        };
        if end as usize > data.len() {
            continue;
        }
        let slice = &mut data[off as usize..end as usize];
        if strip_macho_thin(slice, report).is_ok() {
            any = true;
        }
    }
    if any {
        Ok(())
    } else {
        Err("no Mach-O slices could be stripped".into())
    }
}

/// Strip one thin Mach-O image: LC_SYMTAB plus `__DWARF` segment contents.
fn strip_macho_thin(data: &mut [u8], report: &mut StripReport) -> Result<(), String> {
    if data.len() < 4 {
        return Err("not Mach-O".into());
    }
    let (endian, class64) = match &data[0..4] {
        [0xce, 0xfa, 0xed, 0xfe] => (Endian::Little, false),
        [0xfe, 0xed, 0xfa, 0xce] => (Endian::Big, false),
        [0xcf, 0xfa, 0xed, 0xfe] => (Endian::Little, true),
        [0xfe, 0xed, 0xfa, 0xcf] => (Endian::Big, true),
        _ => return Err("unsupported Mach-O slice magic".into()),
    };
    let e = endian;
    let header_size = if class64 { 32 } else { 28 };
    let ncmds = e.read32(data, 16).ok_or("ncmds")? as usize;
    let sizeofcmds = e.read32(data, 20).ok_or("sizeofcmds")? as u64;
    if header_size as u64 + sizeofcmds > data.len() as u64 {
        return Err("load commands out of bounds".into());
    }

    let nlist_size: u64 = if class64 { 16 } else { 12 };
    let mut off = header_size;
    for _ in 0..ncmds {
        let cmd = e.read32(data, off).ok_or("load command")?;
        let cmdsize = e.read32(data, off + 4).ok_or("cmdsize")? as usize;
        if cmdsize < 8 || off + cmdsize > data.len() {
            break;
        }
        match cmd {
            // LC_SYMTAB
            0x2 if cmdsize >= 24 => {
                let symoff = e.read32(data, off + 8).unwrap_or(0) as u64;
                let nsyms = e.read32(data, off + 12).unwrap_or(0) as u64;
                let stroff = e.read32(data, off + 16).unwrap_or(0) as u64;
                let strsize = e.read32(data, off + 20).unwrap_or(0) as u64;
                let z1 = zero_range(data, symoff, nsyms.saturating_mul(nlist_size));
                let z2 = zero_range(data, stroff, strsize);
                if nsyms > 0 || strsize > 0 {
                    e.write32(data, off + 12, 0);
                    e.write32(data, off + 20, 0);
                    report.removed.push("LC_SYMTAB".into());
                    report.bytes_zeroed += z1 + z2;
                    report.changed = true;
                }
            }
            // LC_SEGMENT_64 / LC_SEGMENT whose name is __DWARF
            0x19 if cmdsize >= 72 => strip_dwarf_segment(data, report, e, off, true),
            0x1 if cmdsize >= 56 => strip_dwarf_segment(data, report, e, off, false),
            _ => {}
        }
        off += cmdsize;
    }
    Ok(())
}

/// Zero the contents and names of a `__DWARF` segment's sections.
fn strip_dwarf_segment(
    data: &mut [u8],
    report: &mut StripReport,
    e: Endian,
    off: usize,
    is64: bool,
) {
    let segname = match data.get(off + 8..off + 24) {
        Some(s) => s,
        None => return,
    };
    if !segname.starts_with(b"__DWARF") {
        return;
    }
    let (nsects_off, sect_start, sect_size) = if is64 {
        (off + 64, off + 72, 80usize)
    } else {
        (off + 48, off + 56, 68usize)
    };
    let nsects = e.read32(data, nsects_off).unwrap_or(0) as usize;
    let mut s = sect_start;
    for _ in 0..nsects {
        if s + sect_size > data.len() {
            break;
        }
        if is64 {
            let size = e.read64(data, s + 40).unwrap_or(0);
            let soff = e.read32(data, s + 48).unwrap_or(0) as u64;
            zero_range(data, soff, size);
            e.write64(data, s + 40, 0);
            e.write32(data, s + 48, 0);
        } else {
            let size = e.read32(data, s + 36).unwrap_or(0) as u64;
            let soff = e.read32(data, s + 40).unwrap_or(0) as u64;
            zero_range(data, soff, size);
            e.write32(data, s + 36, 0);
            e.write32(data, s + 40, 0);
        }
        scrub_name_field(data, s, 16);
        scrub_name_field(data, s + 16, 16);
        s += sect_size;
    }
    scrub_name_field(data, off + 8, 16);
    report.note("Mach-O __DWARF", 1);
}

/// Overwrite a fixed-width name field with spaces (non-token, non-marker).
fn scrub_name_field(data: &mut [u8], off: usize, len: usize) {
    if let Some(field) = data.get_mut(off..off + len) {
        for b in field.iter_mut() {
            if *b != 0 {
                *b = b' ';
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── fixtures ──────────────────────────────────────────────────────────

    fn put16(d: &mut [u8], o: usize, v: u16) {
        d[o..o + 2].copy_from_slice(&v.to_le_bytes());
    }
    fn put32(d: &mut [u8], o: usize, v: u32) {
        d[o..o + 4].copy_from_slice(&v.to_le_bytes());
    }
    fn put64(d: &mut [u8], o: usize, v: u64) {
        d[o..o + 8].copy_from_slice(&v.to_le_bytes());
    }

    /// Minimal ELF64 LE with a NULL, .text, .symtab, .debug_info, .shstrtab.
    fn elf_fixture() -> Vec<u8> {
        let mut d = vec![0u8; 0x400];
        d[0..4].copy_from_slice(b"\x7fELF");
        d[4] = 2; // 64-bit
        d[5] = 1; // little-endian
        put16(&mut d, 0x10, 2); // e_type ET_EXEC
        put16(&mut d, 0x12, 0x3e); // e_machine
        put32(&mut d, 0x14, 1); // e_version
        put64(&mut d, 0x28, 0x100); // e_shoff
        put16(&mut d, 0x34, 64); // e_ehsize
        put16(&mut d, 0x3A, 64); // e_shentsize
        put16(&mut d, 0x3C, 5); // e_shnum
        put16(&mut d, 0x3E, 4); // e_shstrndx

        let sh = |i: usize| 0x100 + i * 64;
        // Section 1: .text
        put32(&mut d, sh(1), 1); // sh_name -> ".text"
        put32(&mut d, sh(1) + 4, 1); // SHT_PROGBITS
        put64(&mut d, sh(1) + 24, 0x300); // sh_offset
        put64(&mut d, sh(1) + 32, 4); // sh_size
                                      // Section 2: .symtab
        put32(&mut d, sh(2), 7); // ".symtab"
        put32(&mut d, sh(2) + 4, 2); // SHT_SYMTAB
        put64(&mut d, sh(2) + 24, 0x200);
        put64(&mut d, sh(2) + 32, 0x18);
        put32(&mut d, sh(2) + 40, 3); // sh_link -> .strtab (reuse shstrtab slot 4? keep 0)
                                      // Section 3: .debug_info
        put32(&mut d, sh(3), 15); // ".debug_info"
        put32(&mut d, sh(3) + 4, 1);
        put64(&mut d, sh(3) + 24, 0x220);
        put64(&mut d, sh(3) + 32, 0x10);
        // Section 4: .shstrtab
        let names = b"\0.text\0.symtab\0.debug_info\0.shstrtab\0";
        put32(&mut d, sh(4), 27); // ".shstrtab"
        put32(&mut d, sh(4) + 4, 3); // SHT_STRTAB
        put64(&mut d, sh(4) + 24, 0x280);
        put64(&mut d, sh(4) + 32, names.len() as u64);
        d[0x280..0x280 + names.len()].copy_from_slice(names);
        // A fake symbol name living in the symtab payload.
        d[0x200..0x203].copy_from_slice(b"foo");
        d
    }

    /// Minimal PE32+ with a COFF symbol table and a debug directory.
    fn pe_fixture() -> Vec<u8> {
        let mut d = vec![0u8; 0x600];
        d[0..2].copy_from_slice(b"MZ");
        put32(&mut d, 0x3C, 0x80); // e_lfanew
                                   // Rich header fingerprint between the DOS stub and the PE header.
        d[0x50..0x54].copy_from_slice(b"Rich");
        put32(&mut d, 0x54, 0x1122_3344);
        d[0x80..0x84].copy_from_slice(b"PE\0\0");
        let coff = 0x84;
        put16(&mut d, coff, 0x8664); // Machine
        put16(&mut d, coff + 2, 1); // NumberOfSections
        put32(&mut d, coff + 4, 0x11223344); // TimeDateStamp
        put32(&mut d, coff + 8, 0x200); // PointerToSymbolTable
        put32(&mut d, coff + 12, 2); // NumberOfSymbols
        put16(&mut d, coff + 16, 0xF0); // SizeOfOptionalHeader
        let opt = coff + 20; // 0x98
        put16(&mut d, opt, 0x20b); // PE32+
        put32(&mut d, opt + 108, 16); // NumberOfRvaAndSizes
        let debug_dir = opt + 112 + 6 * 8;
        put32(&mut d, debug_dir, 0x1000); // RVA
        put32(&mut d, debug_dir + 4, 28); // Size
                                          // One section: RVA 0x1000 -> raw 0x400
        let sect = opt + 0xF0;
        d[sect..sect + 8].copy_from_slice(b".text\0\0\0");
        put32(&mut d, sect + 8, 0x1000); // VirtualSize
        put32(&mut d, sect + 12, 0x1000); // VirtualAddress
        put32(&mut d, sect + 16, 0x200); // SizeOfRawData
        put32(&mut d, sect + 20, 0x400); // PointerToRawData
                                         // Debug directory at raw 0x400 (RVA 0x1000), one entry -> data at 0x420
        put32(&mut d, 0x400 + 12, 2); // Type = codeview
        put32(&mut d, 0x400 + 16, 0x20); // SizeOfData
        put32(&mut d, 0x400 + 20, 0x1000); // AddressOfRawData
        put32(&mut d, 0x400 + 24, 0x420); // PointerToRawData
        let pdb = b"C:\\build\\app.pdb\0";
        d[0x420..0x420 + pdb.len()].copy_from_slice(pdb);
        d
    }

    fn macho_fixture() -> Vec<u8> {
        let mut d = vec![0u8; 0x200];
        d[0..4].copy_from_slice(&[0xcf, 0xfa, 0xed, 0xfe]); // MH_MAGIC_64 LE
        put32(&mut d, 4, 0x0100_0007); // cputype
        put32(&mut d, 8, 3);
        put32(&mut d, 12, 2); // filetype MH_EXECUTE
        put32(&mut d, 16, 1); // ncmds
        put32(&mut d, 20, 24); // sizeofcmds
                               // LC_SYMTAB at 32
        let lc = 32;
        put32(&mut d, lc, 0x2);
        put32(&mut d, lc + 4, 24);
        put32(&mut d, lc + 8, 0x100); // symoff
        put32(&mut d, lc + 12, 2); // nsyms
        put32(&mut d, lc + 16, 0x140); // stroff
        put32(&mut d, lc + 20, 16); // strsize
        let strings = b"_main\0_secret\0";
        d[0x140..0x140 + strings.len()].copy_from_slice(strings);
        d
    }

    /// Thin 64-bit Mach-O with one `__DWARF` segment containing `__debug_info`.
    fn macho_dwarf_fixture() -> Vec<u8> {
        let mut d = vec![0u8; 0x300];
        d[0..4].copy_from_slice(&[0xcf, 0xfa, 0xed, 0xfe]); // MH_MAGIC_64 LE
        put32(&mut d, 4, 0x0100_0007);
        put32(&mut d, 12, 2); // MH_EXECUTE
        put32(&mut d, 16, 1); // ncmds
        put32(&mut d, 20, 72 + 80); // sizeofcmds
        let lc = 32;
        put32(&mut d, lc, 0x19); // LC_SEGMENT_64
        put32(&mut d, lc + 4, 72 + 80);
        d[lc + 8..lc + 24].copy_from_slice(b"__DWARF\0\0\0\0\0\0\0\0\0");
        put32(&mut d, lc + 64, 1); // nsects
        let s = lc + 72;
        d[s..s + 16].copy_from_slice(b"__debug_info\0\0\0\0");
        d[s + 16..s + 32].copy_from_slice(b"__DWARF\0\0\0\0\0\0\0\0\0");
        put64(&mut d, s + 40, 16); // size
        put32(&mut d, s + 48, 0x200); // offset
        d[0x200..0x210].copy_from_slice(b"DWARFDEBUGDATA!!");
        d
    }

    /// Fat (universal) container with a symtab slice and a DWARF slice.
    fn macho_fat_fixture() -> Vec<u8> {
        let slice1 = macho_fixture();
        let slice2 = macho_dwarf_fixture();
        let align = |n: usize| (n + 15) & !15;
        let off1 = align(8 + 2 * 20);
        let off2 = align(off1 + slice1.len());
        let total = off2 + slice2.len();
        let mut d = vec![0u8; total];
        d[0..4].copy_from_slice(&[0xca, 0xfe, 0xba, 0xbe]); // FAT_MAGIC (BE)
        d[4..8].copy_from_slice(&2u32.to_be_bytes()); // nfat_arch
        let arch = |d: &mut [u8], base: usize, off: usize, size: usize| {
            d[base..base + 4].copy_from_slice(&0x0100_0007u32.to_be_bytes());
            d[base + 4..base + 8].copy_from_slice(&3u32.to_be_bytes());
            d[base + 8..base + 12].copy_from_slice(&(off as u32).to_be_bytes());
            d[base + 12..base + 16].copy_from_slice(&(size as u32).to_be_bytes());
            d[base + 16..base + 20].copy_from_slice(&4u32.to_be_bytes());
        };
        arch(&mut d, 8, off1, slice1.len());
        arch(&mut d, 28, off2, slice2.len());
        d[off1..off1 + slice1.len()].copy_from_slice(&slice1);
        d[off2..off2 + slice2.len()].copy_from_slice(&slice2);
        d
    }

    // ── tests ─────────────────────────────────────────────────────────────

    #[test]
    fn detects_formats() {
        assert_eq!(detect(&elf_fixture()), NativeFormat::Elf);
        assert_eq!(detect(&pe_fixture()), NativeFormat::Pe);
        assert_eq!(detect(&macho_fixture()), NativeFormat::MachO);
        assert_eq!(detect(b"not a binary"), NativeFormat::Unknown);
    }

    #[test]
    fn elf_strips_symtab_and_debug() {
        let mut d = elf_fixture();
        assert!(find(&d, b".symtab").is_some());
        assert!(find(&d, b".debug_info").is_some());
        let r = strip_symbols(&mut d);
        assert_eq!(r.format, NativeFormat::Elf);
        assert!(r.changed);
        assert!(r.removed.iter().any(|s| s == ".symtab"));
        assert!(r.removed.iter().any(|s| s == ".debug_info"));
        assert!(
            find(&d, b".symtab").is_none(),
            "symtab literal must be scrubbed"
        );
        assert!(
            find(&d, b".debug_info").is_none(),
            "debug literal must be scrubbed"
        );
    }

    #[test]
    fn elf_leaves_dynamic_sections_alone() {
        let mut d = elf_fixture();
        // .text should survive.
        let r = strip_symbols(&mut d);
        assert!(r.changed);
        assert!(find(&d, b".text").is_some());
    }

    #[test]
    fn pe_clears_symbol_table_and_debug() {
        let mut d = pe_fixture();
        assert!(find(&d, b"app.pdb").is_some());
        let r = strip_symbols(&mut d);
        assert_eq!(r.format, NativeFormat::Pe);
        assert!(r.changed);
        assert!(find(&d, b"app.pdb").is_none(), "pdb path must be zeroed");
        // COFF symbol pointer cleared
        assert_eq!(u32::from_le_bytes(d[0x8C..0x90].try_into().unwrap()), 0);
        // Debug directory cleared
        assert_eq!(
            u32::from_le_bytes(d[0x98 + 112 + 6 * 8..][..4].try_into().unwrap()),
            0
        );
    }

    #[test]
    fn pe_rich_header_scrubbed() {
        let mut d = pe_fixture();
        assert!(find(&d, b"Rich").is_some());
        let r = strip_symbols(&mut d);
        assert!(r.removed.iter().any(|s| s.contains("Rich header")));
        assert!(find(&d, b"Rich").is_none());
    }

    #[test]
    fn macho_dwarf_is_scrubbed() {
        let mut d = macho_dwarf_fixture();
        assert!(find(&d, b"__DWARF").is_some());
        assert!(find(&d, b"__debug_info").is_some());
        let r = strip_symbols(&mut d);
        assert_eq!(r.format, NativeFormat::MachO);
        assert!(r.removed.iter().any(|s| s.contains("__DWARF")));
        assert!(find(&d, b"__DWARF").is_none());
        assert!(find(&d, b"__debug_info").is_none());
        assert!(
            find(&d, b"DWARFDEBUGDATA").is_none(),
            "dwarf data must be zeroed"
        );
    }

    #[test]
    fn macho_fat_slices_are_stripped() {
        let mut d = macho_fat_fixture();
        assert!(find(&d, b"_secret").is_some());
        assert!(find(&d, b"__DWARF").is_some());
        let r = strip_symbols(&mut d);
        assert_eq!(r.format, NativeFormat::MachO);
        assert!(r.changed);
        assert!(
            find(&d, b"_secret").is_none(),
            "symtab slice must be stripped"
        );
        assert!(
            find(&d, b"__DWARF").is_none(),
            "dwarf slice must be stripped"
        );
    }

    #[test]
    fn macho_clears_symtab() {
        let mut d = macho_fixture();
        assert!(find(&d, b"_secret").is_some());
        let r = strip_symbols(&mut d);
        assert_eq!(r.format, NativeFormat::MachO);
        assert!(r.changed);
        assert!(
            find(&d, b"_secret").is_none(),
            "string table must be zeroed"
        );
        assert_eq!(u32::from_le_bytes(d[44..48].try_into().unwrap()), 0); // nsyms
    }

    #[test]
    fn go_build_info_scrubbed() {
        let mut d = vec![0u8; 256];
        d[0x40..0x40 + 14].copy_from_slice(b"\xff Go buildinf:");
        let id = b"Go build ID: \"abc123-def456\"";
        d[0x80..0x80 + id.len()].copy_from_slice(id);
        let r = strip_symbols(&mut d);
        assert!(r.changed);
        assert!(r.removed.iter().any(|s| s.contains("go build info")));
        assert!(r.removed.iter().any(|s| s.contains("go build ID")));
        assert!(find(&d, b"Go buildinf").is_none());
        assert!(find(&d, b"abc123-def456").is_none());
    }

    #[test]
    fn non_go_binary_is_unaffected_by_go_scrub() {
        let mut d = elf_fixture();
        let before_len = d.len();
        let r = strip_symbols(&mut d);
        // ELF stripping still happens; no Go magic present, so no go note.
        assert!(!r.removed.iter().any(|s| s.contains("go build")));
        assert_eq!(d.len(), before_len);
    }

    #[test]
    fn unknown_is_untouched() {
        let mut d = b"hello world, not a binary at all".to_vec();
        let before = d.clone();
        let r = strip_symbols(&mut d);
        assert_eq!(r.format, NativeFormat::Unknown);
        assert!(!r.changed);
        assert_eq!(d, before);
    }

    #[test]
    fn tiny_truncated_input_does_not_panic() {
        // Truncated magic must not panic.
        let _ = strip_symbols(&mut b"\x7fELF".to_vec());
        let _ = strip_symbols(&mut b"MZ".to_vec());
        let _ = strip_symbols(&mut [0xcfu8, 0xfa, 0xed, 0xfe]);
        let empty: &mut [u8] = &mut [];
        let _ = strip_symbols(empty);
    }

    fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
        hay.windows(needle.len()).position(|w| w == needle)
    }
}
