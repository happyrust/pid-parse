"""From the ig* type-code->name table (sub_56448F70) find the sibling factory /
read dispatch: callers of the name lookup, and any second switch on the same
type codes (0x18/0x59/0x61...) that constructs or reads the object.

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
import idautils

ida_hexrays.init_hexrays_plugin()

NAME_TABLE = 0x56448F70

# 1) Who calls the type->name table? Those are error/log sites; their callers or
#    siblings often hold the real create/read switch.
print("\n== callers of sub_56448F70 (type->name) ==", flush=True)
callers = {}
for r in idautils.CodeRefsTo(NAME_TABLE, 0):
    f = ida_funcs.get_func(r)
    if f:
        callers.setdefault(f.start_ea, ida_funcs.get_func_name(f.start_ea))
        print(f"  from {r:#010x} in {ida_funcs.get_func_name(f.start_ea)} @ {f.start_ea:#010x}")
    else:
        print(f"  from {r:#010x} (no func)")

for cea, cname in list(callers.items())[:3]:
    print(f"\n-- decompile caller {cname} @ {cea:#010x} --", flush=True)
    try:
        txt = str(ida_hexrays.decompile(cea))
        print(txt[:5000])
        if len(txt) > 5000:
            print(f"  ... (+{len(txt) - 5000} chars truncated)")
    except Exception as e:  # noqa: BLE001
        print(f"  decompile failed: {e}")

# 2) Scan every function for a switch that mentions several curve type-code
#    constants; the factory/reader will test 0x59 AND 0x61 AND 0x18 together.
print("\n== functions whose body references >=4 ig type-code constants ==", flush=True)
CODES = (0x18, 0x59, 0x61, 0x63, 0x84, 0x5D, 0x7E, 0x4D, 0x20, 0x3D)


def const_hits_in_func(f):
    got = set()
    ea = f.start_ea
    while ea < f.end_ea:
        for op in (idautils.DecodeInstruction(ea),):
            if not op:
                break
            for o in op.ops:
                if o.type == 5:  # immediate
                    if o.value in CODES:
                        got.add(o.value)
        ea = ida_bytes.next_head(ea, f.end_ea)
    return got


hot = []
for fea in idautils.Functions():
    f = ida_funcs.get_func(fea)
    if not f:
        continue
    got = const_hits_in_func(f)
    if len(got) >= 4:
        hot.append((fea, sorted(got)))

for fea, got in hot[:12]:
    print(f"  {fea:#010x}  {ida_funcs.get_func_name(fea)}  codes={[hex(c) for c in got]}")

# 3) Decompile the most promising (max distinct codes), likely the factory.
if hot:
    hot.sort(key=lambda t: -len(t[1]))
    best = hot[0][0]
    print("\n== decompiled best candidate (factory/reader) @ "
          f"{best:#010x} codes={[hex(c) for c in hot[0][1]]} ==", flush=True)
    try:
        txt = str(ida_hexrays.decompile(best))
        print(txt[:9000])
        if len(txt) > 9000:
            print(f"  ... (+{len(txt) - 9000} chars truncated)")
    except Exception as e:  # noqa: BLE001
        print(f"  decompile failed: {e}")

idapro.close_database(save=False)
print("\nclosed")
