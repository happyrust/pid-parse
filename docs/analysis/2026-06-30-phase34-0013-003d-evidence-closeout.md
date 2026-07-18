# Phase 34-C Evidence Closeout: 0x0013 igBoundary2d and 0x003D igSmartFrame2d

> Date: 2026-06-30
> Scope: Phase 34-C read-only evidence closeout for the two remaining
> cross-fixture undecoded Sheet candidates. No parser, schema, writer,
> byte-audit, bundle confidence, or normalized geometry output changed.
> Evidence source: `examples/probe_psm_undecoded_shapes.rs` (relaxed neighbor
> correlation pass added in Phase 34-B).

Command:

```powershell
cargo run --quiet --example probe_psm_undecoded_shapes
```

## Summary Decision

| Type | Hits / fixtures | Ownership | Neighbor correlation | Decision |
|---|---|---|---|---|
| `0x0013 igBoundary2d` | 20 / 3, all `btf=172` | **primary top-level `/Sheet6`** | vertices **exactly** reproduce adjacent canonical `igLine2d` endpoints | **Promising** — defer to a dedicated decoder slice; grammar not yet fully pinned. No emit this turn. |
| `0x003D igSmartFrame2d` | 12 / **all 6**, `btf` 238/246/228 | primary `/Sheet6` | **none**; payload holds page-frame scalars | **Negative for drawable geometry** — structural sheet-frame record; `NeedsReader`. |

Both stay undecoded. `0x0013` is the strongest drawable candidate found in
Phase 34 and is recommended for a future implementation slice; `0x003D` is a
structural frame and is not a geometry target.

## 0x0013 igBoundary2d — Strong Ownership-Clean Candidate

### Facts

- 20 records, all `bytes_to_follow = 172`, in `DWG-0202` (5),
  `publish-DWG-0202` (5), and `工艺管道及仪表流程-1` (10).
- Every hit is in **primary top-level `/Sheet6`** — no mini-sheet, no nested
  JSite. Ownership is clean, unlike `0x0020`.
- Neighbours are **canonical** `igLine2d` records: the strict
  `decode_igline_at` accepts them (`remaining_header == 12`), and the strict
  and relaxed correlations agree exactly.
- The payload carries an 8-byte-stride f64 run starting at `+29` whose values
  **exactly reproduce** (delta = `0.0`, identical f64 bits) the surrounding
  `igLine2d` endpoints. Example, `DWG-0202 /Sheet6` sample at
  `0x0001AE..0x000260`, prev `igLine2d oid=70`
  `start=(0.202486482765, 0.220177232973)`,
  `end=(0.199327955143, 0.220957117689)`:

  | Offset | Value | Matches |
  |---|---|---|
  | `+29` | `0.202486482765` | prev `start.x` (exact) |
  | `+37` | `0.220177232973` | prev `start.y` (exact) |
  | `+45` | `0.199327955143` | prev `end.x` (exact) |
  | `+53` | `0.220957117689` | prev `end.y` (exact) |
  | `+86` | `0.219376969300` | next `start.y` (exact) |
  | `+95` | `0.199327955143` | next `start.x` (exact) |
  | `+111` | `0.202486482765` | next `end.x` (exact) |

  All four samples reproduce this exact-match behaviour against both the
  previous and next line.
- Header shape (payload offsets): `+0` u32 `oid`, `+4` u32 `parent_ref`,
  `+8` u32 `remaining_header = 12`, `+12` u16 `sub_type = 0x0010`, then a
  short sub-header. A recurring `0x67` tag byte precedes vertex groups
  (`+28 = 0x67`, `+61 = 0x67`, ...), so the vertex run is **not** a plain
  packed `f64[2*n]` array — tag bytes are interleaved.

### Why no decoder this turn

The correlation proves these payloads contain real boundary vertices tied to
decoded geometry, but a `Decoded` promotion under
`docs/plans/2026-06-19-pid-parser-roadmap-gates.md` still needs:

- a proven **vertex-count** field and exact **vertex-array byte range**
  (the run starts at the unaligned `+29` and is interrupted by `0x67` tag
  bytes whose meaning and stride are not yet pinned);
