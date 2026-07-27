"""Smoke test: can idalib open one of the vendor IDBs headlessly?

The StyleCluster and curve-family evidence gates both need a native reader,
and the MCP route needs IDA running interactively. idalib would let a probe
run unattended, so check it before planning around it.
"""

import sys

import idapro

db = sys.argv[1] if len(sys.argv) > 1 else r"D:\work\plant-code\cad\pid-parse\dlls\sppid.dll.i64"

rc = idapro.open_database(db, run_auto_analysis=False)
print(f"open_database({db}) -> {rc}")
if rc:
    raise SystemExit(rc)

import ida_funcs
import ida_nalt
import idautils

print("input file :", ida_nalt.get_root_filename())
print("functions  :", ida_funcs.get_func_qty())
names = [n for _, n in idautils.Names()]
print("names      :", len(names))
print("sample     :", names[:10])

idapro.close_database(save=False)
print("closed ok")
