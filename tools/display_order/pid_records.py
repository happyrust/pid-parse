r"""Walk the PSM record chain of a `.pid` sheet stream.

A sheet stream is an 8-byte header followed by records, each a 6-byte
envelope (`u16` type word, `u32` bytes to follow) and its payload. The payload
of a graphic record starts with its oid, its parent's oid and the oid of the
sheet layer it sits on.

    from pid_records import walk
    for record in walk(r"...\drawing.pid", "Sheet6"): ...

usage: python pid_records.py <drawing.pid> [stream]
"""
import io
import struct
import sys
from collections import Counter

import olefile

NAMES = {
    0x0018: "igLine2d",
    0x0084: "igLineString2d",
    0x005E: "igPoint2d",
    0x004D: "igTextBox",
    0x00CE: "igSymbol2d",
    0x0061: "igArc2d",
    0x0059: "igCircle2d",
    0x0020: "igRectangle2d",
    0x005D: "igBspCurve2d",
    0x003D: "igSmartFrame2d",
    0x0013: "igBoundary2d",
    0x0030: "JStyleOverride",
    0x00FA: "DependencyObject",
    0x0010: "sub-record 0x0010",
    0x3FE6: "GLine2d",
    0x007E: "igEllipticalArc2d",
    0x0063: "igEllipse2d",
}


def streams(path):
    ole = olefile.OleFileIO(path)
    return ole, ["/".join(entry) for entry in ole.listdir()]


def walk(path, stream="Sheet6", start=8):
    ole = olefile.OleFileIO(path)
    data = ole.openstream(stream.split("/")).read()
    offset = start
    index = 0
    while offset + 6 <= len(data):
        type_word, follow = struct.unpack_from("<HI", data, offset)
        end = offset + 6 + follow
        if end > len(data) or follow == 0 and type_word == 0:
            break
        payload = data[offset + 6 : end]
        oid = parent = layer = None
        if len(payload) >= 12:
            oid, parent, layer = struct.unpack_from("<III", payload, 0)
        yield {
            "index": index,
            "offset": offset,
            "end": end,
            "type": type_word & 0x3FFF,
            "flags": type_word >> 14,
            "oid": oid,
            "parent": parent,
            "layer": layer,
            "payload": payload,
        }
        index += 1
        offset = end
    yield {"index": index, "offset": offset, "end": len(data), "type": None, "tail": len(data) - offset}


if __name__ == "__main__":
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8")
    path = sys.argv[1]
    if len(sys.argv) < 3:
        ole, names = streams(path)
        for name in names:
            print(f"{ole.get_size(name.split('/')):10d}  {name}")
        sys.exit(0)
    stream = sys.argv[2]
    records = list(walk(path, stream))
    tail = records.pop()
    counts = Counter(record["type"] for record in records)
    print(f"{stream}: {len(records)} record(s) on the chain; {tail['tail']} byte(s) after the last one")
    for type_code, count in sorted(counts.items(), key=lambda kv: -kv[1]):
        print(f"   {count:6d}  {type_code:#06x} {NAMES.get(type_code, '')}")
    oids = [record["oid"] for record in records if record["oid"] is not None]
    rising = sum(1 for a, b in zip(oids, oids[1:]) if b > a)
    print(f"   oids rise from one record to the next {rising} times of {len(oids) - 1}")
    for record in records[:12]:
        print(f"   [{record['index']:5d}] byte {record['offset']:8d} type {record['type']:#06x} oid {record['oid']} parent {record['parent']} layer {record['layer']} len {record['end'] - record['offset']}")
