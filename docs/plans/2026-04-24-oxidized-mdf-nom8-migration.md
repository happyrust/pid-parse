# Oxidized MDF nom 8 Migration Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Incrementally move the vendored `oxidized-mdf` byte parsers onto `nom 8.0` without changing the parent publish pipeline contract.

**Architecture:** Start at the parser envelope layer inside `vendor/oxidized-mdf/src/pages.rs`, where malformed MDF bytes currently concentrate most panic risk. Introduce `nom 8` as an internal parsing engine behind the existing `TryFrom` / `parse_* -> Result<_, &'static str>` surface so `src/publish/mdf_load.rs` and the rest of the parent crate stay unchanged while we swap parsing internals slice by slice.

**Tech Stack:** Rust, `nom 8.0`, vendored `oxidized-mdf`, existing vendored/unit/integration tests, parent publish regression tests.

---

### Task 1: Bootstrap `nom 8` at the record envelope

**Files:**
- Modify: `vendor/oxidized-mdf/Cargo.toml`
- Modify: `vendor/oxidized-mdf/src/pages.rs`
- Test: `vendor/oxidized-mdf/src/pages.rs`

**Step 1: Write the failing test**

Add a focused malformed-record test that exercises truncated variable-column metadata and asserts `Record::try_from` returns `Err` instead of panicking.

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test record_try_from_returns_err_for_truncated_variable_column_metadata --lib
```

Expected: FAIL because the current `VariableColumns::new` path still relies on unchecked byte reads.

**Step 3: Write minimal implementation**

Add `nom = "8"` to the vendored crate and migrate only the record-envelope parsing path:
- `Record::try_from`
- `VariableColumns::new` (or equivalent replacement)

Constraints:
- keep the existing public APIs and error strings stable where possible
- do not rewrite the value decoders yet
- keep the migration contained to `pages.rs`

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test record_try_from_returns_err_for_truncated_variable_column_metadata --lib
```

Expected: PASS.

**Step 5: Run nearby safety net**

Run:

```bash
cargo test --lib
```

inside `vendor/oxidized-mdf`.

Expected: existing page/record parsing unit tests stay green.

### Task 2: Extend `nom 8` to page-pointer and page-header parsing

**Files:**
- Modify: `vendor/oxidized-mdf/src/pages.rs`
- Test: `vendor/oxidized-mdf/src/pages.rs`

**Step 1: Add failing tests**

Cover truncated `PagePointer`, truncated `PageHeader`, and any malformed slot-directory inputs that still depend on unchecked slicing.

**Step 2: Verify RED**

Run only the new targeted tests and confirm the current implementation still panics or returns the wrong failure mode.

**Step 3: Implement minimal migration**

Switch `PagePointer::try_from`, `PageHeader::try_from`, and slot-directory helpers to `nom 8` number/take parsers while preserving current caller APIs.

**Step 4: Verify GREEN**

Run the targeted tests and then the full vendored test suite.

### Task 3: Migrate primitive row-value readers in small slices

**Files:**
- Modify: `vendor/oxidized-mdf/src/pages.rs`
- Possibly modify: `vendor/oxidized-mdf/src/lib.rs`
- Test: `vendor/oxidized-mdf/src/pages.rs`

**Step 1: Choose one primitive family at a time**

Migrate readers in this order:
1. variable-length column cursor + string/binary envelopes
2. fixed-width integers / floats
3. datetime / uuid / decimal helpers

Rationale:
- the variable-length cursor still owns a real malformed-input panic edge
- fixed-width readers are lower-risk and can follow once the cursor contract is safe

**Step 2: Keep each slice test-first**

For every family, add one malformed-input regression plus one happy-path assertion before changing implementation.

**Step 3: Preserve outer contracts**

Keep `Value::parse` and the parent crate behavior stable; no publish-layer rewrites during this task.

### Task 4: Re-run parent publish regression gates after each meaningful slice

**Files:**
- Test: `tests/publish_mdf_load.rs`
- Test: `tests/publish_xml_cli.rs`
- Test: `tests/publish_meta_parity.rs`

**Step 1: Run targeted parent regression**

After each landed `nom` slice, run:

```bash
cargo test --test publish_mdf_load -- --nocapture
```

**Step 2: Run broader publish regression before declaring the migration stable**

Run:

```bash
cargo test --test publish_xml_cli -- --nocapture
cargo test --test publish_meta_parity -- --nocapture
```

Expected: the publish-facing MDF contract remains green while internals shift to `nom 8`.

---

## Completion Status (2026-04-24)

All tasks complete. Summary of what landed:

| Task | Scope | Key outcome |
|---|---|---|
| 1 | Record envelope (`Record::try_from`, `VariableColumns::try_new`) | nom `take` helpers, `Err` on truncated variable-column metadata |
| 2 | Page envelope (`PagePointer`, `PageHeader`, slot directory) | nom helpers, skip+log for impossible slot counts |
| 3a | Variable-length cursor hardening | `parse_variables_bytes_opt` returns `Err` on descending end offsets |
| 3b | Fixed-width readers (`i8/i16/i32/i64/f32/f64/u128`) | `from_le_bytes`, removed `ReadBytesExt::unwrap()` path |
| 3c | Datetime / decimal helpers | `from_le_bytes` + manual i24 sign extension, `byteorder` crate removed |
| 4 | Final regression gate | vendored 31 passed, parent 27 passed (publish_mdf_load + publish_xml_cli + publish_meta_parity) |

**Dependency change:** `byteorder` removed from `Cargo.toml`. All byte decoding now uses `nom::bytes::complete::take` (envelope layer) or `from_le_bytes` (value layer).
