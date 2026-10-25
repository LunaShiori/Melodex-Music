#!/usr/bin/env python3
"""生成 Melodex 应用图标（PNG + ICO），不依赖 Pillow。

用法：python scripts/make-icons.py
输出：src-tauri/icons/{32x32,128x128,128x128@2x,icon}.png 与 icon.ico
"""

import math
import os
import struct
import zlib

OUT_DIR = os.path.join(os.path.dirname(__file__), "..", "src-tauri", "icons")

ACCENT = (0x6B, 0x5B, 0xF0)
ACCENT_2 = (0x22, 0xC1, 0xC3)


def lerp(a, b, t):
    return tuple(int(round(a[i] + (b[i] - a[i]) * t)) for i in range(3))


def rounded_rect_sd(x, y, size, radius):
    """圆角矩形的有符号距离（负值在内部）。"""
    half = size / 2.0
    dx = abs(x - half) - (half - radius)
    dy = abs(y - half) - (half - radius)
    ax, ay = max(dx, 0.0), max(dy, 0.0)
    return math.hypot(ax, ay) + min(max(dx, dy), 0.0) - radius


def render(size, supersample=3):
    big = size * supersample
    radius = big * 0.26
    buf = [[(0, 0, 0, 0)] * size for _ in range(size)]

    for py in range(size):
        for px in range(size):
            acc = [0.0, 0.0, 0.0, 0.0]
            for sy in range(supersample):
                for sx in range(supersample):
                    x = px * supersample + sx + 0.5
                    y = py * supersample + sy + 0.5
                    d = rounded_rect_sd(x, y, big, radius)
                    if d > 0.75:
                        continue
                    cover = 1.0 if d < -0.75 else (0.75 - d) / 1.5
                    t = (x / big * 0.55 + y / big * 0.45)
                    color = lerp(ACCENT, ACCENT_2, min(1.0, max(0.0, t * 1.35)))
                    r, g, b = color
                    # 唱片纹路：两组同心环
                    cx = cy = big / 2.0
                    dist = math.hypot(x - cx, y - cy)
                    for ring_r, width, alpha in ((big * 0.30, big * 0.012, 0.34), (big * 0.20, big * 0.010, 0.26)):
                        if abs(dist - ring_r) < width:
                            r = int(r + (255 - r) * alpha)
                            g = int(g + (255 - g) * alpha)
                            b = int(b + (255 - b) * alpha)
                    if dist < big * 0.062:
                        r, g, b = 255, 255, 255
                    acc[0] += r * cover
                    acc[1] += g * cover
                    acc[2] += b * cover
                    acc[3] += cover
            n = supersample * supersample
            a = acc[3] / n
            if a <= 0.001:
                buf[py][px] = (0, 0, 0, 0)
            else:
                buf[py][px] = (
                    int(acc[0] / acc[3]),
                    int(acc[1] / acc[3]),
                    int(acc[2] / acc[3]),
                    int(round(a * 255)),
                )
    return buf


def write_png(path, pixels):
    size = len(pixels)
    raw = bytearray()
    for row in pixels:
        raw.append(0)
        for r, g, b, a in row:
            raw += bytes((r, g, b, a))

    def chunk(tag, data):
        body = tag + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)

    header = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)
    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", header)
    png += chunk(b"IDAT", zlib.compress(bytes(raw), 9))
    png += chunk(b"IEND", b"")
    with open(path, "wb") as fh:
        fh.write(png)


def write_ico(path, png_files):
    count = len(png_files)
    header = struct.pack("<HHH", 0, 1, count)
    entries = b""
    payload = b""
    offset = 6 + 16 * count
    for size, data in png_files:
        entries += struct.pack(
            "<BBBBHHII", size if size < 256 else 0, size if size < 256 else 0, 0, 0, 1, 32, len(data), offset
        )
        payload += data
        offset += len(data)
    with open(path, "wb") as fh:
        fh.write(header + entries + payload)


def main():
    os.makedirs(OUT_DIR, exist_ok=True)
    targets = {32: "32x32.png", 128: "128x128.png", 256: "128x128@2x.png", 512: "icon.png"}
    cache = {}
    for size in sorted(set(list(targets) + [16, 48, 64])):
        cache[size] = render(size)
        print(f"rendered {size}x{size}")
    for size, name in targets.items():
        write_png(os.path.join(OUT_DIR, name), cache[size])
    ico_parts = []
    for size in (16, 32, 48, 64, 128, 256):
        tmp = os.path.join(OUT_DIR, f".tmp-{size}.png")
        write_png(tmp, cache[size])
        with open(tmp, "rb") as fh:
            ico_parts.append((size, fh.read()))
        os.remove(tmp)
    write_ico(os.path.join(OUT_DIR, "icon.ico"), ico_parts)
    print("icons written to", os.path.abspath(OUT_DIR))


if __name__ == "__main__":
    main()
