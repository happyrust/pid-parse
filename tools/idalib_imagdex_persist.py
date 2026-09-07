r"""imagdex.dex ships full MSVC symbols + RTTI. The four JSite-only geometry
families are classes JCircle2d / JArc2d / JRectangle2d / JBspCurve2d, each with
an IJPersistImp (Intergraph J persistence) that reads/writes the on-stream
fields. This locates and decompiles those persistence methods to recover the
exact field order -- promoting Circle/Arc/Rectangle/BspCurve from corpus to
native-reader.

Read-only (open .i64 run_auto_analysis=False, close save=False).

Usage:
    <idalib-python> tools/idalib_imagdex_persist.py [imagdex.dex.i64]
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
import idautils
import idc

ida_hexrays.init_hexrays_plugin()

SHORT = idc.get_inf_attr(idc.INF_SHORT_DN)


def demangle(nm: str) -> str:
    d = idc.demangle_name(nm, SHORT)
    return d or nm


FAMS = ("Circle2d", "Arc2d", "Rectangle2d", "BspCurve2d")
# Verbs that mark the stream serializer (avoid bare Load/Save -> over-match).
VERB = re.compile(r"(Persist|DoIO|DoStreamIO|StreamIO|IJStream|ReadData|WriteData|::IO\b)", re.I)

# 1. Census: every named function whose (de)mangled name names one of the four
#    families and a serialization verb.
print("\n########## 1. persist/IO methods for the 4 families ##########", flush=True)
cands = []
for ea in idautils.Functions():
    nm = ida_funcs.get_func_name(ea)
    dm = demangle(nm)
    hay = nm + " || " + dm
    if any(f in hay for f in FAMS) and VERB.search(hay):
        cands.append((ea, dm))
cands.sort()
print(f"(matches: {len(cands)})", flush=True)
for ea, dm in cands:
    print(f"  {ea:#012x}  {dm}", flush=True)

# 2. Decompile the ones that are the actual stream IO (DoIO / DoStreamIO /
#    IJPersist worker), family by family. Skip the ExportedVTable* accessors
#    (they only return a vtable address) and pure Delete/Dirty stubs.
print("\n########## 2. decompile stream serializers ##########", flush=True)
WANT = re.compile(r"(DoIO|DoStreamIO|StreamIO|ReadData|WriteData|Persist)", re.I)
SKIP = re.compile(r"(ExportedVTable|Delete|IsDirty|GetSizeMax|GetClassID)", re.I)


def decompile_one(ea: int, label: str):
    print("\n" + "-" * 74, flush=True)
    print(f"[{label} @ {ea:#012x}]", flush=True)
    try:
        txt = str(ida_hexrays.decompile(ea))
    except Exception as e:  # noqa: BLE001
        print(f"  decompile failed: {e}", flush=True)
        return
    print(txt[:9000] + (f"\n  ...(+{len(txt) - 9000})" if len(txt) > 9000 else ""), flush=True)


shown = 0
for ea, dm in cands:
    if SKIP.search(dm):
        continue
    if not WANT.search(dm):
        continue
    decompile_one(ea, dm)
    shown += 1
print(f"\n(decompiled {shown})", flush=True)

# 3. Fallback: if nothing above is an obvious reader, dump each family's
#    PersistImp vtable via its ExportedVTable* accessor so the next pass can
#    walk the slots.
print("\n########## 3. ExportedVTable*PersistImp -> vtable address ##########", flush=True)
for fam in FAMS:
    want = f"?ExportedVTableJ{fam}PersistImp@@YAKXZ"
    ea = ida_name.get_name_ea(0, want)
    if ea == idc.BADADDR:
        # try without the exact suffix
        for cand_ea in idautils.Functions():
            if f"ExportedVTableJ{fam}PersistImp" in ida_funcs.get_func_name(cand_ea):
                ea = cand_ea
                break
    if ea == idc.BADADDR:
        print(f"  {fam}: ExportedVTable*PersistImp not found", flush=True)
        continue
    # collect .rdata data-refs from the accessor body = the vtable it returns
    vtabs = []
    f = ida_funcs.get_func(ea)
    if f:
        h = f.start_ea
        while h < f.end_ea:
            for dr in idautils.DataRefsFrom(h):
                seg = ida_segment.getseg(dr)
                if seg and ida_segment.get_segm_name(seg) == ".rdata":
                    vtabs.append(dr)
            h = ida_bytes.next_head(h, f.end_ea)
    print(f"  {fam}: accessor @ {ea:#012x}  vtable candidates={[hex(v) for v in vtabs]}", flush=True)
    for vt in vtabs[:1]:
        print(f"    -- vtable @ {vt:#012x} slots --", flush=True)
        for i in range(16):
            slot = ida_bytes.get_dword(vt + i * 4)
            sseg = ida_segment.getseg(slot)
            if not sseg or ida_segment.get_segm_name(sseg) != ".text":
                break
            fn = ida_funcs.get_func_name(slot) if ida_funcs.get_func(slot) else f"sub_{slot:X}?"
            print(f"      [{i:2}] {slot:#012x} {demangle(fn)}", flush=True)

idapro.close_database(save=False)
print("\ndone", flush=True)
