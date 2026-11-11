r"""Is a JSheet child's range box in `scale × 2⁻¹⁵` m, `scale` being the f64 at
payload +12?

For every child the paper model draws strokes for (its own, or its
dependents'), the bounding box of those strokes in paper millimetres is set
against the entry's four words under each candidate unit. A unit is right
when the entry's box holds the strokes and is not much larger: the report is
the worst and the median of (box − strokes) per side.

usage: python jsheet_box_unit.py <corpus dir> <models dir> [<name part> ...]
"""
import io
import json
import math
import statistics
import struct
import sys
from pathlib import Path

from jsheet_list import sheet_children
from pid_records import walk

sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8")


def points_of(shape):
    kind = shape["kind"]
    if kind == "line":
        return [shape["start_mm"], shape["end_mm"]]
    if kind in ("polyline", "fill"):
        return shape.get("points_mm") or shape.get("chain_mm") or []
    if kind in ("arc", "circle"):
        (x, y), r = shape["center_mm"], shape["radius_mm"]
        return [[x - r, y - r], [x + r, y + r]] if kind == "circle" else []
    return []


corpus, models = Path(sys.argv[1]), Path(sys.argv[2])
wanted = sys.argv[3:]
print("sheet | scale | children with strokes | unit scale×2⁻¹⁵ m: boxes holding their strokes within 1 unit, median slack mm, worst miss mm | unit 2⁻¹⁴ m: the same")
for model_path in sorted(models.glob("*.model.json")):
    stem = model_path.name[: -len(".model.json")]
    if wanted and not any(part in stem for part in wanted):
        continue
    pid = str(corpus / f"{stem}.pid")
    model = json.load(open(model_path, encoding="utf-8"))
    records = [r for r in walk(pid, "Sheet6") if r["type"] is not None]
    by_offset = {r["offset"]: r for r in records}
    table = sheet_children(pid, 6)
    scale = struct.unpack_from("<d", table["header"], 12)[0]
    boxes = {oid: box for oid, box, _ in table["entries"]}
    extent = {}
    for primitive in model["primitives"]:
        if primitive.get("furniture") or primitive.get("stream_path") != "/Sheet6":
            continue
        record = by_offset.get(primitive["byte_range"][0])
        if record is None:
            continue
        top = record["oid"] if record["parent"] == 6 else record["parent"]
        if top not in boxes:
            continue
        for x, y in points_of(primitive["shape"]):
            box = extent.setdefault(top, [math.inf, math.inf, -math.inf, -math.inf])
            box[0], box[1], box[2], box[3] = min(box[0], x), min(box[1], y), max(box[2], x), max(box[3], y)
    line = [stem[:24], f"{scale:g}", str(len(extent))]
    for unit_mm in (scale * 1000.0 / 32768.0, 1000.0 / 16384.0):
        holding = 0
        slack = []
        worst = 0.0
        for oid, (x0, y0, x1, y1) in extent.items():
            bx0, by0, bx1, by1 = (value * unit_mm for value in boxes[oid])
            if boxes[oid] == (32767, 32767, -32768, -32768):
                continue
            # positive = the entry's box lies outside the strokes on that side
            sides = [x0 - bx0, y0 - by0, bx1 - x1, by1 - y1]
            miss = -min(sides)
            worst = max(worst, miss)
            if miss <= unit_mm:
                holding += 1
            slack.extend(sides)
        median = statistics.median(slack) if slack else float("nan")
        line.append(f"{holding} of {len(extent)}, median slack {median:.3f}, worst miss {worst:.3f}")
    print(" | ".join(line))
