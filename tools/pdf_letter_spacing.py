# -*- coding: utf-8 -*-
r"""The letter-spacing of every text object of the eleven reference plots against ours.

python -X utf8 tools/pdf_letter_spacing.py <reference dir> <our dir> [--list N]

Every `<name>.pdf` of <our dir> is read against the `<name>.pdf` of <reference dir>:
the eleven sheets of the acceptance corpus against pages `pid_plot` wrote for them.
A reference text object of more than one glyph is paired with ours by place and
text: the object of ours that starts within 0.2 mm of it. Ours may show the same
string whole (one object with the reference's glyphs) or with its last glyph
apart (the reference's string less its last glyph, then that glyph); both pair,
and which it was is counted. For every pair the `Tc` in force is compared as the
two files write it, character for character.

An object that pairs with none of ours by place is tried once more against the
nearest object of ours that holds the same text in the same face, wherever it is.

It reads the files with `pdf_text_objects.py`, beside it, and nothing of the crate.
The reading of 2026-11-12 (CHANGELOG, "排字第八批"): 3 986 reference objects, 3 904
paired by place and 80 more by text, every `Tc` the same as written, 2 with no
counterpart.
"""
import collections
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import pdf_text_objects as t  # noqa: E402

MM = 25.4 / 72.0


def canon(raw):
    """A number as written, without a sign on nought or a trailing nought."""
    text = raw.strip()
    if "." in text:
        text = text.rstrip("0").rstrip(".")
    if text in ("-0", "", "+0"):
        text = "0"
    return text


def main():
    args = sys.argv[1:]
    listing = 0
    if "--list" in args:
        at = args.index("--list")
        listing = int(args[at + 1])
        del args[at : at + 2]
    reference_dir, ours_dir = args[0], args[1]
    total = collections.Counter()
    by_font = collections.defaultdict(collections.Counter)
    differing = []
    unpaired = []
    for name in sorted(os.listdir(ours_dir)):
        if not name.endswith(".pdf"):
            continue
        if not os.path.isfile(os.path.join(reference_dir, name)):
            continue
        ref = [r for r in t.shows(os.path.join(reference_dir, name)) if r["glyphs"] > 1]
        ours = t.shows(os.path.join(ours_dir, name))
        grid = collections.defaultdict(list)
        for index, show in enumerate(ours):
            key = (round(show["matrix"][4] * MM / 0.5), round(show["matrix"][5] * MM / 0.5))
            grid[key].append(index)
        used = set()
        page = collections.Counter()
        for r in ref:
            total["reference text objects of more than one glyph"] += 1
            font = (r["font"], r["size_raw"], (r["tm_raw"] or ["?"])[0])
            by_font[font]["objects"] += 1
            rx, ry = r["matrix"][4] * MM, r["matrix"][5] * MM
            cx, cy = round(rx / 0.5), round(ry / 0.5)
            best = None
            for dx in (-1, 0, 1):
                for dy in (-1, 0, 1):
                    for index in grid.get((cx + dx, cy + dy), ()):
                        o = ours[index]
                        if index in used or not o["text"]:
                            continue
                        whole = o["text"] == r["text"]
                        head = len(o["text"]) == len(r["text"]) - 1 and r["text"].startswith(o["text"])
                        if not (whole or head):
                            continue
                        apart = math.hypot(o["matrix"][4] * MM - rx, o["matrix"][5] * MM - ry)
                        if best is None or apart < best[0]:
                            best = (apart, index, whole)
            if best is None or best[0] > 0.2:
                total["not paired with ours"] += 1
                by_font[font]["not paired"] += 1
                # The nearest object of ours with the same text, wherever it is:
                # a label the two pages put in other places is still the same string.
                loose = None
                for index, o in enumerate(ours):
                    if index in used or o["text"] != r["text"] or o["font"].replace(" ", "") != r["font"].replace(" ", ""):
                        continue
                    apart = math.hypot(o["matrix"][4] * MM - rx, o["matrix"][5] * MM - ry)
                    if loose is None or apart < loose[0]:
                        loose = (apart, index)
                if loose is None:
                    total["  of them, no object of ours has the text in the face"] += 1
                    unpaired.append((name[:20], r["font"], r["size_raw"], r["text"][:30], r["tc_raw"]))
                else:
                    used.add(loose[1])
                    same = canon(r["tc_raw"]) == canon(ours[loose[1]]["tc_raw"])
                    total["  of them, same text elsewhere on our page, Tc " + ("the same" if same else "differs")] += 1
                    if not same:
                        differing.append((name[:24], r["font"], r["size_raw"], r["text"][:26], r["tc_raw"], ours[loose[1]]["tc_raw"], r["glyphs"], f"{loose[0]:.2f} mm apart"))
                continue
            _, index, whole = best
            used.add(index)
            o = ours[index]
            total["paired"] += 1
            total["ours shows the string whole" if whole else "ours shows the last glyph apart"] += 1
            same = canon(r["tc_raw"]) == canon(o["tc_raw"])
            total["Tc the same as written" if same else "Tc differs"] += 1
            by_font[font]["same" if same else "differs"] += 1
            page["same" if same else "differs"] += 1
            if not same:
                differing.append((name[:24], r["font"], r["size_raw"], r["text"][:26], r["tc_raw"], o["tc_raw"], r["glyphs"]))
        print(f"{name[:44]:<44} multi-glyph reference objects {len(ref):5d}: Tc same {page['same']:5d}, differs {page['differs']:4d}")
    print()
    for key, value in total.items():
        print(f"  {key}: {value}")
    print("  by font (face, Tf, matrix a): objects / same / differs / not paired")
    for font, row in sorted(by_font.items(), key=lambda item: -item[1]["objects"]):
        print(f"    {font[0]:<12} Tf {font[1]:<6} a={font[2]:<6} {row['objects']:5d} / {row['same']:5d} / {row['differs']:3d} / {row['not paired']:3d}")
    print(f"  differing: {len(differing)}")
    for row in differing[: listing or 12]:
        print("   ", row)
    print(f"  reference objects whose text no object of ours has in that face: {len(unpaired)}")
    for row in unpaired[: listing or 12]:
        print("   ", row)


main()
