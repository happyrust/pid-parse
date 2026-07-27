"""Follow the geometry-record path in radsrvitem.dll to the per-class field
reads: sub_564851F0 (called for geometry type codes by sub_564623D0) and
sub_564459D0 (generic create+emit used by the record handler and the igTextBox
reader). Looking for f64 reads at record offsets matching Phase 36's
0x0059/0x0061 layout (center +18, radius +34, angles +42/+50).

Read-only: reuses radsrvitem.dll.i64, no analysis, no save.
"""

import idapro

db = r"D:\work\plant-code\cad\pid-parse\dlls\radsrvitem.dll.i64"
rc = idapro.open_database(db, run_auto_analysis=False)
print(f"open_database -> {rc}", flush=True)
if rc:
    raise SystemExit(rc)

import ida_hexrays
import ida_name

ida_hexrays.init_hexrays_plugin()

TARGETS = {
    "sub_564851F0": 0x564851F0,   # geometry-type branch in sub_564623D0
    "sub_564459D0": 0x564459D0,   # generic create + emit (record handler / igTextBox)
    "sub_56449240": 0x56449240,   # igTextBox variant-1 payload locator (from sub_564468B0)
    "sub_56447710": 0x56447710,   # igTextBox variant-2 payload locator
    "sub_56447730": 0x56447730,   # igTextBox variant-3 payload locator
}

for name, ea in TARGETS.items():
    print("\n" + "=" * 78)
    print(f"[{name}] @ {ea:#010x}", flush=True)
    try:
        txt = str(ida_hexrays.decompile(ea))
        print(txt[:7000])
        if len(txt) > 7000:
            print(f"  ... (+{len(txt) - 7000} chars truncated)")
    except Exception as e:  # noqa: BLE001
        print(f"  decompile failed: {e}")

idapro.close_database(save=False)
print("\nclosed")
