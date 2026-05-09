# Page Transform Evidence Check

> Date: 2026-05-09
> Scope: Phase 14 CoordinatePageMetadata / page-transform preflight.

## Conclusion

Page transform decoding is blocked by evidence, not implementation mechanics.

Current fixture evidence can prove:

- drawing template names exist in `/TaggedTxtData/Drawing`;
- template names can infer paper class such as A2 (`594 x 420 mm`);
- Sheet streams contain normalized f64 coordinate-domain evidence;
- non-Sheet streams contain at most one page-dimension-like scalar in current fixtures.

Current fixture evidence does not prove:

- a source-backed record containing both page width and height;
- drawing units;
- origin;
- source-to-page scale;
- affine transform matrix;
- a stable `SheetRecordKind::CoordinatePageMetadata` byte layout.

Therefore normalized geometry must keep `PidPageTransform::Unavailable` and must
not promote page transform metadata from template names or isolated scalar hits.

## Current Observations

The cross-fixture Sheet investigation currently reports:

- `fixtures_seen=5`
- `sheets_seen=7`
- `coordinate_metadata_candidates=97`
- `normalized_f64_pair_count=1397`
- `page_dimension_scalar_matches=0`

The non-Sheet stream scan currently reports:

- `fixtures_seen=3`
- `scanned_streams=166`
- `template_stream_hits=3`
- `page_dimension_scalar_hits=1`
- `complete_page_dimension_streams=0`
- `unavailable_context_entities=427/427`

The only non-Sheet page-dimension-like scalar is:

- fixture: `DWG-0201GP06-01.pid`
- stream: `/JSite329/PSMcluster0`
- offset: `35864`
- encoding: `i32`
- value: `420.0`
- context: `35848..35880: 73 00 00 00 00 00 00 00 00 00 81 00 2E 00 00 00 A4 01 00 00 00 00 00 00 00 00 00 00 0E 00 00 00`

This is a lone scalar inside a local `PSMcluster0` integer field sequence. It is
not paired with `594.0`, and it carries no origin, scale, units, or matrix.

## Promotion Gate

Do not emit `PidPageTransform::Available` until at least one independent source
proves all of the following:

- bounded source stream and byte range;
- stable record identity or marker/range shape across independent logical drawings;
- complete page bounds or width/height pair;
- units;
- origin and scale, or an explicit affine matrix;
- a regression demonstrating that raw source coordinates and page coordinates are
  not being conflated.

Until then, template-derived page dimensions may remain an informational
`NormalizedPidGeometry.page_dimensions_mm` hint only.
