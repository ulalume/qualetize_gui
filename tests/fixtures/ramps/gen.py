"""Regenerate the Ramps sort-mode parity fixtures.

    python3 tests/fixtures/ramps/gen.py

Runs .doc/palette-order/recommended.py, the algorithm the Rust port in
src/types/palette_ramps.rs replicates, over 16-color chunks of every sample
palette and three shuffled variants of sweetie-16, and writes the expected
orders as JSON.
"""

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / ".doc" / "palette-order"))

from palcore import all_palettes  # noqa: E402
from recommended import sort_palette  # noqa: E402

OUT = Path(__file__).parent / "cases.json"


def chunk_cases():
    cases = []
    for name, colors in sorted(all_palettes().items()):
        for i in range(0, len(colors), 16):
            chunk = colors[i : i + 16]
            if len(chunk) < 4:
                continue
            cases.append(
                {
                    "name": f"{name}[{i}:{i + len(chunk)}]",
                    "input": chunk,
                    "expected": sort_palette(chunk, pin_first=False),
                }
            )
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
        got = sort_palette(shuffled, pin_first=False)
        assert got == expected, "the order must not depend on the input order"
        cases.append(
            {
                "name": f"sweetie-16-shuffle-{k}",
                "input": shuffled,
                "expected": got,
            }
        )
    return cases


cases = chunk_cases() + shuffle_cases()
OUT.write_text(json.dumps(cases, indent=2) + "\n")
print(f"wrote {len(cases)} cases to {OUT}")
