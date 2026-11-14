r"""Who owns the records of a sheet stream, and does the JSheet table list
exactly the sheet's own: for drawings and for templates, on disk or inside
a RefData zip.

usage: python owners_census.py <file.pid|file.igr|RefData.zip> [...]

A zip is read member by member (every `.igr`), in memory.
"""
import io
import struct
import sys
import zipfile
from collections import Counter
from pathlib import Path

import olefile

NAMES = {
    0x0018: "igLine2d", 0x0084: "igLineString2d", 0x005E: "igPoint2d", 0x004D: "igTextBox",
    0x00CE: "igSymbol2d", 0x0061: "igArc2d", 0x0059: "igCircle2d", 0x0020: "igRectangle2d",
    0x005D: "igBspCurve2d", 0x003D: "igSmartFrame2d", 0x0013: "igBoundary2d",
    0x00FA: "DependencyObject", 0x007E: "igEllipticalArc2d", 0x0063: "igEllipse2d",
    0x0021: "ComplexString", 0x007B: "Group", 0x0019: "KeyPointConstraint", 0x0115: "igDimension",
}
name = lambda t: NAMES.get(t, f"{t:#06x}")
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8")


def walk(data):
    offset = 8
    while offset + 6 <= len(data):
        type_word, follow = struct.unpack_from("<HI", data, offset)
        end = offset + 6 + follow
        if follow == 0 or end > len(data):
            break
        payload = data[offset + 6:end]
        if len(payload) >= 12:
            oid, parent, layer = struct.unpack_from("<III", payload, 0)
            yield {"at": offset, "type": type_word & 0x3FFF, "skip": bool(type_word & 0x8000), "oid": oid, "parent": parent, "payload": payload}
        offset = end


def table_of(cluster, sheet):
    needle = struct.pack("<I", sheet)
    at = cluster.find(needle)
    while at != -1:
        if at >= 6:
            type_word, follow = struct.unpack_from("<HI", cluster, at - 6)
            if type_word == 0x0114 and at + follow <= len(cluster) and follow >= 28:
                payload = cluster[at:at + follow]
                count = struct.unpack_from("<I", payload, 24)[0]
                if 28 + 14 * count <= len(payload):
                    return [struct.unpack_from("<I", payload, 28 + 14 * i)[0] for i in range(count)]
        at = cluster.find(needle, at + 1)
    return None


def census(label, blob, totals):
    try:
        ole = olefile.OleFileIO(io.BytesIO(blob))
    except Exception as error:  # noqa: BLE001
        print(f"{label}: not a compound file ({error})")
        return
    cluster = ole.openstream(["PSMcluster0"]).read() if ole.exists("PSMcluster0") else b""
    sheets = sorted(int(e[0][5:]) for e in ole.listdir() if len(e) == 1 and e[0].startswith("Sheet") and e[0][5:].isdigit())
    for sheet in sheets:
        records = [r for r in walk(ole.openstream([f"Sheet{sheet}"]).read()) if not r["skip"]]
        by_oid = {r["oid"]: r for r in records}
        owned = {r["oid"] for r in records if r["parent"] == sheet}
        listed = table_of(cluster, sheet)
        pairs = Counter()
        for r in records:
            if r["parent"] == sheet:
                continue
            parent = by_oid.get(r["parent"])
            pairs[(name(r["type"]), name(parent["type"]) if parent else "absent", (parent["parent"] == sheet) if parent else None)] += 1
        if listed is None:
            verdict = "no table"
        else:
            both = len(owned & set(listed))
            verdict = f"listed {len(listed)} owned {len(owned)} both {both}" + (" EXACT" if both == len(listed) == len(owned) else " DIFFERS")
        totals["sheets"] += 1
        totals["exact"] += verdict.endswith("EXACT")
        print(f"{label} /Sheet{sheet}: {len(records)} live; {verdict}")
        for (child, parent, parent_top), count in sorted(pairs.items()):
            totals["pairs"][(child, parent, parent_top)] += count
            print(f"      {count:5d}  {child:<18} owned by {parent}" + ("" if parent_top is None else (" (a child of the sheet)" if parent_top else " (NOT a child of the sheet)")))


totals = {"sheets": 0, "exact": 0, "pairs": Counter()}
for arg in sys.argv[1:]:
    path = Path(arg)
    if path.suffix.lower() == ".zip":
        with zipfile.ZipFile(path) as archive:
            for index, info in enumerate(archive.infolist()):
                try:
                    member = info.filename.encode("cp437").decode("gbk", "replace") if not info.flag_bits & 0x800 else info.filename
                except UnicodeEncodeError:
                    member = info.filename
                if member.lower().endswith(".igr"):
                    census(f"{path.parent.name}/{path.name}!{member}", archive.read(info), totals)
    else:
        census(path.name, path.read_bytes(), totals)
print(f"in all: {totals['sheets']} sheet streams, {totals['exact']} whose table lists exactly the records the sheet owns")
for (child, parent, parent_top), count in sorted(totals["pairs"].items()):
    print(f"   {count:6d}  {child:<18} owned by {parent}" + ("" if parent_top is None else (" (a child of the sheet)" if parent_top else " (NOT a child of the sheet)")))
