# -*- coding: utf-8 -*-
r"""Which arithmetic turns a font transform into the text matrix the reference plots write.

The reference plots (written by version 4.5.2.9 of the PDF driver) write
every text matrix that is not `1 0 0 1` as whole 256ths to three decimals:
`0.797` for a SimSun label stretched by 0.8, `0 1.012 -1.012 0` for a turned
Arial one. Version 5.0.1.9 of the driver, which is at hand, computes the same
matrix in floating point:

    len   = hypot(eM21 * 1250000, eM22 * 1250000)
    a     = len / 256
    scale = 1250000 / a
    entry = scale * e            (printed as entry / 256)

This script holds the fourteen matrices the eleven reference plots witness
and asks three things of that routine with whole-number steps put in:

1. does `L = trunc(eM22 * K)`, `Q = L div 256`, `n = trunc(e * K / Q)` with
   `K = 1250000` reproduce all fourteen;
2. which of the three whole-number steps are needed (each is tried
   truncated, rounded, rounded up and, for the division, left exact);
3. how special the constant is: every whole `K` from 1000 to 20 000 000 is
   tried in its place.

`pid-plot/src/write/text_matrix.rs` is the rule this settles; see
`docs/analysis/2026-11-11-the-driver-states-a-font-transform-in-256ths.md`.

The faces' design metrics are read from the font files, so the answer does
not rest on numbers typed in here. Needs fontTools (`pip install fonttools`).

  python -X utf8 tools/driver_text_matrix.py [--fonts DIR] [--no-scan]

`--fonts` names the directory holding `simsun.ttc`, `arial.ttf` and
`ARIALN.TTF` (default `C:\Windows\Fonts`); `--no-scan` skips the third
question, which takes a minute or two.
"""
import itertools
import math
import os
import sys

from fontTools.ttLib import TTFont

FILES = {
    "SimSun": ("simsun.ttc", 0),
    "Arial": ("arial.ttf", None),
    "ArialNarrow": ("ARIALN.TTF", None),
}

# (face, cell in device units, stated average width or None, entry along x 256, entry across x 256,
#  what the reference writes, where)
WITNESSES = [
    ("SimSun", 24, 10, 213, 256, "0.832 0 0 1", "Tf 11.52"),
    ("SimSun", 26, 10, 197, 256, "0.77 0 0 1 and 0 0.77 -1 0", "Tf 12.48"),
    ("SimSun", 28, 11, 201, 256, "0.785 0 0 1", "Tf 13.44"),
    ("SimSun", 30, 12, 204, 256, "0.797 0 0 1", "Tf 14.4"),
    ("SimSun", 31, 12, 198, 256, "0.773 0 0 1", "Tf 14.88"),
    ("SimSun", 35, 13, 190, 256, "0.742 0 0 1", "Tf 16.8"),
    ("SimSun", 37, 14, 193, 256, "0 0.754 -1 0", "Tf 17.76"),
    ("SimSun", 39, 15, 197, 256, "0.77 0 0 1", "Tf 18.72"),
    ("SimSun", 67, 26, 198, 256, "0.773 0 0 1", "Tf 32.16"),
    ("SimSun", 31, None, 256, 256, "0 1 -1 0", "Tf 14.88, turned"),
    ("Arial", 19, 6, 207, 259, "0.809 0 0 1.012", "Tf 8.16, the symbol bodies' M"),
    ("Arial", 29, None, 259, 259, "0 1.012 -1.012 0", "Tf 12.45, turned"),
    ("ArialNarrow", 24, None, 258, 258, "0 1.008 -1.008 0", "Tf 10.17, turned"),
    ("ArialNarrow", 37, None, 259, 259, "0 1.012 -1.012 0", "Tf 15.69, turned"),
]

ROUNDERS = {
    "trunc": math.floor,
    "round": lambda value: math.floor(value + 0.5),
    "ceil": math.ceil,
}


def metrics(directory, name):
    file, index = FILES[name]
    path = os.path.join(directory, file)
    font = TTFont(path, fontNumber=index) if index is not None else TTFont(path)
    os2 = font["OS/2"]
    return {
        "upm": font["head"].unitsPerEm,
        "avg": os2.xAvgCharWidth,
        "cell": os2.usWinAscent + os2.usWinDescent,
        "ascent": os2.usWinAscent,
        "descent": os2.usWinDescent,
        "version": font["name"].getDebugName(5),
    }


