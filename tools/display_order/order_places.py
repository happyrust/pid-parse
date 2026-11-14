r"""The places, in the JSheet's child table, of the top-level objects a
reference plot paints, in the order it paints them — run-length encoded, so
that a plot painting in table order prints one rising run.

usage: python order_places.py <corpus dir> <models dir> <name part> [--oids]
"""
import io
import sys
from pathlib import Path

from jsheet_list import sheet_children
from order_map import painted
from pid_records import NAMES

sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8")
corpus, models, part = Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3]
show_oids = "--oids" in sys.argv
model = next(m for m in sorted(models.glob("*.model.json")) if part in m.name)
stem = model.name[: -len(".model.json")]
pid = str(corpus / f"{stem}.pid")
rows, records_by_offset, primitives, page = painted(str(corpus / f"{stem}.pdf"), str(model), pid)
by_oid = {record["oid"]: record for record in records_by_offset.values()}
table = sheet_children(pid, 6)
place = {oid: index for index, (oid, _, _) in enumerate(table["entries"])}
visits = []
for row in rows:
    obj = row["object"]
    if obj is None or obj[0] != "D":
        key = ("T", -1) if obj and obj[0] == "T" else None
    else:
        record = records_by_offset.get(obj[1])
        top = record["oid"] if record["parent"] == 6 else record["parent"]
        key = ("D", place.get(top, -2), top)
    if key is None:
        continue
    if not visits or visits[-1] != key:
        visits.append(key)
print(f"{stem}: {len(visits)} visits of top-level objects")
# runs of rising places
runs = []
for key in visits:
    if key[0] == "T":
        runs.append(["T"])
        continue
    if runs and runs[-1][0] != "T" and key[1] >= runs[-1][-1][1]:
        runs[-1].append(key)
    else:
        runs.append([key])
print(f"{len(runs)} rising run(s)")
for run in runs[:60]:
    if run[0] == "T":
        print("   template")
        continue
    first, last = run[0], run[-1]
    detail = ""
    if show_oids:
        detail = " oids " + " ".join(str(key[2]) for key in run[:12]) + (" …" if len(run) > 12 else "")
    print(f"   places {first[1]:5d} … {last[1]:5d}  ({len(run)} object(s)){detail}")
