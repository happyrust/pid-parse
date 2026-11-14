# -*- coding: utf-8 -*-
r"""Which strings a plotted page shows whole under a letter-spacing, and which with the last glyph apart.

python -X utf8 tools/pdf_last_glyph_apart.py <pdf or directory> [...] [--list]

The writer states a string's letter-spacing with `Tc` and shows the string as one
text object, all of its glyphs under that `Tc`. One kind of string it still shows in
two: a string another piece carries on from, whose last glyph goes by itself under
`0 Tc`, so that the pen stops where that glyph ends and the piece after it starts
there. This counts both kinds on the first page of every file given, and of every
`.pdf` of every directory given:

  whole   a text object of two glyphs or more under a `Tc` that is not nought, with
          no glyph of its string shown after it;
  apart   a text object under a `Tc` that is not nought, followed -- in the same
          `BT ... ET`, in the same font at the same size, with no operator between
          the two that moves the pen (`Tm`, `Td`, `TD`, `T*`, `'`, `"`) -- by a text
          object of one glyph under `0 Tc`. Counted by whether the piece ahead of
          that glyph holds one glyph or more.

A string under a letter-spacing is one or the other, so the two add up to the strings
a page spaces. `--list` names every string shown apart: its page, its text, and the
font of the text object after it.

It reads the content streams with `pdf_text_objects.py`, beside it (which needs
PyMuPDF), and nothing of the crate.

The reading of 2026-11-12, over the 71 pages `pid_plot` writes one drawing at a time
(53 of the plant backup, the 11 reference sheets, the 7 fixtures): 17 320 whole and
123 apart, 17 443 spaced strings in all. The 123 are all on fixtures -- 121 on
`工艺管道及仪表流程-1`, one on each `DWG-0202GP06-01` -- 33 with two glyphs or more
ahead of the last and 90 with one, 78 of those the two spaces between two borrowed
Han glyphs; every one is carried on from in another font.
"""
import collections
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import pdf_text_objects as t  # noqa: E402

import pymupdf  # noqa: E402

MOVES = ("Tm", "Td", "TD", "T*", "'", '"')


def shows_of(pdf_path):
    """One record per text object of the first page, with whether the pen was moved to it."""
    doc = pymupdf.open(pdf_path)
    page = doc[0]
    fonts = t.page_fonts(doc, page)
    font = None
    size = "0"
    tc = "0"
    block = -1
    moved = True
    operands = []
    array = None
    out = []

    def show(parts):
        nonlocal moved
        face = fonts.get(font)
        codes = []
        for part in parts:
            if isinstance(part, bytes):
                codes.extend(face.codes(part) if face else list(part))
        out.append(
            {
                "block": block,
                "font": font,
                "face": face.basefont if face else font,
                "size": size,
                "tc": tc,
                "glyphs": len(codes),
                "text": face.text(codes) if face else "",
                "moved": moved,
            }
        )
        moved = False

    for kind, value in t.tokenize(page.read_contents()):
        if kind == "[":
            array = []
            continue
        if kind == "]":
            operands.append(("array", array))
            array = None
            continue
        if kind in ("num", "str", "name"):
            if array is not None:
                array.append(value)
            else:
                operands.append((kind, value))
            continue
        op = value
        nums = [v for k, v in operands if k == "num"]
        if op == "BT":
            block += 1
            moved = True
        elif op == "Tf":
            names = [v for k, v in operands if k == "name"]
            if names and nums:
                font, size = names[-1], nums[-1]
        elif op == "Tc" and nums:
            tc = nums[-1]
        elif op in MOVES:
            moved = True
            if op in ("'", '"'):
                strings = [v for k, v in operands if k == "str"]
                if strings:
                    show([strings[-1]])
        elif op == "Tj":
            strings = [v for k, v in operands if k == "str"]
            if strings:
                show([strings[-1]])
        elif op == "TJ":
            arrays = [v for k, v in operands if k == "array"]
            if arrays:
                show(arrays[-1])
        operands = []
    doc.close()
    return out


def spaced(record):
    return float(record["tc"]) != 0.0


def strings_of(shows):
    """(whole, apart): the text objects shown whole, and the (piece, last glyph, next) shown apart."""
    whole = []
    apart = []
    at = 0
    while at < len(shows):
        piece = shows[at]
        last = shows[at + 1] if at + 1 < len(shows) else None
        carried_on = (
            last is not None
            and last["block"] == piece["block"]
            and not last["moved"]
            and last["font"] == piece["font"]
            and last["size"] == piece["size"]
            and last["glyphs"] == 1
            and not spaced(last)
        )
        if spaced(piece) and carried_on:
            after = shows[at + 2] if at + 2 < len(shows) else None
            if after is not None and after["block"] != piece["block"]:
                after = None
            apart.append((piece, last, after))
            at += 2
            continue
        if spaced(piece) and piece["glyphs"] >= 2:
            whole.append(piece)
        at += 1
    return whole, apart


def pdfs_of(paths):
    for path in paths:
        if os.path.isdir(path):
            for name in sorted(os.listdir(path)):
                if name.lower().endswith(".pdf"):
                    yield os.path.join(path, name)
        else:
            yield path


def main():
    args = sys.argv[1:]
    listing = "--list" in args
    args = [arg for arg in args if arg != "--list"]
    if not args:
        print(__doc__)
        return 2
    total = collections.Counter()
    pages = 0
    for pdf in pdfs_of(args):
        shows = shows_of(pdf)
        whole, apart = strings_of(shows)
        pages += 1
        one = sum(1 for piece, _, _ in apart if piece["glyphs"] == 1)
        total["text objects"] += len(shows)
        total["whole"] += len(whole)
        total["apart"] += len(apart)
        total["apart, one glyph ahead of the last"] += one
        total["apart, two or more ahead of the last"] += len(apart) - one
        total["apart, carried on from in another font"] += sum(
            1
            for piece, _, after in apart
            if after is not None and not after["moved"] and after["font"] != piece["font"]
        )
        if apart:
            total["pages with a string apart"] += 1
            print(f"{os.path.basename(pdf)}: {len(whole)} whole, {len(apart)} apart")
            if listing:
                for piece, last, after in apart:
                    then = "nothing" if after is None else f"{after['face']} {after['text']!r}"
                    print(
                        f"    {piece['face']} {piece['size']}: {piece['text']!r} Tc {piece['tc']}"
                        f" + {last['text']!r}, then {then}"
                    )
    print(f"{pages} page(s)")
    for key in (
        "text objects",
        "whole",
        "apart",
        "apart, two or more ahead of the last",
        "apart, one glyph ahead of the last",
        "apart, carried on from in another font",
        "pages with a string apart",
    ):
        print(f"  {key}: {total[key]}")
    print(f"  spaced strings (whole + apart): {total['whole'] + total['apart']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
