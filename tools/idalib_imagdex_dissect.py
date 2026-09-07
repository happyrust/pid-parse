r"""Dissect imagdex.dex.i64 for the four JSite-only geometry families' on-stream
field layout (Circle 0x0059 / Arc 0x0061 / Rectangle 0x0020 / BspCurve 0x005D).

imagdex.dex is a 32-bit DLL (base 0x10000000, __stdcall) -> pointers are 4 bytes.

Three angles, all read-only (open .i64 with run_auto_analysis=False, close save=False):

  1. .data class descriptor: dump the dwords around each family's CLSID const;
     RAD static class objects sit next to their vtable / CreateInstance pointer,
     so a dword there that lands in .text names the class machinery directly.
  2. strings: geometry field names (radius/center/angle/...) and serialization
     verbs (ReadFromStream/IJPersist/DoIO/Serialize/...) with their xref funcs --
     ugeom2d1 shipped an ASCII debug reader that spelled fields literally.
  3. the already-recognised named functions that reference each family's .rdata
     CLSID -- decompiled (truncated), likely QI / factory / reader.

Usage:
    <idalib-python> tools/idalib_imagdex_dissect.py [imagdex.dex.i64]
"""

import struct
import sys

import idapro

DB = sys.argv[1] if len(sys.argv) > 1 else r"D:\work\plant-code\cad\pid-parse\dlls\imagdex.dex.i64"
rc = idapro.open_database(DB, run_auto_analysis=False)
print(f"open_database({DB}) -> {rc}", flush=True)
if rc:
    raise SystemExit(rc)

import ida_bytes
import ida_funcs
import ida_hexrays
import ida_name
import ida_segment
import idautils
import idc

ida_hexrays.init_hexrays_plugin()


def guid_bytes(text: str) -> bytes:
    p = text.strip().strip("{}").split("-")
    return struct.pack("<IHH", int(p[0], 16), int(p[1], 16), int(p[2], 16)) + bytes.fromhex(
        p[3] + p[4]
    )


FAM = [
    (0x0059, "Circle", "902AD280-D3E1-11CD-8AEA-08003601B44A", 0x1052DB9C),
    (0x0061, "Arc", "9D650A00-D3E1-11CD-8AEA-08003601B44A", 0x1052DC3C),
    (0x0020, "Rectangle", "3643B830-D3E1-11CD-8AEA-08003601B44A", 0x1052D728),
    (0x005D, "BspCurve", "94834300-D3E1-11CD-8AEA-08003601B44A", 0x1052DBEC),
    (0x0018, "Line", "2D4E13C0-D3D1-11CD-8AEA-08003601B44A", 0x1052D688),
]

SEGS = []
for s in idautils.Segments():
    seg = ida_segment.getseg(s)
    SEGS.append((s, seg.end_ea, ida_segment.get_segm_name(seg), ida_bytes.get_bytes(s, seg.end_ea - s) or b""))


def find_va(needle: bytes):
    out = []
    for start, _e, _n, data in SEGS:
        at = 0
        while (i := data.find(needle, at)) >= 0:
            out.append(start + i)
            at = i + 1
    return out


def seg_name(ea: int) -> str:
    seg = ida_segment.getseg(ea)
    return ida_segment.get_segm_name(seg) if seg else "?"


def tgt_of(d: int) -> str:
    seg = ida_segment.getseg(d)
    if not seg:
        return ""
    f = ida_funcs.get_func(d)
    nm = ida_name.get_name(d)
    fn = ida_funcs.get_func_name(d) if f else ""
    label = nm or fn
    return f"-> {ida_segment.get_segm_name(seg)} {label}".rstrip()


# ---- 1. .data class descriptor dump ---------------------------------------
print("\n########## 1. .data class descriptor (dwords around CLSID) ##########", flush=True)
for code, fam, clsid, data_va in FAM:
    print(f"\n== 0x{code:04X} {fam}  .data CLSID @ {data_va:#012x} ==", flush=True)
    start = data_va - 0x20
    for off in range(0, 0x20 + 0x60, 4):
        ea = start + off
        d = ida_bytes.get_dword(ea)
        mark = "  <== CLSID[0]" if ea == data_va else ""
        print(f"  {ea:#012x}: {d:#010x} {tgt_of(d)}{mark}", flush=True)

# ---- 2. string census ------------------------------------------------------
print("\n########## 2. strings: field names + serialization verbs ##########", flush=True)
KEYS = (
    "circle", "arc", "rectangle", "bspline", "bspcurve", "bezier", "ellipse",
    "radius", "center", "centre", "angle", "startangle", "endangle",
    "corner", "width", "height", "npole", "npoles", "knot", "degree", "order",
    "readfromstream", "writetostream", "serialize", "ijpersist", "ipersist",
    "docontextio", "doio", "readdata", "writedata", "persist", "fromstream",
)
strs = list(idautils.Strings())
print(f"(total strings: {len(strs)})", flush=True)
hitcount = 0
for st in strs:
    txt = str(st)
    low = txt.lower()
    if any(k in low for k in KEYS):
        hitcount += 1
        ea = st.ea
        refs = []
        for xr in idautils.DataRefsTo(ea):
            f = ida_funcs.get_func(xr)
            if f:
                refs.append(ida_funcs.get_func_name(f.start_ea))
        rs = (" xref=" + ",".join(sorted(set(refs)))) if refs else ""
        print(f"  {ea:#012x} {txt!r}{rs}", flush=True)
print(f"(field/verb string hits: {hitcount})", flush=True)

# ---- 3. decompile the named funcs that reference each .rdata CLSID ---------
print("\n########## 3. decompile named funcs referencing each CLSID ##########", flush=True)
seen = set()
for code, fam, clsid, _dv in FAM:
    vas = [v for v in find_va(guid_bytes(clsid)) if seg_name(v) == ".rdata"]
    named = []
    for va in vas:
        for xr in idautils.DataRefsTo(va):
            f = ida_funcs.get_func(xr)
            if f:
                named.append((f.start_ea, ida_funcs.get_func_name(f.start_ea)))
    named = sorted(set(named))
    print(f"\n== 0x{code:04X} {fam}: {len(named)} named func(s) touch CLSID ==", flush=True)
    for fs, fn in named:
        if fs in seen:
            print(f"  (already shown) {fn} @ {fs:#012x}", flush=True)
            continue
        seen.add(fs)
        print("\n" + "-" * 70, flush=True)
        print(f"[{fn} @ {fs:#012x}]", flush=True)
        try:
            txt = str(ida_hexrays.decompile(fs))
        except Exception as e:  # noqa: BLE001
            print(f"  decompile failed: {e}", flush=True)
            continue
        print(txt[:6000] + (f"\n  ...(+{len(txt) - 6000})" if len(txt) > 6000 else ""), flush=True)

idapro.close_database(save=False)
print("\ndone", flush=True)
