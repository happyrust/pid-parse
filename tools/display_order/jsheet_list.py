r"""The child table of a `JSheet` (PSM type 0x0114) in a document's
`PSMcluster0`: the sheet's top-level objects, each with a range box.

The record's payload is the sheet's oid, its parent, a few fixed words, a
count, and then one 14-byte entry per child:

    u32 oid, i16 x0, i16 y0, i16 x1, i16 y1, u16 flags

The box is in 1/16384 m. The entries' order is the question this is for.

    from jsheet_list import sheet_children
    children = sheet_children(r"...\drawing.pid", 6)   # [(oid, (x0, y0, x1, y1), flags), ...]

usage: python jsheet_list.py <drawing.pid> [storage/]<sheet oid>
"""
import io
import struct
import sys

import olefile

UNIT_MM = 1000.0 / 16384.0


def find_sheet_record(data, sheet_oid):
    """(payload start, payload end) of the 0x0114 record whose oid is `sheet_oid`."""
    needle = struct.pack("<I", sheet_oid)
    at = data.find(needle)
    while at != -1:
        if at >= 6:
            type_word, follow = struct.unpack_from("<HI", data, at - 6)
            if type_word & 0x3FFF == 0x0114 and at + follow <= len(data) and follow >= 28:
                return at, at + follow
        at = data.find(needle, at + 1)
    return None


def sheet_children(path, sheet_oid, storage=""):
    ole = olefile.OleFileIO(path)
    name = (storage.split("/") if storage else []) + ["PSMcluster0"]
    data = ole.openstream(name).read()
    found = find_sheet_record(data, sheet_oid)
    if found is None:
        return None
    start, end = found
    payload = data[start:end]
    count = struct.unpack_from("<I", payload, 24)[0]
    entries = []
    at = 28
    for _ in range(count):
        if at + 14 > len(payload):
            break
        oid, x0, y0, x1, y1, flags = struct.unpack_from("<IhhhhH", payload, at)
        entries.append((oid, (x0, y0, x1, y1), flags))
        at += 14
    return {"count": count, "entries": entries, "header": payload[:28], "trailer": payload[at:], "payload_len": len(payload)}


if __name__ == "__main__":
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8")
    from pid_records import NAMES, walk

    path = sys.argv[1]
    where = sys.argv[2] if len(sys.argv) > 2 else "6"
    storage, _, oid = where.rpartition("/")
    sheet = int(oid)
    table = sheet_children(path, sheet, storage)
    if table is None:
        print(f"no 0x0114 record with oid {sheet}")
        sys.exit(1)
    stream = (storage + "/" if storage else "") + f"Sheet{sheet}"
    records = {record["oid"]: record for record in walk(path, stream) if record["type"] is not None}
    print(f"JSheet {sheet}: payload {table['payload_len']} bytes, header {table['header'].hex(' ')}")
    print(f"   count {table['count']}, {len(table['entries'])} entries read, {len(table['trailer'])} byte(s) after them: {table['trailer'][:48].hex(' ')}")
    known = sum(1 for oid, _, _ in table["entries"] if oid in records)
    print(f"   {known} of the entries name a record of {stream} ({len(records)} records on its chain)")
    listed = {oid for oid, _, _ in table["entries"]}
    top = [r for r in records.values() if r["parent"] == sheet]
    print(f"   records of {stream} whose parent is the sheet: {len(top)}; of them listed {sum(1 for r in top if r['oid'] in listed)}")
    not_top = [oid for oid in listed if oid in records and records[oid]["parent"] != sheet]
    print(f"   listed records whose parent is not the sheet: {len(not_top)} {not_top[:10]}")
    for index, (oid, box, flags) in enumerate(table["entries"][:40]):
        record = records.get(oid)
        kind = NAMES.get(record["type"], hex(record["type"])) if record else "?"
        mm = tuple(round(v * UNIT_MM, 2) for v in box)
        empty = box == (32767, 32767, -32768, -32768)
        print(f"   [{index:4d}] oid {oid:6d} {kind:<18} flags {flags:#06x} box {'empty' if empty else mm}")
