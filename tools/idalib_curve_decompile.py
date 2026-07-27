"""Decompile the ugeom2d1.dll IMG<family>ReadFromStream2d readers to recover
the on-stream field order for the curve families, cross-checking Phase 36's
corpus-statistical decode (roadmap gate ROADMAP-CURVE-FAMILIES / slice 35-B).

Reuses the ugeom2d1.dll.i64 produced by idalib_curve_probe.py (no re-analysis).
"""

import sys

import idapro

db = sys.argv[1] if len(sys.argv) > 1 else r"D:\work\plant-code\cad\pid-parse\dlls\ugeom2d1.dll.i64"

rc = idapro.open_database(db, run_auto_analysis=False)
print(f"open_database({db}) -> {rc}", flush=True)
if rc:
    raise SystemExit(rc)

import ida_hexrays
import ida_name

if not ida_hexrays.init_hexrays_plugin():
    print("!! hexrays not available", flush=True)
    idapro.close_database(save=False)
    raise SystemExit(2)

TARGETS = [
    "IMGLineReadFromStream2d",
    "IMGArcReadFromStream2d",
    "IMGEllipseReadFromStream2d",
    "IMGLineStringReadFromStream2d",
    "IMGBspCurveReadFromStream2d",
    "IMReadArc2d",
    "IMRead_GArc2d",
]

for name in TARGETS:
    ea = ida_name.get_name_ea(0, name)
    print("\n" + "=" * 78, flush=True)
    if ea == 0xFFFFFFFF or ea == 0xFFFFFFFFFFFFFFFF:
        print(f"[{name}] NOT FOUND")
        continue
    print(f"[{name}] @ {ea:#010x}")
    try:
        cf = ida_hexrays.decompile(ea)
        print(str(cf))
    except Exception as e:  # noqa: BLE001
        print(f"  decompile failed: {e}")

idapro.close_database(save=False)
print("\nclosed")
