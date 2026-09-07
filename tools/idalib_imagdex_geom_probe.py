r"""Recon imagdex.dex for the four JSite-only geometry serializers still stuck
at corpus level:

    Circle 0x0059, Arc 0x0061, Rectangle 0x0020, BspCurve 0x005D

`tools/psm_type_clsid.py` resolved all four to classes implemented in
imagdex.dex. This walks the guide's §10 entry path on imagdex instead of
style.dll: locate each family's CLSID constant in .rdata/.data, find who
references it (the class-factory branch), and decompile those factory
functions so we can see which vtable each CLSID constructs.

The imagdex.dex database is *unpacked* next to the .dex (id0/id1/id2/nam/til,
no .i64). We continue-open it read-only: run_auto_analysis=False and
close_database(save=False). The id* files were backed up to
dlls/_imagdex-idb-backup-20260831 before running this.

Usage:
    <idalib-python> tools/idalib_imagdex_geom_probe.py
"""

import struct
import sys

import idapro

DB = sys.argv[1] if len(sys.argv) > 1 else r"D:\work\plant-code\cad\pid-parse\dlls\imagdex.dex"

print(f"opening (continue unpacked db): {DB}", flush=True)
rc = idapro.open_database(DB, run_auto_analysis=False)
print(f"open_database -> {rc}", flush=True)
if rc:
    raise SystemExit(rc)

import ida_bytes
import ida_funcs
import ida_hexrays
import ida_name
import ida_nalt
import ida_segment
import idautils
import idc

print(f"root: {ida_nalt.get_root_filename()}", flush=True)
print(f"imagebase: {ida_nalt.get_imagebase():#x}", flush=True)
print(f"functions: {sum(1 for _ in idautils.Functions())}", flush=True)


def guid_bytes(text: str) -> bytes:
    p = text.strip().strip("{}").split("-")
    return struct.pack("<IHH", int(p[0], 16), int(p[1], 16), int(p[2], 16)) + bytes.fromhex(
        p[3] + p[4]
    )


FAM = [
    (0x0059, "Circle", "902AD280-D3E1-11CD-8AEA-08003601B44A"),
    (0x0061, "Arc", "9D650A00-D3E1-11CD-8AEA-08003601B44A"),
    (0x0020, "Rectangle", "3643B830-D3E1-11CD-8AEA-08003601B44A"),
    (0x005D, "BspCurve", "94834300-D3E1-11CD-8AEA-08003601B44A"),
    (0x0018, "Line(known)", "2D4E13C0-D3D1-11CD-8AEA-08003601B44A"),
]

# Snapshot every segment's bytes once so we can do plain byte searches without
# leaning on the IDA search API (which shifts between versions).
SEGS = []
for s in idautils.Segments():
    seg = ida_segment.getseg(s)
    data = ida_bytes.get_bytes(s, seg.end_ea - s) or b""
    SEGS.append((s, seg.end_ea, ida_segment.get_segm_name(seg), data))
print(f"segments: {[(hex(a), n, len(d)) for a, _, n, d in SEGS]}", flush=True)


def find_va(needle: bytes):
    hits = []
    for start, _end, _name, data in SEGS:
        at = 0
        while True:
            i = data.find(needle, at)
            if i < 0:
                break
            hits.append(start + i)
            at = i + 1
    return hits


def seg_name(ea: int) -> str:
    seg = ida_segment.getseg(ea)
    return ida_segment.get_segm_name(seg) if seg else "?"


def func_of(ea: int):
    f = ida_funcs.get_func(ea)
    if not f:
        return None, None
    return f.start_ea, ida_funcs.get_func_name(f.start_ea)


# Census: for every family, where the CLSID sits and which functions touch it.
# .data hits usually sit in a class descriptor / registration array (closer to
# construction + persistence); .rdata hits are the QI-compare / factory switch.
for code, fam, clsid in FAM:
    gb = guid_bytes(clsid)
    vas = find_va(gb)
    print(f"\n== 0x{code:04X} {fam}  {clsid} : {len(vas)} const hit(s) ==", flush=True)
    for va in vas:
        print(f"  const @ {va:#012x} seg={seg_name(va)}", flush=True)
        for xr in idautils.DataRefsTo(va):
            fs, fn = func_of(xr)
            tag = f"func={fn} @ {fs:#012x}" if fs is not None else "(no func)"
            print(f"    xref {xr:#012x}  seg={seg_name(xr)}  {tag}", flush=True)

print("\n== exports / names of interest ==", flush=True)
ENTRIES = {}
for want in ("HGeomGetLayer", "HGeomPutLayer", "DllGetClassObject", "DllGetClassObject_0"):
    ea = ida_name.get_name_ea(0, want)
    print(f"  {want}: {ea:#012x}" if ea != idc.BADADDR else f"  {want}: not found", flush=True)
    if ea != idc.BADADDR:
        ENTRIES[want] = ea

# Decompile the two factory entries: they show which CLSID constructs which
# class (and thus which vtable holds the per-class stream reader).
print("\n== decompiling factory entries ==", flush=True)
if not ida_hexrays.init_hexrays_plugin():
    print("!! hexrays not available; skipping decompile", flush=True)
else:
    for want in ("DllGetClassObject", "DllGetClassObject_0"):
        ea = ENTRIES.get(want)
        if ea is None:
            continue
        print("\n" + "=" * 78, flush=True)
        print(f"[{want} @ {ea:#012x}]", flush=True)
        try:
            txt = str(ida_hexrays.decompile(ea))
        except Exception as e:  # noqa: BLE001
            print(f"  decompile failed: {e}", flush=True)
            continue
        if len(txt) > 14000:
            print(txt[:14000] + f"\n  ... (+{len(txt) - 14000} chars truncated)", flush=True)
        else:
            print(txt, flush=True)

idapro.close_database(save=False)
print("\ndone", flush=True)
