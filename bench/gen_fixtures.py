#!/usr/bin/env python3
"""Materialize the synthetic native bench fixtures (PE, Mach-O, jar, APK).

These mirror the minimal fixtures used by `native_post`'s unit tests: a PE32+
with a COFF symbol table, a debug directory and a `.pdb` path, and a thin 64-bit
Mach-O with a `__DWARF` segment. They are synthetic so the bench stays
toolchain-free; `bench/generate.sh` skips them when Python is unavailable. The
`.jar` fixture is a real deflated zip with a `.class` carrying debug attributes
(B-058) and the APK is a zip whose `classes.dex` carries debug info (B-042).
"""

import io
import json
import struct
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent


def pe_fixture() -> bytes:
    d = bytearray(0x600)
    d[0:2] = b"MZ"
    struct.pack_into("<I", d, 0x3C, 0x80)  # e_lfanew
    d[0x50:0x54] = b"Rich"  # Rich header fingerprint
    struct.pack_into("<I", d, 0x54, 0x11223344)
    d[0x80:0x84] = b"PE\0\0"

    coff = 0x84
    struct.pack_into("<H", d, coff, 0x8664)  # Machine
    struct.pack_into("<H", d, coff + 2, 1)  # NumberOfSections
    struct.pack_into("<I", d, coff + 4, 0x11223344)  # TimeDateStamp
    struct.pack_into("<I", d, coff + 8, 0x200)  # PointerToSymbolTable
    struct.pack_into("<I", d, coff + 12, 2)  # NumberOfSymbols
    struct.pack_into("<H", d, coff + 16, 0xF0)  # SizeOfOptionalHeader

    opt = coff + 20
    struct.pack_into("<H", d, opt, 0x20B)  # PE32+
    struct.pack_into("<I", d, opt + 108, 16)  # NumberOfRvaAndSizes
    debug_dir = opt + 112 + 6 * 8
    struct.pack_into("<I", d, debug_dir, 0x1000)  # RVA
    struct.pack_into("<I", d, debug_dir + 4, 28)  # Size

    sect = opt + 0xF0
    d[sect : sect + 8] = b".text\0\0\0"
    struct.pack_into("<I", d, sect + 8, 0x1000)  # VirtualSize
    struct.pack_into("<I", d, sect + 12, 0x1000)  # VirtualAddress
    struct.pack_into("<I", d, sect + 16, 0x200)  # SizeOfRawData
    struct.pack_into("<I", d, sect + 20, 0x400)  # PointerToRawData

    struct.pack_into("<I", d, 0x400 + 12, 2)  # Type = codeview
    struct.pack_into("<I", d, 0x400 + 16, 0x20)  # SizeOfData
    struct.pack_into("<I", d, 0x400 + 20, 0x1000)  # AddressOfRawData
    struct.pack_into("<I", d, 0x400 + 24, 0x420)  # PointerToRawData
    pdb = b"C:\\build\\app.pdb\0"
    d[0x420 : 0x420 + len(pdb)] = pdb
    return bytes(d)


def macho_fixture() -> bytes:
    d = bytearray(0x300)
    d[0:4] = bytes([0xCF, 0xFA, 0xED, 0xFE])  # MH_MAGIC_64 (LE)
    struct.pack_into("<I", d, 4, 0x01000007)  # cputype
    struct.pack_into("<I", d, 12, 2)  # filetype MH_EXECUTE
    struct.pack_into("<I", d, 16, 1)  # ncmds
    struct.pack_into("<I", d, 20, 72 + 80)  # sizeofcmds

    lc = 32
    struct.pack_into("<I", d, lc, 0x19)  # LC_SEGMENT_64
    struct.pack_into("<I", d, lc + 4, 72 + 80)  # cmdsize
    d[lc + 8 : lc + 24] = b"__DWARF\0\0\0\0\0\0\0\0\0"
    struct.pack_into("<I", d, lc + 64, 1)  # nsects

    s = lc + 72
    d[s : s + 16] = b"__debug_info\0\0\0\0"
    d[s + 16 : s + 32] = b"__DWARF\0\0\0\0\0\0\0\0\0"
    struct.pack_into("<Q", d, s + 40, 16)  # size
    struct.pack_into("<I", d, s + 48, 0x200)  # offset
    d[0x200:0x210] = b"DWARFDEBUGDATA!!"
    return bytes(d)


