"""Decompile radsrvitem's igSmartFrame2d functions.

The scan found three of them:

  sub_564464D0  names igSmartFrame2d and picks between
                "_Empty" / "_Embedded" / "_Locally Linked" SmartFrame2d
  sub_56449A90  formats "smartframe:%x"
  sub_5644BCC0  parses "smartframe:"

The variant classifier is the interesting one: whatever field it tests is a
bounded structural field of the record, which is what ROADMAP-SMARTFRAME-003D
asks for before anything may be named.

Read-only: existing .i64, auto-analysis off, never saved.
"""

import idapro
import ida_funcs
import ida_hexrays
import ida_nalt

DB = r"D:\work\plant-code\cad\pid-parse\dlls\radsrvitem.dll.i64"
TARGETS = [
    (0x564464D0, "variant classifier (Empty / Embedded / Locally Linked)"),
    (0x56449A90, 'formats "smartframe:%x"'),
    (0x5644BCC0, 'parses "smartframe:"'),
]

rc = idapro.open_database(DB, run_auto_analysis=False)
print(f"# {DB} open->{rc}", flush=True)
if rc:
    raise SystemExit(1)
print(f"  input: {ida_nalt.get_root_filename()}  funcs: {ida_funcs.get_func_qty()}")

if not ida_hexrays.init_hexrays_plugin():
    print("  !! hexrays unavailable")
    idapro.close_database(save=False)
    raise SystemExit(1)

for ea, why in TARGETS:
    print("\n" + "=" * 78)
    print(f"== {ea:#010x}  {why}", flush=True)
    func = ida_funcs.get_func(ea)
    if not func:
        print("   (no function here)")
        continue
    try:
        code = ida_hexrays.decompile(func.start_ea)
    except Exception as exc:  # noqa: BLE001
        print(f"   !! decompile failed: {exc}")
        continue
    print(str(code))

idapro.close_database(save=False)
print("\ndone")
