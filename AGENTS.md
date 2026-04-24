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

DWG-specific tests soft-skip when `test-file/backup-test/DWG-0202GP06-01_p/extracted/Export.mdf` is absent.

## Common commands

```bash
cargo test                                        # 806 tests (607 unit + 199 integration, 2 DWG-gated skipped)
cargo test --test publish_xml_cli -- --nocapture   # CLI integration
cd vendor/oxidized-mdf && cargo test --lib         # vendored unit tests (31 tests)
```

## Pre-commit gates (CI mirrors these)

Run **all four** before `git push`. CI (`.github/workflows/ci.yml`) fails
on any drift and will block merges:

```bash
cargo build --locked --workspace --all-targets
cargo test  --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check        # apply with `cargo fmt --all`
```

- `--workspace` makes the vendored `oxidized-mdf` crate a hard gate.
- `RUSTFLAGS=-Dwarnings` (set in CI env) promotes compiler warnings to
  errors; keep local output clean by the same bar.
