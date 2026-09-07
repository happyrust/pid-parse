r"""Walk each family's IJPersistImp vtable in imagdex.dex.i64 and decompile the
stream-IO slot, to recover the on-stream field order for
JCircle2d / JArc2d / JRectangle2d / JBspCurve2d.

The ExportedVTableJ<Fam>2dPersistImp accessors just `return &vtable`; we read
the .rdata address they reference, dump the slots (demangled -> the slot named
DoIO/Load/Save is the serializer), then decompile it.

Read-only (open .i64 run_auto_analysis=False, close save=False).

Usage:
    <idalib-python> tools/idalib_imagdex_vtable.py [imagdex.dex.i64]
"""

import re
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
import ida_name
import ida_segment
import ida_ua
import idautils
import idc

ida_hexrays.init_hexrays_plugin()
SHORT = idc.get_inf_attr(idc.INF_SHORT_DN)


def demangle(nm: str) -> str:
    return idc.demangle_name(nm, SHORT) or nm


def seg_is(ea: int, name: str) -> bool:
    seg = ida_segment.getseg(ea)
    return bool(seg) and ida_segment.get_segm_name(seg) == name


# The exported names are thunks (JUMPOUT); these are the real accessor bodies
# they jump to, which `return &vtable`.
ACCESSORS = {
    "Circle2d": 0x10154800,
    "Arc2d": 0x10143B20,
    "Rectangle2d": 0x101693C0,
    "BspCurve2d": 0x10159420,
}


def rdata_refs_in(func_ea: int, maxins: int = 12):
    """Disassemble up to maxins instructions from func_ea (creating the function
    if IDA never bounded it) and return every .rdata address in an operand."""
    if not ida_funcs.get_func(func_ea):
        ida_funcs.add_func(func_ea)
    out = []
    ea = func_ea
    for _ in range(maxins):
        insn = ida_ua.insn_t()
        n = ida_ua.decode_insn(insn, ea)
        if n <= 0:
            break
        print(f"    {ea:#012x}: {idc.generate_disasm_line(ea, 0)}", flush=True)
        for op in insn.ops:
            if op.type == 0:
                break
            for cand in (op.addr, op.value):
                seg = ida_segment.getseg(cand) if cand else None
                if seg and ida_segment.get_segm_name(seg) in (".rdata", ".data"):
                    out.append(cand)
        mnem = insn.get_canon_mnem()
        ea += n
        if mnem in ("retn", "ret", "jmp"):
            break
    seen = set()
    uniq = []
    for a in out:
        if a not in seen:
            seen.add(a)
            uniq.append(a)
    return uniq


serializer_targets = []

for fam, acc in ACCESSORS.items():
    print("\n" + "=" * 78, flush=True)
    print(f"== {fam}: accessor @ {acc:#012x} ==", flush=True)
    vtabs = rdata_refs_in(acc)
    print(f"  .rdata refs: {[hex(v) for v in vtabs]}", flush=True)
    for vt in vtabs:
        print(f"  -- vtable @ {vt:#012x} slots --", flush=True)
        for i in range(24):
            slot = ida_bytes.get_dword(vt + i * 4)
            if not seg_is(slot, ".text"):
                break
            fn = ida_funcs.get_func_name(slot) if ida_funcs.get_func(slot) else f"sub_{slot:X}"
            dfn = demangle(fn)
            print(f"    [{i:2}] {slot:#012x} {dfn}", flush=True)
            if re.search(r"(DoIO|DoStreamIO|StreamIO|::Load|::Save|ReadData)", dfn, re.I):
                serializer_targets.append((fam, i, slot, dfn))

print("\n" + "#" * 78, flush=True)
print("## decompile serializer slots ##", flush=True)
seen = set()
for fam, i, slot, dfn in serializer_targets:
    if slot in seen:
        continue
    seen.add(slot)
    print("\n" + "-" * 74, flush=True)
    print(f"[{fam} slot {i}: {dfn} @ {slot:#012x}]", flush=True)
    try:
        txt = str(ida_hexrays.decompile(slot))
    except Exception as e:  # noqa: BLE001
        print(f"  decompile failed: {e}", flush=True)
        continue
    print(txt[:10000] + (f"\n  ...(+{len(txt) - 10000})" if len(txt) > 10000 else ""), flush=True)

idapro.close_database(save=False)
print("\ndone", flush=True)
