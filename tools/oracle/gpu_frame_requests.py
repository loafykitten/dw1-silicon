#!/usr/bin/env python3
"""Requests for oracle/call-sdk.lua that redraw the polygons of one frame of the game in Redux.

Usage: gpu_frame_requests.py DUMP VRAM OUT_DIR [--clip-y0 N]

DUMP is the list of polygon packets of a native run (lines `P [w0, w1, ...] tp=.. ofs=x,y
clip=x0,y0,x1,y1 dith=.. mask=..`, one per polygon, in drawing order; a local debugging patch of
`Gpu::polygon` writes them), VRAM the native VRAM at that frame (1 MB, little endian). The last
batch of polygons with the given draw-area top (0, the upper frame buffer, by default) is taken.
The texture pages and CLUTs it uses are loaded into Redux at the same coordinates, the frame
buffer is cleared, the polygons are drawn in order and the frame buffer is read back in bands
of 40 rows. Writes OUT_DIR/req.txt and OUT_DIR/frame.txt (the polygon words with their draw
state, one per line, for `crates/backend/tests/gpu_frames.rs`, which also needs the VRAM file).
Run req.txt like the others (`oracle/call-sdk.lua`, results as OUT_DIR/results.txt).
"""
import os
import re
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from gpu_requests import Gen, chain_bytes, hexb, w_xy  # noqa: E402


def parse(dump, clip_y0):
    batches = []
    cur = []
    for l in open(dump):
        if not l.startswith("P "):
            continue
        words = [int(x, 16) for x in re.findall(r"\b[0-9a-f]{8}\b", l.split("tp=")[0])]
        st = dict(re.findall(r"(\w+)=(\S+)", "tp=" + l.split("tp=")[1]))
        ofs = tuple(map(int, st["ofs"].split(",")))
        clip = tuple(map(int, st["clip"].split(",")))
        dith = st["dith"] == "true"
        prim = (words, int(st["tp"], 16), ofs, clip, dith)
        if cur and cur[-1][3] != clip:
            batches.append(cur)
            cur = []
        cur.append(prim)
    batches.append(cur)
    return [b for b in batches if b[-1][3][1] == clip_y0][-1]


def main():
    dump, vramf, out = sys.argv[1:4]
    clip_y0 = int(sys.argv[sys.argv.index("--clip-y0") + 1]) if "--clip-y0" in sys.argv else 0
    batch = parse(dump, clip_y0)
    if "--prims" in sys.argv:
        a, b = map(int, sys.argv[sys.argv.index("--prims") + 1].split(":"))
        batch = batch[a:b]
    vram = struct.unpack("<%dH" % (1024 * 512), open(vramf, "rb").read())
    g = Gen(1)
    # textures and CLUTs used
    need = set()
    for words, tp, ofs, clip, dith in batch:
        op = words[0] >> 24
        if not op & 4:
            continue
        j = 1
        clut = tpage = None
        for k in range(4 if op & 8 else 3):
            if op & 0x10 and k > 0:
                j += 1
            j += 1
            if k == 0:
                clut = words[j] >> 16
            if k == 1:
                tpage = words[j] >> 16
            j += 1
        mode = (tpage >> 7) & 3
        mode = 2 if mode == 3 else mode
        w = [64, 128, 256][mode]
        need.add(("tex", (tpage & 15) * 64, ((tpage >> 4) & 1) * 256, w, 256))
        need.add(("clut", (clut & 63) * 16, clut >> 6, [16, 256, 0][mode], 1))
    packets = []
    size = 0

    def flush():
        nonlocal packets, size
        if packets:
            g.draw(packets)
        packets, size = [], 0

    # a transfer that runs past x = 1023 wraps in the model but not in Redux: send it in two parts
    split = set()
    for kind, x, y, w, h in need:
        if x + w > 1024:
            split |= {(kind, x, y, 1024 - x, h), (kind, 0, y, x + w - 1024, h)}
        else:
            split.add((kind, x, y, w, h))
    for kind, x, y, w, h in sorted(split):
        if w == 0:
            continue
        for y0 in range(0, h, 3):
            n = min(3, h - y0)
            words = [0xA0000000, w_xy(x, y + y0), w_xy(w, n)]
            px = [vram[((y + y0 + r) & 511) * 1024 + ((x + c) & 1023)] for r in range(n) for c in range(w)]
            if len(px) % 2:
                px.append(0)
            words += [px[i] | px[i + 1] << 16 for i in range(0, len(px), 2)]
            packets.append(words)
            size += 4 * (len(words) + 1)
            if size > 0x6000:
                flush()
    flush()
    # clear the frame buffer, then state and polygons
    chain = [[0x02000000, w_xy(0, clip_y0), w_xy(320, 240)]]
    state = None
    lines = ["Y %d" % clip_y0]
    for words, tp, ofs, clip, dith in batch:
        st = (ofs, clip, dith)
        if st != state:
            state = st
            e = [
                [0xE1000000 | (tp & 0x9FF) | (int(dith) << 9)],
                [0xE2000000],
                [0xE3000000 | clip[0] | (clip[1] << 10)],
                [0xE4000000 | clip[2] | (clip[3] << 10)],
                [0xE5000000 | (ofs[0] & 0x7FF) | ((ofs[1] & 0x7FF) << 11)],
            ]
            chain += e
            lines.append("E " + " ".join("%x" % p[0] for p in e))
        chain.append(words)
        lines.append("P " + " ".join("%x" % w for w in words))
    # draw in pieces that fit the scratch buffer (the draw state carries over between calls)
    piece, size = [], 0
    for p in chain:
        piece.append(p)
        size += 4 * (len(p) + 1)
        if size > 0x5000:
            g.draw(piece)
            piece, size = [], 0
    if piece:
        g.draw(piece)
    for band in range(6):
        rect = (0, clip_y0 + band * 40, 320, 40)
        g.req.append("StoreImage B+7F00 B+8000 0 0 in=B+7F00:%s" % hexb(struct.pack("<4h", *rect)))
        g.req.append("DrawSync 0 0 0 0 out=B+8000:%x" % (320 * 40 * 2))
    os.makedirs(out, exist_ok=True)
    open(os.path.join(out, "req.txt"), "w").write("\n".join(g.req) + "\n")
    open(os.path.join(out, "frame.txt"), "w").write("\n".join(lines) + "\n")
    print("%d polygons, %d requests" % (len(batch), len(g.req)))


if __name__ == "__main__":
    main()