- proof of the **boundary semantics** — `igBoundary2d` may be a closed
  drawable boundary, a hatch/fill boundary, or an association that simply
  re-lists member-segment endpoints; the exact-match to neighbour endpoints is
  consistent with all three;
- panic-safety entry, schema coverage, byte-audit movement, and cross-fixture
  count/distribution ratchets.

These belong in a focused implementation slice, not an evidence closeout.

### Recommendation

Open a dedicated `0x0013 igBoundary2d` decoder slice (candidate Phase 34-D)
that first pins the `0x67` tag grammar and vertex-count field on the 20
records, then decides whether to emit a closed `PidGraphicKind::Polyline`
(boundary) or to treat it as an audit-only association. Do not emit geometry
until the vertex-array byte range is exact and ratcheted.

## 0x003D igSmartFrame2d — Structural Sheet-Frame, Not Drawable

### Facts

- 12 records across **all 6 fixtures** (`D06` 1, `DWG-0201` 6, `DWG-0202` 1,
  `A01` 2, `publish-DWG-0202` 1, `工艺管道-1` 1), `btf` mostly 238/246. Roughly
  one to a few per drawing — a per-sheet/per-frame singleton shape.
- In `DWG-0201 /Sheet6` the payload is byte-identical across its samples:
  `B3 03 00 00 06 00 00 00 0E 00 00 00 ...` → `oid = 947`, `parent_ref = 6`,
  `remaining_header = 14` (note: `0x0E`, not the `12` of true `igLine2d` /
  `igBoundary2d`).
- The finite f64 candidates are page-frame scalars, **not** geometry:
  `+76 = 0.594305...` ≈ A2 page width in metres, `+84 = 0.420313...` ≈ A2
  page height in metres, `+148 ≈ 0.7072` ≈ `1/√2` (ISO A-series aspect ratio),
  plus `+19/+32 ≈ 2.0`, `+60 = 1.0` (scale/flag-like).
- **No neighbour correlation.** The nearest strict/relaxed `igLine2d` is far
  away (e.g. `oid=3377` hundreds of bytes off); every candidate-to-line delta
  is large (`0.01`–`1.4`). The "matches" the probe prints are spurious
  nearest-value picks, not real field equalities.

### Decision

`0x003D igSmartFrame2d` is a structural **sheet-frame / drawing-border**
record carrying page extent and scale metadata. It is **not** a drawable line
primitive and must not emit `PidGraphicKind` geometry.

Critical guardrail cross-link: the page-dimension-like values (`0.594`,
`0.420`) are exactly the kind of evidence that `ROADMAP-PAGE-TRANSFORM` in
`docs/plans/2026-06-19-pid-parser-roadmap-gates.md` forbids promoting to an
available page transform. Finding A2-shaped scalars in `0x003D` is
**page-size evidence only**; it does not prove coordinate space, units,
origin, direction, or a complete transform. Do not set
`PidPageTransform::Available` from this.

### Recommendation

Keep `0x003D` as `StructuralCandidate / NeedsReader`. A future native-reader
or controlled-fixture pass could name its frame/scale fields and, separately
and only if every required component is proven, feed the long-blocked
page-transform investigation — but never as drawable geometry.

## Phase 34-C Closeout

- `0x0013 igBoundary2d`: strong, ownership-clean, vertex-correlated candidate;
  promote only via a dedicated decoder slice (recommended Phase 34-D). No
  geometry emission this turn.
- `0x003D igSmartFrame2d`: structural sheet-frame; not drawable; page-dim
  scalars do not satisfy the page-transform gate.
- No parser / schema / writer / byte-audit / bundle confidence change.

With `0x0020` (Phase 34-B), `0x0013`, and `0x003D` now characterised, the
remaining vendor geometry families (`igCircle2d 0x0059`, `igArc2d 0x0061`,
`igEllipse2d 0x0063`, `igEllipticalArc2d 0x007E`, `igBSplineCurve2d 0x005D`)
have zero hits in the six local fixtures and remain blocked on new fixtures or
native-reader evidence.
