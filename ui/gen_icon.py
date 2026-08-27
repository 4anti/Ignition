import math
import struct
import zlib
from pathlib import Path

W = H = 1024
cx = cy = 511.5


def lerp(a, b, t):
    return a + (b - a) * t


def mix(c0, c1, t):
    return tuple(int(lerp(a, b, t)) for a, b in zip(c0, c1))


def px(x, y):
    dx, dy = x - cx, y - cy
    r = math.hypot(dx, dy)
    # circular icon
    if r > 500:
        return (0, 0, 0, 0)
    edge = min(1.0, (500 - r) / 8)
    bg = mix((11, 12, 9), (22, 24, 16), (dy + 512) / 1024)
    # copper ring
    ring = 1.0 - min(1.0, abs(r - 430) / 18)
    # vertical flame bar
    bar = 0.0
    if abs(dx) < 46 and -210 < dy < 250:
        bar = 1.0 - abs(dx) / 46
        # pointed top
        tip = (-210 - dy) / 70 if dy < -210 else 0
        if dy < -210:
            bar = 0
        if -280 < dy < -210 and abs(dx) < (46 * (1 - (-210 - dy) / 70)):
            bar = 1.0 - abs(dx) / max(8, 46 * (1 - (-210 - dy) / 70))
        _ = tip
    # diamond spark at top
    spark = 0.0
    sx, sy = dx, dy + 250
    if abs(sx) + abs(sy) < 48:
        spark = 1.0 - (abs(sx) + abs(sy)) / 48

    col = bg
    col = mix(col, (42, 45, 34), max(0, ring) * 0.4)
    col = mix(col, (212, 137, 58), max(0.0, ring) ** 1.4)
    col = mix(col, (243, 181, 106), bar * 0.95)
    col = mix(col, (255, 224, 170), spark)
    a = int(255 * edge)
    return (col[0], col[1], col[2], a)


def chunk(tag, data):
    return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)


raw = bytearray()
for y in range(H):
    raw.append(0)
    for x in range(W):
        raw.extend(px(x, y))

ihdr = struct.pack(">IIBBBBB", W, H, 8, 6, 0, 0, 0)
png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(bytes(raw), 9)) + chunk(b"IEND", b"")
out = Path(__file__).resolve().parents[1] / "src-tauri" / "icons" / "icon-source.png"
out.parent.mkdir(parents=True, exist_ok=True)
out.write_bytes(png)
print(out, len(png))
