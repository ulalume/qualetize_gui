#!/usr/bin/env python3
"""Single source of truth for the tpq reference-fixture cases.

Used by gen.sh two ways:
  gen_manifest.py --print-commands   emits one shell-quoted `tpq ...` invocation
                                      per line, in `NAME<TAB>args...` form, for
                                      gen.sh to run against the built tpq binary.
  gen_manifest.py                    (no args) writes manifest.json, inspecting
                                      whichever output PNGs actually exist so
                                      failed cases are recorded rather than
                                      silently skipped.
"""
import json
import os
import sys

FIXDIR = os.path.dirname(os.path.abspath(__file__))

# ---------------------------------------------------------------------------
# Option defaults, mirrored from tilepalquant/src/options.h
# (tile_w/tile_h defaults are 8/8 too, but every case here pins tile 8x8 anyway)
DEFAULTS = {
    "tile_w": 8,
    "tile_h": 8,
    "num_pals": 8,
    "cols_per_pal": 4,
    "bits_per_chan": 5,
    "col_zero": "unique",
    "shared_col": [0, 0, 0],
    "transp_col": [255, 0, 255],
    "dither": "off",
    "dither_pat": "diag4",
    "dither_wt": 0.50,
    "frac_of_px": 0.1,
    "rand_seed": 0,
}


def params(**overrides):
    p = dict(DEFAULTS)
    p.update(overrides)
    return p


# Base params shared by most "3bit" cases.
BASE_3BIT = dict(num_pals=4, cols_per_pal=16, bits_per_chan=3, col_zero="unique", dither="off")

CASE_PARAMS = {
    "unique_off_3bit": params(**BASE_3BIT),
    "shared_off_3bit": params(**{**BASE_3BIT, "col_zero": "shared", "shared_col": [0, 0, 0]}),
    "transp_off_3bit": params(**{**BASE_3BIT, "col_zero": "transp"}),
    "transpcol_off_3bit": params(**{**BASE_3BIT, "col_zero": "transp_color", "transp_col": [255, 0, 255]}),
    "unique_fast_3bit": params(**{**BASE_3BIT, "dither": "fast", "dither_pat": "diag4", "dither_wt": 0.5}),
    "unique_slow_3bit": params(**{**BASE_3BIT, "dither": "slow", "dither_pat": "diag4", "dither_wt": 0.5}),
    "unique_off_5bit": params(**{**BASE_3BIT, "bits_per_chan": 5}),
    "shared_fast_5bit_horiz2": params(
        num_pals=4, cols_per_pal=16, bits_per_chan=5, col_zero="shared", shared_col=[8, 16, 24],
        dither="fast", dither_pat="horiz2", dither_wt=0.3,
    ),
    "unique_off_2pal4col": params(num_pals=2, cols_per_pal=4, bits_per_chan=2, col_zero="unique", dither="off"),
    "unique_off_frac05": params(**{**BASE_3BIT, "frac_of_px": 0.05}),
}

# Which cases run against which input image.
IMAGE_CASES = {
    "gradient_32x16.png": [
        "unique_off_3bit", "shared_off_3bit", "unique_fast_3bit", "unique_slow_3bit",
        "unique_off_5bit", "shared_fast_5bit_horiz2", "unique_off_2pal4col", "unique_off_frac05",
    ],
    "photo_64x32.png": [
        "unique_off_3bit", "shared_off_3bit", "unique_fast_3bit", "unique_slow_3bit",
        "unique_off_5bit", "shared_fast_5bit_horiz2", "unique_off_2pal4col", "unique_off_frac05",
    ],
    "alpha_32x32.png": ["transp_off_3bit", "unique_off_3bit"],
    "key_32x32.png": ["transpcol_off_3bit", "unique_off_3bit"],
    "flat_16x8.png": ["unique_off_3bit", "unique_off_2pal4col"],
}


def cli_args(case_name, p):
    args = [
        "-tile_w", str(p["tile_w"]),
        "-tile_h", str(p["tile_h"]),
        "-num_pals", str(p["num_pals"]),
        "-cols_per_pal", str(p["cols_per_pal"]),
        "-bits_per_chan", str(p["bits_per_chan"]),
        "-col_zero", p["col_zero"],
        "-dither", p["dither"],
        "-dither_pat", p["dither_pat"],
        "-dither_wt", str(p["dither_wt"]),
        "-frac_of_px", str(p["frac_of_px"]),
        "-rand_seed", str(p["rand_seed"]),
    ]
    if p["col_zero"] == "shared":
        args += ["-shared_col", ",".join(str(v) for v in p["shared_col"])]
    if case_name == "transpcol_off_3bit":
        args += ["-transp_col", ",".join(str(v) for v in p["transp_col"])]
    return args


def all_entries():
    entries = []
    for input_name, case_names in IMAGE_CASES.items():
        stem = os.path.splitext(input_name)[0]
        for case_name in case_names:
            p = CASE_PARAMS[case_name]
            output_name = f"{stem}__{case_name}.png"
            entries.append((input_name, case_name, output_name, p))
    return entries


def print_commands():
    # Tab-separated: input, output, case_name, then each CLI arg as its own
    # field (already atomic — no further shell quoting/splitting needed).
    for input_name, case_name, output_name, p in all_entries():
        args = cli_args(case_name, p)
        line = "\t".join([input_name, output_name, case_name] + args)
        print(line)


def build_manifest():
    try:
        from PIL import Image
    except ImportError:
        Image = None

    cases = []
    for input_name, case_name, output_name, p in all_entries():
        entry = {
            "case": case_name,
            "input": input_name,
            "output": output_name,
            "params": {
                "tile_w": p["tile_w"],
                "tile_h": p["tile_h"],
                "num_pals": p["num_pals"],
                "cols_per_pal": p["cols_per_pal"],
                "bits_per_chan": p["bits_per_chan"],
                "col_zero": p["col_zero"],
                "shared_col": p["shared_col"],
                "transp_col": p["transp_col"],
                "dither": p["dither"],
                "dither_pat": p["dither_pat"],
                "dither_wt": p["dither_wt"],
                "frac_of_px": p["frac_of_px"],
                "rand_seed": p["rand_seed"],
            },
        }
        out_path = os.path.join(FIXDIR, output_name)
        if os.path.exists(out_path):
            entry["status"] = "ok"
            entry["output_size_bytes"] = os.path.getsize(out_path)
            if Image is not None:
                try:
                    img = Image.open(out_path)
                    entry["output_format"] = f"{img.mode} PNG, {img.width}x{img.height}"
                except Exception as e:
                    entry["output_format_error"] = str(e)
        else:
            entry["status"] = "failed"
            entry["error"] = "output file not produced (tpq run failed or was skipped)"
        cases.append(entry)

    manifest = {
        "generator": "tilepalquant tpq (perf-fixes branch, TPQ_FIXED_SHUFFLE=1)",
        "notes": (
            "All outputs are 8-bit palette PNGs (PLTE only). tpq never writes a "
            "tRNS chunk; for col_zero transp/transp_color modes, palette index 0 "
            "is simply set to an opaque RGB color (transp_col, default "
            "255,0,255) with no PNG-level alpha/transparency encoded."
        ),
        "cases": cases,
    }
    with open(os.path.join(FIXDIR, "manifest.json"), "w") as f:
        json.dump(manifest, f, indent=2)
        f.write("\n")
    print(f"wrote {os.path.join(FIXDIR, 'manifest.json')} ({len(cases)} cases)")


if __name__ == "__main__":
    if "--print-commands" in sys.argv:
        print_commands()
    else:
        build_manifest()
