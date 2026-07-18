# Blockers: Phase 34 Full Sheet Geometry Decode

> Plannotator gate approved on 2026-06-23. Phase 34-A..F scoped closeout is
> complete as of 2026-07-10. `0x0020` was rejected, `0x0013` landed as a typed
> association, and curve-family decoders remain follow-up work.

## Open Questions

### Q1 - Plannotator gate availability [RESOLVED]

The goal package was approved on 2026-06-23. Slice 34-A proceeded as a
read-only inventory, not as parser implementation.

### Q2 - `0x0020` layout proof [CLOSED — NEGATIVE]

`0x0020 igRectangle2d` has 4 normalized fixture records (`btf=78`), all in
`/Sheet6615` mini-sheet or nested `/JSite204\Sheet6`, never primary `/Sheet6`.
The relaxed neighbor correlation
(`docs/analysis/2026-06-30-phase34-0020-relaxed-neighbor-correlation.md`)
shows the candidate f64 fields carry inconsistent semantics across the two
ownership families: in `/Sheet6615` `+18/+26/+34` exactly match the adjacent
line's corner/extent, but in nested `/JSite204\Sheet6` they look page-frame
related with no neighbor correlation. `+42` is always `0.0` (no height /
second extent), and every neighbor is a non-canonical `0x0018`
(`remaining_header ∈ {8, 6996}`). No stable rectangle layout is provable, so
`0x0020` is closed as an ownership-gated negative with no decoder. Reopen only
with a controlled fixture or native reader proving both ownership projection
and a two-extent field set.

### Q3 - Geometry representation [DEFERRED — NO `0x0020` DECODER]

Phase 34-B rejected the current `0x0020` samples, so no public representation
decision is needed in this phase. If controlled-fixture or native-reader
evidence later reopens the family, decide whether to emit as:

- closed polyline using existing `PidGraphicKind::Polyline`; or
- a new rectangle-specific public kind.

Default: closed polyline unless a downstream contract requires a new enum.

### Q4 - `0x0013` / `0x003D` semantics [CLOSED — EVIDENCE + 34-D DECODER]

Closeout: `docs/analysis/2026-06-30-phase34-0013-003d-evidence-closeout.md`.

- `0x0013 igBoundary2d`: **decoder landed in Phase 34-D** (2026-07-07,
  `docs/analysis/2026-07-07-phase34d-0013-igboundary2d-grammar-decode.md`).
  The dedicated probe (`examples/probe_0013_igboundary2d_grammar.rs`) pinned
  the grammar on all 20 records: the `0x67` byte is a fixed per-segment tag
  (`segment_count` groups of `0x67 + 4×f64`, 33 B each from `+28`), the count
  field is `u32` at `+22` (= trailer `member_count`), and
  `btf == 49 + 41 × segment_count` exactly. The trailer member references
  resolve to same-stream canonical `igLine2d` records whose geometry equals
  the same-index segment forward (60/60) — proving the **association**
  semantics. `decode_igboundaries` ships as a fully-typed **audit-only**
  family (`SheetGeometry::decoded_igboundaries`, byte-audit `Decoded`);
  no normalized geometry is emitted because it would double-count the
  member lines already emitted as `Decoded Line` entities.
- `0x003D igSmartFrame2d`: 12 hits across all 6 fixtures, ~1 per drawing,
  `remaining_header=14`, carrying page-frame scalars (`+76≈0.594` A2 width,
  `+84≈0.420` A2 height, `+148≈0.707`=1/√2) with no neighbor correlation. It
  is a structural sheet-frame record, **not** drawable geometry; its
  page-dim scalars do **not** satisfy `ROADMAP-PAGE-TRANSFORM`. Keep as
  `StructuralCandidate / NeedsReader`.

### Q5 - Missing geometry families [REVISED — LOCAL EVIDENCE FOUND]

Phase 34-E (2026-07-07,
`docs/analysis/2026-07-07-phase34e-missing-geometry-fixture-plan.md`)
overturned the "NeedsFixture" framing: the curve families were only
missing from **top-level `Sheet*` streams**. The all-stream corpus scan
(`examples/probe_curve_family_corpus_scan.rs`) finds them locally:

- registered fixtures' nested `/JSite*\PSMcluster0` streams:
  `igCircle2d ×79`, `igArc2d ×29`, `igEllipticalArc2d ×4`,
  `igBSplineCurve2d ×2`;
- `test-file/backup-test/*/RefData~4~683` `/StyleCluster`:
  `igCircle2d ×34 + igArc2d ×6`;
- backup `RefData~4~681.zip` symbol library (1826 `.sym` CFB files, 270
  with curve hits): `0x0059 ×616 / 0x0061 ×279 / 0x0063 ×44 /
  0x007E ×50 / 0x005D ×55` — including tiny ideal fixtures
  (`2-Way Angle Globe Valve.sym` `/Sheet6` = 683 bytes, igCircle2d×6).

Only `igEllipse2d 0x0063` has zero hits in registered files; it needs
`.sym` extraction (e.g. `Design/Annotation/Graphics/Ellipse.sym`).
Next: E-1 extract representative `.sym` fixtures → E-2 per-family byte
probes → per-family decoder slices. Nested-JSite ownership projection
remains unproven (layout evidence only, no geometry emission).

## Stop Conditions

- Plannotator rejects or defers the goal package.
- `0x0020` field offsets are ambiguous.
- Any implementation tries to emit geometry from `0x0010`, `0x00FA`,
  `0x0013`, or `0x003D` without evidence.
- Any implementation changes writer behavior.
- User has not explicitly authorized commit or index cleanup.

## Current Status

Slice 34-A inventory is complete. Slice 34-B `0x0020 igRectangle2d` is closed
as an ownership-gated negative (relaxed neighbor correlation, 2026-06-30): no
parser slice, no schema / writer / byte-audit / confidence change; it reopens
only with controlled-fixture or native-reader proof of ownership projection
and a two-extent rectangle layout. Slice 34-C evidence closeout is complete.
Slice 34-D (`0x0013 igBoundary2d` dedicated decoder, 2026-07-07) is
complete: grammar pinned on 20/20 records, fully-typed audit-only decoder
landed (`decode_igboundaries` → `SheetGeometry::decoded_igboundaries`),
byte-audit claims the records as `Decoded`, all five pre-commit gates green.
No writer change; no normalized geometry emission (association semantics).
Slice 34-E (fixture expansion plan, 2026-07-07) is complete with a major
revision: four of the five "missing" curve families have local evidence in
nested `/JSite*\PSMcluster0` streams and the backup `.sym` symbol library
(see Q5); only `igEllipse2d` has no hit in the registered PID set, while the
backup `.sym` corpus contains it. Phase 34-F contract/status synchronization
completed on 2026-07-10: atlas, roadmap, task plan and goal verification now
use the same scoped completion language. Phase 34-A..F is complete for the
current six-PID evidence scope; this is not a claim of full vendor geometry or
writer support. Next work is a follow-up sequence: E-1 extract representative
`.sym` fixtures, E-2 probe `igCircle2d` first, then open one evidence-gated
decoder slice per curve family.
