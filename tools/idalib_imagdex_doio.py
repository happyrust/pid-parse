r"""Recover the exact on-stream field order for the four JSite-only geometry
families by decompiling each class's IJPersist DoIO.

Slot 3 of each family's PersistImp vtable is the persistence entry: it calls
jengine_1076 (version getter, same primitive style.dll uses -- guide s10) then
dispatches to the version-specific DoIO worker that reads/writes each field via
jengine_1075(stream, size, &member). We decompile slot 3 plus its worker
callees in full.

Read-only (open .i64 run_auto_analysis=False, close save=False).

Usage:
    <idalib-python> tools/idalib_imagdex_doio.py [imagdex.dex.i64]
"""

import sys

import idapro

DB = sys.argv[1] if len(sys.argv) > 1 else r"D:\work\plant-code\cad\pid-parse\dlls\imagdex.dex.i64"
rc = idapro.open_database(DB, run_auto_analysis=False)
print(f"open_database -> {rc}", flush=True)
if rc:
    raise SystemExit(rc)

import ida_bytes
import ida_funcs
import ida_hexrays
import ida_segment
import ida_ua

ida_hexrays.init_hexrays_plugin()

VTABLES = {
    "Circle2d": 0x1052A460,
    "Arc2d": 0x1052996C,
    "Rectangle2d": 0x1052AD80,
    "BspCurve2d": 0x1052A5FC,
}


def seg_text(ea):
    seg = ida_segment.getseg(ea)
    return bool(seg) and ida_segment.get_segm_name(seg) == ".text"


def follow(ea, depth=5):
    for _ in range(depth):
        if not ida_funcs.get_func(ea):
            ida_funcs.add_func(ea)
        insn = ida_ua.insn_t()
        if ida_ua.decode_insn(insn, ea) > 0 and insn.get_canon_mnem() == "jmp":
            t = insn.ops[0].addr or insn.ops[0].value
            if t and seg_text(t):
                ea = t
                continue
        break
    return ea


def decomp(ea):
    if not ida_funcs.get_func(ea):
        ida_funcs.add_func(ea)
    try:
        cf = ida_hexrays.decompile(ea)
        return str(cf) if cf else "<none>"
    except Exception as e:  # noqa: BLE001
        return f"<<fail {e}>>"


def callees(func_ea):
    out = []
    f = ida_funcs.get_func(func_ea)
    if not f:
        return out
    h = f.start_ea
    while h < f.end_ea:
        insn = ida_ua.insn_t()
        n = ida_ua.decode_insn(insn, h)
        if n > 0 and insn.get_canon_mnem() == "call":
            t = insn.ops[0].addr or insn.ops[0].value
            if t and seg_text(t):
                r = follow(t)
                nm = ida_funcs.get_func_name(r) or ""
                out.append((r, nm))
        h = ida_bytes.next_head(h, f.end_ea)
    seen = set()
    uniq = []
    for r, nm in out:
        if r not in seen:
            seen.add(r)
            uniq.append((r, nm))
    return uniq


def show(ea, label, cap=9000):
    print("\n" + "-" * 74, flush=True)
    print(f"[{label} @ {ea:#012x}]", flush=True)
    txt = decomp(ea)
    print(txt[:cap] + (f"\n ...(+{len(txt) - cap})" if len(txt) > cap else ""), flush=True)


for fam, vt in VTABLES.items():
    print("\n" + "=" * 78, flush=True)
    slot3 = follow(ida_bytes.get_dword(vt + 3 * 4))
    print(f"##### {fam}: slot 3 (persist entry) @ {slot3:#012x} #####", flush=True)
    show(slot3, f"{fam} slot3 dispatch")
    # decompile the worker callees (skip the version getter jengine_1076)
    for r, nm in callees(slot3):
        if nm.startswith("jengine_") or nm.startswith("gsu") or nm.startswith("hcnstr"):
            print(f"   (callee {nm or hex(r)} -- primitive, skipped)", flush=True)
            continue
        show(r, f"{fam} DoIO worker {nm or hex(r)}")

idapro.close_database(save=False)
print("\ndone", flush=True)
