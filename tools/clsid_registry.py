r"""Resolve a RAD framework CLSID to its implementing module and friendly name.

`jutil.dll` carries the RAD CLSID registry as a flat array of 96-byte entries:

```text
  +0   GUID   16 bytes
  +16  char[] implementing module, NUL-padded to 16 bytes
  +32  char[] friendly name, NUL-terminated
```

Phase 16 used this to prove PSM `0x0030` is `JStyleOverride`; this reads the
same table from the file so the mapping is reproducible without IDA.

`jutil.dll` is not in `dlls/`. On this machine it is under `D:\pid\`; point
`--jutil` at wherever a RAD install has it.

Usage:
  python tools/clsid_registry.py 47FCC338-2D0F-11D0-A1FF-080036A1CF02
  python tools/clsid_registry.py --grep "Facelet"
  python tools/clsid_registry.py --self-test
"""

import argparse
import struct
import sys
from pathlib import Path

DEFAULT_JUTIL = Path(r"D:\pid\RADInstallA~\jutil.dll")

ENTRY = 96
MODULE_AT = 16
NAME_AT = 32

# Phase 16's published result, used as an independent control: if this does not
# come back exactly, the table layout has changed and nothing else is credible.
CONTROL = ("47FCC338-2D0F-11D0-A1FF-080036A1CF02", "style.dll", "JSL Override Style")


def parse_guid(text: str) -> bytes:
    parts = text.strip().strip("{}").split("-")
    if len(parts) != 5:
        raise ValueError(f"not a GUID: {text}")
    d1, d2, d3 = int(parts[0], 16), int(parts[1], 16), int(parts[2], 16)
    rest = bytes.fromhex(parts[3] + parts[4])
    if len(rest) != 8:
        raise ValueError(f"not a GUID: {text}")
    return struct.pack("<IHH", d1, d2, d3) + rest


def format_guid(raw: bytes) -> str:
    d1, d2, d3 = struct.unpack_from("<IHH", raw, 0)
    rest = raw[8:16]
    tail = "".join(f"{b:02X}" for b in rest)
    return f"{d1:08X}-{d2:04X}-{d3:04X}-{tail[:4]}-{tail[4:]}"


def cstr(data: bytes, at: int, limit: int) -> str:
    end = data.find(b"\0", at)
    if end < 0 or end - at > limit:
        end = at + limit
    return data[at:end].decode("latin1", "replace")


def entry_at(data: bytes, at: int) -> tuple[str, str, str]:
    return (
        format_guid(data[at : at + 16]),
        cstr(data, at + MODULE_AT, 16),
        cstr(data, at + NAME_AT, 64),
    )


def lookup(data: bytes, guid: str) -> tuple[str, str, str] | None:
    at = data.find(parse_guid(guid))
    return entry_at(data, at) if at >= 0 else None


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("guids", nargs="*", help="CLSIDs to resolve")
    ap.add_argument("--jutil", type=Path, default=DEFAULT_JUTIL)
    ap.add_argument("--grep", help="list entries whose name matches (case-insensitive)")
    ap.add_argument("--self-test", action="store_true", help="check the control entry only")
    args = ap.parse_args()

    if not args.jutil.is_file():
        print(f"jutil.dll not found at {args.jutil}", file=sys.stderr)
        return 2
    data = args.jutil.read_bytes()

    got = lookup(data, CONTROL[0])
    if got is None or (got[1], got[2]) != (CONTROL[1], CONTROL[2]):
        print(f"CONTROL FAILED: expected {CONTROL[1:]}, got {got}", file=sys.stderr)
        return 1
    print(f"control ok: {CONTROL[0]} -> {CONTROL[1]} / {CONTROL[2]}")
    if args.self_test:
        return 0

    if args.grep:
        needle = args.grep.lower()
        # The array is not aligned to a known base, so walk every 16-byte slot
        # and keep the ones that look like a populated entry.
        seen = set()
        for at in range(0, len(data) - ENTRY, 16):
            module = cstr(data, at + MODULE_AT, 16)
            name = cstr(data, at + NAME_AT, 64)
            if not module.endswith((".dll", ".dex", ".exe")) or needle not in name.lower():
                continue
            guid = format_guid(data[at : at + 16])
            if guid in seen:
                continue
            seen.add(guid)
            print(f"  {guid}  {module:<16} {name}")
        return 0

    for guid in args.guids:
        found = lookup(data, guid)
        if found is None:
            print(f"  {guid}: not in registry")
        else:
            print(f"  {found[0]}  {found[1]:<16} {found[2]}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
