#!/usr/bin/env python3
"""Convert PCSX-Redux Lua screenshots (oracle/*.lua writes <name>.raw + <name>.txt with
"width height bpp", bpp being PCSX-Redux's ScreenShotBPP enum: 0 = 16-bit BGR555, 1 = 24-bit RGB)
to PNG, with only the standard library.

Usage: raw2png.py DIR   (converts every .raw in DIR)
"""
import struct
import sys
import zlib
from pathlib import Path


def png(path: Path, w: int, h: int, rows: list[bytes]) -> None:
    def chunk(tag: bytes, data: bytes) -> bytes:
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data))

    raw = b"".join(b"\0" + r for r in rows)
    path.write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
                     + chunk(b"IDAT", zlib.compress(raw, 6)) + chunk(b"IEND", b""))


def convert(raw_path: Path) -> None:
    w, h, bpp = map(int, raw_path.with_suffix(".txt").read_text().split())
    data = raw_path.read_bytes()
    rows = []
    if bpp == 0:
        for y in range(h):
            row = bytearray()
            for x in range(w):
                (p,) = struct.unpack_from("<H", data, (y * w + x) * 2)
                for c in (p & 31, (p >> 5) & 31, (p >> 10) & 31):
                    row.append((c << 3) | (c >> 2))
            rows.append(bytes(row))
    else:
        rows = [data[y * w * 3:(y + 1) * w * 3] for y in range(h)]
    png(raw_path.with_suffix(".png"), w, h, rows)


if __name__ == "__main__":
    for p in sorted(Path(sys.argv[1]).glob("*.raw")):
        convert(p)
        print(p.with_suffix(".png"))
