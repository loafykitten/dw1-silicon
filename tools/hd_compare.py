#!/usr/bin/env python3
"""Compares headless HD shots with their 1x pictures: for each `shot_<vblank>.png` that has a
`shot_<vblank>_1x.png` next to it (DW1_SCALE above 1, DW1_SHOTS_1X=1), counts the 1x pixels whose
S x S HD block's first pixel differs. 0 is expected on 2D-only frames (the HD path is the 1x
picture, sharper); 3D frames differ along polygon edges and where the HD rasterizer rounds
differently.

    .venv/bin/python tools/hd_compare.py DIR [--blocks]

--blocks also counts pixels where the block is not uniform (the HD picture has more detail than
the 1x one: edges of 3D polygons, lines).
Needs pypng (in .venv).
"""
import glob
import os
import re
import sys

import png


def rows(path):
    w, h, data, _ = png.Reader(filename=path).asRGB8()
    return w, h, [bytes(r) for r in data]


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    if len(args) != 1:
        sys.exit(__doc__)
    blocks = "--blocks" in sys.argv
    total_bad = total_px = 0
    for one in sorted(glob.glob(os.path.join(args[0], "shot_*_1x.png"))):
        hd = one.replace("_1x.png", ".png")
        if not os.path.exists(hd):
            continue
        vb = int(re.search(r"shot_(\d+)_1x", one).group(1))
        w, h, r1 = rows(one)
        W, H, rh = rows(hd)
        s = W // w
        if (W, H) != (w * s, h * s):
            print(f"{vb}: size mismatch {w}x{h} vs {W}x{H}")
            continue
        bad = nonuni = 0
        for y in range(h):
            a = r1[y]
            b = rh[y * s]
            for x in range(w):
                if a[3 * x : 3 * x + 3] != b[3 * s * x : 3 * s * x + 3]:
                    bad += 1
        if blocks:
            for y in range(h):
                for k in range(s):
                    b = rh[y * s + k]
                    for x in range(w):
                        px = b[3 * s * x : 3 * s * x + 3]
                        if any(b[3 * (s * x + j) : 3 * (s * x + j) + 3] != px for j in range(1, s)):
                            nonuni += 1
                            break
        colours = len({a[3 * x : 3 * x + 3] for a in r1 for x in range(w)})
        total_bad += bad
        total_px += w * h
        extra = f", {nonuni} block rows not uniform" if blocks else ""
        print(f"vblank {vb:5d}  {w}x{h} at {s}x: {bad:6d} of {w * h} first pixels differ, {colours} colours{extra}")
    print(f"total {total_bad} of {total_px}")


if __name__ == "__main__":
    main()
