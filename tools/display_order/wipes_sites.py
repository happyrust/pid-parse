r"""The sites where a reference plot paints a stroke in the paper's colour on
ink, and whether our page paints the same two things in the same order.

A stroke in the paper's colour takes a piece out of the ink painted before it
and none out of the ink painted after it, so which of the two comes first is
visible on the paper. A *site* is one white path of the reference and one
piece of ink it touches — an inked stroke whose pen overlaps it, or a glyph
whose box it runs into (the same test `white_over_what.py` counted 91 sites
with on the eleven references).

For each site the two things are looked up on our page by where they are — a
stroke by its two ends, a glyph by its character and origin, the white path
by its segments — and our order is read from our own content stream:

  same        ours paints the ink on the same side of the white stroke
  apart       ours paints it on the other side
  not on ours the white path, or the ink, has no counterpart on our page

usage: python wipes_sites.py <reference.pdf> <ours.pdf> [--list] [--tol mm]
"""
import io
import math
import sys

import fitz

sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8")
K = 72.0 / 25.4


def seg_dist(a, b, c, d):
    def point_seg(p, s, e):
        dx, dy = e[0] - s[0], e[1] - s[1]
        l2 = dx * dx + dy * dy
        t = 0.0 if l2 < 1e-18 else max(0.0, min(1.0, ((p[0] - s[0]) * dx + (p[1] - s[1]) * dy) / l2))
        return math.hypot(p[0] - (s[0] + dx * t), p[1] - (s[1] + dy * t))

    def cross(o, p, q):
        return (p[0] - o[0]) * (q[1] - o[1]) - (p[1] - o[1]) * (q[0] - o[0])

    d1, d2, d3, d4 = cross(c, d, a), cross(c, d, b), cross(a, b, c), cross(a, b, d)
    if ((d1 > 0) != (d2 > 0)) and ((d3 > 0) != (d4 > 0)):
        return 0.0
    return min(point_seg(a, c, d), point_seg(b, c, d), point_seg(c, a, b), point_seg(d, a, b))


def seg_rect(a, b, rect):
    x0, y0, x1, y1 = rect
    if x0 <= a[0] <= x1 and y0 <= a[1] <= y1:
        return 0.0
    edges = [((x0, y0), (x1, y0)), ((x1, y0), (x1, y1)), ((x1, y1), (x0, y1)), ((x0, y1), (x0, y0))]
    return min(seg_dist(a, b, e[0], e[1]) for e in edges)


def page_content(path):
    """White paths, inked segments and glyphs of the first page, in points from the page's lower left."""
    page = fitz.open(path)[0]
    height = page.rect.height
    up = lambda p: (p[0], height - p[1])
    whites, inks, glyphs = [], [], []
    for drawing in page.get_drawings():
        if drawing.get("type") not in ("s", "fs"):
            continue
        segments = [(up((i[1].x, i[1].y)), up((i[2].x, i[2].y))) for i in drawing["items"] if i[0] == "l"]
        if not segments:
            continue
        width = drawing.get("width") or 0.0
        if drawing.get("color") == (1.0, 1.0, 1.0):
            whites.append({"seq": drawing["seqno"], "width": width, "segments": segments})
        else:
            for segment in segments:
                inks.append({"seq": drawing["seqno"], "width": width, "segment": segment})
    for span in page.get_texttrace():
        for char in span["chars"]:
            if chr(char[0]).strip():
                x0, y0, x1, y1 = char[3]
                glyphs.append({"seq": span["seqno"], "char": chr(char[0]), "origin": up(char[2]), "box": (x0, height - y1, x1, height - y0)})
    return whites, inks, glyphs


def touches_stroke(white, ink):
    reach = (white["width"] + ink["width"]) / 2.0
    c, d = ink["segment"]
    return any(seg_dist(a, b, c, d) < reach - 0.01 for a, b in white["segments"])


