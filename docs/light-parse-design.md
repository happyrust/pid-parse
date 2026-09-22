# Light Parse Design

## Problem

`PidParser::parse_file` and `parse_package` currently default to full-fidelity
decoding. That is the right default for round-trip writing and detailed
inspection, but bulk scan callers often need only a cheap inventory: CFB tree,
stream list, summary fields, and enough lightweight structure to classify a
file.

The light path must be explicit. Existing callers should not see derived fields
silently disappear because they upgraded the crate.

## Contract

Add an explicit profile to `ParseOptions`:

```rust
pub enum ParseProfile {
    Full,
    Light,
}
```

`Full` remains the default and preserves current behavior.

`Light` is an opt-in profile for inventory and triage. It may skip expensive
semantic and derived passes, but it must still return a coherent document.

## Output Matrix

| Output / pass | Full | Light |
|---|---:|---:|
| CFB tree | yes | yes |
| stream inventory | yes | yes |
| package raw streams (`PidPackage`) | yes | yes |
| summary streams | yes | yes |
| tagged text XML bodies | yes | no by default |
| `JSite` property decoding | yes | no by default |
| clusters / sheet discovery | yes | yes |
| dynamic attributes | yes | no by default |
| PSM tables / doc registry / DocVersion2 | yes | undecided; measure first |
| sheet endpoint records | yes | no by default unless relationship data is decoded |
| object inventory / object graph | yes | no |
| cross-reference graph | yes | no |
| layout derivation | yes | no |

## Geometry Profile (2026-09-22)

A third profile, `ParseProfile::Geometry` / `ParseOptions::geometry()`, for a
renderer: it runs every pass whose output reaches the drawing and none of the
passes only `pid_inspect` and the reverse-engineering probes read. Plan
`docs/plans/2026-09-21-a-geometry-parse-profile.md`; the measurement that fixed
its columns is G1 there.

The reader no longer compares profiles. Each pass asks one predicate on
`ParseOptions` -- `runs_summary` / `runs_tagged_text` / `runs_jsites` /
`runs_sheet_probes` / `runs_semantic_passes` / `runs_registry` /
`runs_derived_passes` -- and each profile answers for itself:

| Output / pass | Full | Light | Geometry |
|---|---:|---:|---:|
| CFB tree, stream inventory, package raw streams | yes | yes | yes |
| summary streams | yes | yes | **no** |
| tagged text XML bodies | yes | no by default | yes (`drawing_meta.Template` is the page-size fallback) |
| `JSite` property decoding, cached bodies, stroke styles, parametric chain | yes | no by default | yes |
| clusters / sheet discovery, record families, both censuses, style tables | yes | yes | yes |
| sheet text / coordinate probes, spatial analysis | yes | yes | **no** (`ProbeOnly` and inferred points only) |
| string scan (`scan_strings`), unknown-stream diagnostics | yes | yes / yes | **no / no** |
| dynamic attributes, PSM tables, sheet endpoint records | yes | no | yes |
| object graph, cross-reference, geometry hints | yes | no | **yes** -- the connectivity links a renderer draws take their endpoint positions from the geometry hints, which are scored against the graph and the cross-reference's relationship links |
| object inventory, layout derivation | yes | no | no |
| doc registry, `DocVersion2` | yes | undecided | no |

What this buys is not speed first (on the corpus the four small fixtures parse
8-20% faster; `DWG-0201` is dominated by a pass both profiles run) but one
fewer place where "two paths may disagree": `tests/geometry_profile.rs` pins,
on every fixture, that the Decoded entities, the Inferred lines, the cached
symbol bodies, the page size and both loss censuses are identical to `Full`'s,
and that everything `Full` has and `Geometry` lacks is probe yield.

## Compatibility Rules

- `ParseOptions::default()` remains full fidelity.
- Existing fine-grained flags stay available.
- `PidPackage` raw stream retention remains independent of light/full mode.
- Light mode must leave skipped derived fields as `None` / empty collections,
  not partially-populated placeholders.
- Any new public enum or field needs rustdoc because the missing-docs gate is
  locked at zero.

## Measurement Plan

Before implementing the profile, collect baseline command lines and timings for
at least:

- a synthetic/minimal CFB parser API test fixture,
- the A01 real fixture used by publish/parser tests,
- one larger DWG fixture when present locally.

Recommended first commands:

```bash
cargo test --lib api::tests
cargo test --test publish_xml_cli cli_writes_both_data_and_meta_xml_for_real_drawing -- --nocapture
cargo test --test writer_real_files real_file_passthrough_produces_empty_diff_full -- --nocapture
```

If those commands are too coarse for timing, add a dedicated benchmark or small
CLI measurement harness in a follow-up task. Do not claim performance wins from
test runtime alone.

Initial baseline on 2026-04-27:

- `cargo test --lib api::tests`: 6 passed, 0 failed.
- `cargo test --test publish_xml_cli cli_writes_both_data_and_meta_xml_for_real_drawing -- --nocapture`:
  1 passed, finished in about 2.8s on the current machine.
- `cargo test --test writer_real_files real_file_passthrough_produces_empty_diff_full -- --nocapture`:
  soft-skipped the missing `test-file/DWG-0201GP06-01.pid` fixture and passed.

## Implementation Sketch

Implemented initial shape:

```rust
pub struct ParseOptions {
    pub profile: ParseProfile,
    pub scan_strings: bool,
    pub parse_xml: bool,
    pub parse_jsite_properties: bool,
    pub keep_unknown_streams: bool,
    pub max_preview_strings: usize,
}
```

`parse_pid_package_from_cfb` should use small helper predicates such as
`options.should_parse_dynamic_attributes()` instead of scattering
`profile == Light` checks through the reader.

Initial implementation note: the first cut gated the expensive passes directly
inside `parse_pid_package_from_cfb` on one `light_profile` boolean. The
`Geometry` profile (2026-09-22) replaced that boolean with the `runs_*`
predicates above, so the reader has one `if` per pass and a profile is one row
in `config.rs`.

## Open Questions

- Should light mode still parse PSM tables for coverage reports?
- Should summary streams always be parsed even in light mode? Current answer:
  yes, because they are cheap and useful for triage.
- Should `scan_strings` default to false in light mode? Current answer:
  measure first; string scans may dominate on large files but feed useful
  inventory output.
- Should light mode apply to `PidPackage::from_bytes`? Current answer: only if
  a caller can provide custom `ParseOptions`; the convenience constructor should
  stay full fidelity.
