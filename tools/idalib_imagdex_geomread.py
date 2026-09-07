r"""Final step: from each family's slot-3 persist entry, follow the CURRENT-version
DoIO and its geometry-reader callee(s), decompiled in full, to nail the exact
f64 field order (center / radius / angles / corners / poles).

jengine_1075(stream) reads one field; the field byte size is set in the stack
slot right before the call (2=u16, 4=u32, 8=f64, 1=u8, 16=16 bytes).

Read-only (open .i64 run_auto_analysis=False, close save=False).

Usage:
    <idalib-python> tools/idalib_imagdex_geomread.py [imagdex.dex.i64]
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

PRIMS = ("jengine_", "gsu", "hcnstr", "JCoCreate", "RCfatal", "DebugBreak")


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
                if r not in [o[0] for o in out]:
                    out.append((r, nm))
        h = ida_bytes.next_head(h, f.end_ea)
    return out


def is_prim(nm):
    return any(nm.startswith(p) for p in PRIMS)


def show(ea, label, cap=8000):
    print("\n" + "-" * 74, flush=True)
    print(f"[{label} @ {ea:#012x}]", flush=True)
    txt = decomp(ea)
    print(txt[:cap] + (f"\n ...(+{len(txt) - cap})" if len(txt) > cap else ""), flush=True)
    return txt


def current_target(slot3_ea):
    """The dispatch target used when version matches (first non-primitive call)."""
    f = ida_funcs.get_func(slot3_ea)
    if not f:
        return None
    h = f.start_ea
    saw_version = False
    while h < f.end_ea:
        insn = ida_ua.insn_t()
        n = ida_ua.decode_insn(insn, h)
        if n > 0 and insn.get_canon_mnem() == "call":
            t = insn.ops[0].addr or insn.ops[0].value
            if t and seg_text(t):
                r = follow(t)
                nm = ida_funcs.get_func_name(r) or ""
                if nm.startswith("jengine_"):
                    saw_version = True
                elif saw_version and not is_prim(nm):
                    return r
        h = ida_bytes.next_head(h, f.end_ea)
    return None


for fam, vt in VTABLES.items():
    print("\n" + "=" * 78, flush=True)
    slot3 = follow(ida_bytes.get_dword(vt + 3 * 4))
    print(f"##### {fam}: slot3 {slot3:#012x} #####", flush=True)
    cur = current_target(slot3)
    if cur is None:
        print("  (could not resolve current DoIO target)", flush=True)
        continue
    txt = show(cur, f"{fam} CURRENT DoIO {ida_funcs.get_func_name(cur)}")
    # one level into non-primitive callees = the geometry reader(s)
    for r, nm in callees(cur):
        if is_prim(nm):
            continue
        show(r, f"{fam} geom-callee {nm or hex(r)}", cap=6000)

idapro.close_database(save=False)
print("\ndone", flush=True)