def elements(faces, face, cell, width):
    """(eM11, eM22): device units per design unit along and across the baseline, as GDI states a font."""
    m = faces[face]
    e22 = cell / m["cell"]
    e11 = e22 if width is None else width / m["avg"]
    return e11, e22


def written(faces, face, cell, width, k=1250000.0, length="trunc", quotient="trunc", entry="trunc", scale="float"):
    e11, e22 = elements(faces, face, cell, width)
    big = ROUNDERS[length](e22 * k)
    q = big / 256.0 if quotient == "exact" else ROUNDERS[quotient](big / 256.0)
    if q <= 0:
        return None
    factor = math.floor(k / q) if scale == "int" else k / q
    return ROUNDERS[entry](factor * e11), ROUNDERS[entry](factor * e22), factor * e11, factor * e22


def main():
    args = sys.argv[1:]
    directory = r"C:\Windows\Fonts"
    if "--fonts" in args:
        directory = args[args.index("--fonts") + 1]
    faces = {name: metrics(directory, name) for name in FILES}

    print("== the faces")
    for name, m in faces.items():
        print(
            f"   {name:<12} units per em {m['upm']:>5}, xAvgCharWidth {m['avg']:>5},"
            f" usWinAscent {m['ascent']} + usWinDescent {m['descent']} = {m['cell']}  ({m['version']})"
        )

    print("== the rule: L = trunc(eM22 x 1250000); Q = L div 256; n = trunc(e x 1250000 / Q)")
    closest = 1.0
    hits = 0
    for face, cell, width, x256, y256, text, where in WITNESSES:
        nx, ny, fx, fy = written(faces, face, cell, width)
        ok = (nx, ny) == (x256, y256)
        hits += ok
        closest = min(closest, fx - math.floor(fx), math.ceil(fx) - fx, fy - math.floor(fy), math.ceil(fy) - fy)
        _, e22 = elements(faces, face, cell, width)
        print(
            f"   {face:<12} cell {cell:>3} width {str(width):>4} | eM22 x 1.25e6 = {e22 * 1250000:12.4f}"
            f" Q {math.floor(math.floor(e22 * 1250000) / 256):>5} | along {fx:9.4f} -> {nx:>3} (reference {x256})"
            f" across {fy:9.4f} -> {ny:>3} (reference {y256}) {'ok' if ok else 'DIFFERS'} | {text} ({where})"
        )
    print(f"   {hits} of {len(WITNESSES)} witnessed matrices reproduced; the entry nearest a whole number is {closest:.4f} from it")

    print("== which whole-number steps are needed (combinations reproducing at least 12 of the 14)")
    for length, quotient, entry, scale in itertools.product(
        ["trunc", "round", "ceil"], ["trunc", "round", "ceil", "exact"], ["trunc", "round", "ceil"], ["float", "int"]
    ):
        good = 0
        for face, cell, width, x256, y256, *_ in WITNESSES:
            got = written(faces, face, cell, width, length=length, quotient=quotient, entry=entry, scale=scale)
            good += got is not None and got[:2] == (x256, y256)
        if good >= len(WITNESSES) - 2:
            mark = "all" if good == len(WITNESSES) else ""
            print(f"   length {length:<5} division {quotient:<5} entry {entry:<5} scale {scale:<5}: {good:>2} of {len(WITNESSES)} {mark}")

    if "--no-scan" in args:
        return
    print("== how special the constant is: every whole K from 1000 to 20,000,000")
    spans = []
    start = last = None
    passing = 0
    for k in range(1000, 20_000_001):
        ok = True
        for face, cell, width, x256, y256, *_ in WITNESSES:
            got = written(faces, face, cell, width, k=float(k))
            if got is None or got[:2] != (x256, y256):
                ok = False
                break
        if ok:
            passing += 1
            if start is None:
                start = k
            last = k
        elif start is not None:
            spans.append((start, last))
            start = None
    if start is not None:
        spans.append((start, last))
    print(f"   {passing} values reproduce all {len(WITNESSES)}: {spans}")
    for value in (1250000, 125000, 1000000, 65536, 1048576, 2500000, 625000):
        good = 0
        for face, cell, width, x256, y256, *_ in WITNESSES:
            got = written(faces, face, cell, width, k=float(value))
            good += got is not None and got[:2] == (x256, y256)
        print(f"   K = {value:>8}: {good} of {len(WITNESSES)}")


if __name__ == "__main__":
    main()
