r"""Dump the full decompilation of every non-IUnknown slot of one family's
vtable in imagdex.dex.i64, to eyeball which slot is the IJPersist DoIO/Load
(the one that reads the geometry fields off the stream).

Usage:
    <idalib-python> tools/idalib_imagdex_dumpslots.py <vtable_hex> [start_idx]
    e.g. Circle2d: ... 0x1052A460 3
"""

import sys

import idapro

DB = r"D:\work\plant-code\cad\pid-parse\dlls\imagdex.dex.i64"
VT = int(sys.argv[1], 16) if len(sys.argv) > 1 else 0x1052A460
START = int(sys.argv[2]) if len(sys.argv) > 2 else 3

rc = idapro.open_database(DB, run_auto_analysis=False)
print(f"open_database -> {rc}  vtable={VT:#012x} start={START}", flush=True)
if rc:
    raise SystemExit(rc)

import ida_bytes
import ida_funcs
import ida_hexrays
import ida_segment
import ida_ua

ida_hexrays.init_hexrays_plugin()


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


i = 0
while True:
    slot = ida_bytes.get_dword(VT + i * 4)
    if not seg_text(slot):
        break
    if i >= START:
        real = follow(slot)
        if not ida_funcs.get_func(real):
            ida_funcs.add_func(real)
        print("\n" + "=" * 76, flush=True)
        print(f"[slot {i}] thunk {slot:#012x} -> {real:#012x}", flush=True)
        try:
            cf = ida_hexrays.decompile(real)
            txt = str(cf) if cf else "<none>"
        except Exception as e:  # noqa: BLE001
            txt = f"<<fail {e}>>"
        print(txt[:5500] + (f"\n ...(+{len(txt) - 5500})" if len(txt) > 5500 else ""), flush=True)
    i += 1
    if i > 45:
        break

idapro.close_database(save=False)
print("\ndone", flush=True)
