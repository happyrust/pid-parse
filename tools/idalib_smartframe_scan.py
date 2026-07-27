"""Find whoever reads igSmartFrame2d (PSM 0x003D).

ROADMAP-SMARTFRAME-003D holds the record at IdentifiedOnly and will only take
named fields on native-reader or controlled-fixture evidence -- the A2-shaped
scalars at payload +76/+84 are explicitly not enough on their own. Phase 36's
IDA pass showed radsrvitem treats a PSM record as an opaque RAD object, so the
field-level reader, if it exists, is in another module.

This scans the databases already on disk for anything frame-shaped: symbols and
strings naming SmartFrame, sheet/page/border/drawing size, plus the callers of
the igSmartFrame2d class-name string in radsrvitem.

Read-only: opens existing .i64 files with auto-analysis off and never saves.
"""

import re

import idapro
import ida_bytes
import ida_funcs
import ida_nalt
import idautils

DLLS = r"D:\work\plant-code\cad\pid-parse\dlls"
DBS = [
    f"{DLLS}\\radsrvitem.dll.i64",
    f"{DLLS}\\sppid.dll.i64",
    f"{DLLS}\\core.dll.i64",
    f"{DLLS}\\sppidautomation.dll.i64",
    f"{DLLS}\\ugeom2d1.dll.i64",
]

# Frame-shaped vocabulary. "smartframe" is the exact class; the rest are the
# names a sheet-border reader would plausibly carry.
NEEDLES = (
    "smartframe",
    "sheetframe",
    "drawingframe",
    "pagesetup",
    "pagesize",
    "sheetsize",
    "drawingsize",
    "papersize",
    "border",
)
# Narrower: only interesting when paired with a frame word above.
WEAK = ("frame", "page", "sheet")
VERB = ("read", "load", "doio", "serial", "persist", "stream", "restore")


def hits(text):
    low = text.lower()
    if any(n in low for n in NEEDLES):
        return "strong"
    if any(w in low for w in WEAK) and any(v in low for v in VERB):
        return "weak"
    return None


def scan(db):
    rc = idapro.open_database(db, run_auto_analysis=False)
    print("\n" + "#" * 78)
    print(f"# {db}  open->{rc}", flush=True)
    if rc:
        return
    print(f"  input: {ida_nalt.get_root_filename()}  funcs: {ida_funcs.get_func_qty()}")

    strong, weak = [], []
    for ea, name in idautils.Names():
        kind = hits(name)
        if kind == "strong":
            strong.append((ea, name, "sym"))
        elif kind == "weak":
            weak.append((ea, name, "sym"))

    smartframe_eas = []
    for s in idautils.Strings():
        text = str(s)
        kind = hits(text)
        if kind == "strong":
            strong.append((s.ea, text, "str"))
            if "smartframe" in text.lower():
                smartframe_eas.append((s.ea, text))
        elif kind == "weak":
            weak.append((s.ea, text, "str"))

    print(f"  -- strong ({len(strong)}) --", flush=True)
    for ea, text, kind in strong[:80]:
        print(f"    {ea:#010x} [{kind}] {text!r}")
    print(f"  -- weak ({len(weak)}) --", flush=True)
    for ea, text, kind in weak[:40]:
        print(f"    {ea:#010x} [{kind}] {text!r}")

    # Whoever mentions the class name is the closest thing to an owner.
    for ea, text in smartframe_eas:
        print(f"  -- xrefs to {text!r} @ {ea:#010x} --", flush=True)
        seen = set()
        for xref in idautils.DataRefsTo(ea):
            func = ida_funcs.get_func(xref)
            where = f"{func.start_ea:#010x}" if func else "(no func)"
            if where in seen:
                continue
            seen.add(where)
            fname = ida_funcs.get_func_name(func.start_ea) if func else "?"
            print(f"    from {xref:#010x} in {where} {fname}")

    idapro.close_database(save=False)


for db in DBS:
    try:
        scan(db)
    except Exception as exc:  # noqa: BLE001
        print(f"  !! {db}: {exc}", flush=True)

print("\ndone")
