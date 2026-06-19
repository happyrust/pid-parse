# PID Export Bundle Contract

> Status: Draft (Phase 32-B)  
> Scope: file-based export format for one SmartPlant / Smart P&ID `.pid` parse result  
> Related: `docs/analysis/2026-06-16-pid-format-atlas-cn.md`

## 1. Purpose

The export bundle turns one `.pid` file into a directory of files that can be inspected, archived, diffed, and consumed by downstream CAD/data tooling without losing raw provenance.

The bundle is not a new semantic truth layer. It is a packaging contract over existing parser outputs:

- decoded facts remain decoded;
- typed audit records remain audit;
- probe hints remain probe;
- unknown bytes remain raw/inventory only.

## 2. Directory Shape

```text
<drawing>.pid.bundle/
  manifest.json
  raw/
    streams.json
    streams/
      <escaped-stream-path>.bin
  decoded/
    document.json
    metadata.json
    object_graph.json
    cross_reference.json
    import_view.json
    layout.json
  geometry/
    normalized_geometry.json
    decoded_entities.json
    audit_entities.json
    probe_entities.json
  audit/
    coverage.json
    byte_audit.json
    unknown_streams.json
    confidence_ledger.json
  writer/
    round_trip_plan.json
    diff_summary.json
  publish/
    data.xml
    meta.xml
    publish_diff.json
```

Only `manifest.json`, `raw/streams.json`, `decoded/document.json`, and `audit/confidence_ledger.json` are mandatory for a minimal bundle. Other files are emitted when the corresponding parser output exists or the export plan enables that subtree.

Phase 32-C6 implementation note: the current writer emits decoded split
views and geometry files for the default plan, including
`decoded/import_view.json`. `PidImportView` and its slim child DTOs now derive
serde specifically to make that UI projection an explicit export surface.

## 3. Defaults

| Output | Default | Rationale |
|---|---:|---|
| `manifest.json` | on | identity and provenance |
| `raw/streams.json` | on | stream inventory without dumping bytes |
| `raw/streams/*.bin` | off | raw fixture bytes may be large/proprietary |
| `decoded/document.json` | on | canonical structured parser output |
| `decoded/*` split views | on when available | downstream convenience |
| `geometry/*` | on when available | decoded/audit/probe separation |
| `audit/*` | on | confidence and unknown tracking |
| `writer/*` | on for package parses | safe editing boundaries |
| `publish/*` | off | requires MDF/SQLite input, not `.pid` alone |

Recommended CLI flags:

```text
pid_inspect drawing.pid --export-bundle out.bundle
pid_inspect drawing.pid --export-bundle out.bundle --export-bundle-raw-streams
pid_inspect drawing.pid --export-bundle out.bundle --export-bundle-publish Export.mdf
```

## 4. `manifest.json`

Draft schema:

```json
{
  "bundle_schema_version": 1,
  "tool": {
    "name": "pid-parse",
    "version": "<crate-version>",
    "command": "pid_inspect drawing.pid --export-bundle out.bundle"
  },
  "source": {
    "path": "drawing.pid",
    "sha256": "<hex>",
    "size_bytes": 12345,
    "modified_at": "<optional-rfc3339>"
  },
  "features": {
    "raw_stream_bytes": false,
    "publish_xml": false,
    "writer_round_trip": true
  },
  "counts": {
    "streams": 0,
    "decoded_geometry_entities": 0,
    "audit_geometry_entities": 0,
    "probe_geometry_entities": 0,
    "unknown_streams": 0
  },
  "inputs": {
    "pid": {
      "path": "drawing.pid",
      "sha256": "<hex>"
    },
    "publish_mdf": null
  }
}
```

If `publish/` is emitted, `inputs.publish_mdf` must include path/hash/size. Never imply publish XML was derived solely from `.pid` raw bytes.

## 5. Raw Stream Path Escaping

CFBF stream paths can contain slashes, control characters, and non-ASCII names such as `\001Ole`.

`raw/streams.json` is the source of truth:

```json
{
  "streams": [
    {
      "path": "/JSite0/\u0001Ole",
      "escaped_filename": "2f4a53697465302f00014f6c65.bin",
      "size_bytes": 100,
      "sha256": "<hex>",
      "magic": "<optional>",
      "confidence": "IdentifiedOnly"
    }
  ]
}
```

Recommended escaping for `streams/*.bin` is hex of UTF-8 path bytes plus `.bin`. This is reversible, Windows-safe, and avoids lossy filename normalization.

## 6. Confidence Ledger

`audit/confidence_ledger.json` records how facts moved into bundle files:

