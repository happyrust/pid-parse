"""Who reads an igTextBox's character-style runs, and does the paragraph style
get applied underneath them or only in their absence?

Established by t-42 (`docs/analysis/2026-08-22-igtextbox-tail-kind-2-and-
formatting-runs.md`): `IGDSFactoryText` keeps two run arrays on the text sink
that lives at `this + 20`:

    this+60 u16  character count      this+64  text buffer
    this+68 u16  count of selector-1 runs   this+72  their 8-byte array
    this+76 u16  count of selector-2 runs   this+80  their 8-byte array

`??_7IGDSFactoryText@@6B@_0` @ 0x56666748 (22 slots) is the sink's vtable;
`+68` appends a selector-1 run (`sub_564978B0`), `+72` a selector-2 run
(`sub_56497930`). t-42 only found *writers* of those arrays plus `GetSize`
and `Save`. This pass looks for the **readers**: whichever slot hands the runs
back out, and who calls it.

Read-only: reuses radsrvitem.dll.i64, no auto-analysis, no save.

    python tools/idalib_igtextbox_run_consumers.py
"""

import pathlib
import shutil
import tempfile

import idapro

# See tools/idalib_igtextbox_style_apply.py for why the packed database is
# copied out of dlls/ before opening it.
SRC = r"D:\work\plant-code\cad\pid-parse\dlls\radsrvitem.dll.i64"
SCRATCH = pathlib.Path(tempfile.gettempdir()) / "pidparse-ida-t42"
SCRATCH.mkdir(parents=True, exist_ok=True)
DB = SCRATCH / "radsrvitem.dll.i64"
if not DB.exists():
    shutil.copyfile(SRC, DB)

rc = idapro.open_database(str(DB), run_auto_analysis=False)
print(f"open_database -> {rc}", flush=True)
if rc:
    raise SystemExit(rc)

import ida_bytes  # noqa: E402
import ida_funcs  # noqa: E402
import ida_hexrays  # noqa: E402
import ida_name  # noqa: E402
import idautils  # noqa: E402

ida_hexrays.init_hexrays_plugin()

SINK_VTABLE = 0x56666748
SINK_SLOTS = 22

# Object-relative offsets, in the sink's own coordinates (sink = object + 20).
RUN_FIELDS = ("+ 40)", "+ 44)", "+ 48)", "+ 52)", "+ 56)", "+ 60)")


def pseudo(ea):
    try:
        return str(ida_hexrays.decompile(ea))
    except Exception as exc:  # noqa: BLE001
        return f"<decompile failed: {exc}>"


def dump(name, ea, limit=7000):
    print("\n" + "=" * 78)
    print(f"[{name}] @ {ea:#010x}", flush=True)
    txt = pseudo(ea)
    print(txt[:limit])
    if len(txt) > limit:
        print(f"  ... (+{len(txt) - limit} chars truncated)")
    return txt


print("\n" + "#" * 78)
print("# the text sink's 22 slots -- which ones read the run arrays?")
print("#" * 78)

slots = []
for idx in range(SINK_SLOTS):
    ea = SINK_VTABLE + idx * 4
    val = ida_bytes.get_dword(ea)
    nm = ida_name.get_ea_name(val) or f"sub_{val:X}"
    slots.append((idx * 4, val, nm))
    print(f"  +{idx * 4:<4} {val:#010x}  {nm}")

readers = []
for off, val, nm in slots:
    txt = pseudo(val)
    touched = [f for f in RUN_FIELDS if f in txt]
    if touched:
        readers.append((off, val, nm, touched))

print("\nslots that touch the run/text fields:")
for off, val, nm, touched in readers:
    print(f"  +{off:<4} {nm:<16} {touched}")

for off, val, nm, _touched in readers:
    dump(f"sink +{off}  {nm}", val)

# --- who calls the sink slots from outside? --------------------------------
print("\n" + "#" * 78)
print("# callers of each sink implementation (direct calls, if any)")
print("#" * 78)
for off, val, nm in slots:
    refs = list(idautils.CodeRefsTo(val, 0))
    if not refs:
        continue
    print(f"\n  +{off} {nm}:")
    for ref in refs:
        fn = ida_funcs.get_func(ref)
        owner = ida_name.get_ea_name(fn.start_ea) if fn else "?"
        print(f"    {ref:#010x} in {owner}")

# --- anything that mentions the paragraph style class ----------------------
print("\n" + "#" * 78)
print("# strings naming the text style classes, and who references them")
print("#" * 78)
for s in idautils.Strings():
    text = str(s)
    if "TextPara" in text or "TextChar" in text or "StyleText" in text:
        print(f"\n  {s.ea:#010x}  {text!r}")
        for ref in idautils.DataRefsTo(s.ea):
            fn = ida_funcs.get_func(ref)
            owner = ida_name.get_ea_name(fn.start_ea) if fn else "?"
            print(f"    referenced at {ref:#010x} in {owner}")

idapro.close_database(save=False)
print("\nclosed")
