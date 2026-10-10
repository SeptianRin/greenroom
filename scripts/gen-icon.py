#!/usr/bin/env python3
"""Generate assets/AppIcon.icns for Greenroom — no third-party deps.

Motif: a dark stage with a spotlight and three agent dots (the middle one lit).
Renders one 1024x1024 RGBA PNG, then sips/iconutil build the .icns.
"""
import struct
import subprocess
import sys
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "assets"
W = 1024


def write_png(path, pixels):
    def chunk(tag, data):
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    raw = b"".join(
        b"\x00" + bytes(pixels[y * W * 4 : (y + 1) * W * 4]) for y in range(W)
    )
    png = (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", W, W, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )
    path.write_bytes(png)


def render():
    px = bytearray(W * W * 4)
    inset, radius = 56.0, 208.0
    lo, hi = inset, W - inset
    glow_cx, glow_cy, glow_r = 512.0, 342.0, 470.0
    dots = [(332.0, 778.0, 54.0, (96, 102, 112)), (512.0, 770.0, 62.0, (95, 226, 156)), (692.0, 778.0, 54.0, (96, 102, 112))]

    for y in range(W):
        ty = y / (W - 1)
        base = (
            int(17 + ty * 16),
            int(19 + ty * 18),
            int(23 + ty * 22),
        )
        for x in range(W):
            # rounded-rect mask
            inside = lo <= x <= hi and lo <= y <= hi
            if not inside:
                alpha = 0
            elif (x < lo + radius and y < lo + radius and (x - (lo + radius)) ** 2 + (y - (lo + radius)) ** 2 > radius * radius) or \
                 (x > hi - radius and y < lo + radius and (x - (hi - radius)) ** 2 + (y - (lo + radius)) ** 2 > radius * radius) or \
                 (x < lo + radius and y > hi - radius and (x - (lo + radius)) ** 2 + (y - (hi - radius)) ** 2 > radius * radius) or \
                 (x > hi - radius and y > hi - radius and (x - (hi - radius)) ** 2 + (y - (hi - radius)) ** 2 > radius * radius):
                alpha = 0
            else:
                alpha = 255
            if alpha == 0:
                continue
            r, g, b = base
            # spotlight glow
            dx, dy = x - glow_cx, y - glow_cy
            d = (dx * dx + dy * dy) ** 0.5
            if d < glow_r:
                a = 52.0 * (1.0 - d / glow_r) ** 2
                r = min(255, int(r + a * 1.00))
                g = min(255, int(g + a * 0.97))
                b = min(255, int(b + a * 0.88))
            # agent dots
            for cx, cy, cr, col in dots:
                dx, dy = x - cx, y - cy
                if dx * dx + dy * dy <= cr * cr:
                    r, g, b = col
                    break
            i = (y * W + x) * 4
            px[i], px[i + 1], px[i + 2], px[i + 3] = r, g, b, 255
    return px


def main():
    ASSETS.mkdir(exist_ok=True)
    master = ASSETS / "icon_1024.png"
    print("rendering 1024x1024…")
    write_png(master, render())

    iconset = ASSETS / "AppIcon.iconset"
    subprocess.run(["rm", "-rf", str(iconset)], check=True)
    iconset.mkdir()
    sizes = [16, 32, 64, 128, 256, 512]
    for s in sizes:
        subprocess.run(
            ["sips", "-z", str(s), str(s), str(master), "--out", str(iconset / f"icon_{s}x{s}.png")],
            check=True, capture_output=True,
        )
        subprocess.run(
            ["sips", "-z", str(s * 2), str(s * 2), str(master), "--out", str(iconset / f"icon_{s}x{s}@2x.png")],
            check=True, capture_output=True,
        )
    subprocess.run(
        ["iconutil", "-c", "icns", str(iconset), "-o", str(ASSETS / "AppIcon.icns")],
        check=True,
    )
    subprocess.run(["rm", "-rf", str(iconset), str(master)], check=True)
    print("OK:", ASSETS / "AppIcon.icns")


if __name__ == "__main__":
    sys.exit(main())
