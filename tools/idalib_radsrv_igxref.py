"""Follow xrefs from radsrvitem.dll's ig* class-name strings to the code that
registers / reads those graphics objects, to pin the ig* PSM record reader
(0x0059 igCircle2d, 0x0061 igArc2d ...) — the layer ugeom2d1 turned out NOT to be.

Read-only: reuses radsrvitem.dll.i64, no analysis, no save.
"""

import idapro

db = r"D:\work\plant-code\cad\pid-parse\dlls\radsrvitem.dll.i64"
rc = idapro.open_database(db, run_auto_analysis=False)
print(f"open_database -> {rc}", flush=True)
if rc:
    raise SystemExit(rc)

import ida_bytes
import ida_funcs
import ida_hexrays
import ida_name
import ida_xref
import idautils
import idc

ida_hexrays.init_hexrays_plugin()

STRINGS = {
    "igArc2d": 0x5665F4E0,
    "igEllipticalArc2d": 0x5665F4E8,
    "igLine2d": 0x5665F4FC,
    "igCircle2d": 0x5665F508,
    "igEllipse2d": 0x5665F514,
    "igLineString2d": 0x5665F520,
    "igBSplineCurve2d": 0x5665F560,
    "igPoint2d": 0x5665F544,
    "igRectangle2d": 0x5665F550,
    "igBoundary2d": 0x5665F574,
    "igSymbol2d": 0x5665F584,
    "igTextBox": 0x5665F070,
}

# 1) Is 0x5665f4e0.. a string table with a parallel pointer/type-code table?
print("\n== bytes around the ig* string block (0x5665f4d0..0x5665f5a0) ==", flush=True)
ea = 0x5665F4D0
while ea < 0x5665F5A0:
    b = ida_bytes.get_bytes(ea, 16) or b""
    print(f"  {ea:#010x}  {b.hex(' ')}  {b!r}")
    ea += 16

# 2) xrefs into each string -> containing function
print("\n== xrefs to ig* strings ==", flush=True)
funcs_of_interest = {}
for label, sea in STRINGS.items():
    refs = list(idautils.DataRefsTo(sea)) + list(idautils.CodeRefsTo(sea, 0))
    print(f"  [{label}] @ {sea:#010x}  xrefs={len(refs)}")
    for r in refs:
        f = ida_funcs.get_func(r)
        fname = ida_funcs.get_func_name(r) if f else "(no func)"
        fstart = f.start_ea if f else r
        print(f"      from {r:#010x}  in {fname} @ {fstart:#010x}")
        funcs_of_interest.setdefault(fstart, fname)

# 3) decompile the small set of referencing functions (dedup, cap size)
print("\n== decompiled referencing functions ==", flush=True)
for fstart, fname in list(funcs_of_interest.items())[:6]:
    print("\n" + "-" * 74)
    print(f"[{fname}] @ {fstart:#010x}")
    try:
        cf = ida_hexrays.decompile(fstart)
        txt = str(cf)
        print(txt[:6000])
        if len(txt) > 6000:
            print(f"  ... (+{len(txt) - 6000} chars truncated)")
    except Exception as e:  # noqa: BLE001
        print(f"  decompile failed: {e}")

idapro.close_database(save=False)
print("\nclosed")
