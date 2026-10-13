#!/usr/bin/env python3
"""Compare a native screenshot with an emulator one at the PS1's colour depth (15-bit), since
the two expand 5-bit channels to 8 bits differently.

Usage: compare_frames.py NATIVE.png REFERENCE.(png|raw) [--diff OUT.png]

A Redux .raw (with its .txt: "width height bpp") is read directly, else a PNG. Prints the
size, the number of differing pixels and the bounding box of the differences; --diff writes
an image with differing pixels in magenta over a dimmed copy of the reference. Exit status 0
when identical, 1 when not, 2 on errors.
"""
import struct
import sys
import zlib
from pathlib import Path


def read_png(path):
    data = Path(path).read_bytes()
    if data[:8] != b'\x89PNG\r\n\x1a\n':
        raise ValueError(f'{path}: not a PNG')
    pos, idat, w = 8, b'', None
    while pos < len(data):
        n, tag = struct.unpack('>I4s', data[pos:pos + 8])
        body = data[pos + 8:pos + 8 + n]
        if tag == b'IHDR':
            w, h, depth, ctype = struct.unpack('>IIBB', body[:10])
            if depth != 8 or ctype not in (2, 6):
                raise ValueError(f'{path}: only 8-bit RGB/RGBA PNGs')
            bpp = 3 if ctype == 2 else 4
        elif tag == b'IDAT':
            idat += body
        pos += 12 + n
    raw = zlib.decompress(idat)
    stride = w * bpp
    rows, prev = [], bytearray(stride)
    i = 0
    for _ in range(h):
        ftype = raw[i]
        line = bytearray(raw[i + 1:i + 1 + stride])
        i += 1 + stride
        for x in range(stride):
            a = line[x - bpp] if x >= bpp else 0
            b = prev[x]
            c = prev[x - bpp] if x >= bpp else 0
            if ftype == 1:
                line[x] = (line[x] + a) & 255
            elif ftype == 2:
                line[x] = (line[x] + b) & 255
            elif ftype == 3:
                line[x] = (line[x] + (a + b) // 2) & 255
            elif ftype == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[x] = (line[x] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        rows.append(bytes(line))
        prev = line
    px = [[tuple(r[x * bpp:x * bpp + 3]) for x in range(w)] for r in rows]
    return w, h, px


def read_raw(path):
    w, h, bpp = map(int, Path(path).with_suffix('.txt').read_text().split())
    data = Path(path).read_bytes()
    px = []
    for y in range(h):
        row = []
        for x in range(w):
            if bpp == 0:
                c = struct.unpack_from('<H', data, (y * w + x) * 2)[0]
                row.append(((c & 31) << 3, ((c >> 5) & 31) << 3, ((c >> 10) & 31) << 3))
            else:
                o = (y * w + x) * 3
                row.append(tuple(data[o:o + 3]))
        px.append(row)
    return w, h, px


def load(path):
    return read_raw(path) if str(path).endswith('.raw') else read_png(path)


def write_png(path, w, h, px):
    def chunk(tag, data):
        return struct.pack('>I', len(data)) + tag + data + struct.pack('>I', zlib.crc32(tag + data))
    raw = b''.join(b'\0' + bytes(c for p in row for c in p) for row in px)
    Path(path).write_bytes(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 2, 0, 0, 0))
                           + chunk(b'IDAT', zlib.compress(raw, 6)) + chunk(b'IEND', b''))


def main(argv):
    if len(argv) < 3:
        print(__doc__)
        return 2
    native, ref = argv[1], argv[2]
    diff_out = argv[argv.index('--diff') + 1] if '--diff' in argv else None
    wa, ha, a = load(native)
    wb, hb, b = load(ref)
    if (wa, ha) != (wb, hb):
        print(f'size differs: native {wa}x{ha}, reference {wb}x{hb}')
        return 1
    diffs, box = 0, None
    out = []
    for y in range(ha):
        row = []
        for x in range(wa):
            pa = tuple(c >> 3 for c in a[y][x])
            pb = tuple(c >> 3 for c in b[y][x])
            if pa != pb:
                diffs += 1
                box = (x, y, x, y) if box is None else (min(box[0], x), min(box[1], y), max(box[2], x), max(box[3], y))
                row.append((255, 0, 255))
            else:
                row.append(tuple(c // 3 for c in b[y][x]))
        out.append(row)
    print(f'{wa}x{ha}: {diffs} differing pixels' + (f', box {box}' if box else ''))
    if diff_out:
        write_png(diff_out, wa, ha, out)
    return 0 if diffs == 0 else 1


if __name__ == '__main__':
    sys.exit(main(sys.argv))
