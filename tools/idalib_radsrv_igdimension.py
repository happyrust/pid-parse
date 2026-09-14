"""Decompile the igDimension reader chain in radsrvitem.dll.

Plan 2026-09-07 (OpenCADStudio), J1 step three: cross-check the flag word of
the `0x0115 JDim` record against a native reading of it. `sub_564BA320` is
the igDimension(277) deserialisation entry; 08-04
(`annotation-families-risk` s3) followed it as far as the chain of five
sub-readers and quoted three bitfield reads out of the first one, but the
bodies were never written down.

The offsets to watch, with the probe's reading of the record beside them
(`examples/probe_jdim_bytes.rs`): if the reader's `a2` is the record's
header rather than its payload, `a2+18` is the payload's `+12` and
`a2+32` is the payload's `+26` -- the flag word whose `0x0300` decides
whether the record carries its closing word. What the reader pulls out of
that dword, and in what order it reads the rest, is what this is for.

Read-only: reuses radsrvitem.dll.i64, no analysis, no save.

Usage:
    <idalib-python> tools/idalib_radsrv_igdimension.py [radsrvitem.dll.i64]
"""

import sys

import idapro

DB = sys.argv[1] if len(sys.argv) > 1 else r"D:\work\plant-code\cad\pid-parse\dlls\radsrvitem.dll.i64"
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

# The chain 08-04 walked, entry first.
CHAIN = [
    ("sub_564BA320", 0x564BA320, "igDimension deserialisation entry"),
    ("sub_564BB990", 0x564BB990, "checks *(WORD*)payload == 277, header + symbology bitfields"),
    ("sub_564BB8B0", 0x564BB8B0, "second sub-reader"),
    ("sub_564BC3B0", 0x564BC3B0, "third sub-reader"),
    ("sub_564BBC40", 0x564BBC40, "fourth sub-reader"),
    ("sub_564BB630", 0x564BB630, "fifth sub-reader"),
    ("sub_564B82A0", 0x564B82A0, "the alignment-code remap the first sub-reader calls"),
    # The second sub-reader switches on the u16 at a2+20 -- the payload's +14,
    # which is 1 on all 18 records of the corpus -- and hands each kind its own
    # reader starting at a2+40, the payload's +34. Kind 1 is the one the corpus
    # uses; sub_56446B50 is how the later sub-readers skip past that block, so
    # between them they are the block grammar the byte probe could not pin.
    ("sub_564BBAE0", 0x564BBAE0, "kind 1/8 block reader, from payload +34"),
    ("sub_56446B50", 0x56446B50, "the size of the kind block -- how the tail is found"),
]
CAP = 14000


def seg_of(ea: int) -> str:
    segment = ida_segment.getseg(ea)
    return ida_segment.get_segm_name(segment) if segment else ""


def decompile(ea: int) -> str:
    if not ida_funcs.get_func(ea):
        ida_funcs.add_func(ea)
    try:
        code = ida_hexrays.decompile(ea)
        return str(code) if code else "<no pseudocode>"
    except Exception as error:  # noqa: BLE001
        return f"<<decompile failed: {error}>>"


def callees(func_ea: int) -> list[tuple[int, str]]:
    func = ida_funcs.get_func(func_ea)
    if not func:
        return []
    out, seen = [], set()
    at = func.start_ea
    while at < func.end_ea:
        insn = ida_ua.insn_t()
        size = ida_ua.decode_insn(insn, at)
        if size > 0 and insn.get_canon_mnem() == "call":
            target = insn.ops[0].addr or insn.ops[0].value
            if target and seg_of(target) == ".text" and target not in seen:
                seen.add(target)
                out.append((target, idc.get_func_name(target) or f"sub_{target:X}"))
        at = ida_bytes.next_head(at, func.end_ea)
    return out


for name, ea, note in CHAIN:
    print("\n" + "=" * 78, flush=True)
    print(f"[{name} @ {ea:#010x}] {note}", flush=True)
    print(f"  callees: {[callee_name for _, callee_name in callees(ea)][:20]}", flush=True)
    text = decompile(ea)
    print(text[:CAP] + (f"\n  ...(+{len(text) - CAP} more)" if len(text) > CAP else ""), flush=True)

idapro.close_database(save=False)
print("\ndone", flush=True)
