#!/usr/bin/env python3
"""Materialize the synthetic native bench fixtures (PE, Mach-O).

These mirror the minimal fixtures used by `native_post`'s unit tests: a PE32+
with a COFF symbol table, a debug directory and a `.pdb` path, and a thin 64-bit
Mach-O with a `__DWARF` segment. They are synthetic so the bench stays
toolchain-free; `bench/generate.sh` skips them when Python is unavailable.
"""

import json
import struct
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


def main() -> None:
    for d in ("native-pe", "native-macho", "electron-asar"):
        (ROOT / d).mkdir(exist_ok=True)
    (ROOT / "native-pe" / "app.exe").write_bytes(pe_fixture())
    (ROOT / "native-macho" / "app").write_bytes(macho_fixture())
    (ROOT / "electron-asar" / "app.asar").write_bytes(asar_fixture())
    print("synthetic native/asar fixtures ready (PE, Mach-O, asar)")


if __name__ == "__main__":
    main()
