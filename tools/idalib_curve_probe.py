"""Locate the curve-family (igCircle2d 0x0059 / igArc2d 0x0061) serialization
read order inside ugeom2d1.dll, to promote Phase 36's corpus-statistical
decode to ida-proven (roadmap gate ROADMAP-CURVE-FAMILIES, slice 35-B).

First run analyses the DLL and writes ugeom2d1.dll.i64; re-runs reuse it.

Usage:
    python idalib_curve_probe.py [path-to-dll-or-i64]
"""

import sys

import idapro

db = sys.argv[1] if len(sys.argv) > 1 else r"D:\work\plant-code\cad\pid-parse\dlls\ugeom2d1.dll"

# Fresh DLL needs auto-analysis; an existing .i64 does not. Opening the raw
# .dll the first time and saving persists the database next to it.
rc = idapro.open_database(db, run_auto_analysis=True)
print(f"open_database({db}) -> {rc}", flush=True)
if rc:
    raise SystemExit(rc)

import ida_funcs
import ida_nalt
import idautils

print("input file :", ida_nalt.get_root_filename())
print("functions  :", ida_funcs.get_func_qty())

names = [(ea, n) for ea, n in idautils.Names()]
print("names total:", len(names))

KEYS = (
    "circle", "arc2d", "ellip", "spline", "bspline", "curve",
    "igcircle", "igarc", "read", "load", "store", "doio",
    "serial", "persist", "streamin", "streamout", "::in", "::out",
)


def hits(keys):
    out = []
    for ea, n in names:
        low = n.lower()
        if any(k in low for k in keys):
            out.append((ea, n))
    return out


print("\n== curve-family / serialization symbol hits ==", flush=True)
for ea, n in hits(KEYS):
    print(f"  {ea:#010x}  {n}")

# RTTI class names, if the compiler emitted them, name the geometry classes
# directly (e.g. igCircle2d) and their vtables point at the read methods.
print("\n== RTTI-ish class strings (igXxx2d) ==", flush=True)
import ida_bytes
import ida_segment

seen = set()
for s in idautils.Strings():
    txt = str(s)
    low = txt.lower()
    if ("2d" in low and low.startswith("ig")) or "circle" in low or "arc" in low:
        if txt not in seen:
            seen.add(txt)
            print(f"  {s.ea:#010x}  {txt!r}")

idapro.close_database(save=True)
print("\nclosed (saved .i64)")
