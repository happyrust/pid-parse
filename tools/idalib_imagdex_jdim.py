r"""Recover the on-stream field order of `0x0115 JDim Object` by decompiling
the `IJPersist` DoIO of imagdex's `JDim` class.

Plan 2026-09-07 (OpenCADStudio), J1 step two, the same route 08-31 took for
the four JSite-only curve families (`idalib_imagdex_vtable.py` ->
`idalib_imagdex_doio.py`): slot 3 of a class's PersistImp vtable is the
persistence entry -- it asks `jengine_1076` for the stream version and then
dispatches to the version's DoIO worker, which reads or writes each member
through `jengine_1075(stream, size, &member)`. The field order of that
worker is the record layout.

What differs here is finding the class. The curve families export
`ExportedVTableJ<Fam>2dPersistImp` accessors that hand out the vtable
address; `JDim` exports none. So this walks RTTI instead: the type
descriptor `.?AVJDim@@` is referenced by the class's complete object
locators, each locator is the word before a vtable, and the vtable that
matters is the one whose slots call the jengine persistence primitives.
That discovery is printed, so a reader can check the identification rather
than trust an address baked into a script.

The `JDim` record and `JDimGroup` (`0x0058`) sit next to each other in the
corpus -- a dimension's second reference slot often names its group -- so
this does both classes.

Result of the first run (2026-09-14): **the 08-31 route does not transfer.**
`JDim` and `JDimGroup` each reach exactly one vtable, of 8 and 7 slots, and
no slot of either reaches `jengine_1075` / `jengine_1076`; imagdex carries
no `IJPersistImp@JDim@@` sub-object the way it carries
`IJPersistImp@JStyleBase@@` and the rest. What it does carry is
`tagDimPersistData` / `tagDimGrpPersistData` / `tagAnnotPersistData`, whose
single virtual is a deleting destructor and whose constructor
(`sub_1032F87D` for the dimension) zeroes a structure of some 736 bytes --
the in-memory payload, not the stream layout. So the reader is somewhere
this walk does not reach, and the next move is either the other native
entry the plan names (`radsrvitem.dll!sub_564BA320`, the five-level
igDimension reader) or a wider sweep of `jengine_1075` callers: this IDB
has almost no function bounds stored, so the 2161 call sites resolve to no
function until each one's prologue is walked back to by hand.

Read-only (open .i64 run_auto_analysis=False, close save=False).

Usage:
    <idalib-python> tools/idalib_imagdex_jdim.py [imagdex.dex.i64]
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
import idautils
import idc

ida_hexrays.init_hexrays_plugin()
SHORT = idc.get_inf_attr(idc.INF_SHORT_DN)

# The RTTI type descriptors to walk. `JDim` and `JDimGroup` are the classes
# behind PSM `0x0115` and `0x0058`; the three `tag*PersistData` structs are
# what imagdex calls the persistence payload of a dimension, its group and an
# annotation, and are the only persistence-named types the module carries for
# them.
CLASSES = [
    ".?AVJDim@@",
    ".?AVJDimGroup@@",
    ".?AUtagDimPersistData@@",
    ".?AUtagDimGrpPersistData@@",
    ".?AUtagAnnotPersistData@@",
]
# The jengine ordinals the curve families' DoIO workers are built out of:
# 1076 hands back the stream version, 1075 moves one member.
PERSIST_PRIMITIVES = ("jengine_1075", "jengine_1076")
# Enough of a worker to read the field order without dumping the whole file.
CAP = 12000



def demangle(name: str) -> str:
    return idc.demangle_name(name, SHORT) or name


def seg_of(ea: int) -> str:
    seg = ida_segment.getseg(ea)
    return ida_segment.get_segm_name(seg) if seg else ""


def follow(ea: int, depth: int = 5) -> int:
    """Step through thunks to the function that does the work."""
    for _ in range(depth):
        if not ida_funcs.get_func(ea):
            ida_funcs.add_func(ea)
        insn = ida_ua.insn_t()
        if ida_ua.decode_insn(insn, ea) > 0 and insn.get_canon_mnem() == "jmp":
            target = insn.ops[0].addr or insn.ops[0].value
            if target and seg_of(target) == ".text":
                ea = target
                continue
        break
    return ea


def name_of(ea: int) -> str:
    func = ida_funcs.get_func(ea)
    if not func:
        return f"sub_{ea:X}"
    return demangle(ida_funcs.get_func_name(func.start_ea))


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
            if target and seg_of(target) == ".text":
                resolved = follow(target)
                if resolved not in seen:
                    seen.add(resolved)
                    out.append((resolved, name_of(resolved)))
        at = ida_bytes.next_head(at, func.end_ea)
    return out


def references_primitive(func_ea: int) -> bool:
    return any(
        any(name.startswith(prefix) for prefix in PERSIST_PRIMITIVES)
        for _, name in callees(func_ea)
    )


def type_descriptor(name: str) -> int | None:
    for string in idautils.Strings():
        if str(string) == name:
            return string.ea - 8  # the descriptor's vftable/spare words
    return None


def word_holders(value: int) -> list[int]:
    """Every address of an initialised segment holding `value` as a dword.

    IDA's data xrefs miss pointer tables it never turned into offsets, and
    both steps of the RTTI walk are exactly such tables, so this scans the
    bytes instead.
    """
    needle = value.to_bytes(4, "little")
    out = []
    for start in idautils.Segments():
        segment = ida_segment.getseg(start)
        if ida_segment.get_segm_name(segment) not in (".rdata", ".data"):
            continue
        blob = ida_bytes.get_bytes(segment.start_ea, segment.end_ea - segment.start_ea) or b""
        at = 0
        while (at := blob.find(needle, at)) >= 0:
            out.append(segment.start_ea + at)
            at += 4 if at % 4 == 0 else 1
    return out


def vtables_of(descriptor: int) -> list[int]:
    """Every vtable whose complete object locator names this descriptor.

    32-bit MSVC RTTI: a complete object locator holds the type descriptor's
    address at `+12`, and the word before a vtable holds the locator's.
    """
    out = []
    for at in word_holders(descriptor):
        locator = at - 12
        if ida_bytes.get_dword(locator) != 0:  # signature, 0 in 32-bit images
            continue
        for slot in word_holders(locator):
            vtable = slot + 4
            if seg_of(ida_bytes.get_dword(vtable)) == ".text" and vtable not in out:
                out.append(vtable)
    return out


def slots_of(vtable: int, limit: int = 32) -> list[tuple[int, int, str]]:
    out = []
    for index in range(limit):
        entry = ida_bytes.get_dword(vtable + index * 4)
        if seg_of(entry) != ".text":
            break
        out.append((index, entry, name_of(entry)))
    return out


def show(ea: int, label: str) -> None:
    print("\n" + "-" * 74, flush=True)
    print(f"[{label} @ {ea:#012x}]", flush=True)
    text = decompile(ea)
    print(text[:CAP] + (f"\n ...(+{len(text) - CAP} more)" if len(text) > CAP else ""), flush=True)


for class_name in CLASSES:
    print("\n" + "=" * 78, flush=True)
    print(f"##### {class_name} #####", flush=True)
    descriptor = type_descriptor(class_name)
    if descriptor is None:
        print("  no RTTI type descriptor -- the class is not in this module", flush=True)
        continue
    print(f"  type descriptor @ {descriptor:#012x}", flush=True)
    vtables = vtables_of(descriptor)
    print(f"  {len(vtables)} vtables reach it", flush=True)

    persist = []
    for vtable in vtables:
        slots = slots_of(vtable)
        marked = [
            (index, entry, name)
            for index, entry, name in slots
            if references_primitive(follow(entry))
        ]
        flag = " <- persistence" if marked else ""
        print(f"\n  -- vtable @ {vtable:#012x}, {len(slots)} slots{flag}", flush=True)
        for index, entry, name in slots:
            mark = "*" if any(index == i for i, _, _ in marked) else " "
            print(f"    {mark}[{index:2}] {entry:#012x} {name}", flush=True)
        if marked:
            persist.append((vtable, slots, marked))

    for vtable, slots, marked in persist:
        print("\n" + "#" * 74, flush=True)
        print(f"## {class_name} persistence vtable {vtable:#012x} ##", flush=True)
        for index, entry, name in marked:
            target = follow(entry)
            show(target, f"{class_name} slot {index} {name}")
            for callee, callee_name in callees(target):
                if any(callee_name.startswith(prefix) for prefix in PERSIST_PRIMITIVES):
                    print(f"   (callee {callee_name} -- jengine primitive)", flush=True)
                    continue
                if re.match(r"^(gsu|hcnstr|operator|__)", callee_name):
                    print(f"   (callee {callee_name} -- runtime, skipped)", flush=True)
                    continue
                show(callee, f"{class_name} worker {callee_name}")

idapro.close_database(save=False)
print("\ndone", flush=True)