def asar_fixture() -> bytes:
    """A minimal valid asar archive with one JS file (no integrity field)."""
    files = {
        "index.js": b"var x=1;\n//# sourceMappingURL=index.js.map\nfunction s(a){return a*42+7;}\n",
        "package.json": b'{"name":"bench","version":"1.0.0","main":"index.js"}',
    }
    data = bytearray()
    entries = {}
    for name, content in files.items():
        offset = len(data)
        data += content
        entries[name] = {"size": len(content), "offset": str(offset)}
    json_bytes = json.dumps({"files": entries}, separators=(",", ":")).encode()
    pad = (4 - len(json_bytes) % 4) % 4
    payload_len = 4 + len(json_bytes) + pad
    header_buf_len = 4 + payload_len
    out = bytearray()
    out += struct.pack("<I", 4)
    out += struct.pack("<I", header_buf_len)
    out += struct.pack("<I", payload_len)
    out += struct.pack("<I", len(json_bytes))
    out += json_bytes + b"\0" * pad + bytes(data)
    return bytes(out)


def jar_fixture() -> bytes:
    """A real deflated jar with a `.class` carrying a debug attribute name."""
    cls = bytearray([0xCA, 0xFE, 0xBA, 0xBE, 0x00, 0x00, 0x00, 0x34])
    cls += struct.pack(">H", 2)  # constant_pool_count = 2
    cls += b"\x01" + struct.pack(">H", 10) + b"SourceFile"
    out = io.BytesIO()
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("META-INF/MANIFEST.MF", "Manifest-Version: 1.0\r\n\r\n")
        z.writestr("App.class", bytes(cls))
    return out.getvalue()


def dex_fixture() -> bytes:
    """A minimal DEX with one method carrying a `debug_info_off` pointer."""
    d = bytearray(192)
    d[0:8] = b"dex\n035\0"

    def put(off: int, v: int) -> None:
        struct.pack_into("<I", d, off, v)

    put(32, len(d))  # file_size
    put(36, 112)  # header_size
    put(96, 1)  # class_defs_size
    put(100, 112)  # class_defs_off
    put(112 + 16, 0xFFFFFFFF)  # source_file_idx = NO_INDEX
    put(112 + 24, 144)  # class_data_off
    d[146] = 1  # direct_methods_size
    d[148] = 0  # method_idx_diff
    d[149] = 1  # access_flags
    d[150] = 0xB0  # code_off = 176 (uleb)
    d[151] = 0x01
    put(176 + 8, 1)  # debug_info_off
    return bytes(d)


def apk_fixture() -> bytes:
    """A minimal APK (zip) whose `classes.dex` carries debug info."""
    out = io.BytesIO()
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("AndroidManifest.xml", b"<manifest/>")
        z.writestr("classes.dex", dex_fixture())
    return out.getvalue()


def main() -> None:
    for d in ("native-pe", "native-macho", "electron-asar", "java-jar", "android-apk"):
        (ROOT / d).mkdir(exist_ok=True)
    (ROOT / "native-pe" / "app.exe").write_bytes(pe_fixture())
    (ROOT / "native-macho" / "app").write_bytes(macho_fixture())
    (ROOT / "electron-asar" / "app.asar").write_bytes(asar_fixture())
    (ROOT / "java-jar" / "app.jar").write_bytes(jar_fixture())
    (ROOT / "android-apk" / "app.apk").write_bytes(apk_fixture())
    print("synthetic native/asar/jar/apk fixtures ready")


if __name__ == "__main__":
    main()
