# pid-parse Agent Guide

Layered parser for SmartPlant / Smart P&ID `.pid` files with a
publish XML pipeline (`Export.mdf → oxidized-mdf → drawing graph → _Data.xml / _Meta.xml`).

## Architecture

| Layer | Path | Role |
|---|---|---|
| Vendored MDF parser | `vendor/oxidized-mdf/` | Reads SQL Server MDF files, GPL-3.0 |
| Publish adapter | `src/publish/mdf_load.rs` | MDF → in-memory SQLite staging |
| Publish loader | `src/publish/sqlite_load.rs` | SQLite → `PublishDrawing` DTO |
| XML writer | `src/publish/xml_writer.rs` | DTO → `_Data.xml` / `_Meta.xml` |
| CLI entry | `src/bin/pid_publish_xml.rs` | End-to-end CLI |

## Vendored `oxidized-mdf` (key constraints)

- **Sync API** — no async runtime needed; `MdfDatabase::open()` / `db.rows()` return sync `Result` / `Iterator`.
- **Panic-free** — all three source files (`pages.rs`, `lib.rs`, `sys.rs`) propagate errors via `Result` / `Error::ParseError`; zero `panic!` / `unwrap()` / `todo!()` in production code. GPL-3.0 §5(a) modification notices at top of each file.
- **Parsing engine** — `nom 8` for envelope-layer byte parsing; stdlib `from_le_bytes` for value-layer decoding.
- **Dependencies** — bitvec, chrono, encoding_rs, log, nom, rust_decimal, uuid. No async crates, no byteorder.
- **License** — GPL-3.0. Parent crate is MIT/Apache-2.0; combined binary is GPL-3.0 (see README License section).
- **License** — GPL-3.0 (vendored from [f3rn0s/oxidized-mdf](https://gitlab.com/f3rn0s/oxidized-mdf)). OK for internal use; public distribution of `pid-parse` requires license alignment.
- **Page reader** — forward-only; `mdf_load.rs` re-opens per table to stay deterministic.

## Test gates

| Test file | Scope | Fixture |
|---|---|---|
| `vendor/oxidized-mdf` unit tests | Parser internals | Inline byte vectors |
| `tests/publish_mdf_load.rs` | MDF → SQLite staging | `test-file/…/Export.mdf` |
| `tests/publish_xml_cli.rs` | End-to-end CLI | `test-file/…/Export.mdf` |
| `tests/publish_meta_parity.rs` | Meta XML shape + DWG compare | A01 ref + optional DWG fixture |
| `tests/publish_a01_raw_residual.rs` | Residual value scanning | `test-file/…/Export.mdf` |
| `tests/parse_real_files.rs::primitive_line_decoder_emits_decoded_lines_with_provenance` | Phase 14 GLine2d cross-fixture | `test-file/*.pid` |
| `tests/parse_real_files.rs::primitive_arc_decoder_emits_decoded_arcs_with_provenance` | Phase 14 GArc2d cross-fixture | `test-file/*.pid` |
| `tests/parse_real_files.rs::iglines_decoder_emits_decoded_iglines_with_provenance` | Phase 14 Slice J igLine2d (PSM 0x0018) | `test-file/*.pid` |
| `tests/parse_real_files.rs::iglinestrings_decoder_emits_decoded_polylines_with_provenance` | Phase 14 Slice K igLineString2d (PSM 0x0084) | `test-file/*.pid` |
| `tests/parse_real_files.rs::igpoints_decoder_emits_decoded_points_with_provenance` | Phase 14 Slice L igPoint2d (PSM 0x005E) | `test-file/*.pid` |
| `tests/parse_real_files.rs::igtextboxes_decoder_emits_decoded_texts_with_provenance` | Phase 14 Slice M igTextBox (PSM 0x004D) | `test-file/*.pid` |
| `tests/parse_real_files.rs::igsymbols_decoder_emits_decoded_symbols_with_provenance` | Phase 14 Slice N igSymbol2d (PSM 0x00CE) | `test-file/*.pid` |
| `tests/parse_real_files.rs::dwg0201_emits_decoded_primitive_lines_without_inferred_regression` | Phase 14 Slice E AC8 guard | `DWG-0201GP06-01.pid` |
| `tests/parse_real_files.rs::dwg0201_emits_decoded_primitive_arcs_without_regression` | Phase 14 Slice G AC8 guard | `DWG-0201GP06-01.pid` |

DWG-specific tests soft-skip when `test-file/backup-test/DWG-0202GP06-01_p/extracted/Export.mdf` is absent.

## Phase 14 SmartPlant Sheet geometry decoder — 8 PSM type families

`src/parsers/sheet_records.rs` ships PSM-record decoders for **8
SmartPlant `Sheet*` stream primitives**:

| Slice | PSM Type | Decoder | DTO | Sigma Class |
|---|---|---|---|---|
| D-E | `0x3FE6` | `decode_primitive_lines` | `SheetPrimitiveLineDecoded` | `GLine2d` (SmartPlant ext.) |
| F-I | `0x0030` | `decode_primitive_arcs` | `SheetPrimitiveArcDecoded` | `GArc2d` ≥ `GEllipse2d` |
| J | `0x0018` | `decode_iglines` | `SheetIgLine2dDecoded` | `igLine2d` (IGDS standard) |
| K | `0x0084` | `decode_iglinestrings` | `SheetIgLineString2dDecoded` | `igLineString2d` (IGDS standard) |
| L | `0x005E` | `decode_igpoints` | `SheetIgPoint2dDecoded` | `igPoint2d` (IGDS standard) |
| M | `0x004D` | `decode_igtextboxes` | `SheetIgTextBoxDecoded` | `igTextBox` (IGDS, UTF-16LE) |
| N | `0x00CE` | `decode_igsymbols` | `SheetIgSymbol2dDecoded` | `igSymbol2d` (IGDS, SmartPlant symbols) |

**769 decoded geometry entities cross-fixture** (3 GLine2d + 48 GArc2d
+ 284 igLine2d + 119 polyline + 146 point + 142 text + 27 symbol).
All decoders are panic-safe (validated via
`tests/parser_panic_safety.rs` adversarial matrix) and bounds-checked.

Decoded records flow through `streams/cluster.rs` →
`model::SheetGeometry::decoded_{primitive_lines, primitive_arcs,
iglines, iglinestrings, igpoints, igtextboxes, igsymbols}` →
`geometry::build_normalized_geometry` to emit `PidGraphicEntity {
confidence: Decoded, kind: Line | Arc | Polyline | Point | Text |
SymbolInstance, source: PidGraphicProvenance { stream_path,
byte_range, record_kind, graphic_oid, note } }`. The `note` carries
the byte-level evidence chain.

**Key insight (Slice J discovery)**: Intergraph Sigma uses its IGDS
class tags directly as PSM type codes for standard primitives.
SmartPlant extends with `GLine2d` (`0x3FE6`) and `GArc2d`
(`0x0030`), but the bulk of geometry is standard IGDS records using
IGDS class tags as PSM type codes. This unlocked Slices J–N
without needing `radsrvitem.dll` decompilation.

**Caveats**:
- `GArc2d` (Slice F-I): byte positions IDA-confirmed but some
  geometric field semantics (e.g. `axis_ratio` interpretation) remain
  hypothesis. See `docs/analysis/2026-05-14-radsrvitem-psm-serialize-bytes.md`.
- SmartPlant fixtures don't use standard IGDS `igCircle2d` (0x0059),
  `igRectangle2d` (0x0020), `igArc2d` (0x0061), or
  `igEllipticalArc2d` (0x007E) — zero hits cross-fixture.
- `0x0010` (638 hits) appears to be embedded sub-records / attribute
  fragments inside other record types, not a standalone geometry
  type.

Phase 14 milestones are tracked in
`goals/phase14-sppid-sheet-geometry/progress.jsonl`. See
`docs/plans/2026-05-14-phase14-decoder-suite-final-summary.md`
for the full Phase 14 summary report.

### Reusable seven-layer decoder template

Each new decoder follows the same template (validated 6× in this
phase):

1. **Probe**: `examples/probe_<type>_shape.rs` dumps fixture bytes
2. **Layout discovery** via byte dump
3. **Decoder API**: `decode_<type>s` + `decode_<type>_at` +
   `Sheet<Type>Decoded` DTO + public constants
4. **Validation rules**: type code + size consistency + finite
   coords + non-degenerate values
5. **Unit tests**: 6–12 covering canonical + every validation
   rejection + panic safety
6. **Model DTO**: `Decoded<Type>Record` + `From` + `SheetGeometry`
   field + schema ratchet
7. **Pipeline**: `cluster.rs` + `geometry.rs` emit
   `PidGraphicEntity { confidence: Decoded, ..., source: full
   provenance }`

## Common commands

```bash
cargo test                                        # 928+ tests (840 unit + 88 integration, 2 DWG-gated skipped)
cargo test --test publish_xml_cli -- --nocapture   # CLI integration
cd vendor/oxidized-mdf && cargo test --lib         # vendored unit tests (31 tests)
```

## Pre-commit gates (CI mirrors these)

Run **all five** before `git push`. CI (`.github/workflows/ci.yml`)
fails on any drift and will block merges:

```bash
cargo build --locked --workspace --all-targets
cargo test  --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check        # apply with `cargo fmt --all`
bash .github/scripts/check-missing-docs.sh   # rustdoc ratchet
```

### `missing_docs` ratchet

`.github/missing-docs-baseline.txt` stores the maximum number of
`rustdoc -W missing-docs` warnings the crate is allowed to produce.
The gate is **ratchet-down only**:

- Adding a `pub` item without `///` docs bumps the count → CI fails.
  Document the new item.
- Intentionally lowering the count (by documenting more items)
  requires also editing the baseline file to the new lower number,
  in the same PR. That keeps every doc improvement visible in
  git history.
- Raising the baseline is almost never the right answer; treat it
  as a temporary crutch only.

See `.github/scripts/check-missing-docs.sh` for the exact command
(`cargo rustdoc --lib --locked -- -W missing-docs` + grep count).

- `--workspace` makes the vendored `oxidized-mdf` crate a hard gate.
- `RUSTFLAGS=-Dwarnings` (set in CI env) promotes compiler warnings to
  errors; keep local output clean by the same bar.

### Security audit (CI-only)

CI also runs an independent `cargo audit` job in parallel with
the test matrix. It scans `Cargo.lock` against the [RustSec
advisory database](https://rustsec.org/) and fails the build the
moment a known CVE surfaces in any transitive dependency.

- Reproduce locally: `cargo install cargo-audit --locked && cargo audit`.
  The `--locked` belongs to `cargo install` (so the install
  obeys the lockfile of `cargo-audit` itself); `cargo audit` does
  not accept `--locked` as a flag and errors out if given one.
- `cargo audit` reads but never rewrites the committed
  `Cargo.lock`, so the scan is byte-for-byte deterministic on
  every CI run.
- Failure path: bump the offending crate in `Cargo.toml`
  (or whichever `vendor/<crate>/Cargo.toml` carries it),
  run `cargo update -p <crate>`, then re-run `cargo audit`
  to confirm the advisory disappears before pushing.

## Parser hardening playbook

Cadence used by PRs #3–#7 to harden `src/parsers/*` one edge case
at a time. Every PR follows the same TDD loop and ships as a
single squash-merge:

1. **Reproduce as a red test.** Add a focused `#[test]` in the
   parser's own `#[cfg(test)] mod tests` that constructs the
   panicking / wrong-result input and asserts the desired
   behaviour. Run `cargo test --lib parsers::<module>` and
   confirm it fails with the exact panic / diagnostic you expect.
2. **Minimum patch.** Prefer one of these guards over
   restructuring:
   - `text.get(a..b)` / `slice.get(a..b)` instead of
     `&text[a..b]` for char-boundary or out-of-range safety.
   - `checked_add` / `checked_mul` for size arithmetic on
     untrusted `u32` lengths.
   - `(cursor + len).min(data.len())` to clamp end indexes after
     the corresponding `cursor + len <= data.len()` check.
   - Early `return None` for `0`-valued lengths and obviously
     bogus discriminators / magic bytes.
3. **Run all five pre-commit gates** (see above) and confirm the
   new test goes green, every other test still passes, and the
   `missing_docs` count is unchanged.
4. **One commit, one PR.** Commit message format
   `fix(parser): <imperative summary>`. PR body keeps a short
   rationale + the test plan checklist. Squash-merge with branch
   deletion: `gh pr merge <n> --squash --delete-branch`.
5. **Smoke-test guard.** `tests/parser_panic_safety.rs` walks
   every public byte-level parser entry against an adversarial
   corpus on every CI run; new entry points must be added there
   in the same PR that introduces them.

### Where to look for the next target

When sweeping for new edge cases, start with these patterns:

- `String::from_utf8_lossy(data)` followed by `&text[a..b]`
  slicing — non-ASCII bytes inflate to the 3-byte `U+FFFD` and
  shift offsets; the trailing index can land inside a multi-byte
  char and panic. Guard with `text.get(a..b)` (regression: PR #6,
  `scan_guids`).
- Untrusted `u32 as usize` lengths fed into `cursor + len > …` —
  watch for missing `checked_add` / `checked_mul`, especially on
  32-bit targets where `len * 2` can wrap.
- `data[i]` / `data[i..j]` direct indexing without an upstream
  `i + N <= data.len()` guard.
- `s.find(…)` on `from_utf8_lossy` output where the byte index is
  later used with `&str` slicing.
- `unwrap()` / `expect()` on `Iterator::next()` after `peek()`
  returns `Some` — sound by contract but cosmetic noise to remove
  when the surrounding diff already touches the function.
