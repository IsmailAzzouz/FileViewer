#!/usr/bin/env python3
"""Generate the FileViewer application icon as PNG, with no external tools.

The artwork mirrors the README header: an F cut from three strokes on near
black, plus a three-level node spine for the tree view. Written by hand so the
build needs no ImageMagick, rsvg-convert, or Inkscape.

Usage: python3 packaging/linux/make_icon.py assets/icon.png [size]
"""

import struct
import sys
import zlib

BG = (0x11, 0x11, 0x15)
FG = (0xF4, 0xF4, 0xF4)
ACCENT = (0x60, 0x8C, 0xF6)


def blank(size):
    return [[BG for _ in range(size)] for _ in range(size)]


def rect(px, x0, y0, x1, y1, color):
    h = len(px)
    w = len(px[0])
    for y in range(max(0, y0), min(h, y1)):
        row = px[y]
        for x in range(max(0, x0), min(w, x1)):
            row[x] = color


def disc(px, cx, cy, r, color):
    h = len(px)
    w = len(px[0])
    r2 = r * r
    for y in range(max(0, cy - r), min(h, cy + r + 1)):
        dy = y - cy
        row = px[y]
        for x in range(max(0, cx - r), min(w, cx + r + 1)):
            dx = x - cx
            if dx * dx + dy * dy <= r2:
                row[x] = color


def draw(size):
    px = blank(size)

    # Work in a 0..100 unit grid so every size renders the same proportions.
    def u(v):
        return int(round(v * size / 100.0))

    # F: stem, top arm, middle arm.
    rect(px, u(26), u(18), u(34), u(76), FG)
    rect(px, u(34), u(18), u(64), u(28), FG)
    rect(px, u(34), u(42), u(55), u(52), FG)

    # Node spine: three levels, connected by a vertical rule.
    rect(px, u(72), u(28), u(74), u(66), ACCENT)
    for y in (30, 46, 64):
        rect(px, u(74), u(y), u(80), u(y + 2), ACCENT)
        disc(px, u(84), u(y + 1), max(1, u(4)), ACCENT)

    return px


def write_png(path, px):
    size = len(px)
    raw = bytearray()
    for row in px:
        raw.append(0)  # filter type 0 (None)
        for r, g, b in row:
            raw += bytes((r, g, b))

    def chunk(tag, data):
        out = struct.pack(">I", len(data)) + tag + data
        return out + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 2, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(bytes(raw), 9))
    png += chunk(b"IEND", b"")

    with open(path, "wb") as fh:
        fh.write(png)


def main():
    path = sys.argv[1] if len(sys.argv) > 1 else "assets/icon.png"
    size = int(sys.argv[2]) if len(sys.argv) > 2 else 256
    write_png(path, draw(size))
    print(f"wrote {path} ({size}x{size})")


if __name__ == "__main__":
    main()
