#!/usr/bin/env python3
"""Draws the app's icon, packaging/icon-1024.png: an original pixel-art egg on a dark rounded
square (no game art). Standard library only, so anyone can redraw it:

    python3 tools/make-icon.py

tools/make-app.sh turns it into the .icns the app uses.
"""
import math
import os
import struct
import zlib

SIZE = 1024
# macOS icon grid: the body is 824 px with ~186 px corners, centred
BODY, RADIUS = 824, 186
# the egg is drawn on a grid of big pixels
CELL = 27
GW, GH = 18, 22

TOP, BOTTOM = (28, 62, 88), (10, 22, 38)
SHELL, SHELL_SHADE, SHELL_LIGHT = (244, 236, 214), (214, 200, 170), (255, 252, 240)
OUTLINE = (40, 32, 30)
SPOT, SPOT_SHADE = (86, 186, 116), (60, 146, 92)
GROUND = (6, 14, 24)


def in_body(x, y):
    """Inside the rounded square (a superellipse-ish rounded rect)."""
    m = (SIZE - BODY) / 2
    lx, ly = x - m, y - m
    if not (0 <= lx < BODY and 0 <= ly < BODY):
        return False
    cx = min(max(lx, RADIUS), BODY - RADIUS)
    cy = min(max(ly, RADIUS), BODY - RADIUS)
    return (lx - cx) ** 2 + (ly - cy) ** 2 <= RADIUS ** 2


def egg_cell(i, j):
    """Grid cell (i, j) is inside the egg: wider below its middle, narrower on top."""
    x = (i + 0.5 - GW / 2) / (GW / 2)
    y = (j + 0.5 - GH / 2) / (GH / 2)
    width = 0.80 + 0.17 * y  # the bottom is broader
    return (x / width) ** 2 + y ** 2 <= 1.0


SPOTS = [(5.5, 7.5, 2.1), (12.0, 11.0, 2.6), (6.5, 15.0, 1.7), (12.5, 4.5, 1.2)]


def egg_colour(i, j):
    inside = egg_cell(i, j)
    if not inside:
        return None
    edge = any(
        not egg_cell(i + di, j + dj) for di, dj in ((1, 0), (-1, 0), (0, 1), (0, -1))
    )
    if edge:
        return OUTLINE
    shade = i >= GW * 0.62 or j >= GH * 0.8
    for sx, sy, r in SPOTS:
        if (i + 0.5 - sx) ** 2 + (j + 0.5 - sy) ** 2 <= r * r:
            return SPOT_SHADE if shade else SPOT
    if (i - 5) ** 2 + (j - 4) ** 2 <= 3:
        return SHELL_LIGHT
    return SHELL_SHADE if shade else SHELL


def main():
    ox = (SIZE - GW * CELL) // 2
    oy = (SIZE - GH * CELL) // 2 - 20
    rows = []
    for y in range(SIZE):
        row = bytearray()
        for x in range(SIZE):
            if not in_body(x, y):
                row += b"\0\0\0\0"
                continue
            t = (y - (SIZE - BODY) / 2) / BODY
            c = tuple(round(a + (b - a) * t) for a, b in zip(TOP, BOTTOM))
            # a soft shadow under the egg
            dx = (x - SIZE / 2) / (GW * CELL * 0.42)
            dy = (y - (oy + GH * CELL + 6)) / 34
            if dx * dx + dy * dy <= 1:
                k = 0.55 * (1 - math.sqrt(dx * dx + dy * dy))
                c = tuple(round(a + (b - a) * k) for a, b in zip(c, GROUND))
            i, j = (x - ox) // CELL, (y - oy) // CELL
            if 0 <= i < GW and 0 <= j < GH and x >= ox and y >= oy:
                e = egg_colour(i, j)
                if e is not None:
                    c = e
            row += bytes(c) + b"\xff"
        rows.append(bytes(row))
    raw = b"".join(b"\0" + r for r in rows)

    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))

    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(raw, 9))
    png += chunk(b"IEND", b"")
    out = os.path.join(os.path.dirname(__file__), "..", "packaging", "icon-1024.png")
    os.makedirs(os.path.dirname(out), exist_ok=True)
    with open(out, "wb") as f:
        f.write(png)
    print(os.path.normpath(out))


if __name__ == "__main__":
    main()
