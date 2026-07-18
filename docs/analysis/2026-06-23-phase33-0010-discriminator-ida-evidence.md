# Phase 33 PSM 0x0010 Discriminator IDA Evidence

> Date: 2026-06-23  
> Scope: `0x0010` discriminator availability check and negative closeout.  
> Confidence impact: no parser, schema, writer, bundle, or roadmap confidence
> change.

## Result

The Phase 33 IDA availability check is tooling-gated. The only reachable IDA
instances are:

| Module | Host | Port | IDB path | Role | Result |
|---|---:|---:|---|---|---|
| `sppid.dll` | `127.0.0.1` | `13337` | `D:\work\plant-code\cad\pid-parse\dlls\sppid.dll.i64` | SmartPlant application / COM layer already treated as insufficient for raw `.pid` persistence promotion | Reachable, active, not a valid promotion source by itself |
| `core.dll` | `127.0.0.1` | `13338` | `D:\AVEVA\Everything3D3.1\core.dll.i64` | AVEVA E3D platform module, not SmartPlant raw `.pid` Sheet persistence | Reachable, unrelated for this gate |

No relevant target from the Phase 33 preferred order is currently reachable:

- `radsrvitem.dll`
- `J2DSrv.dll`
- `style.dll`
- `jengine.dll`
- `XceedRAD.dll`
- `OLESITE.dll`
- `OLECRT.dll`

Per the Phase 33 plan, this satisfies the stop condition: if only `sppid.dll`
and/or `core.dll` are reachable, write a tooling-gated closeout and stop before
parser work.

## Anchors

The Phase 33 anchor set remains:

- GUID `1D1928C0-0000-0000-C000-000000000046`
- PSM type `0x0010`
- parent alias `0x0115`
- `PSMSerializeIn`, `PSMSerializeOut`, `IOContext`, `DoIO`, `Read`, `Load`,
  `Write`, `Save`
- nearby families `JStyleOverride`, `GraphicGroup`, `RAD_OBJECT_TYPE`

These anchors were not re-searched in `sppid.dll` or `core.dll` during this
slice because both modules are already out of scope for broad search unless a
relevant persistence module xref leads there.

## Promotion Decision

`ROADMAP-0010` remains `TypedAudit`.

The existing parser surface stays unchanged:

- `SheetSubRecord0x0010Decoded` continues to preserve the PSM envelope, raw
  payload bytes, and positional `leading_word`.
- `leading_word` remains `payload[0..2]` evidence only; it is not renamed to
  `sub_kind`.
- No geometry, schema, writer, or bundle confidence is added for `0x0010`.
- No parser code was changed in this closeout.

The reason is evidence insufficiency, not a negative proof of the vendor
format. The currently reachable modules do not expose the persisted reader,
`IOContext::DoIO` sequence, stable discriminator offset, or bounded byte-field
ranges required by the promotion gate.

## Re-open Trigger

Re-open Phase 33 only when at least one of the following is available:

- a reachable IDA instance for `radsrvitem.dll`, `J2DSrv.dll`, `style.dll`,
  `jengine.dll`, `XceedRAD.dll`, `OLESITE.dll`, or `OLECRT.dll`;
- a new xref from one of those modules into `sppid.dll` / `core.dll` that ties
  the target to persisted `0x0010` reads;
- a controlled fixture that proves a discriminator and field layout with
  bounded byte ranges and cross-fixture ratchets.

Until then, the next valid action is review/commit boundary work or waiting for
the relevant IDA module, not parser implementation.
