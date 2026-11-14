r"""Is the order a reference plot paints in the order of the sheet's layers, or
of where the objects lie?

For each sheet: every painted element of the reference plot is matched to its
record (`order_map.painted`), and for the records in the order they are first
painted this prints

  layers   how many layers the painted records sit on (payload +8), and how
           many runs of one layer the painting order has: a plot that drew
           layer by layer would have as many runs as layers
  by layer the longest run of records whose layer oids never fall
  hops     the median distance, in millimetres, from one record's first
           painted element to the next record's, against the same median
           for the records taken in oid order: a plot that drew neighbour
           after neighbour would hop much less than the oid order does

usage: python order_layers.py <corpus dir> <models dir> [<name part> ...]
"""
import bisect
import io
import math
import statistics
import struct
import sys
from pathlib import Path

from order_map import painted

sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8")


def longest_nondecreasing(keys):
    tails = []
    for key in keys:
        at = bisect.bisect_right(tails, key)
        if at == len(tails):
            tails.append(key)
        else:
            tails[at] = key
    return len(tails)


corpus, models = Path(sys.argv[1]), Path(sys.argv[2])
wanted = sys.argv[3:]
print("sheet | records painted | layers | runs of one layer | by layer | median hop mm in painting order | in oid order")
for model in sorted(models.glob("*.model.json")):
    stem = model.name[: -len(".model.json")]
    if wanted and not any(part in stem for part in wanted):
        continue
    rows, records_by_offset, primitives, page = painted(str(corpus / f"{stem}.pdf"), str(model), str(corpus / f"{stem}.pid"))
    seen = set()
    firsts = []
    for row in rows:
        obj = row["object"]
        if obj is None or obj[0] != "D":
            continue
        record = records_by_offset.get(obj[1])
        if record is None or record["oid"] in seen:
            continue
        seen.add(record["oid"])
        layer = struct.unpack_from("<I", record["payload"], 8)[0] if len(record["payload"]) >= 12 else None
        firsts.append((record, layer, row["at"]))
    layers = [layer for _, layer, _ in firsts]
    runs = sum(1 for index, layer in enumerate(layers) if index == 0 or layer != layers[index - 1])
    hops = [math.hypot(b[2][0] - a[2][0], b[2][1] - a[2][1]) for a, b in zip(firsts, firsts[1:])]
    by_oid = sorted(firsts, key=lambda item: item[0]["oid"])
    oid_hops = [math.hypot(b[2][0] - a[2][0], b[2][1] - a[2][1]) for a, b in zip(by_oid, by_oid[1:])]
    print(
        f"{stem[:26]} | {len(firsts)} | {len(set(layers))} | {runs} | {longest_nondecreasing([layer or 0 for layer in layers])} | "
        f"{statistics.median(hops):.1f} | {statistics.median(oid_hops):.1f}"
    )
