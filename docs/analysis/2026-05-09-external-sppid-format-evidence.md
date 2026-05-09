# External SPPID Format Evidence Check

> Date: 2026-05-09
> Scope: Phase 14 external evidence search for Sheet geometry decoders.

## Conclusion

Publicly reachable SPPID material does not currently provide enough record-level
evidence to implement a decoded Sheet geometry primitive parser.

The useful public evidence is directional:

- `.pid` files contain drawing/template information and operation history;
- editing arbitrary bytes can make a drawing fail to open;
- Drawing Manager can recreate drawings from `.pid` state;
- SPPID Bridge export packages contain SmartPlant Graphics and Data;
- Design Validation utilities can import P&ID data through mapper files and then
  compare object attributes/topology against PDMS or ISO data.

This evidence supports the current investigation strategy, but it does not prove
the byte layout of `PrimitiveLine`, `PrimitivePolyline`, `PrimitiveCircle`,
`PrimitiveArc`, `TextPlacementStyle`, `SymbolPlacement`, or
`CoordinatePageMetadata`.

## Sources Checked

- Intergraph/SmartPlant community blog post on the relationship between `.pid`
  files and SPPID drawings.
- Bentley PlantSight SPPID Bridge documentation for exporting drawings from
  SmartPlant P&ID Drawing Manager.
- SmartPlant P&ID Design Validation Utility documentation, especially command
  mode and mapper file sections.

## What Would Help Next

The next useful external evidence should be generated, not searched:

- Controlled operation diff pairs:
  - open a copy of a fixture;
  - place exactly one line, polyline, circle, arc, text, or symbol;
  - save;
  - compare the before/after `.pid` at CFB stream and byte range level.
- Matching SPPID Bridge export packages:
  - keep the `.pid`;
  - keep the exported SmartPlant Graphics and Data ZIP;
  - correlate exported graphics/entity identifiers with Sheet stream byte ranges.
- Mapper output from Design Validation:
  - run SPPID mapper export for the same drawing;
  - use object attribute/topology output as semantic labels for existing DA and
    Sheet evidence.
- Additional independent fixtures:
  - prefer drawings with known visible primitives and minimal clutter;
  - include at least one fixture per primitive class.

## Promotion Gate Impact

None of the public sources checked can relax the current promotion gates.

Decoded geometry still requires source-backed byte ranges, stable record identity,
coordinate semantics, and fixture or controlled-diff replication before emitting
`PidGeometryConfidence::Decoded`.
