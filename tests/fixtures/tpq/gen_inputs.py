#!/usr/bin/env python3
"""Generate the small RGBA PNG input images used as tilepalquant test fixtures.

Run from anywhere; writes into the same directory this script lives in.
"""
import os
from PIL import Image

FIXDIR = os.path.dirname(os.path.abspath(__file__))
# Reference C++ repo, used only to source a crop for photo_64x32.png.
REFREPO = os.path.normpath(os.path.join(FIXDIR, "..", "..", "..", "..", "tilepalquant"))


def save(img, name):
    path = os.path.join(FIXDIR, name)
    img.save(path)
    print("wrote", path, img.size, img.mode)


def make_gradient():
    """32x16 opaque, smooth RGB gradient (many distinct colors per tile)."""
    w, h = 32, 16
    img = Image.new("RGBA", (w, h))
    px = img.load()
    for y in range(h):
        for x in range(w):
            r = int(255 * (x / (w - 1)))
            g = int(255 * (y / (h - 1)))
            b = int(255 * (((x + y) % (w + h)) / (w + h - 1)))
            px[x, y] = (r, g, b, 255)
    save(img, "gradient_32x16.png")


def make_photo():
    """64x32 crop of carina-nebula.png (or a synthesized noisy multi-hue image)."""
    src_path = os.path.join(REFREPO, "carina-nebula.png")
    if os.path.exists(src_path):
        src = Image.open(src_path).convert("RGBA")
        sw, sh = src.size
        left = max(0, min(min(100, sw // 3), sw - 64))
        top = max(0, min(min(100, sh // 3), sh - 32))
        crop = src.crop((left, top, left + 64, top + 32))
        save(crop, "photo_64x32.png")
    else:
        import random
        random.seed(42)
        w, h = 64, 32
        img = Image.new("RGBA", (w, h))
        px = img.load()
        for y in range(h):
            for x in range(w):
                r = (x * 4 + random.randint(0, 40)) % 256
                g = (y * 8 + random.randint(0, 40)) % 256
                b = ((x + y) * 3 + random.randint(0, 40)) % 256
                px[x, y] = (r, g, b, 255)
        save(img, "photo_64x32.png")


def make_alpha():
    """32x32: fully transparent, partially transparent, and opaque pixels;
    tile (0,0) is entirely transparent."""
    w, h = 32, 32
    img = Image.new("RGBA", (w, h))
    px = img.load()
    for y in range(h):
        for x in range(w):
            tile_x, tile_y = x // 8, y // 8
            r = (x * 8) % 256
            g = (y * 8) % 256
            b = ((x + y) * 5) % 256
            if tile_x == 0 and tile_y == 0:
                a = 0
            elif (x + y) % 5 == 0:
                a = 0
            elif (x + y) % 3 == 0:
                a = 128
            else:
                a = 255
            px[x, y] = (r, g, b, a)
    save(img, "alpha_32x32.png")


def make_key():
    """32x32 opaque; a region and one whole 8x8 tile are exact key-color magenta."""
    w, h = 32, 32
    img = Image.new("RGBA", (w, h))
    px = img.load()
    for y in range(h):
        for x in range(w):
            r = (x * 8) % 256
            g = (y * 8) % 256
            b = ((x + y) * 5) % 256
            px[x, y] = (r, g, b, 255)
    for y in range(4, 12):
        for x in range(4, 12):
            px[x, y] = (255, 0, 255, 255)
    for y in range(24, 32):
        for x in range(24, 32):
            px[x, y] = (255, 0, 255, 255)
    save(img, "key_32x32.png")


def make_flat():
    """16x8 with only 3 distinct colors (fewer than colors-per-palette)."""
    w, h = 16, 8
    img = Image.new("RGBA", (w, h))
    px = img.load()
    colors = [(200, 50, 50, 255), (50, 200, 50, 255), (50, 50, 200, 255)]
    for y in range(h):
        for x in range(w):
            idx = (x // 5 + y // 4) % 3
            px[x, y] = colors[idx]
    save(img, "flat_16x8.png")


if __name__ == "__main__":
    make_gradient()
    make_photo()
    make_alpha()
    make_key()
    make_flat()
