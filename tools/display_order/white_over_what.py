"""What each white stroke of a reference plot is drawn over, and what is drawn
over it.

A white stroke un-inks whatever was painted before it where it lies; ink
painted after it covers it again. For every white stroke this finds the inked
strokes and the glyphs whose ink it touches, and files each by whether the
plot paints it before the white stroke (the stroke takes a nick out of it) or
after (it does not).

usage: python white_over_what.py <pdf> [<pdf> ...]
"""
import io
import math
import sys

import fitz

sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8")


def seg_dist(a, b, c, d):
    """Distance between segments a-b and c-d."""

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


total = {"strokes before": 0, "strokes after": 0, "glyphs before": 0, "glyphs after": 0, "whites": 0, "touching nothing": 0}
for path in sys.argv[1:]:
    doc = fitz.open(path)
    page = doc[0]
    drawings = page.get_drawings()
    whites = []
    inks = []
    for drawing in drawings:
        if drawing.get("type") not in ("s", "fs"):
            continue
        segments = [((i[1].x, i[1].y), (i[2].x, i[2].y)) for i in drawing["items"] if i[0] == "l"]
        entry = (drawing["seqno"], drawing.get("width") or 0.0, segments)
        if drawing.get("color") == (1.0, 1.0, 1.0):
            whites.append(entry)
        else:
            inks.append(entry)
    glyphs = []
    for span in page.get_texttrace():
        for char in span["chars"]:
            if chr(char[0]).strip():
                glyphs.append((span["seqno"], char[3]))
    counts = {"strokes before": 0, "strokes after": 0, "glyphs before": 0, "glyphs after": 0}
    alone = 0
    for seqno, width, segments in whites:
        touched = False
        for other_seq, other_width, other_segments in inks:
            reach = (width + other_width) / 2.0
            if any(seg_dist(a, b, c, d) < reach - 0.01 for a, b in segments for c, d in other_segments):
                counts["strokes before" if other_seq < seqno else "strokes after"] += 1
                touched = True
        for glyph_seq, bbox in glyphs:
            # A glyph's ink is well inside its box: hold the stroke's centre line to the box shrunk by a fifth.
            x0, y0, x1, y1 = bbox
            inset_x, inset_y = (x1 - x0) * 0.2, (y1 - y0) * 0.2
            rect = (x0 + inset_x, y0 + inset_y, x1 - inset_x, y1 - inset_y)
            if any(seg_rect(a, b, rect) < width / 2.0 for a, b in segments):
                counts["glyphs before" if glyph_seq < seqno else "glyphs after"] += 1
                touched = True
        alone += not touched
    name = path.split("\\")[-1]
    print(f"{name}: {len(whites)} white stroke(s); touching {counts}; {alone} touch nothing")
    for key, value in counts.items():
        total[key] += value
    total["whites"] += len(whites)
    total["touching nothing"] += alone
print(f"in all: {total}")
