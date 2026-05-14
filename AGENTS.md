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
| `tests/parse_real_files.rs::dwg0201_emits_decoded_primitive_lines_without_inferred_regression` | Phase 14 Slice E AC8 guard | `DWG-0201GP06-01.pid` |
| `tests/parse_real_files.rs::dwg0201_emits_decoded_primitive_arcs_without_regression` | Phase 14 Slice G AC8 guard | `DWG-0201GP06-01.pid` |

DWG-specific tests soft-skip when `test-file/backup-test/DWG-0202GP06-01_p/extracted/Export.mdf` is absent.

## Phase 14 SmartPlant Sheet geometry decoder

`src/parsers/sheet_records.rs` ships PSM-record decoders for two
SmartPlant `Sheet*` stream primitives:

- `decode_primitive_lines(&[u8]) -> Vec<SheetPrimitiveLineDecoded>` —
  PSM type code `0x3FE6` (`GLine2d`); 6 × f64 parametric line
  representation `origin + t * direction` for
  `t ∈ [param_start, param_end]`. Validated to unit direction
  vector and sorted params. **Accuracy verified against fixture
  byte content.**
- `decode_primitive_arcs(&[u8]) -> Vec<SheetPrimitiveArcDecoded>` —
  PSM type code `0x0030` (`GArc2d` ≥ `GEllipse2d` parent + sweep).
  Validates `axis_a.y ≈ 0` (GEllipse2d "majorAxis along X"),
  `axis_ratio ∈ [0, 1+1e-6]`, `sweep_direction ∈ {0, 1}`. **Byte
  positions IDA-confirmed; some geometric field semantics
  (e.g. `axis_ratio` interpretation) remain hypothesis pending
  further reverse-engineering.** See
  `docs/analysis/2026-05-14-radsrvitem-psm-serialize-bytes.md`
  for full evidence chain (IGDS class tag table, PSM dispatch,
  GEllipse2d / GArc2d class hierarchy, fixture byte dump
  caveat).

Decoded records flow through `streams/cluster.rs` →
`model::SheetGeometry::decoded_primitive_{lines,arcs}` →
`geometry::build_normalized_geometry` to emit
`PidGraphicEntity { confidence: Decoded, kind: Line | Arc, source:
PidGraphicProvenance { stream_path, byte_range, record_kind,
graphic_oid, note } }`. The `note` carries the full IDA-evidence
chain back to `radsrvitem.dll` for downstream provenance.

`pid_inspect <fixture> --geometry-json` exposes the full decoded
+ inferred + probe-only entity catalogue with byte-level
provenance. On `DWG-0201GP06-01.pid`: 13 decoded (2 lines + 11
arcs) + 166 inferred (49 lines + 117 points) + 19 probe-only
entities.

Phase 14 milestones are tracked in
`goals/phase14-sppid-sheet-geometry/progress.jsonl`. Next
milestone: resolve the GArc2d geometric field semantics caveat
or extend the decoder family with `GLineString2d` / `GCircle2d`
/ `GText` / etc.

## Common commands

```bash
cargo test                                        # 881+ tests (798 unit + 83 integration, 2 DWG-gated skipped)
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
