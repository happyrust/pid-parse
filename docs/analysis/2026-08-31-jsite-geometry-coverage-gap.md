# JSite geometry ownership and transform coverage gap (2026-08-31)

## Decision

Do not emit geometry from chain-bearing `LdcSite/PSMcluster0` streams yet.
The corpus proves that these streams own real, non-duplicate geometry, but it
does not provide the page transform required to place that geometry safely.
Circle, arc, rectangle, B-spline and nested-boundary emission therefore remain
closed behind the existing stream-admission gate.

This does not apply to `A01/JSite204/Sheet*`. Those `Sheet` streams already
enter the normalized pipeline and are proven page content by their own A2
border: 80 axis-aligned lines span exactly `0..594 x 0..420 mm`.

## Evidence

Commands:

```powershell
cargo run --quiet --example probe_who_owns_the_nested_jsite_geometry
cargo run --quiet --example probe_phase40_jsite_sheet_is_page_content
cargo run --quiet --example probe_psmspacemap_tag181_is_the_parent_ref
```

Results across D06, 0201, 0202, the process fixture, and A01:

- Ten chain-bearing nested sites resolve to top-level `0x004A LdcSite`
  objects. No `igSymbol2d.jsite_ref` names any of them, so they are not symbol
  bodies and must not receive a symbol placement transform.
- Each nested storage declares its own `JSheetLayer` table and contributes
  geometry not duplicated by the already-opened sheets (0 repeated segments
  in every fixture).
- The `LdcSite` payload is only 12 bytes and yields no finite placement matrix
  or translation. Its nested coordinate span is 15% to 75% of the opened
  sheet, sometimes containing the origin; those statistics do not establish
  page coordinates.
- Tag 181 is not a serialized parent field: 0 of 84 values match payload `+4`.
  Instead, all 84 targets are drawable `0x00CE`/`0x003D` records and all 84
  target records mention the entry as a back-reference. It is an incoming
  graph edge and cannot supply the missing page transform.
- `A01/JSite204` is a different `0x004C` OLE client-site case. Its admitted
  `Sheet6`/`Sheet12` streams are already page-positioned and draw today.

Full transcripts are stored with the acceptance bundle as
`w4-jsite-ownership.txt`, `w4-jsite-page-transform.txt`, and
`w4-tag181-parent-chain.txt`.

## Exit gate for future work

Emission may resume only after a controlled fixture or native-reader trace
identifies the transform from an `LdcSite` storage to the page and shows the
same nested primitive at the resulting reference position. The first decoder
order remains circle, arc, rectangle, B-spline, then nested boundary. Each
family needs a count ratchet and a no-duplicate screen comparison before the
next family opens.

## 2026-09-07 update: circle and arc are decoded, still held

The first half of the gate is met for the first two families. `decode_igcircles`
/ `decode_igarcs` read the layouts the `imagdex.dex` `DoIO` workers read
(`2026-08-31-imagdex-geometry-doio-ida.md`), find the same 12 + 12 records the
layer-edge roster counted, every one on a layer its own storage declares, and
surface them on `JSite::nested_geometry`. `build_normalized_geometry` names the
held-back counts per storage in its warnings. The decision above stands: nothing
is emitted.

The decoded values point at where the transform is. Ten of the twelve circles
are centred on the origin with radii 1.27 / 1.59 / 6.35 / 7.57 mm -- 6.35 mm is
the half-inch instrument balloon -- and the arcs come in mirrored pairs about the
origin or about a shared `y`. That is the shape of a symbol glyph drawn around
its own insertion point, not of page content, and `symbol_library.rs` reads
`.sym` bodies with exactly these record layouts.

Tested the same day (`probe_nested_site_curves_are_symbol_bodies`,
`2026-09-07-nested-site-curves-are-embedded-symbol-bodies.md`): **20 of the 24
curves are, to a nanometre, a circle or arc of a `.sym` body the same drawing
places**, and every matched symbol has its whole curve set reproduced. The
nested `LdcSite` is the drawing's embedded symbol-definition cache in
symbol-local coordinates. The 12-byte `LdcSite` payload never held a page
transform because none exists at that level: the transform is each
`igSymbol2d` placement's own insertion and matrix, and the open question is now
the definition-to-instance link, not a transform. The four unmatched curves are
a probable parametric resize (`/JSite396`) and two circles of a symbol absent
from the local library (`/JSite7559`); none contradicts the reading.