```json
{
  "entries": [
    {
      "source_path": "/Sheet6",
      "bundle_path": "geometry/decoded_entities.json",
      "family": "igLine2d",
      "confidence": "Decoded",
      "evidence": [
        "typed decoder",
        "cross-fixture ratchet",
        "byte_range provenance"
      ],
      "blockers": []
    },
    {
      "source_path": "/Sheet6",
      "bundle_path": "geometry/audit_entities.json",
      "family": "GraphicGroup 0x00FA",
      "confidence": "TypedAudit",
      "evidence": ["header + raw tail audit decoder"],
      "blockers": ["payload child/reference semantics not proven"]
    }
  ]
}
```

This file is the guardrail preventing decoded, audit, and probe outputs from being flattened into ambiguous JSON.

Mission atlas note (2026-06-19): the implemented ledger shape now carries
`source_path`, `bundle_path`, `family`, `confidence`, `evidence`, `blockers`,
and `summary`. Aggregates such as `decoded/document.json` do not use ad hoc
classes like `Mixed` or `Raw`; they are ledgered as `IdentifiedOnly` at the
file level and must be interpreted through the authoritative atlas rows in
`docs/analysis/2026-06-19-authoritative-pid-format-atlas.md`.

## 7. Geometry Files

| File | Contents |
|---|---|
| `geometry/normalized_geometry.json` | existing normalized geometry contract, preserving coordinate context |
| `geometry/decoded_entities.json` | `PidGraphicEntity` values whose source family is decoded |
| `geometry/audit_entities.json` | typed audit records such as GraphicGroup / `0x0010` raw audit |
| `geometry/probe_entities.json` | investigation-only windows, scores, text/coordinate hints |

`PidPageTransform::Unavailable` must remain explicit. Bundle consumers must not infer page-space coordinates from template-derived page size alone.

Phase 32-C4 maps `PidGeometryConfidence::Decoded` to
`decoded_entities.json`, `PidGeometryConfidence::Inferred` to
`audit_entities.json`, and `PidGeometryConfidence::ProbeOnly` to
`probe_entities.json`. This is a packaging split only; it does not promote
inferred or probe evidence to decoded geometry.

## 8. Writer Files

`writer/round_trip_plan.json` should describe safe edit surfaces:

```json
{
  "editable": [
    {
      "surface": "TaggedTxtData/Drawing XML tag",
      "method": "set_xml_tag",
      "confidence": "Decoded"
    },
    {
      "surface": "arbitrary stream replacement",
      "method": "replace_stream",
      "confidence": "raw passthrough"
    }
  ],
  "read_only": [
    {
      "surface": "Sheet geometry decoded/probe JSON",
      "reason": "no source writer contract"
    }
  ]
}
```

Forbidden in Phase 32:

- writing Sheet bytes from `geometry/*.json`;
- compacting unknown streams;
- regenerating CFBF tree without passthrough verification.

Phase 32-C5 implementation note: default exports now write
`writer/round_trip_plan.json` and `writer/diff_summary.json`.
`round_trip_plan.json` includes a passthrough `WritePlan::default()`,
the supported edit surfaces, read-only bundle views, and the forbidden
operations above. `diff_summary.json` is explicitly `status = "not_run"`
because bundle export does not perform a writer round-trip or package diff.

## 9. Publish Subtree

`publish/` is optional and MDF-backed:

```text
pid_inspect drawing.pid \
  --export-bundle out.bundle \
  --export-bundle-publish Export.mdf \
  --publish-drawing <UID> \
  --plant TEST02
```

`publish/` files must record their MDF input in `manifest.json`. If DWG reference fixture is absent, bundle should write an explicit status such as:

```json
{
  "publish_dwg_status": "not_verified",
  "reason": "DWG Export.mdf fixture absent; tests soft-skipped"
}
```

Phase 32-C7 planning note: publish bundle support should be a thin wrapper
around the existing `pid_publish_xml` pipeline, not a second publish
implementation inside the `.pid` parser. The bundle writer may call the same
library functions (`open_mdf_as_sqlite`, `load_drawing_graph`,
`write_data_xml`, `write_meta_xml`) once the caller explicitly supplies an
MDF/legacy SQLite input.

Planned `pid_inspect` flags:

```text
pid_inspect drawing.pid \
  --export-bundle out.bundle \
  --export-bundle-publish Export.mdf \
  --publish-drawing <UID> \
  --publish-plant TEST02 \
  --publish-style a01|dwg
```

Planned bundle files:

```text
publish/
  data.xml
  meta.xml
  status.json
```

Planned `manifest.json` extension:

```json
{
  "inputs": {
    "pid": {
      "path": "drawing.pid",
      "sha256": "<hex>",
      "size_bytes": 12345
    },
    "publish_mdf": {
      "path": "Export.mdf",
      "sha256": "<hex>",
      "size_bytes": 67890,
      "kind": "mdf"
    }
  }
}
```

Phase 32-C8 implementation note: `manifest.json` now includes an `inputs`
object. When the package has a source path, `inputs.pid` records path,
SHA-256, size, and kind=`pid`. When an export plan carries a publish input,
`inputs.publish_mdf` records the same identity fields with kind=`mdf` or
kind=`sqlite` for legacy `.sqlite` / `.db` inputs. This is identity plumbing
only; C8 does not emit publish XML.

