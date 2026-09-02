"""Regenerate the color-management fixtures.

    python3 tests/fixtures/icc/gen.py

Reads the system profiles from /System/Library/ColorSync/Profiles, so it runs
on macOS.
"""

from pathlib import Path

from PIL import Image

PROFILES = Path("/System/Library/ColorSync/Profiles")
OUT = Path(__file__).parent


def profile(name: str) -> bytes:
    return (PROFILES / f"{name}.icc").read_bytes()


def tagged_red(path: Path, icc_name: str) -> None:
    """8x8 of raw (255, 0, 0, 255) carrying `icc_name`."""
    img = Image.new("RGBA", (8, 8), (255, 0, 0, 255))
    img.save(path, icc_profile=profile(icc_name))


def rotated(path: Path) -> None:
    """32x16 stored, left half red, tagged Exif orientation 6 (rotate 90 CW).

    Displayed it is 16x32 with the red half on top.
    """
    img = Image.new("RGB", (32, 16), (0, 0, 0))
    for y in range(16):
        for x in range(16):
            img.putpixel((x, y), (255, 0, 0))
    exif = Image.Exif()
    exif[274] = 6
    img.save(path, quality=95, exif=exif)


tagged_red(OUT / "red_p3.png", "Display P3")
tagged_red(OUT / "red_srgb.png", "sRGB Profile")
rotated(OUT / "rotated_90.jpg")
