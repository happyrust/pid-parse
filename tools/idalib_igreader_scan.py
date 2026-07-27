"""Locate the ig* 2D graphics-object readers (igCircle2d 0x0059, igArc2d 0x0061,
igLine2d 0x0018, igLineString2d 0x0084) whose on-stream PSM record layout was
established statistically in Phase 36. ugeom2d1's IMG*ReadFromStream2d proved to
be the geometry-math layer (ellipse+majoraxis+ratio), NOT the ig* record layer,
so scan the already-analysed SmartPlant DLLs for the real ig* reader carrier.

Read-only: reuses existing .i64 files, no analysis, no save.
"""

import idapro
import ida_funcs
import ida_nalt
import idautils

DBS = [
    r"D:\work\plant-code\cad\pid-parse\dlls\sppid.dll.i64",
    r"D:\work\plant-code\cad\pid-parse\dlls\core.dll.i64",
    r"D:\work\plant-code\cad\pid-parse\dlls\radsrvitem.dll.i64",
]

# ig* graphics classes + serialization verbs. Keep it broad; report grouped.
FAMILY = ("igcircle", "igarc", "igline", "igellip", "igbspline", "igpoint",
          "igrectangle", "igboundary", "igsymbol", "igtextbox", "ig2d", "igcurve")
VERB = ("read", "load", "store", "doio", "serial", "persist", "streamin",
        "streamout", "getfromstream", "puttostream", "fromstream", "tostream")


def scan(db):
    rc = idapro.open_database(db, run_auto_analysis=False)
    print("\n" + "#" * 78)
    print(f"# {db}  open->{rc}", flush=True)
    if rc:
        return
    print("  input:", ida_nalt.get_root_filename(), " funcs:", ida_funcs.get_func_qty())
    fam_hits, verb_hits = [], []
    for ea, n in idautils.Names():
        low = n.lower()
        if any(f in low for f in FAMILY):
            fam_hits.append((ea, n))
        elif any(low.startswith(v) or ("::" + v) in low for v in VERB) and "2d" in low:
            verb_hits.append((ea, n))
    print(f"  -- ig* family symbols ({len(fam_hits)}) --", flush=True)
    for ea, n in fam_hits[:120]:
        print(f"    {ea:#010x}  {n}")
    print(f"  -- other *2d (de)serialization verbs ({len(verb_hits)}) --", flush=True)
    for ea, n in verb_hits[:60]:
        print(f"    {ea:#010x}  {n}")
    idapro.close_database(save=False)


for db in DBS:
    try:
        scan(db)
    except Exception as e:  # noqa: BLE001
        print(f"  !! {db}: {e}", flush=True)

print("\ndone")