def touches_glyph(white, glyph):
    x0, y0, x1, y1 = glyph["box"]
    inset_x, inset_y = (x1 - x0) * 0.2, (y1 - y0) * 0.2
    rect = (x0 + inset_x, y0 + inset_y, x1 - inset_x, y1 - inset_y)
    return any(seg_rect(a, b, rect) < white["width"] / 2.0 for a, b in white["segments"])


def near(p, q, tol):
    return math.hypot(p[0] - q[0], p[1] - q[1]) <= tol


def same_segment(s, t, tol):
    return (near(s[0], t[0], tol) and near(s[1], t[1], tol)) or (near(s[0], t[1], tol) and near(s[1], t[0], tol))


def compare(reference_pdf, our_pdf, listing=False, tol_mm=0.5, shift_mm=(0.0, 0.0)):
    tol = tol_mm * K
    ref_whites, ref_inks, ref_glyphs = page_content(reference_pdf)
    our_whites, our_inks, our_glyphs = page_content(our_pdf)
    sx, sy = shift_mm[0] * K, shift_mm[1] * K
    moved = lambda p: (p[0] + sx, p[1] + sy)
    our_white_segments = [(white["seq"], segment) for white in our_whites for segment in white["segments"]]
    result = {"sites": 0, "ink before the white stroke": 0, "same": 0, "apart": 0, "not on ours": 0}
    for white in ref_whites:
        # Our strokes in the paper's colour that are this path's segments.
        ours_of_mark = [seq for seq, segment in our_white_segments if any(same_segment((moved(a), moved(b)), segment, tol) for a, b in white["segments"])]
        sites = [("stroke", ink) for ink in ref_inks if touches_stroke(white, ink)]
        sites += [("glyph", glyph) for glyph in ref_glyphs if touches_glyph(white, glyph)]
        for kind, ink in sites:
            result["sites"] += 1
            before = ink["seq"] < white["seq"]
            result["ink before the white stroke"] += before
            if kind == "stroke":
                target = (moved(ink["segment"][0]), moved(ink["segment"][1]))
                found = [o["seq"] for o in our_inks if same_segment(target, o["segment"], tol)]
                where = f"stroke ({target[0][0] / K:.2f}, {target[0][1] / K:.2f})-({target[1][0] / K:.2f}, {target[1][1] / K:.2f}) mm"
            else:
                target = moved(ink["origin"])
                found = [o["seq"] for o in our_glyphs if o["char"] == ink["char"] and near(target, o["origin"], tol)]
                where = f"glyph {ink['char']!r} at ({target[0] / K:.2f}, {target[1] / K:.2f}) mm"
            at = white["segments"][0][0]
            mark = f"white path at ({at[0] / K:.2f}, {at[1] / K:.2f}) mm"
            if not ours_of_mark or not found:
                result["not on ours"] += 1
                if listing:
                    missing = "the white path" if not ours_of_mark else "the ink"
                    print(f"      not on ours ({missing}): {mark}; {where}; the reference paints the ink {'before' if before else 'after'} it")
                continue
            # Ours is before when the ink is painted ahead of every stroke of the mark, after when behind every one.
            ours_before = all(seq < mark_seq for seq in found for mark_seq in ours_of_mark)
            ours_after = all(seq > mark_seq for seq in found for mark_seq in ours_of_mark)
            if (before and ours_before) or (not before and ours_after):
                result["same"] += 1
            else:
                result["apart"] += 1
                if listing:
                    print(f"      apart: {mark}; {where}; the reference paints the ink {'before' if before else 'after'} it, ours {'before' if ours_before else 'after' if ours_after else 'in between'}")
    return result


if __name__ == "__main__":
    args = sys.argv[1:]
    listing = "--list" in args
    args = [a for a in args if a != "--list"]
    tol = 0.5
    if "--tol" in args:
        at = args.index("--tol")
        tol = float(args[at + 1])
        del args[at : at + 2]
    print(compare(args[0], args[1], listing, tol))
