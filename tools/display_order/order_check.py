r"""Does the sheet's child table say what order a reference plot paints in?

For each sheet: every painted element of the reference plot (a line segment
of a stroked or filled path, a run of text) is matched to the record of the
`.pid` that was laid out there (`order_map.painted`), and the record is given
its place in the order the hypothesis states:

    the index, in the JSheet's child table, of the record's top-level object

A record whose parent is the sheet is its own top-level object; any other
record's top-level object is its parent, a DependencyObject. An element of
the border template takes the place of the sheet's border frame.

Reported per sheet:

  elements     painted elements matched to a record or to the template
  in order     the longest run of them, in painting order, whose places never
               fall (strict: each element is the record nearest it; lenient:
               where two records lie on the same ink, either may be the one)
  objects      top-level objects of the drawing the plot paints, and how many
               of them first appear in the table's order
  by oid, by stream
               the same count if the order were the records' oids, or their
               place in the sheet stream

usage: python order_check.py <corpus dir> <models dir> [--list N] [<name part> ...]
"""
import bisect
import io
import struct
import sys
from collections import Counter, defaultdict
from pathlib import Path

from jsheet_list import sheet_children
from order_map import painted
from pid_records import NAMES, walk

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


def longest_nondecreasing_choices(choices):
    """Longest non-decreasing chain taking at most one key from each set."""
    tails = []
    for keys in choices:
        updates = []
        for key in sorted(set(keys), reverse=True):
            at = bisect.bisect_right(tails, key)
            updates.append((at, key))
        for at, key in updates:
            if at == len(tails):
                tails.append(key)
            elif key < tails[at]:
                tails[at] = key
    return len(tails)


def dependents_in_order(record, owned):
    payload = record["payload"]
    order = []
    at = 12
    while at + 4 <= len(payload):
        value = struct.unpack_from("<I", payload, at)[0]
        if value in owned and value not in order:
            order.append(value)
            at += 4
        else:
            at += 2
    return order


