r"""One-time full auto-analysis of imagdex.dex -> imagdex.dex.i64.

The unpacked db shipped next to the .dex (id0/id1/id2/nam/til) only had 3
recognised functions -- a prior session opened it but never finished analysis.
This resumes that db, runs the full analysis, and saves a packed .i64 so every
follow-on decompile pass opens in seconds with run_auto_analysis=False.

The id* files were backed up to dlls/_imagdex-idb-backup-20260831 first; saving
packs them into imagdex.dex.i64 and removes the loose id* (standard IDA behaviour).

Usage:
    <idalib-python> tools/idalib_imagdex_build.py
"""

import time
import sys

import idapro

DB = sys.argv[1] if len(sys.argv) > 1 else r"D:\work\plant-code\cad\pid-parse\dlls\imagdex.dex"

print(f"analyzing (full auto-analysis): {DB}", flush=True)
t0 = time.time()
rc = idapro.open_database(DB, run_auto_analysis=True)
print(f"open_database -> {rc}  ({time.time() - t0:.1f}s)", flush=True)
if rc:
    raise SystemExit(rc)

import ida_auto
import idautils

ida_auto.auto_wait()
print(f"analysis done ({time.time() - t0:.1f}s)", flush=True)
print(f"functions: {sum(1 for _ in idautils.Functions())}", flush=True)

t1 = time.time()
idapro.close_database(save=True)
print(f"saved i64 ({time.time() - t1:.1f}s), total {time.time() - t0:.1f}s", flush=True)
print("BUILD_DONE", flush=True)
