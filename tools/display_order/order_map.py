"""Which record of the `.pid` is each thing a reference plot paints, in the
order the plot paints it?

Inputs: the reference PDF, the page model as JSON (a `PaperModel` dump: every
primitive with the stream and byte range of the record it was laid out from),
and the `.pid`.

Every line segment of every path the plot strokes or fills, and every run of
text it letters, is matched to the model primitive that lies there, and
through the primitive's byte range to the record: its type, oid, parent and
layer. The output is one line per painted element, in content order.

    from order_map import painted
    elements, records = painted(reference_pdf, model_json, pid)

usage: python order_map.py <reference.pdf> <model.json> <drawing.pid> <out.tsv>
"""
import io
import json
import math
import sys
from collections import Counter, defaultdict

import fitz

from pid_records import NAMES, walk

K = 72.0 / 25.4
CELL = 2.0
ON_MM = 0.25


def arc_points(center, radius, start, end, clockwise, step_mm=0.3):
    two_pi = 2.0 * math.pi
    if clockwise:
        sweep = -((start - end) % two_pi)
        if abs(sweep) < 1e-9:
            sweep = -two_pi
    else:
        sweep = (end - start) % two_pi
        if abs(sweep) < 1e-9:
            sweep = two_pi
    steps = max(4, int(abs(sweep) * radius / step_mm) + 1)
    return [
        (center[0] + radius * math.cos(start + sweep * i / steps), center[1] + radius * math.sin(start + sweep * i / steps))
        for i in range(steps + 1)
    ]


def shape_chains(shape):
    kind = shape["kind"]
    if kind == "line":
        return [[tuple(shape["start_mm"]), tuple(shape["end_mm"])]]
    if kind == "polyline":
        points = [tuple(p) for p in shape["points_mm"]]
        if shape.get("closed") and len(points) >= 2:
            points.append(points[0])
        return [points]
    if kind == "arc":
        return [arc_points(shape["center_mm"], shape["radius_mm"], shape["start_angle"], shape["end_angle"], shape.get("clockwise", False))]
    if kind == "circle":
        return [arc_points(shape["center_mm"], shape["radius_mm"], 0.0, 0.0, True)]
    if kind == "fill":
        points = [tuple(p) for p in shape["chain_mm"]]
        if len(points) >= 2:
            points.append(points[0])
        return [points]
    return []


def dist_point_seg(p, a, b):
    dx, dy = b[0] - a[0], b[1] - a[1]
    l2 = dx * dx + dy * dy
    t = 0.0 if l2 < 1e-18 else max(0.0, min(1.0, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2))
    return math.hypot(p[0] - (a[0] + dx * t), p[1] - (a[1] + dy * t))