def check(reference_pdf, model_json, pid, sheet=6, listing=0):
    rows, records_by_offset, primitives, page = painted(reference_pdf, model_json, pid)
    records = list(records_by_offset.values())
    by_oid = {record["oid"]: record for record in records}
    children_of = defaultdict(set)
    for record in records:
        children_of[record["parent"]].add(record["oid"])
    table = sheet_children(pid, sheet)
    place = {oid: index for index, (oid, _, _) in enumerate(table["entries"])}
    frame = next((index for index, (oid, _, _) in enumerate(table["entries"]) if oid in by_oid and by_oid[oid]["type"] == 0x003D), None)
    rank = {}
    for record in records:
        if record["type"] == 0x00FA:
            for index, oid in enumerate(dependents_in_order(record, children_of[record["oid"]])):
                rank[oid] = index

    def place_of(obj):
        if obj is None:
            return None
        if obj[0] == "T":
            return frame
        if obj[0] == "D":
            record = records_by_offset.get(obj[1])
            if record is None:
                return None
            top = record["oid"] if record["parent"] == sheet else record["parent"]
            return place.get(top)
        return None

    sequence = []
    unplaced = Counter()
    for row in rows:
        here = place_of(row["object"])
        if here is None:
            unplaced["unmatched" if row["object"] is None else "not in the table"] += 1
            continue
        others = [p for p in (place_of(obj) for obj in row["also_objects"]) if p is not None]
        record = records_by_offset.get(row["object"][1]) if row["object"][0] == "D" else None
        sequence.append((here, others, row, record))
    strict = longest_nondecreasing([item[0] for item in sequence])
    lenient = longest_nondecreasing_choices([[item[0]] + item[1] for item in sequence])
    # the drawing's records, each at its first appearance
    seen = set()
    firsts = []
    for here, _, row, record in sequence:
        if record is not None and record["oid"] not in seen:
            seen.add(record["oid"])
            firsts.append((here, record))
    tops_seen = set()
    tops = []
    for here, _, row, record in sequence:
        if record is not None and here not in tops_seen:
            tops_seen.add(here)
            tops.append(here)
    result = {
        "page": page,
        "painted": len(rows),
        "elements": len(sequence),
        "unplaced": dict(unplaced),
        "strict": strict,
        "lenient": lenient,
        "records": len(firsts),
        "records_by_table": longest_nondecreasing([here for here, _ in firsts]),
        "records_by_oid": longest_nondecreasing([record["oid"] for _, record in firsts]),
        "records_by_stream": longest_nondecreasing([record["offset"] for _, record in firsts]),
        "tops": len(tops),
        "tops_in_order": longest_nondecreasing(tops),
        "entries": len(table["entries"]),
        "frame_place": frame,
        "first_place": sequence[0][0] if sequence else None,
    }
    # inner order: within one visit of a DependencyObject, do its dependents come in payload order?
    groups = 0
    groups_in_order = 0
    current = None
    ranks = []

    def close():
        nonlocal groups, groups_in_order
        distinct = []
        for value in ranks:
            if not distinct or distinct[-1] != value:
                distinct.append(value)
        if len(set(distinct)) >= 2:
            groups += 1
            if distinct == sorted(distinct) and len(distinct) == len(set(distinct)):
                groups_in_order += 1

    for here, _, row, record in sequence:
        if record is None or record["parent"] == sheet or row["kind"] == "fill":
            if current is not None:
                close()
            current, ranks = None, []
            continue
        if record["parent"] != current:
            if current is not None:
                close()
            current, ranks = record["parent"], []
        ranks.append(rank.get(record["oid"], -1))
    if current is not None:
        close()
    result["groups_of_dependents"] = groups
    result["groups_in_payload_order"] = groups_in_order
    if listing:
        high = -1
        shown = 0
        for here, others, row, record in sequence:
            if here < high and all(other < high for other in others):
                if shown < listing:
                    name = "template" if record is None else f"{NAMES.get(record['type'], hex(record['type']))} oid {record['oid']} parent {record['parent']}"
                    print(f"      falls back: seq {row['seqno']} {row['kind']} {row['length']:.3f} mm at ({row['at'][0]:.2f}, {row['at'][1]:.2f}) {row['text']!r} is {name}, place {here}, after place {high}")
                shown += 1
            else:
                high = max(high, min([here] + [other for other in others if other >= high]) if here < high else here)
        result["falls_back_greedy"] = shown
    return result


if __name__ == "__main__":
    args = sys.argv[1:]
    listing = 0
    if "--list" in args:
        at = args.index("--list")
        listing = int(args[at + 1])
        del args[at : at + 2]
    corpus, models = Path(args[0]), Path(args[1])
    wanted = args[2:]
    totals = Counter()
    print("sheet | painted | placed | in order strict | lenient | records | by table | by oid | by stream | top-level objects in order | frame at | first painted")
    for model in sorted(models.glob("*.model.json")):
        stem = model.name[: -len(".model.json")]
        if wanted and not any(part in stem for part in wanted):
            continue
        result = check(str(corpus / f"{stem}.pdf"), str(model), str(corpus / f"{stem}.pid"), listing=listing)
        print(
            f"{stem} | {result['painted']} | {result['elements']} | {result['strict']} | {result['lenient']} | {result['records']} | "
            f"{result['records_by_table']} | {result['records_by_oid']} | {result['records_by_stream']} | {result['tops_in_order']} of {result['tops']} | "
            f"{result['frame_place']} | {result['first_place']} | dependents' groups in payload order {result['groups_in_payload_order']} of {result['groups_of_dependents']} | unplaced {result['unplaced']}"
        )
        for key in ("painted", "elements", "strict", "lenient", "records", "records_by_table", "records_by_oid", "records_by_stream", "tops", "tops_in_order", "groups_of_dependents", "groups_in_payload_order"):
            totals[key] += result[key]
    print("in all | " + " | ".join(f"{key} {value}" for key, value in totals.items()))