`publish/status.json` records at least:

- requested drawing UID;
- plant;
- style (`a01` / `dwg`);
- whether `data.xml` and `meta.xml` were written;
- whether reference comparison was run;
- if skipped, the reason (for example missing DWG MDF fixture).

Phase 32-C9 implementation note: when an `ExportBundlePlan` carries a publish
input, bundle export now writes `publish/status.json`. The current status is a
deferred XML state: `data_xml_written=false`, `meta_xml_written=false`,
`reference_comparison.state="not_run"`, and `status.state="skipped"` with a
reason stating that publish XML generation is not implemented in bundle export
yet. `publish/data.xml` and `publish/meta.xml` are still not written by C9.

Phase 32-C10 implementation note: `export_bundle_publish_xml(...)` is the
explicit library-level helper for producing publish XML inside a bundle
`publish/` directory. It requires a publish input plus drawing UID, reuses the
existing MDF/legacy SQLite publish pipeline (`open_mdf_as_sqlite` or
`sqlite_load::open_readonly` → `load_drawing_graph` → `write_data_xml` /
`write_meta_xml`), writes `publish/data.xml`, `publish/meta.xml`, and replaces
`publish/status.json` with `status.state="written"`. The default
`export_bundle(...)` function still does not generate XML; CLI flags remain a
separate follow-up slice.

Phase 32-C11 implementation note: `pid_inspect --export-bundle` now accepts
publish flags:

```text
--export-bundle-publish <Export.mdf|Export_v2.sqlite>
--publish-drawing <UID>
--publish-plant <NAME>
--publish-style a01|dwg
--publish-diff-against <Data.xml>
```

`--export-bundle-publish` requires `--publish-drawing` and all publish flags
require `--export-bundle <dir>`. When provided, the CLI first writes the normal
bundle, then calls `export_bundle_publish_xml(...)` to write
`publish/data.xml`, `publish/meta.xml`, and success `publish/status.json`.
When `--publish-diff-against` is supplied, the helper also writes
`publish/publish_diff.json` with PID tag and Rel DefUID summary counts, then
sets `reference_comparison.state` to `clean` or `findings`.

Implementation boundary:

- no publish files are emitted unless `--export-bundle-publish` is present;
- publish input identity must be recorded separately from `.pid` source
  identity;
- publish XML remains MDF-backed and must not be described as decoded from
  `.pid` raw bytes;
- tests may soft-skip when real MDF fixtures are absent, but synthetic or
  legacy SQLite fixture tests should still cover flag parsing, manifest shape,
  and status-file behavior;
- the existing `pid_publish_xml` CLI remains the fidelity reference until the
  bundle path proves parity.

## 10. Implementation Slices

| Slice | Code target | Test target |
|---|---|---|
| C1 | `ExportBundlePlan` DTO | unit default/option tests |
| C2 | pure `export_bundle` writer | synthetic package shape |
| C3 | `pid_inspect --export-bundle` CLI | CLI smoke output |
| C4 | raw stream opt-in | raw bytes absent/present tests |
| C5 | geometry split files | decoded/audit/probe separation test |
| C6 | publish opt-in | MDF soft-skip / manifest input hash |
| C7 | publish opt-in planning | contract-only boundary review |
| C8 | manifest input identities | PID/MDF hash and kind tests |
| C9 | publish status DTO/writer | deferred status.json shape test |
| C10 | publish XML library helper | legacy SQLite / MDF-backed output test |
| C11 | `pid_inspect` publish flags | CLI success + validation tests |
| C12 | publish reference diff artifact | MDF/reference smoke + JSON summary |

## 11. Verification

Minimum implementation gate:

```powershell
cargo test --locked --lib export_bundle -- --nocapture
cargo test --locked --test parse_real_files export_bundle -- --nocapture
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
```

Release-quality gate remains the repository five-gate set:

```powershell
cargo build --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
bash .github/scripts/check-missing-docs.sh
```

On Windows where the bash docs gate is known to fail for environment reasons, use:

```powershell
cargo rustdoc --lib --locked -- -W missing-docs
```

and record the bash failure separately, as in previous project history.

## 12. Open Questions

1. Should raw stream bytes be opt-in per stream family, or one global flag?
2. Should `decoded/document.json` be full `PidDocument`, or should default bundle only emit split views plus an optional full document?
3. Should bundle schema golden files live under `tests/fixtures/export_bundle/` or `docs/examples/`?
4. Should publish XML be exposed through `pid_inspect`, or remain in `pid_publish_xml` with a separate `--bundle-out` flag?

Default answers for first implementation:

- one global raw opt-in flag;
- emit full `PidDocument` plus split views;
- keep golden examples under tests, copied into docs only when stable;
- implement publish bundle only after core `.pid` bundle lands.
