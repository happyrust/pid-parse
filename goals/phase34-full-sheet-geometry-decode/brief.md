# Phase 34: Full Sheet Geometry Decode

> Plannotator gate approved on 2026-06-23. Slice 34-A inventory is recorded in
> `docs/analysis/2026-06-26-phase34-geometry-completeness-inventory.md`.
> Phase 34-F status synchronized on 2026-07-10.

## Goal

Turn the user goal "fully parse all geometry in PID files" into evidence-gated
parser work. The phase must improve decoded geometry coverage without promoting
probe, audit-only, relation, or structural records as drawable geometry.

## Current Evidence

Current geometry support decodes the main Sheet PSM families:

- GLine2d / igLine2d
- igLineString2d
- igPoint2d
- igTextBox
- igSymbol2d
- JStyleOverride annotation
- igBoundary2d as a fully typed association (audit-only geometry policy)

The latest local fixture probe still shows `/Sheet6` byte-audit leftover,
`ProbeOnly Unknown` entities, and unresolved page-transform semantics, so
vendor geometry is not fully decoded.

Phase 34 classification outcomes:

- `0x0013 igBoundary2d`: 20 hits / 3 fixture paths, grammar proven and typed
  decoder landed; no normalized geometry emission because 60/60 referenced
  member lines already carry the same geometry.
- `0x003D igSmartFrame2d`: 12 hits / 6 fixtures, structural page-frame
  evidence; no drawable decoder and no page-transform promotion.
- `0x0020 igRectangle2d`: 4 hits / 3 fixtures, drawable type name but
  closed negative because `/Sheet6615` and nested JSite records have
  contradictory field semantics and unproven ownership.
- Curve families `0x0059/0x0061/0x0063/0x007E/0x005D`: locally available in
  nested JSite streams and the backup `.sym` library, but still
  `IdentifiedOnly / NeedsParser`.

## Recommended Next Slice

Extract representative minimal `.sym` fixtures, then probe
`igCircle2d 0x0059` first. Do not combine the five curve families into one
speculative decoder, and do not use nested JSite presence as proof of
top-level geometry ownership.

## Non-Goals

- Do not claim the vendor PID geometry format is fully decoded.
- Do not emit geometry from `0x0010` or `0x00FA`.
- Do not restore the retired `0x0030` arc interpretation.
- Do not implement arc/circle/ellipse/bspline decoders without fixture or native
  reader evidence.
- Do not commit or push unless the user explicitly authorizes it.

## Done Means

1. Goal package is reviewed by Plannotator.
2. Geometry completeness inventory exists.
3. `0x0020` is rejected with evidence and `0x0013` is accepted only after its
   complete grammar and association semantics are proven.
4. All parser promotions have byte ranges, fixture ratchets, byte-audit
   movement, schema coverage, and panic-safety.
5. Curve-family sample availability is separated from parser readiness and
   ownership projection.
6. Atlas, roadmap, task plan and verification use the same scoped completion
   language.
