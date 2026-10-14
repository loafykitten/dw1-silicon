#!/usr/bin/env python3
"""Requests for oracle/call-sdk.lua that redraw the game's own fade quad on the game's own picture.

Usage: gpu_snap_requests.py OUT_DIR

Run it with DW1_CALL_AT=1832 (the landing screen's fade, where the game's quad is on screen), so
the VRAM holds the game's textures and frame buffers. For every draw it reads back a window of
the frame buffer before and after, plus the few texels and CLUT entries the quad samples;
`crates/backend/tests/gpu_oracle.rs` loads that snapshot into the model, draws the same packet
and compares. Writes OUT_DIR/snap_req.txt and OUT_DIR/snap_cases.txt (one line per draw: its GP0
words and the window). Keep the emulator's results as OUT_DIR/snap_results.txt.

The quad is the one the game draws: 320 x 240 at the draw offset, flat colour, semi-transparent
(subtract), 4-bit texture whose coordinate moves only 2 steps across it.
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from gpu_requests import chain_bytes, hexb  # noqa: E402

WIN_W, WIN_H = 64, 96
WINDOWS = [(256, 0), (256, 240)]  # the two frame buffers (draw offset y 120 and 360)


def main():
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    req, cases = [], []
    spot = {"tex": 0x1D000, "clut": 0x1D100, "pre": 0x11000, "post": 0x15000}

    def store(rect, dest, length):
        req.append("StoreImage B+8000 B+%x 0 0 in=B+8000:%s" % (dest - 0x10000, hexb(struct.pack("<4h", *rect))))
        req.append("DrawSync 0 0 0 0 out=B+%x:%x" % (dest - 0x10000, length))

    store((952, 509, 8, 3), spot["tex"], 48)
    store((256, 502, 16, 1), spot["clut"], 32)
    for col in list(range(1, 0x50, 1)) + [0x80, 0xB0, 0xFF]:
        for wi, (wx, wy) in enumerate(WINDOWS[: 2 if col in (0x28, 0x2E) else 1]):
            oy = 120 + 240 * wi
            clip_y0 = 240 * wi
            words = [
                [0xE100008C],
                [0xE2000000],
                [0xE3000000 | 0 | (clip_y0 << 10)],
                [0xE4000000 | 319 | ((clip_y0 + 239) << 10)],
                [0xE5000000 | 160 | (oy << 11)],
                [0x2E000000 | col * 0x010101, 0xFF88FF60, 0x7D90FDFA, 0xFF8800A0, 0x005EFDFC,
                 0x0078FF60, 0x0000FFFA, 0x007800A0, 0x0000FFFC],
            ]
            store((wx, wy, WIN_W, WIN_H), spot["pre"], WIN_W * WIN_H * 2)
            req.append("DrawOTag B+0 0 0 0 in=B+0:%s" % hexb(chain_bytes(words, 0)))
            req.append("DrawSync 0 0 0 0")
            store((wx, wy, WIN_W, WIN_H), spot["post"], WIN_W * WIN_H * 2)
            flat = [w for p in words for w in p]
            cases.append("%d %d %s" % (wx, wy, " ".join("%x" % w for w in flat)))
    with open(os.path.join(out, "snap_req.txt"), "w") as f:
        f.write("\n".join(req) + "\n")
    with open(os.path.join(out, "snap_cases.txt"), "w") as f:
        f.write("\n".join(cases) + "\n")
    print("%d draws, %d requests" % (len(cases), len(req)))


if __name__ == "__main__":
    main()