class Index:
    def __init__(self):
        self.cells = defaultdict(list)
        self.segments = []

    def add(self, a, b, owner):
        index = len(self.segments)
        self.segments.append((a, b, owner))
        length = math.hypot(b[0] - a[0], b[1] - a[1])
        steps = max(1, int(length / (CELL / 2)) + 1)
        seen = set()
        for i in range(steps + 1):
            x = a[0] + (b[0] - a[0]) * i / steps
            y = a[1] + (b[1] - a[1]) * i / steps
            for cx in (int((x - ON_MM) // CELL), int((x + ON_MM) // CELL)):
                for cy in (int((y - ON_MM) // CELL), int((y + ON_MM) // CELL)):
                    if (cx, cy) not in seen:
                        seen.add((cx, cy))
                        self.cells[(cx, cy)].append(index)

    def near(self, p, direction=None):
        """(distance, owner) of every segment within ON_MM of p, nearest first;
        with a direction, only those running within 30 degrees of it."""
        hits = {}
        for index in self.cells.get((int(p[0] // CELL), int(p[1] // CELL)), ()):
            a, b, owner = self.segments[index]
            d = dist_point_seg(p, a, b)
            if d > ON_MM:
                continue
            if direction is not None:
                length = math.hypot(b[0] - a[0], b[1] - a[1])
                if length > 1e-9:
                    dot = abs(((b[0] - a[0]) * direction[0] + (b[1] - a[1]) * direction[1]) / length)
                    if dot < 0.866:
                        continue
            if owner not in hits or d < hits[owner]:
                hits[owner] = d
        return sorted((d, owner) for owner, d in hits.items())


def normal(text):
    return "".join(text.split())


def reference_elements(pdf):
    """Painted elements in content order: ('seg', seqno, a, b, width, white) and
    ('run', seqno, origin, text, font, size)."""
    doc = fitz.open(pdf)
    page = doc[0]
    height = page.rect.height
    out = []
    for drawing in page.get_drawings():
        colour = drawing.get("color") if drawing.get("type") != "f" else drawing.get("fill")
        white = colour == (1.0, 1.0, 1.0)
        for item in drawing["items"]:
            if item[0] == "l":
                a = (item[1].x / K, (height - item[1].y) / K)
                b = (item[2].x / K, (height - item[2].y) / K)
                out.append((drawing["seqno"], len(out), "seg", a, b, drawing.get("width") or 0.0, white, drawing["type"]))
            elif item[0] == "re":
                r = item[1]
                corners = [(r.x0, r.y0), (r.x1, r.y0), (r.x1, r.y1), (r.x0, r.y1), (r.x0, r.y0)]
                for p, q in zip(corners, corners[1:]):
                    a = (p[0] / K, (height - p[1]) / K)
                    b = (q[0] / K, (height - q[1]) / K)
                    out.append((drawing["seqno"], len(out), "seg", a, b, drawing.get("width") or 0.0, white, drawing["type"]))
    for span in page.get_texttrace():
        direction = span.get("dir", (1.0, 0.0))
        run = []
        last = None
        size = span["size"]
        for char in span["chars"]:
            origin = (char[2][0], char[2][1])
            box = char[3]
            if last is not None:
                # A new run where the next origin is not where the glyph before it ended.
                gap = math.hypot(origin[0] - last[0], origin[1] - last[1])
                if gap > 1.6 * size:
                    out.append(text_run(span, run, height, len(out)))
                    run = []
            run.append(char)
            # where this glyph ends: its origin plus its box's extent along the direction
            advance = (box[2] - box[0]) if abs(direction[0]) > abs(direction[1]) else (box[3] - box[1])
            last = (origin[0] + direction[0] * advance, origin[1] + direction[1] * advance)
        if run:
            out.append(text_run(span, run, height, len(out)))
    out.sort(key=lambda element: (element[0], element[1]))
    return out, (page.rect.width / K, height / K)


def text_run(span, run, height, order):
    origin = run[0][2]
    return (
        span["seqno"],
        order,
        "run",
        (origin[0] / K, (height - origin[1]) / K),
        "".join(chr(c[0]) for c in run),
        span["font"],
        span["size"] / K,
        tuple(span.get("dir", (1.0, 0.0))),
    )


def painted(reference_pdf, model_json, pid, stream="Sheet6"):
    records = {record["offset"]: record for record in walk(pid, stream) if record["type"] is not None}
    model = json.load(open(model_json, encoding="utf-8"))
    primitives = model["primitives"]
    index = Index()
    labels = defaultdict(list)
    for number, primitive in enumerate(primitives):
        shape = primitive["shape"]
        if shape["kind"] == "text":
            labels[normal(shape["value"])].append(number)
            continue
        for chain in shape_chains(shape):
            for a, b in zip(chain, chain[1:]):
                index.add(a, b, number)
    elements, page = reference_elements(reference_pdf)
    rows = []
    for element in elements:
        seqno, _, kind = element[0], element[1], element[2]
        if kind == "seg":
            a, b, width, white, how = element[3:]
            length = math.hypot(b[0] - a[0], b[1] - a[1])
            mid = ((a[0] + b[0]) / 2, (a[1] + b[1]) / 2)
            direction = ((b[0] - a[0]) / length, (b[1] - a[1]) / length) if length > 1e-6 else None
            hits = index.near(mid, direction) or index.near(mid)
            owner = hits[0][1] if hits else None
            also = [o for d, o in hits[1:] if d - hits[0][0] < 0.05]
            rows.append({"seqno": seqno, "kind": "fill" if how == "f" else "stroke", "at": mid, "length": length, "width": width, "white": white, "primitive": owner, "also": also, "text": ""})
        else:
            origin, text, font, size, direction = element[3:]
            want = normal(text)
            best = None
            candidates = []
            for value, numbers in labels.items():
                if not want or want not in value:
                    continue
                for number in numbers:
                    shape = primitives[number]["shape"]
                    reach = max(len(shape["value"]), 1) * (shape.get("height_mm") or 3.0) * 1.6 + 4.0
                    d = math.hypot(shape["origin_mm"][0] - origin[0], shape["origin_mm"][1] - origin[1])
                    if d <= reach:
                        # an exact spelling beats a label the run is only part of
                        candidates.append((0 if value == want else 1, d, number))
            if candidates:
                best = min(candidates)[2]
            rows.append({"seqno": seqno, "kind": "text", "at": origin, "length": 0.0, "width": 0.0, "white": False, "primitive": best, "also": [], "text": text})
    def as_object(number):
        primitive = primitives[number]
        if primitive.get("furniture"):
            return ("T", primitive["byte_range"][0] if primitive.get("byte_range") else -1)
        if primitive.get("byte_range") and primitive.get("stream_path") == "/" + stream:
            return ("D", primitive["byte_range"][0])
        return ("?", number)

    for row in rows:
        number = row["primitive"]
        row["object"] = None if number is None else as_object(number)
        row["also_objects"] = sorted({as_object(other) for other in row["also"]} - {row["object"]})
    return rows, records, primitives, page


def describe(obj, records, primitives, number):
    if obj is None:
        return "unmatched"
    if obj[0] == "T":
        return f"template@{obj[1]}"
    if obj[0] == "D":
        record = records.get(obj[1])
        if record is None:
            return f"record@{obj[1]}?"
        symbol = primitives[number].get("symbol") or ""
        name = symbol.replace("\\", "/").split("/")[-1]
        return f"{NAMES.get(record['type'], hex(record['type']))} oid {record['oid']} parent {record['parent']} layer {record['layer']} @{obj[1]} {name}".rstrip()
    return f"primitive {obj[1]}"


if __name__ == "__main__":
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8")
    reference_pdf, model_json, pid, out = sys.argv[1:5]
    rows, records, primitives, page = painted(reference_pdf, model_json, pid)
    with open(out, "w", encoding="utf-8") as handle:
        handle.write("seqno\tkind\tx\ty\tlength_mm\twidth_pt\tpaint\tprimitive\tdomain\toffset\ttype\toid\tparent\tlayer\tsymbol\ttext\talso\n")
        for row in rows:
            obj = row["object"]
            record = records.get(obj[1]) if obj and obj[0] == "D" else None
            number = row["primitive"]
            symbol = (primitives[number].get("symbol") or "").replace("\\", "/").split("/")[-1] if number is not None else ""
            handle.write(
                "\t".join(
                    str(v)
                    for v in (
                        row["seqno"],
                        row["kind"],
                        f"{row['at'][0]:.3f}",
                        f"{row['at'][1]:.3f}",
                        f"{row['length']:.3f}",
                        f"{row['width']:.2f}",
                        "white" if row["white"] else "ink",
                        "" if number is None else number,
                        obj[0] if obj else "",
                        obj[1] if obj else "",
                        "" if record is None else f"{record['type']:#06x}",
                        "" if record is None else record["oid"],
                        "" if record is None else record["parent"],
                        "" if record is None else record["layer"],
                        symbol,
                        row["text"],
                        ",".join(f"{d}{o}" for d, o in row["also_objects"]),
                    )
                )
                + "\n"
            )
    matched = sum(1 for row in rows if row["object"] is not None)
    kinds = Counter((row["kind"], row["object"][0] if row["object"] else "unmatched") for row in rows)
    print(f"{len(rows)} painted element(s) on {page[0]:.1f} x {page[1]:.1f} mm; {matched} matched to a primitive")
    for key, count in sorted(kinds.items()):
        print(f"   {count:6d}  {key[0]:<7} {key[1]}")
