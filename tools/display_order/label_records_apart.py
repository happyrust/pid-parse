"""Where do the records of one label differ between two drawings that both hold it?

    python tools/display_order/label_records_apart.py <a.pid> <symbol oid> <b.pid> <symbol oid> [stream]

A label on a sheet is three records: its symbol (`igSymbol2d`, `0x00CE`), the
`DependencyObject` (`0x00FA`) that names the symbol's oid at payload `+16` and
owns the text, and the text (`igTextBox`, `0x004D`). Given the oid of the
label's symbol in each of two drawings, this finds the three records in each
(in `Sheet6` unless another stream is named), and prints, record by record,
every run of payload bytes in which the two differ, with the little-endian
values each side holds there. It reads the compound files with `olefile` and
walks the record chain itself — an eight-byte header, then records end to
end, each a type word, a length and a payload that opens with the record's
oid and its owner's — so it rests on nothing of the crate. Read-only.

The label `P70` of `一级闪蒸分离器A` (symbol oid 26) against the same label of
`一级闪蒸分离器B` (symbol oid 1225), 2026-11-12: the three records are of the
same lengths on both sheets (113, 98 and 74 bytes) and 20 bytes differ, every
one of them part of a number of the drawing's own:

    symbol   +0 its oid (26 / 1225); +25 the style its body is drawn in
             (46 / 63); +29 the site that names its `.sym` (85 / 94); +105
             and +109 the cached body, the definition sheet and the site
             that caches it (315 / 354, 3773 / 2895)
    owner    +0 its oid (2370 / 1139); +16 the symbol's oid; +22 the oid of
             an object that is no live record of the stream (2794 / 2723);
             +34 the text's oid (2438 / 255)
    text     +0 its oid; +4 its owner's oid; +14 its paragraph style
             (36 / 58); +26 the character style of its one run (34 / 56)

The type words, the layers, the placement matrix and insertion point, the
characters `P70` and where they stand are the same bytes; neither sheet holds
a superseded copy of any of the three.
"""

import struct
import sys

import olefile

SYMBOL, OWNER, TEXT = 0x00CE, 0x00FA, 0x004D
SUPERSEDED = 0x8000


def records(path: str, stream: str):
    """Every record of the stream's chain: (offset, type word, payload)."""
    ole = olefile.OleFileIO(path)
    try:
        data = ole.openstream(stream).read()
    finally:
        ole.close()
    out = []
    at = 8
    while at + 6 <= len(data):
        kind, length = struct.unpack_from("<HI", data, at)
        payload = data[at + 6 : at + 6 + length]
        if len(payload) < length:
            break
        out.append((at, kind, payload))
        at += 6 + length
    if at != len(data):
        raise SystemExit(f"{path}: the chain of {stream} stops at +{at} of {len(data)} bytes")
    return out


def u32(payload: bytes, at: int):
    return struct.unpack_from("<I", payload, at)[0] if at + 4 <= len(payload) else None


def u16(payload: bytes, at: int):
    return struct.unpack_from("<H", payload, at)[0] if at + 2 <= len(payload) else None


def label(path: str, stream: str, symbol_oid: int):
    """The label's symbol, owner and text, and how many superseded copies of them the stream keeps."""
    every = records(path, stream)
    live = [record for record in every if not record[1] & SUPERSEDED]
    symbols = [r for r in live if r[1] == SYMBOL and u32(r[2], 0) == symbol_oid]
    if len(symbols) != 1:
        raise SystemExit(f"{path}: {len(symbols)} live igSymbol2d record(s) carry the oid {symbol_oid}")
    owners = [
        r
        for r in live
        if r[1] == OWNER
        and u32(r[2], 16) == symbol_oid
        and any(t[1] == TEXT and u32(t[2], 4) == u32(r[2], 0) for t in live)
    ]
    if len(owners) != 1:
        raise SystemExit(f"{path}: {len(owners)} owner(s) of a text name the symbol {symbol_oid}")
    texts = [t for t in live if t[1] == TEXT and u32(t[2], 4) == u32(owners[0][2], 0)]
    if len(texts) != 1:
        raise SystemExit(f"{path}: the owner of the symbol {symbol_oid} holds {len(texts)} text boxes")
    mine = {u32(r[2], 0) for r in (symbols[0], owners[0], texts[0])}
    older = sum(1 for r in every if r[1] & SUPERSEDED and u32(r[2], 0) in mine)
    return (symbols[0], owners[0], texts[0]), older, len(every), len(live)


def main() -> int:
    if len(sys.argv) not in (5, 6):
        print(__doc__)
        return 2
    stream = sys.argv[5] if len(sys.argv) == 6 else "Sheet6"
    sides = []
    for path, oid in ((sys.argv[1], int(sys.argv[2])), (sys.argv[3], int(sys.argv[4]))):
        found, older, every, live = label(path, stream, oid)
        print(f"{path}")
        print(
            f"   /{stream}: {every} records, {live} live; symbol +{found[0][0]} oid {oid}, "
            f"owner +{found[1][0]} oid {u32(found[1][2], 0)}, text +{found[2][0]} oid {u32(found[2][2], 0)}; "
            f"superseded copies of the three: {older}"
        )
        sides.append(found)
    differing = 0
    for name, a, b in zip(("symbol igSymbol2d", "owner DependencyObject", "text igTextBox"), *sides):
        pa, pb = a[2], b[2]
        print(f"{name}: type word 0x{a[1]:04X} / 0x{b[1]:04X}; payload {len(pa)} / {len(pb)} bytes")
        if len(pa) != len(pb):
            print("   the payloads are of different lengths")
            continue
        runs = []
        for at in (i for i in range(len(pa)) if pa[i] != pb[i]):
            if runs and at == runs[-1][1] + 1:
                runs[-1][1] = at
            else:
                runs.append([at, at])
        apart = sum(end - start + 1 for start, end in runs)
        differing += apart
        print(f"   {apart} of {len(pa)} bytes differ, in {len(runs)} run(s)")
        for start, end in runs:
            print(
                f"   +{start}..+{end}: {pa[start:end + 1].hex()} / {pb[start:end + 1].hex()}"
                f"   as u16 at +{start}: {u16(pa, start)} / {u16(pb, start)}"
                f"   as u32 at +{start}: {u32(pa, start)} / {u32(pb, start)}"
            )
    print(f"{differing} byte(s) differ over the three records")
    return 0


if __name__ == "__main__":
    sys.exit(main())
