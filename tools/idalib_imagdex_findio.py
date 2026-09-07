r"""Find the stream serializer slot in each family's IJPersistImp vtable by
scoring every slot on how many fixed-size field reads it performs, then
decompile the winner. A geometry serializer reads its coordinates as doubles
(size 8): Circle center+radius = 3, Arc center+radius+start+end = 5.

vtables (from idalib_imagdex_vtable.py):
  Circle2d 0x1052A460  Arc2d 0x1052996C  Rectangle2d 0x1052AD80  BspCurve2d 0x1052A5FC

Read-only (open .i64 run_auto_analysis=False, close save=False).

Usage:
    <idalib-python> tools/idalib_imagdex_findio.py [imagdex.dex.i64]
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
import ida_segment
import ida_ua
import idc

ida_hexrays.init_hexrays_plugin()


def follow_thunk(ea: int, depth: int = 5) -> int:
    """Chase jmp-thunks to the real function body."""
    for _ in range(depth):
        if not ida_funcs.get_func(ea):
            ida_funcs.add_func(ea)
        insn = ida_ua.insn_t()
        if ida_ua.decode_insn(insn, ea) > 0 and insn.get_canon_mnem() == "jmp":
            t = insn.ops[0].addr or insn.ops[0].value
            seg = ida_segment.getseg(t) if t else None
            if seg and ida_segment.get_segm_name(seg) == ".text":
                ea = t
                continue
        break
    return ea

VTABLES = {
    "Circle2d": 0x1052A460,
    "Arc2d": 0x1052996C,
    "Rectangle2d": 0x1052AD80,
    "BspCurve2d": 0x1052A5FC,
}


def seg_is_text(ea: int) -> bool:
    seg = ida_segment.getseg(ea)
    return bool(seg) and ida_segment.get_segm_name(seg) == ".text"


def slots_of(vt: int):
    out = []
    i = 0
    while True:
        slot = ida_bytes.get_dword(vt + i * 4)
        if not seg_is_text(slot):
            break
        out.append(slot)
        i += 1
        if i > 40:
            break
    return out


def count_sz(text: str, n) -> int:
    pat = rf'(?<![\w])(?:{n})(?:i64|u|LL)?[,)]'
    return len(re.findall(pat, text))


def decomp(ea: int):
    ea = follow_thunk(ea)
    if not ida_funcs.get_func(ea):
        ida_funcs.add_func(ea)
    try:
        cf = ida_hexrays.decompile(ea)
    except Exception as e:  # noqa: BLE001
        return f"<<decompile failed: {e}>>"
    return str(cf) if cf is not None else "<<none>>"


def io_calls(text: str) -> int:
    return text.count("jengine_") + text.count("gsugeom2d1_") + text.count("gsugeom")


best = {}
for fam, vt in VTABLES.items():
    slots = slots_of(vt)
    print(f"\n===== {fam}: vtable {vt:#012x}, {len(slots)} slots =====", flush=True)
    print("  idx  addr          jio n8  n16 chars", flush=True)
    scored = []
    for i, slot in enumerate(slots):
        txt = decomp(slot)
        jio = io_calls(txt)
        n8 = count_sz(txt, 8)
        n16 = count_sz(txt, "16|0x10")
        score = jio * 10 + n8 * 2 + n16
        scored.append((score, i, slot, jio, n8, n16, len(txt), txt))
        flag = "  <-- IO-heavy" if jio >= 3 else ""
        print(f"  [{i:2}] {slot:#012x}  {jio:<3} {n8:<3} {n16:<3} {len(txt)}{flag}", flush=True)
    scored_body = [s for s in scored if s[1] >= 3]
    scored_body.sort(reverse=True)
    best[fam] = scored_body[:2]

print("\n" + "#" * 78, flush=True)
print("## decompiled serializer candidates (top-2 by IO density) ##", flush=True)
for fam, vt in VTABLES.items():
    for score, i, slot, jio, n8, n16, ln, txt in best.get(fam, []):
        print("\n" + "=" * 74, flush=True)
        print(f"[{fam} slot {i} @ {slot:#012x}  jio={jio} n8={n8} n16={n16}]", flush=True)
        print(txt[:11000] + (f"\n  ...(+{len(txt) - 11000})" if len(txt) > 11000 else ""), flush=True)

idapro.close_database(save=False)
print("\ndone", flush=True)
