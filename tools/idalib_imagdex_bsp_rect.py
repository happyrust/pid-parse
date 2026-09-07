r"""Nail BspCurve2d (variable-length control points) and Rectangle2d semantics.

- BspCurve: slot3 dispatch, the current-version DoIO (hard-decompile with
  auto_wait + disasm fallback), plus sub_102DA280 (the control-point loop) and
  their non-primitive callees, to read the pole/knot/degree/count order.
- Rectangle: re-decompile slot3 in full to confirm the dispatch target and
  whether the current worker has the u16+u32 prefix; decompile its geom callee.

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
import idc

ida_hexrays.init_hexrays_plugin()

PRIMS = ("jengine_", "gsu", "hcnstr", "JCoCreate", "RCfatal", "DebugBreak",
         "memset", "memcpy", "free", "malloc", "operator", "unknown_libname",
         "__security", "_except", "__CxxThrow", "std::")


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


def disasm_range(ea, n=160):
    lines = []
    f = ida_funcs.get_func(ea)
    end = f.end_ea if f else ea + 0x400
    h = ea
    c = 0
    while h < end and c < n:
        lines.append(f"    {h:#012x}: {idc.generate_disasm_line(h, 0)}")
        h = ida_bytes.next_head(h, end)
        c += 1
    return "\n".join(lines)


def hard_decomp(ea):
    if not ida_funcs.get_func(ea):
        ida_funcs.add_func(ea)
    ida_auto.auto_wait()
    try:
        cf = ida_hexrays.decompile(ea)
        if cf:
            return str(cf)
    except Exception as e:  # noqa: BLE001
        return f"<<fail {e}>>\n" + disasm_range(ea)
    return "<none> -- raw disasm:\n" + disasm_range(ea)


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


def show(ea, label, cap=8500):
    print("\n" + "-" * 74, flush=True)
    print(f"[{label} @ {ea:#012x}]", flush=True)
    txt = hard_decomp(ea)
    print(txt[:cap] + (f"\n ...(+{len(txt) - cap})" if len(txt) > cap else ""), flush=True)


# ---- Rectangle -------------------------------------------------------------
print("\n" + "=" * 78, flush=True)
print("##### Rectangle2d #####", flush=True)
rect_slot3 = follow(ida_bytes.get_dword(0x1052AD80 + 3 * 4))
show(rect_slot3, "Rect slot3 dispatch")

# ---- BspCurve --------------------------------------------------------------
print("\n" + "=" * 78, flush=True)
print("##### BspCurve2d #####", flush=True)
bsp_slot3 = follow(ida_bytes.get_dword(0x1052A5FC + 3 * 4))
show(bsp_slot3, "Bsp slot3 dispatch")
bsp_cur = follow(0x1000CC89)
show(bsp_cur, "Bsp CURRENT DoIO (sub_1000CC89 followed)")
for r, nm in callees(bsp_cur):
    if is_prim(nm):
        continue
    show(r, f"Bsp cur-callee {nm or hex(r)}", cap=6500)

print("\n" + "=" * 78, flush=True)
print("##### BspCurve control-point loop sub_102DA280 #####", flush=True)
show(0x102DA280, "sub_102DA280 (pole loop)")
for r, nm in callees(0x102DA280):
    if is_prim(nm):
        continue
    show(r, f"poleloop-callee {nm or hex(r)}", cap=5000)

idapro.close_database(save=False)
print("\ndone", flush=True)
