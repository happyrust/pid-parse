"""Decompile the igTextBox Load in radsrvitem.dll and the tail reader it hands
the placement block to.

Two structures were still unexplained after 2026-08-13:

* tail kind 2 (payload +20 == 2) hangs 32 or 40 further bytes past the
  36-byte placement block;
* shape 3 (payload +18 == 3) carries `A + B` entries of 8 bytes each, walked
  with a 1/2 selector and dispatched to two different vtable slots.

Chain found by the earlier passes:

* `IGDSFactoryText` (`operator new(0x5C)`, built by `sub_56497EE0`) carries the
  Load at `??_7IGDSFactoryText@@6B@+60` and GetSize at `+64`;
* the text sink is the second base at `this + 20`
  (`??_7IGDSFactoryText@@6B@_0`, 22 slots): `+56` SetText, `+68` append run
  with selector 1, `+72` append run with selector 2, `+76` reset,
  `+84` get-or-create the shape object;
* `+84` stores an `IGDSFactoryTextPointRectShape` at `this + 88`
  (`??_7IGDSFactoryTextPointRectShape@@6B@` @ 0x566667cc), and the placement
  tail is `slot +24 (…, 0, kind, ptr)` on it. `IGDSFactoryText::GetSize` adds
  that object's own `slot +28` size to the head+body size, so this class is
  what closes the byte account.

Read-only: reuses radsrvitem.dll.i64, no auto-analysis, no save.

    python tools/idalib_igtextbox_style_apply.py
"""

import pathlib
import shutil
import tempfile

import idapro

# `dlls/radsrvitem.dll.i64` cannot be opened in place: an *unpacked* database
# (.id0/.id1/.id2/.nam/.til) was left next to it by an IDA session that did not
# close cleanly. Headless IDA answers that prompt with "restore packed base",
# which wants to delete those files, and the .id0 refuses with "permission
# denied" -> error 4. Copying the packed database to a scratch directory sits
# beside no leftovers, so it opens, and nothing in the repo is touched.
SRC = r"D:\work\plant-code\cad\pid-parse\dlls\radsrvitem.dll.i64"
SCRATCH = pathlib.Path(tempfile.gettempdir()) / "pidparse-ida-t42"
SCRATCH.mkdir(parents=True, exist_ok=True)
DB = SCRATCH / "radsrvitem.dll.i64"
if not DB.exists():
    shutil.copyfile(SRC, DB)

rc = idapro.open_database(str(DB), run_auto_analysis=False)
print(f"open_database({str(DB)!r}) -> {rc}", flush=True)
if rc:
    raise SystemExit(rc)

import ida_bytes  # noqa: E402
import ida_hexrays  # noqa: E402
import ida_name  # noqa: E402

ida_hexrays.init_hexrays_plugin()


def pseudo(ea):
    try:
        return str(ida_hexrays.decompile(ea))
    except Exception as exc:  # noqa: BLE001
        return f"<decompile failed: {exc}>"


def dump(name, ea, limit=24000):
    print("\n" + "=" * 78)
    print(f"[{name}] @ {ea:#010x}", flush=True)
    txt = pseudo(ea)
    print(txt[:limit])
    if len(txt) > limit:
        print(f"  ... (+{len(txt) - limit} chars truncated)")
    return txt


def vtable(label, head, count):
    print("\n" + "#" * 78)
    print(f"# {label} @ {head:#010x}")
    print("#" * 78)
    out = []
    for idx in range(count):
        ea = head + idx * 4
        val = ida_bytes.get_dword(ea)
        nm = ida_name.get_ea_name(val) or f"sub_{val:X}"
        print(f"  +{idx * 4:<4} {ea:#010x}  {val:#010x}  {nm}")
        out.append(val)
    return out


vtable("??_7IGDSFactoryTextShapeRoot@@6B@", 0x566667A4, 9)
vtable("??_7IGDSFactoryTextPointRectShape@@6B@", 0x566667CC, 10)

TAIL = {
    "PointRectShape +24  Load(0, kind, ptr)": 0x56498780,
    "PointRectShape +28  GetSize": 0x56497AE0,
    "PointRectShape +32  Save": 0x56497BF0,
    "PointRectShape +36": 0x56497A10,
    "PointRectShape ctor sub_56497850": 0x56497850,
    "shape object allocator sub_56497680": 0x56497680,
}
for name, ea in TAIL.items():
    dump(name, ea)

idapro.close_database(save=False)
print("\nclosed")
