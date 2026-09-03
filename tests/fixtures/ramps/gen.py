"""Regenerate the Ramps sort-mode parity fixtures.

    python3 tests/fixtures/ramps/gen.py

Runs .doc/palette-order/ramps_v3.py, the algorithm the Rust port in
src/types/palette_ramps.rs replicates, over 16-color chunks of every sample
palette, three shuffled variants of sweetie-16 and two quantizer palettes, and
writes the expected orders as JSON. Each case carries the hue gap it was
generated with.
"""

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / ".doc" / "palette-order"))

from palcore import all_palettes  # noqa: E402
from ramps_v3 import BLOCK_GAP, sort_palette  # noqa: E402

OUT = Path(__file__).parent / "cases.json"


def case(name, colors, hue_gap=BLOCK_GAP):
    return {
        "name": name,
        "hue_gap": hue_gap,
        "input": colors,
        "expected": sort_palette(colors, pin_first=False, block_gap=hue_gap),
    }


def chunk_cases():
    cases = []
    for name, colors in sorted(all_palettes().items()):
        for i in range(0, len(colors), 16):
            chunk = colors[i : i + 16]
            if len(chunk) < 4:
                continue
            cases.append(case(f"{name}[{i}:{i + len(chunk)}]", chunk))
    return cases


def shuffle_cases():
    colors = all_palettes()["sweetie-16"]
    shuffles = [
        list(reversed(colors)),
        colors[8:] + colors[:8],
        [colors[i] for i in [3, 11, 0, 15, 7, 1, 9, 14, 2, 10, 5, 13, 6, 12, 4, 8]],
    ]
    expected = sort_palette(colors, pin_first=False)
    cases = []
    for k, shuffled in enumerate(shuffles):
        assert sorted(shuffled) == sorted(colors), "shuffle must keep the same colors"
        entry = case(f"sweetie-16-shuffle-{k}", shuffled)
        assert entry["expected"] == expected, "the order must not depend on the input order"
        cases.append(entry)
    return cases


# A tilepalquant palette (carina-nebula.png, Genesis levels): 16 colors spread
# over the whole gamut, far sparser than a hand-made palette.
QUANTIZED = [
    (0x31, 0x31, 0x31),
    (0xAE, 0xCE, 0xCE),
    (0x57, 0x92, 0xCE),
    (0x31, 0x77, 0xCE),
    (0x31, 0x57, 0x92),
    (0x31, 0x31, 0x77),
    (0x31, 0x00, 0x31),
    (0x57, 0x31, 0x31),
    (0x77, 0x57, 0x57),
    (0x92, 0x77, 0x77),
    (0xAE, 0x57, 0x31),
    (0xCE, 0x77, 0x57),
    (0xCE, 0x92, 0x77),
    (0xFF, 0xAE, 0x77),
    (0xFF, 0xCE, 0x92),
    (0xFF, 0xFF, 0xCE),
]


# A Qualetize palette (a.png, Genesis full palettes, palette 2) with its pinned
# index 0 dropped: one wide hue block of greens, olives, yellows and a brown.
QUALETIZE = [
    (0xCE, 0xCE, 0xCE),
    (0x31, 0x57, 0x31),
    (0x92, 0xAE, 0x77),
    (0xAE, 0x77, 0x31),
    (0x57, 0x92, 0x31),
    (0x77, 0xAE, 0x31),
    (0x77, 0x77, 0x31),
    (0x92, 0x92, 0x31),
    (0x92, 0x92, 0x57),
    (0xAE, 0xAE, 0x77),
    (0xCE, 0xCE, 0x92),
    (0xCE, 0xAE, 0x31),
    (0xAE, 0xCE, 0x31),
    (0xCE, 0xCE, 0x31),
    (0x31, 0x92, 0xAE),
]


def quantized_cases():
    """The two quantizer palettes, the second one twice to exercise the hue gap."""
    cases = [
        case("carina-nebula-tilepalquant", QUANTIZED),
        case("a-png-qualetize-palette-2", QUALETIZE),
        case("a-png-qualetize-palette-2-gap10", QUALETIZE, hue_gap=10.0),
    ]
    assert cases[1]["expected"] != cases[2]["expected"], "the hue gap must change the order"
    return cases


cases = chunk_cases() + shuffle_cases() + quantized_cases()
OUT.write_text(json.dumps(cases, indent=2) + "\n")
print(f"wrote {len(cases)} cases to {OUT}")
