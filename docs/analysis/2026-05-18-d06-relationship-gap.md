# D06 relationship graph gap analysis

> Date: 2026-05-18  
> Fixture: `test-file/D06.pid`  
> Scope: explain why D06 initially produced relationship probes but no
> `ObjectGraph.relationships`, and record the conservative fix.

## Baseline

Before the fix, D06 parsed successfully but reported:

| Signal | Count |
|---|---:|
| `dynamic_attributes.relationship_probes` | 10 |
| `object_graph.objects` | 10 |
| `object_graph.relationships` | 0 |

The geometry side was not failing: `pid_inspect --geometry-summary`
reported 97 total entities and 25 decoded entities.

## Evidence

D06 has no relationship trailers with `class_id == 0xF6`.

Observed DA trailer class histogram:

| `class_id` | Count |
|---:|---:|
| 225 | 3 |
| 237 | 10 |
| 256 | 2 |
| 257 | 1 |
| 259 | 1 |
| 260 | 7 |
| 262 | 1 |

However, D06 does have 10 `P&IDAttributes` records with:

```text
ModelItemType = Relationship
ModelID       = Relationship.<32-hex GUID>
DrawingID     = ""
```

Those 10 `ModelID` GUIDs match the 10 `Relationship.<GUID>` byte probes.
The records do not expose `record_id`, `field_x`, source endpoint, or target
endpoint through the existing DA trailer path.

## Root Cause

`build_object_graph` only created `PidRelationship` entries from DA trailers
where `class_id == 0xF6`. That is correct for existing DWG fixtures, but D06
stores relationship identity in parsed `P&IDAttributes` fields instead of
the `0xF6` trailer shape.

As a result, the parser saw the GUIDs in probe/attribute evidence but dropped
them from `ObjectGraph.relationships`.

## Fix

The fix is deliberately narrow:

- Keep trailer-based relationships as the primary source.
- Only if the trailer pass produced zero relationships, scan
  `P&IDAttributes` records for `ModelItemType=Relationship` and
  `ModelID=Relationship.<GUID>`.
- Require the GUID to also appear in `relationship_probes`.
- Emit these as unresolved `PidRelationship` entries:
  - `model_id = Relationship.<GUID>`
  - `guid = <GUID>`
  - `record_id = None`
  - `field_x = None`
  - `source_drawing_id = None`
  - `target_drawing_id = None`

This prevents D06 from losing known relationship identity while avoiding
double-counting in fixtures that already have trailer-backed relationships.

## Verification

Commands run:

```powershell
cargo test --test parse_real_files relationship -- --nocapture
cargo test --test parse_real_files d06_pid_parses_with_expected_structure_and_geometry_summary -- --nocapture
```

Results:

- `relationship`: 9 passed.
- D06 targeted baseline: 1 passed.

Updated D06 graph summary:

| Signal | Count |
|---|---:|
| `object_graph.objects` | 10 |
| `object_graph.relationships` | 10 |
| `counts_by_type["Relationship"]` | 10 |

All D06 relationships remain unresolved until a Sheet-level `field_x` link is
available.

## Boundaries

This does not implement endpoint resolution for D06. It also does not change
`RelationshipProbe` semantics: probes remain byte-level evidence adjacent to
`Relationship.<GUID>` tags, not full relationship decoders.
