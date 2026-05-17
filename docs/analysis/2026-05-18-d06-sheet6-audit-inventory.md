# D06 `/Sheet6` audit inventory

> Date: 2026-05-18  
> Fixture: `test-file/D06.pid`  
> Sheet stream: `/Sheet6`  
> Scope: decoded geometry, audit-only PSM records, and remaining probe-only
> evidence after the D06 baseline and relationship graph fix.

## Summary

D06 `/Sheet6` is a compact Sheet stream with useful decoded coverage but no
decoded line primitives. The parser currently recovers 25 decoded geometry
entities and 41 audit-only records from this sheet.

| Bucket | Count | Status |
|---|---:|---|
| `igLineString2d` polylines | 6 | decoded geometry |
| `igPoint2d` points | 10 | decoded geometry |
| `igTextBox` texts | 4 | decoded geometry |
| `igSymbol2d` symbols | 2 | decoded geometry |
| `JStyleOverride` annotations | 3 | decoded geometry |
| `GraphicGroup` / `GraphicPersist` | 21 | audit-only |
| PSM `0x0010` sub-records | 20 | audit-only |
| Text probes without decoded position | 8 | probe-only unknown |

No `GLine2d` (`0x3FE6`) or `igLine2d` (`0x0018`) records decode from D06.
That is now captured in
`d06_pid_parses_with_expected_structure_and_geometry_summary`.

## Decoded Geometry

`pid_inspect --geometry-summary` reports:

| Kind | Count |
|---|---:|
| Decoded lines | 0 |
| Decoded polylines | 6 |
| Decoded points | 10 |
| Decoded texts | 4 |
| Decoded symbols | 2 |
| Decoded annotations | 3 |
| Inferred coordinate points | 64 |
| Probe-only unknown | 8 |
| Total normalized entities | 97 |

Sample decoded texts:

```text
"TK 264"
"  BV01"
"PT"
"  "
```

Sample decoded symbol OIDs:

```text
156
3298
```

## GraphicGroup Audit Records

`/Sheet6` has 21 `GraphicGroup` / `GraphicPersist` audit records. They remain
audit-only because the variable reference tail is not yet proven as stable
child-OID semantics across sub-type buckets.

Sample records:

| Byte range | OID | `group_kind_word` | `sub_type_word` | Tail len |
|---|---:|---:|---:|---:|
| `0x273..0x2E1` | 291 | 2 | `0x009C` | 86 |
| `0x375..0x3B1` | 320 | 1 | `0x009C` | 36 |
| `0x3B1..0x3F9` | 321 | 2 | `0x00D1` | 48 |
| `0x421..0x469` | 324 | 2 | `0x016C` | 48 |
| `0x491..0x4E3` | 333 | 5 | `0x016C` | 58 |
| `0x4E3..0x583` | 364 | 4 | `0x00EF` | 136 |

Engineering conclusion:

- Keep these in `SheetGeometry::decoded_graphic_groups`.
- Do not promote `raw_reference_payload` into `child_oids`.
- If a later phase needs group resolution, first build a cross-fixture
  reference-tail histogram by `sub_type_word` and tail length.

## PSM `0x0010` Audit Records

`/Sheet6` has 20 PSM `0x0010` audit records. These records still follow the
Phase 18/19 contract: 6-byte PSM header + raw payload + `leading_word` as
byte-position evidence only.

Sample records:

| Byte range | `bytes_to_follow` | Flags | `leading_word` | Payload prefix |
|---|---:|---:|---:|---|
| `0x187..0x19D` | 16 | 0 | `0x4C1C` | `1C 4C 00 00 01 00 0B 1B 00 00 00 2C 01 00 00 02` |
| `0x2F3..0x308` | 15 | 0 | `0x0002` | `02 00 01 00 06 00 01 00 12 00 00 00 06 00 54` |
| `0x40B..0x427` | 22 | 0 | `0x2DCA` | `CA 2D F4 79 3E 6F CE 3F CD 61 56 A4 78 77 CB 3F` |
| `0x47B..0x497` | 22 | 0 | `0x1797` | `97 17 A7 12 17 EC CE 3F CD 61 56 A4 78 77 CB 3F` |
| `0x595..0x5B7` | 28 | 0 | `0x0002` | `02 00 00 00 01 02 97 17 A7 12 17 EC CE 3F CD 61` |
| `0x61B..0x637` | 22 | 0 | `0x02AB` | `AB 02 84 2A BD F5 D2 3F CC 61 56 A4 78 77 CB 3F` |
| `0x68B..0x6A7` | 22 | 0 | `0x18DF` | `DF 18 D1 91 E4 78 D2 3F CC 61 56 A4 78 77 CB 3F` |
| `0x6F1..0x718` | 33 | 0 | `0x0002` | `02 00 01 00 06 00 01 00 22 00 00 00 06 00 20 00` |

Engineering conclusion:

- `leading_word == 0x0002` appears in D06 as in the Phase 19 global
  distribution, but it still must not be named `sub_kind`.
- Several payload prefixes contain plausible floating-point byte patterns,
  but D06 alone is not enough evidence for typed DTO fields.
- Keep `0x0010` audit-only until a future phase recovers a real type name,
  IO sequence, or discriminator.

## Probe-Only Unknowns

D06 has 8 probe-only unknown entities, all from text probes whose placement is
not decoded yet.

| ID | Byte range | Note |
|---|---|---|
| `/Sheet6:text-probe:0` | `0x1C5..0x1D1` | text position is not decoded yet |
| `/Sheet6:text-probe:1` | `0x242..0x250` | text position is not decoded yet |
| `/Sheet6:text-probe:2` | `0x2C4..0x2CC` | text position is not decoded yet |
| `/Sheet6:text-probe:3` | `0x2D2..0x2DA` | text position is not decoded yet |
| `/Sheet6:text-probe:4` | `0x307..0x315` | text position is not decoded yet |
| `/Sheet6:text-probe:5` | `0x360..0x368` | text position is not decoded yet |
| `/Sheet6:text-probe:6` | `0x416..0x420` | text position is not decoded yet |
| `/Sheet6:text-probe:7` | `0x486..0x490` | text position is not decoded yet |

Engineering conclusion:

- These unknowns are not new shape records; they are text runs without
  decoded placement.
- A future text-placement phase should use D06 as a small fixture for
  matching raw text probes with nearby `igTextBox` / `JStyleOverride`
  / coordinate evidence.

## Recommended Next Steps

1. Skip a new CLI flag for now. Existing `--geometry-summary`, `--geometry-json`,
   and `--json` are enough for this slice.
2. Treat D06 as a relationship and text-placement regression fixture.
3. Do not start `0x0010` typed DTO work from this inventory alone.
4. If continuing Phase 21, move to Slice E: update closeout docs and run the
   targeted gates listed in the plan.

## Verification

Commands used while collecting this inventory:

```powershell
target\debug\pid_inspect.exe "test-file/D06.pid" --json
target\debug\pid_inspect.exe "test-file/D06.pid" --geometry-json
cargo test --test parse_real_files d06_pid_parses_with_expected_structure_and_geometry_summary -- --nocapture
cargo test --test parse_real_files relationship -- --nocapture
```

The D06 targeted test and relationship test set pass after the relationship
graph fix.
