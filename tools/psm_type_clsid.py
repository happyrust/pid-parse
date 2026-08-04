r"""Resolve a PSM type code to its CLSID and class name, from files only.

`radsrvitem.dll!dword_5667B068` is the PSM `type_code -> CLSID` table. Entries
are 20 bytes: a 16-byte GUID plus 4 bytes this does not interpret. Names come
from the RAD CLSID registry in `jutil.dll` (see `clsid_registry.py`).

The base and stride are *solved*, not assumed, from three independently known
(type code, class) pairs:

  * `0x0013` igBoundary2d    -- named in radsrvitem's own ig* class table
  * `0x0030` JStyleOverride  -- phase 16, via the jutil registry
  * `0x003D` igSmartFrame2d  -- named in radsrvitem's own ig* class table

Requiring all three to line up on one (base, stride) is what makes this
trustworthy. An earlier attempt derived the base from a single control and
then "validated" with that same control, which is circular and produced a
table that was wrong by a factor of five in stride; the tell was that
`0x004D` did not come back as a text box. Any future change here must keep
the multi-anchor solve.

Usage:
  python tools/psm_type_clsid.py                 # the codes phase 37 cares about
  python tools/psm_type_clsid.py 0xFA 0x77 0x07
"""

import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RADSRV = ROOT / "dlls" / "radsrvitem.dll"
JUTIL = Path(r"D:\pid\RADInstallA~\jutil.dll")

# (type code, label, CLSID) -- each established without reference to this table.
ANCHORS = [
    (0x0013, "igBoundary2d", "1EB2FA20-D1AD-11CE-A9B0-08003601B487"),
    (0x0030, "JStyleOverride", "47FCC338-2D0F-11D0-A1FF-080036A1CF02"),
    (0x003D, "igSmartFrame2d", "5B552E30-7C2D-11CE-A80E-08003601DADA"),
]

JUTIL_ENTRY_MODULE_AT = 16
JUTIL_ENTRY_NAME_AT = 32

DEFAULT_CODES = [0x07, 0x13, 0x18, 0x30, 0x3D, 0x4D, 0x5E, 0x77, 0x84, 0xCE, 0xFA, 0xFF]


def guid_bytes(text: str) -> bytes:
    p = text.strip().strip("{}").split("-")
    return struct.pack("<IHH", int(p[0], 16), int(p[1], 16), int(p[2], 16)) + bytes.fromhex(
        p[3] + p[4]
    )


def guid_text(raw: bytes) -> str:
    d1, d2, d3 = struct.unpack_from("<IHH", raw, 0)
    tail = "".join(f"{b:02X}" for b in raw[8:16])
    return f"{d1:08X}-{d2:04X}-{d3:04X}-{tail[:4]}-{tail[4:]}"


def all_offsets(data: bytes, needle: bytes) -> list[int]:
    out, at = [], 0
    while (at := data.find(needle, at)) >= 0:
        out.append(at)
        at += 1
    return out


def solve_layout(data: bytes) -> tuple[int, int]:
    """The single (base, stride) that puts every anchor at its own index."""
    spots = {code: all_offsets(data, guid_bytes(clsid)) for code, _, clsid in ANCHORS}
    (c0, _, _), (c1, _, _), (c2, _, _) = ANCHORS
    found = set()
    for a in spots[c0]:
        for b in spots[c1]:
            span = b - a
            if span <= 0 or span % (c1 - c0):
                continue
            stride = span // (c1 - c0)
            if b + (c2 - c1) * stride in spots[c2]:
                found.add((a - c0 * stride, stride))
    if len(found) != 1:
        raise SystemExit(f"anchors do not pin down one layout: {found}")
    return found.pop()


def cstr(data: bytes, at: int, limit: int) -> str:
    end = data.find(b"\0", at)
    if end < 0 or end - at > limit:
        end = at + limit
    return data[at:end].decode("latin1", "replace")


def name_of(jutil: bytes | None, clsid: str) -> str:
    if jutil is None:
        return "(jutil.dll unavailable)"
    at = jutil.find(guid_bytes(clsid))
    if at < 0:
        return "(not in RAD registry)"
    return f"{cstr(jutil, at + JUTIL_ENTRY_MODULE_AT, 16):<14} {cstr(jutil, at + JUTIL_ENTRY_NAME_AT, 64)}"


def main() -> None:
    data = RADSRV.read_bytes()
    base, stride = solve_layout(data)
    print(f"table base = file 0x{base:X}, stride = {stride} bytes (solved from {len(ANCHORS)} anchors)")

    jutil = JUTIL.read_bytes() if JUTIL.is_file() else None
    if jutil is None:
        print(f"note: {JUTIL} not found, CLSIDs will not be named")

    codes = [int(c, 0) for c in sys.argv[1:]] or DEFAULT_CODES
    for code in codes:
        at = base + code * stride
        if at + 16 > len(data):
            print(f"  0x{code:04X}: past end of file")
            continue
        raw = data[at : at + 16]
        if not any(raw):
            print(f"  0x{code:04X}: empty slot")
            continue
        clsid = guid_text(raw)
        print(f"  0x{code:04X}: {clsid}  {name_of(jutil, clsid)}")


if __name__ == "__main__":
    main()
