r"""Nail Rectangle2d current format: slot3 (sub_10166D30) is a pure switch over
versions 1..5, so the newest worker (case 5 = sub_1000A69B) is the current
layout. Decompile case 5 and case 4 workers plus their geom callees.

Read-only (open .i64 run_auto_analysis=False, close save=False).
"""

import sys

import idapro

DB = sys.argv[1] if len(sys.argv) > 1 else r"D:\work\plant-code\cad\pid-parse\dlls\imagdex.dex.i64"
rc = idapro.open_database(DB, run_auto_analysis=False)
print(f"open_database -> {rc}", flush=True)
if rc:
    raise SystemExit(rc)

import ida_auto
import ida_bytes
import ida_funcs
import ida_hexrays
import ida_segment
import ida_ua

ida_hexrays.init_hexrays_plugin()

PRIMS = ("jengine_", "gsu", "hcnstr", "JCoCreate", "RCfatal", "DebugBreak",
         "memset", "memcpy", "free", "malloc", "operator", "unknown_libname",
         "__security", "j_j_", "SkPrefs")


def seg_text(ea):
    seg = ida_segment.getseg(ea)
    return bool(seg) and ida_segment.get_segm_name(seg) == ".text"


def follow(ea, depth=6):
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


def hard_decomp(ea):
    if not ida_funcs.get_func(ea):
        ida_funcs.add_func(ea)
    ida_auto.auto_wait()
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
                if r not in [o[0] for o in out]:
                    out.append((r, nm))
        h = ida_bytes.next_head(h, f.end_ea)
    return out


def is_prim(nm):
    return any(nm.startswith(p) or p in nm for p in PRIMS)


def show(ea, label, cap=8000):
    print("\n" + "-" * 74, flush=True)
    print(f"[{label} @ {ea:#012x}]", flush=True)
    txt = hard_decomp(ea)
    print(txt[:cap] + (f"\n ...(+{len(txt) - cap})" if len(txt) > cap else ""), flush=True)


for label, ea in (("Rect case5 (current) sub_1000A69B", 0x1000A69B),
                  ("Rect case4 sub_1000C4E1", 0x1000C4E1)):
    r = follow(ea)
    print("\n" + "=" * 78, flush=True)
    show(r, label)
    for cr, nm in callees(r):
        if is_prim(nm):
            continue
        show(cr, f"  callee {nm or hex(cr)}", cap=5000)

idapro.close_database(save=False)
print("\ndone", flush=True)
